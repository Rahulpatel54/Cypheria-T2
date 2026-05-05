use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    models::note::{NoteInput, NoteView},
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::notes,
};

/// Get all notes (title + content decrypted).
#[tauri::command]
pub async fn get_all_notes(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Vec<NoteView>, CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|key_store, vault_store| async move {
            vault_store
                .data
                .notes
                .iter()
                .map(|n| notes::decrypt_note(key_store.vault_key_bytes(), n))
                .collect::<Result<Vec<_>, _>>()
        })
        .await
}

/// Save a note — add if new, update (with key rotation) if existing.
#[tauri::command]
pub async fn save_note(
    note_id: Option<String>,
    input: NoteInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|key_store, vault_store| async move {
            match note_id {
                Some(id) => {
                    notes::update_note(key_store.vault_key_bytes(), &mut vault_store.data, &id, input)?;
                    Ok(id)
                }
                None => {
                    notes::add_note(key_store.vault_key_bytes(), &mut vault_store.data, input)
                }
            }
        })
        .await
}

/// Delete a note permanently.
#[tauri::command]
pub async fn delete_note(
    note_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|_key_store, vault_store| async move {
            let pre_len = vault_store.data.notes.len();
            vault_store.data.notes.retain(|n| n.id != note_id);
            if vault_store.data.notes.len() == pre_len {
                return Err(CypheriaError::NoteNotFound(note_id));
            }
            Ok(())
        })
        .await
}
