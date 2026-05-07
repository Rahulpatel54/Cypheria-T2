//! Entry commands — CRUD operations exposed to the frontend via Tauri IPC.

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    models::entry::{EntryInput, EntryView},
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::entry,
};

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
    session.with_session(|key_store, vault_store| {
        vault_store
            .data
            .entries
            .iter()
            .map(|e| entry::decrypt_entry(key_store.vault_key_bytes(), e))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
}

#[tauri::command]
pub async fn get_entry_password(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    autolock.bump_activity();
    if entry_id.len() != 36 {
        validate_uuid(&entry_id)?;
    }
    session
        .with_session(|key_store, vault_store| {
            entry::get_entry_password(key_store.vault_key_bytes(), &vault_store.data, &entry_id)
        })
        .await
}

#[tauri::command]
pub async fn add_entry(
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    autolock.bump_activity();
    session
        .with_session_mut(|key_store, vault_store| {
            entry::add_entry(key_store.vault_key_bytes(), &mut vault_store.data, input)
        })
        .await
}

#[tauri::command]
pub async fn update_entry(
    validate_uuid(&entry_id)?;
    entry_id: String,
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    session
        .with_session_mut(|key_store, vault_store| {
            entry::update_entry(key_store.vault_key_bytes(), &mut vault_store.data, &entry_id, input)
        })
        .await
}

#[tauri::command]
pub async fn delete_entry(
    validate_uuid(&entry_id)?;
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
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
}

#[tauri::command]
pub async fn toggle_favorite(
    validate_uuid(&entry_id)?;
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<bool, CypheriaError> {
    autolock.bump_activity();
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
}