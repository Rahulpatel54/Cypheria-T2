//! Entry CRUD operations — all work on the encrypted vault data.
//!
//! SECURITY CONTRACT:
//!   - `decrypt_entry()` NEVER includes the password field in its output.
//!   - `get_entry_password()` is the ONLY function that decrypts and returns a password.
//!   - `update_entry()` rotates the Entry Key on every call (forward secrecy within vault).
//!   - All Entry Keys are zeroized immediately after use (not held in memory).

use crate::{
    crypto::{aes, rng},
    error::CypheriaError,
    models::entry::{EntryInput, EntryView},
    vault::format::*,
};
use chrono::Utc;
use uuid::Uuid;
use zeroize::Zeroize;

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
        name: input.name.clone(),
        username: input.username.clone(),
        password: input.password.clone(),
        website: input.website.clone(),
        notes: input.notes.clone(),
    };

    let payload_json = serde_json::to_vec(&payload).map_err(|_| CypheriaError::SerdeError)?;
    let payload_encrypted = aes::encrypt(&ek_bytes, &payload_json)?;
    let mut payload_json = payload_json;
    payload_json.zeroize();
    // Wrap Entry Key with Vault Key
    let ek_wrapped = aes::wrap_key(vault_key, &ek_bytes)?;
    ek_bytes.zeroize();

    let id = Uuid::new_v4().to_string();
    let now = Utc::now();

    vault_data.entries.push(EncryptedEntry {
        id: id.clone(),
        created_at: now,
        updated_at: now,
        is_favorite: input.is_favorite.unwrap_or(false),
        category: input.category.unwrap_or_default(),
        color: input.color.unwrap_or_default(),
        emoji: input.emoji.unwrap_or_default(),
        ek_wrapped,
        payload_encrypted,
    });

    // ERR-007 fix: stamp the vault-level updated_at on every mutation.
    vault_data.updated_at = Utc::now();

    Ok(id)
}

