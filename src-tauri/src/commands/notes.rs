use std::sync::Arc;
use tauri::State;
use crate::models::note::{NoteInput, NoteView, NoteContentView};
use crate::commands::validate_uuid;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::notes,
};

#[tauri::command]
pub async fn get_all_notes(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Vec<NoteView>, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        session
            .with_session(|key_store, vault_store| {
                vault_store
                    .data
                    .notes
                    .iter()
                    .map(|n| notes::decrypt_note(key_store.vault_key_bytes(), n))
                    .collect::<Result<Vec<_>, _>>()
            })
            .await
    })
}

#[tauri::command]
pub async fn get_note_content(
    note_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<NoteContentView, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        // BUG-HIGH-002 FIX: validate UUID before use
        validate_uuid(&note_id)?;
        session
            .with_session(|key_store, vault_store| {
                notes::get_note_content(key_store.vault_key_bytes(), &vault_store.data, &note_id)
            })
            .await
    })
}

#[tauri::command]
pub async fn save_note(
    note_id: Option<String>,
    input: NoteInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        session
            if let Some(ref id) = note_id {
                validate_uuid(id)?;
            }
            .with_session_mut(|key_store, vault_store| match note_id {
                Some(ref id) => {
                    notes::update_note(
                        key_store.vault_key_bytes(),
                        &mut vault_store.data,
                        id,
                        input,
                    )?;
                    Ok(id.clone())
                }
                None => notes::add_note(key_store.vault_key_bytes(), &mut vault_store.data, input),
            })
            .await
    })
}

#[tauri::command]
pub async fn delete_note(
    note_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        validate_uuid(&note_id)?;
        session
            .with_session_mut(|_key_store, vault_store| {
                let pre_len = vault_store.data.notes.len();
                vault_store.data.notes.retain(|n| n.id != note_id);
                if vault_store.data.notes.len() == pre_len {
                    return Err(CypheriaError::NoteNotFound(note_id.clone()));
                }
                vault_store.data.updated_at = chrono::Utc::now();
                Ok(())
            })
            .await
    })
}