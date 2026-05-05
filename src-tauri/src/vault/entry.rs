//! Entry CRUD operations — all work on the encrypted vault data.
//!
//! SECURITY CONTRACT:
//!   - `decrypt_entry()` NEVER includes the password field in its output.
//!   - `get_entry_password()` is the ONLY function that decrypts and returns a password.
//!   - `update_entry()` rotates the Entry Key on every call (forward secrecy within vault).
//!   - All Entry Keys are zeroized immediately after use (not held in memory).

use uuid::Uuid;
use chrono::Utc;
use zeroize::Zeroize;
use crate::{
    crypto::{aes, rng},
    error::CypheriaError,
    vault::format::*,
    models::entry::{EntryInput, EntryView},
};

/// Encrypt a new entry and append it to vault data.
///
/// Security steps:
///   1. Validate all input fields
///   2. Generate a fresh random Entry Key (32 bytes)
///   3. Encrypt the credential payload as JSON with EK
///   4. Wrap EK with the Vault Key (AES-GCM)
///   5. EK is dropped (ZeroizeOnDrop) after this function returns
pub fn add_entry(
    vault_key: &[u8; 32],
    vault_data: &mut VaultData,
    input: EntryInput,
) -> Result<String, CypheriaError> {
    validate_entry_input(&input)?;

    // Fresh random Entry Key — unique per entry
    let mut ek_bytes = rng::entry_key();

    let payload = EntryPayload {
        name:     input.name.clone(),
        username: input.username.clone(),
        password: input.password.clone(),
        website:  input.website.clone(),
        notes:    input.notes.clone(),
    };

    let payload_json = serde_json::to_vec(&payload).map_err(|_| CypheriaError::SerdeError)?;
    let payload_encrypted = aes::encrypt(&ek_bytes, &payload_json)?;

    // Wrap Entry Key with Vault Key
    let ek_wrapped = aes::wrap_key(vault_key, &ek_bytes)?;
    ek_bytes.zeroize();

    let id  = Uuid::new_v4().to_string();
    let now = Utc::now();

    vault_data.entries.push(EncryptedEntry {
        id: id.clone(),
        created_at:  now,
        updated_at:  now,
        is_favorite: input.is_favorite.unwrap_or(false),
        category:    input.category.unwrap_or_default(),
        color:       input.color.unwrap_or_default(),
        emoji:       input.emoji.unwrap_or_default(),
        ek_wrapped,
        payload_encrypted,
    });

    Ok(id)
}

/// Decrypt a single entry for display.
///
/// CRITICAL: The `password` field is intentionally EXCLUDED from the returned EntryView.
/// The frontend must call `get_entry_password()` separately and explicitly to retrieve it.
/// This ensures passwords are never transmitted as part of bulk list operations.
pub fn decrypt_entry(
    vault_key: &[u8; 32],
    encrypted_entry: &EncryptedEntry,
) -> Result<EntryView, CypheriaError> {
    let mut ek_bytes = aes::unwrap_key(vault_key, &encrypted_entry.ek_wrapped)?;
    let plaintext    = aes::decrypt(&ek_bytes, &encrypted_entry.payload_encrypted)?;
    ek_bytes.zeroize();

    let payload: EntryPayload =
        serde_json::from_slice(&plaintext).map_err(|_| CypheriaError::VaultCorrupted)?;

    Ok(EntryView {
        id:              encrypted_entry.id.clone(),
        created_at:      encrypted_entry.created_at.to_rfc3339(),
        updated_at:      encrypted_entry.updated_at.to_rfc3339(),
        is_favorite:     encrypted_entry.is_favorite,
        category:        encrypted_entry.category.clone(),
        color:           encrypted_entry.color.clone(),
        emoji:           encrypted_entry.emoji.clone(),
        name:            payload.name.clone(),
        username:        payload.username.clone(),
        website:         payload.website.clone(),
        notes:           payload.notes.clone(),
        password_masked: true, // password intentionally withheld
    })
    // payload.zeroize() fires on drop (ZeroizeOnDrop)
}