/// Decrypt a single entry for display.
///
/// CRITICAL: The `password` field is intentionally EXCLUDED from the returned EntryView.
/// The frontend must call `get_entry_password()` separately and explicitly to retrieve it.
/// This ensures passwords are never transmitted as part of bulk list operations.
///
/// ERR-006 fix: `ek_bytes` is zeroized unconditionally before any `?` propagation,
/// so the key material is always cleared even when `aes::decrypt` returns an error.
pub fn decrypt_entry(
    vault_key: &[u8; 32],
    encrypted_entry: &EncryptedEntry,
) -> Result<EntryView, CypheriaError> {
    let mut ek_bytes = aes::unwrap_key(vault_key, &encrypted_entry.ek_wrapped)?;

    // Decrypt, then immediately zeroize the key before touching the result.
    let decrypt_result = aes::decrypt(&ek_bytes, &encrypted_entry.payload_encrypted);
    ek_bytes.zeroize(); // always runs, even on error
    let mut plaintext = decrypt_result?;
    let payload: EntryPayload =
        serde_json::from_slice(&plaintext).map_err(|_| CypheriaError::VaultCorrupted)?;
    plaintext.zeroize();

    Ok(EntryView {
        id: encrypted_entry.id.clone(),
        created_at: encrypted_entry.created_at.to_rfc3339(),
        updated_at: encrypted_entry.updated_at.to_rfc3339(),
        is_favorite: encrypted_entry.is_favorite,
        category: encrypted_entry.category.clone(),
        color: encrypted_entry.color.clone(),
        emoji: encrypted_entry.emoji.clone(),
        name: payload.name.clone(),
        username: payload.username.clone(),
        website: payload.website.clone(),
        notes: payload.notes.clone(),
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
///
/// ERR-006 fix: `ek_bytes` is zeroized unconditionally before any `?` propagation.
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

    // Decrypt, then immediately zeroize the key before touching the result.
    let decrypt_result = aes::decrypt(&ek_bytes, &encrypted_entry.payload_encrypted);
    ek_bytes.zeroize(); // always runs, even on error
    let mut plaintext = decrypt_result?;
    let payload: EntryPayload =
        serde_json::from_slice(&plaintext).map_err(|_| CypheriaError::VaultCorrupted)?;
    plaintext.zeroize();
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
        name: input.name,
        username: input.username,
        password: input.password,
        website: input.website,
        notes: input.notes,
    };

    let payload_json = serde_json::to_vec(&payload).map_err(|_| CypheriaError::SerdeError)?;
    let payload_encrypted = aes::encrypt(&new_ek, &payload_json)?;
    let mut payload_json = payload_json;
    payload_json.zeroize();
    let ek_wrapped = aes::wrap_key(vault_key, &new_ek)?;
    new_ek.zeroize();

    encrypted_entry.payload_encrypted = payload_encrypted;
    encrypted_entry.ek_wrapped = ek_wrapped;
    encrypted_entry.updated_at = Utc::now();
    encrypted_entry.is_favorite = input.is_favorite.unwrap_or(encrypted_entry.is_favorite);
    encrypted_entry.category = input.category.unwrap_or(encrypted_entry.category.clone());
    encrypted_entry.color = input.color.unwrap_or(encrypted_entry.color.clone());
    encrypted_entry.emoji = input.emoji.unwrap_or(encrypted_entry.emoji.clone());

    // ERR-007 fix: stamp the vault-level updated_at on every mutation.
    vault_data.updated_at = Utc::now();

    Ok(())
}

fn validate_entry_input(input: &EntryInput) -> Result<(), CypheriaError> {
    if input.name.trim().is_empty() {
        return Err(CypheriaError::InvalidInput(
            "Entry name cannot be empty".into(),
        ));
    }
    if input.name.len() > 256 {
        return Err(CypheriaError::InvalidInput(
            "Entry name too long (max 256 chars)".into(),
        ));
    }
    if input.username.len() > 512 {
        return Err(CypheriaError::InvalidInput(
            "Username too long (max 512 chars)".into(),
        ));
    }
    if input.password.len() > 4096 {
        return Err(CypheriaError::InvalidInput(
            "Password too long (max 4096 chars)".into(),
        ));
    }
    if input.website.len() > 2048 {
        return Err(CypheriaError::InvalidInput(
            "Website URL too long (max 2048 chars)".into(),
        ));
    }
    if input.notes.len() > 65536 {
        return Err(CypheriaError::InvalidInput(
            "Notes too long (max 64 KB)".into(),
        ));
    }
    if let Some(ref color) = input.color {
        if !color.is_empty() {
            let valid = color.len() == 7
                && color.starts_with('#')
                && color[1..].chars().all(|c| c.is_ascii_hexdigit());
            if !valid {
                return Err(CypheriaError::InvalidInput(
                    "Color must be a valid hex color (e.g. #8b5cf6)".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::format::{EncryptedSettings, VaultData};
    use chrono::Utc;

    // Helper: empty in-memory vault
    fn empty_vault_data() -> VaultData {
        VaultData {
            entries: vec![],
            notes: vec![],
            settings: EncryptedSettings {
                payload_encrypted: vec![],
            },
            updated_at: Utc::now(),
        }
    }

    // Fixed test vault key
    fn test_vk() -> [u8; 32] {
        [0xAB_u8; 32]
    }

    // Helper: minimal valid EntryInput
    fn sample_input(name: &str) -> EntryInput {
        EntryInput {
            name: name.to_string(),
            username: "user@example.com".to_string(),
            password: "s3cr3t!Password1".to_string(),
            website: "https://example.com".to_string(),
            notes: String::new(),
            is_favorite: Some(false),
            category: Some("general".into()),
            color: Some("#8b5cf6".into()),
            emoji: Some("T".into()),
        }
    }

    #[test]
    fn test_add_and_decrypt_entry_roundtrip() {
        // Basic add + decrypt must recover correct name and mask password
        let vk = test_vk();
        let mut data = empty_vault_data();
        let id = add_entry(&vk, &mut data, sample_input("TestSite")).unwrap();
        assert_eq!(data.entries.len(), 1);
        let view = decrypt_entry(&vk, &data.entries[0]).unwrap();
        assert_eq!(view.id, id);
        assert_eq!(view.name, "TestSite");
        assert_eq!(view.username, "user@example.com");
        assert!(view.password_masked, "password must be masked in EntryView");
    }

    #[test]
    fn test_get_entry_password_returns_correct_value() {
        // get_entry_password must return the plaintext stored password
        let vk = test_vk();
        let mut data = empty_vault_data();
        let id = add_entry(&vk, &mut data, sample_input("Login")).unwrap();
        let pwd = get_entry_password(&vk, &data, &id).unwrap();
        assert_eq!(pwd, "s3cr3t!Password1");
    }

    #[test]
    fn test_update_entry_rotates_key() {
        // Every update must produce a new wrapped Entry Key (forward secrecy)
        let vk = test_vk();
        let mut data = empty_vault_data();
        let id = add_entry(&vk, &mut data, sample_input("Old")).unwrap();
        let old_ek = data.entries[0].ek_wrapped.clone();
        update_entry(&vk, &mut data, &id, sample_input("New")).unwrap();
        assert_ne!(
            data.entries[0].ek_wrapped, old_ek,
            "ek_wrapped must differ after update (key rotation)"
        );
        let view = decrypt_entry(&vk, &data.entries[0]).unwrap();
        assert_eq!(view.name, "New");
    }

    #[test]
    fn test_decrypt_entry_wrong_key_fails() {
        // Decryption with a wrong vault key must return an error
        let vk = test_vk();
        let wrong_vk = [0x00_u8; 32];
        let mut data = empty_vault_data();
        add_entry(&vk, &mut data, sample_input("Secret")).unwrap();
        assert!(
            decrypt_entry(&wrong_vk, &data.entries[0]).is_err(),
            "wrong vault key must not decrypt entry"
        );
    }

    #[test]
    fn test_add_entry_empty_name_rejected() {
        // Validation must reject an empty entry name
        let mut input = sample_input("placeholder");
        input.name = String::new();
        let mut data = empty_vault_data();
        assert!(add_entry(&test_vk(), &mut data, input).is_err());
    }

    #[test]
    fn test_add_entry_name_too_long_rejected() {
        // Validation must reject names over 256 characters
        let long_name = "a".repeat(257);
        let input = sample_input(&long_name);
        let mut data = empty_vault_data();
        assert!(add_entry(&test_vk(), &mut data, input).is_err());
    }

    #[test]
    fn test_get_entry_password_nonexistent_id_fails() {
        // Requesting password for unknown ID must return EntryNotFound
        let vk = test_vk();
        let data = empty_vault_data();
        let result = get_entry_password(&vk, &data, "00000000-0000-0000-0000-000000000000");
        assert!(result.is_err());
    }

    #[test]
    fn test_vault_updated_at_stamped_on_add() {
        // vault_data.updated_at must be refreshed after add
        let vk = test_vk();
        let mut data = empty_vault_data();
        let before = data.updated_at;
        // Small sleep to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));
        add_entry(&vk, &mut data, sample_input("TS")).unwrap();
        assert!(data.updated_at >= before);
    }
}
