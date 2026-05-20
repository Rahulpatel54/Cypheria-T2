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

    let nk_bytes = rng::entry_key(); // Note Key

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
    let mut payload_json = payload_json;
    payload_json.zeroize();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::format::{VaultData, EncryptedSettings};
    use chrono::Utc;

    fn empty_vault_data() -> VaultData {
        VaultData {
            entries: vec![],
            notes: vec![],
            settings: EncryptedSettings { payload_encrypted: vec![] },
            updated_at: Utc::now(),
        }
    }

    fn test_vk() -> [u8; 32] { [0xAB_u8; 32] }

    fn sample_note_input(title: &str) -> NoteInput {
        NoteInput {
            title:   title.to_string(),
            content: "This is the note body.".to_string(),
        }
    }

    // ── 1 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_add_and_decrypt_note_roundtrip() {
        // add_note + decrypt_note must recover title and mask content
        let vk = test_vk();
        let mut data = empty_vault_data();
        let id = add_note(&vk, &mut data, sample_note_input("My Secret")).unwrap();
        assert_eq!(data.notes.len(), 1);
        let view = decrypt_note(&vk, &data.notes[0]).unwrap();
        assert_eq!(view.id, id);
        assert_eq!(view.title, "My Secret");
        assert!(view.content_masked, "content must be masked in NoteView");
    }

    // ── 2 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_get_note_content_returns_correct_value() {
        // get_note_content must return the plaintext content stored at add time
        let vk = test_vk();
        let mut data = empty_vault_data();
        let id = add_note(&vk, &mut data, sample_note_input("Title")).unwrap();
        let full = get_note_content(&vk, &data, &id).unwrap();
        assert_eq!(full.title,   "Title");
        assert_eq!(full.content, "This is the note body.");
    }

    // ── 3 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_update_note_rotates_key() {
        // Every update must produce a new wrapped Note Key (forward secrecy)
        let vk = test_vk();
        let mut data = empty_vault_data();
        let id = add_note(&vk, &mut data, sample_note_input("Old Title")).unwrap();
        let old_ek = data.notes[0].ek_wrapped.clone();
        update_note(&vk, &mut data, &id, NoteInput {
            title:   "New Title".to_string(),
            content: "Updated body.".to_string(),
        }).unwrap();
        assert_ne!(
            data.notes[0].ek_wrapped, old_ek,
            "ek_wrapped must differ after update (key rotation)"
        );
        let view = decrypt_note(&vk, &data.notes[0]).unwrap();
        assert_eq!(view.title, "New Title");
    }

    // ── 4 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_decrypt_note_wrong_key_fails() {
        // Decryption with a wrong vault key must return an error
        let vk       = test_vk();
        let wrong_vk = [0x00_u8; 32];
        let mut data = empty_vault_data();
        add_note(&vk, &mut data, sample_note_input("Secret")).unwrap();
        assert!(
            decrypt_note(&wrong_vk, &data.notes[0]).is_err(),
            "wrong vault key must not decrypt note"
        );
    }

    // ── 5 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_add_note_empty_title_rejected() {
        // Validation must reject an empty note title
        let mut data  = empty_vault_data();
        let bad_input = NoteInput { title: "".to_string(), content: "body".to_string() };
        assert!(add_note(&test_vk(), &mut data, bad_input).is_err());
    }

    // ── 6 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_add_note_whitespace_only_title_rejected() {
        // A title of only whitespace must be rejected (trim check)
        let mut data  = empty_vault_data();
        let bad_input = NoteInput { title: "   ".to_string(), content: "body".to_string() };
        assert!(add_note(&test_vk(), &mut data, bad_input).is_err());
    }

    // ── 7 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_add_note_title_too_long_rejected() {
        // Validation must reject titles over 256 characters
        let long_title = "a".repeat(257);
        let mut data   = empty_vault_data();
        let bad_input  = NoteInput { title: long_title, content: "body".to_string() };
        assert!(add_note(&test_vk(), &mut data, bad_input).is_err());
    }

    // ── 8 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_get_note_content_nonexistent_id_fails() {
        // Requesting content for unknown ID must return NoteNotFound
        let vk   = test_vk();
        let data = empty_vault_data();
        let result = get_note_content(&vk, &data, "00000000-0000-0000-0000-000000000000");
        assert!(result.is_err());
    }

    // ── 9 ──────────────────────────────────────────────────────────────────
    #[test]
    fn test_vault_updated_at_stamped_on_add_note() {
        // vault_data.updated_at must be refreshed after add_note
        let vk       = test_vk();
        let mut data = empty_vault_data();
        let before   = data.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        add_note(&vk, &mut data, sample_note_input("TS")).unwrap();
        assert!(data.updated_at >= before);
    }
}