/// Get only the password for a specific entry.
///
/// This is an AUDITABLE, SEPARATE command — not bundled with list operations.
/// The frontend must invoke this explicitly. This means:
///   - The full entry list never contains raw password bytes
///   - Password access is a distinct, intentional operation
///   - Future audit logging can target this command specifically
pub fn get_entry_password(
    vault_key: &[u8; 32],
    vault_data: &VaultData,
    entry_id: &str,
) -> Result<String, CypheriaError> {
    let encrypted_entry = vault_data
        .entries
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| CypheriaError::EntryNotFound(entry_id.to_string()))?;

    let mut ek_bytes = aes::unwrap_key(vault_key, &encrypted_entry.ek_wrapped)?;
    let plaintext    = aes::decrypt(&ek_bytes, &encrypted_entry.payload_encrypted)?;
    ek_bytes.zeroize();

    let payload: EntryPayload =
        serde_json::from_slice(&plaintext).map_err(|_| CypheriaError::VaultCorrupted)?;

    let password = payload.password.clone();
    // payload.zeroize() fires on drop
    Ok(password)
}

/// Update an existing entry with new data.
///
/// KEY ROTATION: Every update generates a fresh Entry Key and re-encrypts the payload.
/// This provides forward secrecy within the vault — if an old snapshot of the vault
/// file is stolen, the updated entry is encrypted under a different key.
pub fn update_entry(
    vault_key: &[u8; 32],
    vault_data: &mut VaultData,
    entry_id: &str,
    input: EntryInput,
) -> Result<(), CypheriaError> {
    validate_entry_input(&input)?;

    let encrypted_entry = vault_data
        .entries
        .iter_mut()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| CypheriaError::EntryNotFound(entry_id.to_string()))?;

    // Fresh Entry Key on every update (key rotation)
    let mut new_ek = rng::entry_key();

    let payload = EntryPayload {
        name:     input.name,
        username: input.username,
        password: input.password,
        website:  input.website,
        notes:    input.notes,
    };

    let payload_json      = serde_json::to_vec(&payload).map_err(|_| CypheriaError::SerdeError)?;
    let payload_encrypted = aes::encrypt(&new_ek, &payload_json)?;
    let ek_wrapped        = aes::wrap_key(vault_key, &new_ek)?;
    new_ek.zeroize();

    encrypted_entry.payload_encrypted = payload_encrypted;
    encrypted_entry.ek_wrapped        = ek_wrapped;
    encrypted_entry.updated_at        = Utc::now();
    encrypted_entry.is_favorite       = input.is_favorite.unwrap_or(encrypted_entry.is_favorite);
    encrypted_entry.category          = input.category.unwrap_or(encrypted_entry.category.clone());
    encrypted_entry.color             = input.color.unwrap_or(encrypted_entry.color.clone());
    encrypted_entry.emoji             = input.emoji.unwrap_or(encrypted_entry.emoji.clone());

    Ok(())
}

fn validate_entry_input(input: &EntryInput) -> Result<(), CypheriaError> {
    if input.name.trim().is_empty() {
        return Err(CypheriaError::InvalidInput("Entry name cannot be empty".into()));
    }
    if input.name.len() > 256 {
        return Err(CypheriaError::InvalidInput("Entry name too long (max 256 chars)".into()));
    }
    if input.username.len() > 512 {
        return Err(CypheriaError::InvalidInput("Username too long (max 512 chars)".into()));
    }
    if input.password.len() > 4096 {
        return Err(CypheriaError::InvalidInput("Password too long (max 4096 chars)".into()));
    }
    if input.website.len() > 2048 {
        return Err(CypheriaError::InvalidInput("Website URL too long (max 2048 chars)".into()));
    }
    if input.notes.len() > 65536 {
        return Err(CypheriaError::InvalidInput("Notes too long (max 64 KB)".into()));
    }
    Ok(())
}
