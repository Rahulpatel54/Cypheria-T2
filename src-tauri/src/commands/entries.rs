//! Entry commands — CRUD operations exposed to the frontend via Tauri IPC.

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    models::entry::{EntryInput, EntryView},
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::entry,
};

// BUG-006 fix: panic boundary macro — see auth.rs for rationale.
macro_rules! safe_command {
    ($body:block) => {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)) {
            Ok(result) => result,
            Err(_) => Err(CypheriaError::InternalError(
                "Unexpected internal error".into(),
            )),
        }
    };
}

fn validate_uuid(id: &str) -> Result<(), CypheriaError> {
    uuid::Uuid::parse_str(id)
        .map_err(|_| CypheriaError::InvalidInput("Invalid ID format".into()))?;
    Ok(())
}


#[tauri::command]
pub async fn get_all_entries(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Vec<EntryView>, CypheriaError> {
    autolock.bump_activity();
    safe_command!({
        session.with_session(|key_store, vault_store| {
            vault_store
                .data
                .entries
                .iter()
                .map(|e| entry::decrypt_entry(key_store.vault_key_bytes(), e))
                .collect::<Result<Vec<_>, _>>()
        })
        .await
    })
}

#[tauri::command]
pub async fn get_entry_password(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    autolock.bump_activity();
    // BUG-008 fix: the previous code wrapped validate_uuid in
    // `if entry_id.len() != 36 { ... }` which is backwards — it only validated
    // IDs that are NOT 36 chars, meaning every properly-formatted UUID (which
    // is always 36 chars) was never validated at all.
    // Fix: call validate_uuid unconditionally so all IDs are checked.
    validate_uuid(&entry_id)?;
    safe_command!({
        session
            .with_session(|key_store, vault_store| {
                entry::get_entry_password(key_store.vault_key_bytes(), &vault_store.data, &entry_id)
            })
            .await
    })
}

#[tauri::command]
pub async fn add_entry(
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    autolock.bump_activity();
    safe_command!({
        session
            .with_session_mut(|key_store, vault_store| {
                entry::add_entry(key_store.vault_key_bytes(), &mut vault_store.data, input)
            })
            .await
    })
}

#[tauri::command]
pub async fn update_entry(
    entry_id: String,
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    // BUG-008 fix: unconditional validation (no length pre-check).
    validate_uuid(&entry_id)?;
    safe_command!({
        session
            .with_session_mut(|key_store, vault_store| {
                entry::update_entry(key_store.vault_key_bytes(), &mut vault_store.data, &entry_id, input)
            })
            .await
    })
}

#[tauri::command]
pub async fn delete_entry(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    // BUG-008 fix: unconditional validation.
    validate_uuid(&entry_id)?;
    safe_command!({
        session
            .with_session_mut(|_key_store, vault_store| {
                let pre_len = vault_store.data.entries.len();
                vault_store.data.entries.retain(|e| e.id != entry_id);
                if vault_store.data.entries.len() == pre_len {
                    return Err(CypheriaError::EntryNotFound(entry_id));
                }
                Ok(())
            })
            .await
    })
}

#[tauri::command]
pub async fn toggle_favorite(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<bool, CypheriaError> {
    autolock.bump_activity();
    // BUG-008 fix: unconditional validation.
    validate_uuid(&entry_id)?;
    safe_command!({
        session
            .with_session_mut(|_key_store, vault_store| {
                let e = vault_store
                    .data
                    .entries
                    .iter_mut()
                    .find(|e| e.id == entry_id)
                    .ok_or_else(|| CypheriaError::EntryNotFound(entry_id.clone()))?;
                e.is_favorite = !e.is_favorite;
                Ok(e.is_favorite)
            })
            .await
    })
}
