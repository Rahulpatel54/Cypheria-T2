//! Notes CRUD operations.
//! Same encryption pattern as entries — unique Note Key per note, wrapped with VK.

use uuid::Uuid;
use chrono::Utc;
use zeroize::Zeroize;
use crate::{
    crypto::{aes, rng},
    error::CypheriaError,
    vault::format::*,
    models::note::{NoteInput, NoteView},
};

/// Encrypt and add a new note to the vault.
pub fn add_note(
    vault_key: &[u8; 32],
    vault_data: &mut VaultData,
    input: NoteInput,
) -> Result<String, CypheriaError> {
    validate_note_input(&input)?;

    let mut nk_bytes = rng::entry_key(); // Note Key

    let payload = NotePayload {
        title:   input.title.clone(),
        content: input.content.clone(),
    };

    let payload_json = serde_json::to_vec(&payload).map_err(|_| CypheriaError::SerdeError)?;
    // BUG-HIGH-001 FIX: zeroize serialised plaintext after encryption
    let payload_encrypted = aes::encrypt(&nk_bytes, &payload_json)?;
    let mut payload_json = payload_json; // rebind as mutable to allow zeroize
    payload_json.zeroize();
    let ek_wrapped = aes::wrap_key(vault_key, &nk_bytes)?;

    let id  = Uuid::new_v4().to_string();
    let now = Utc::now();

    vault_data.notes.push(EncryptedNote {
        id: id.clone(),
        created_at: now,
        updated_at: now,
        ek_wrapped,
        payload_encrypted,
    });

    // ERR-007 fix: stamp the vault-level updated_at on every mutation.
    vault_data.updated_at = Utc::now();

    Ok(id)
}

/// Decrypt a note for display.
///
/// ERR-006 fix: `nk_bytes` is now zeroized unconditionally before any `?` propagation,
/// so the key material is always cleared even when `aes::decrypt` returns an error.
pub fn decrypt_note(
    vault_key: &[u8; 32],
    encrypted_note: &EncryptedNote,
) -> Result<NoteView, CypheriaError> {
    // We only need the title — still must decrypt payload to get it.
    let mut nk_bytes = aes::unwrap_key(vault_key, &encrypted_note.ek_wrapped)?;
    let decrypt_result = aes::decrypt(&nk_bytes, &encrypted_note.payload_encrypted);
    nk_bytes.zeroize();
    let mut plaintext = decrypt_result?;

    let payload: NotePayload =
        serde_json::from_slice(&plaintext).map_err(|_| CypheriaError::VaultCorrupted)?;
    // BUG-HIGH-001 FIX: zeroize raw plaintext bytes before drop
    plaintext.zeroize();

    Ok(NoteView {
        id:             encrypted_note.id.clone(),
        created_at:     encrypted_note.created_at.to_rfc3339(),
        updated_at:     encrypted_note.updated_at.to_rfc3339(),
        title:          payload.title.clone(),
        content_masked: true,
    })
}

// BUG-CRIT-002 FIX: new function — decrypts and returns note content for ONE note.
// Called only when user explicitly opens a note modal.
pub fn get_note_content(
    vault_key: &[u8; 32],
    vault_data: &VaultData,
    note_id: &str,
) -> Result<crate::models::note::NoteContentView, CypheriaError> {
    let encrypted_note = vault_data
        .notes
        .iter()
        .find(|n| n.id == note_id)
        .ok_or_else(|| CypheriaError::NoteNotFound(note_id.to_string()))?;

    let mut nk_bytes = aes::unwrap_key(vault_key, &encrypted_note.ek_wrapped)?;
    let decrypt_result = aes::decrypt(&nk_bytes, &encrypted_note.payload_encrypted);
    nk_bytes.zeroize();
    let mut plaintext = decrypt_result?;

    let payload: NotePayload =
        serde_json::from_slice(&plaintext).map_err(|_| CypheriaError::VaultCorrupted)?;
    // BUG-HIGH-001 FIX: zeroize raw plaintext bytes
    plaintext.zeroize();

    let result = crate::models::note::NoteContentView {
        id:      encrypted_note.id.clone(),
        title:   payload.title.clone(),
        content: payload.content.clone(),
    };
    Ok(result)
}

/// Update a note. Rotates the Note Key (same forward-secrecy pattern as entries).
pub fn update_note(
    vault_key: &[u8; 32],
    vault_data: &mut VaultData,
    note_id: &str,
    input: NoteInput,
) -> Result<(), CypheriaError> {
    validate_note_input(&input)?;

    let encrypted_note = vault_data
        .notes
        .iter_mut()
        .find(|n| n.id == note_id)
        .ok_or_else(|| CypheriaError::NoteNotFound(note_id.to_string()))?;

    let mut new_nk = rng::entry_key();

    let payload = NotePayload {
        title:   input.title,
        content: input.content,
    };

    let payload_json      = serde_json::to_vec(&payload).map_err(|_| CypheriaError::SerdeError)?;
    let payload_encrypted = aes::encrypt(&new_nk, &payload_json)?;
    let ek_wrapped        = aes::wrap_key(vault_key, &new_nk)?;
    new_nk.zeroize();

    encrypted_note.payload_encrypted = payload_encrypted;
    encrypted_note.ek_wrapped        = ek_wrapped;
    encrypted_note.updated_at        = Utc::now();

    // ERR-007 fix: stamp the vault-level updated_at on every mutation.
    vault_data.updated_at = Utc::now();

    Ok(())
}

fn validate_note_input(input: &NoteInput) -> Result<(), CypheriaError> {
    let trimmed_title = input.title.trim();
    if trimmed_title.is_empty() {
        return Err(CypheriaError::InvalidInput("Note title cannot be empty".into()));
    }
    if trimmed_title.len() > 256 {
        return Err(CypheriaError::InvalidInput("Note title too long (max 256 chars)".into()));
    }
    if input.content.len() > 1_048_576 {
        return Err(CypheriaError::InvalidInput("Note content too long (max 1 MB)".into()));
    }
    Ok(())
}