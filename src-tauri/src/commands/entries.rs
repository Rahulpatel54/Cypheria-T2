//! Entry commands — CRUD operations exposed to the frontend via Tauri IPC.

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    models::entry::{EntryInput, EntryView},
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::entry,
};
use crate::commands::validate_uuid;

#[tauri::command]
pub async fn get_all_entries(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Vec<EntryView>, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        session
            .with_session(|key_store, vault_store| {
                catch_sync_panic!({
                    vault_store
                        .data
                        .entries
                        .iter()
                        .map(|e| entry::decrypt_entry(key_store.vault_key_bytes(), e))
                        .collect::<Result<Vec<_>, _>>()
                })
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
    safe_command!({
        autolock.bump_activity();
        // BUG-008 fix: unconditional validation so all IDs are checked.
        validate_uuid(&entry_id)?;
        session
            .with_session(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::get_entry_password(key_store.vault_key_bytes(), &vault_store.data, &entry_id)
                })
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
    safe_command!({
        autolock.bump_activity();
        session
            .with_session_mut(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::add_entry(key_store.vault_key_bytes(), &mut vault_store.data, input)
                })
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
    safe_command!({
        autolock.bump_activity();
        // BUG-008 fix: unconditional validation.
        validate_uuid(&entry_id)?;
        if input.password.is_empty() {
            return Err(CypheriaError::InvalidInput(
                "Password cannot be empty. Use update_entry_keep_password to keep the existing password.".into()
            ));
        }
        session
            .with_session_mut(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::update_entry(
                        key_store.vault_key_bytes(),
                        &mut vault_store.data,
                        &entry_id,
                        input,
                    )
                })
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
    safe_command!({
        autolock.bump_activity();
        // BUG-008 fix: unconditional validation.
        validate_uuid(&entry_id)?;
        session
            .with_session_mut(|_key_store, vault_store| {
                catch_sync_panic!({
                    let pre_len = vault_store.data.entries.len();
                    vault_store.data.entries.retain(|e| e.id != entry_id);
                    if vault_store.data.entries.len() == pre_len {
                        return Err(CypheriaError::EntryNotFound(entry_id.clone()));
                    }
                    // ERR-007 fix: stamp the vault-level updated_at on deletion.
                    vault_store.data.updated_at = chrono::Utc::now();
                    Ok(())
                })
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
    safe_command!({
        autolock.bump_activity();
        // BUG-008 fix: unconditional validation.
        validate_uuid(&entry_id)?;
        session
            .with_session_mut(|_key_store, vault_store| {
                catch_sync_panic!({
                    let e = vault_store
                        .data
                        .entries
                        .iter_mut()
                        .find(|e| e.id == entry_id)
                        .ok_or_else(|| CypheriaError::EntryNotFound(entry_id.clone()))?;
                    e.is_favorite = !e.is_favorite;
                    Ok(e.is_favorite)
                })
            })
            .await
    })
}

#[tauri::command]
pub async fn update_entry_keep_password(
    entry_id: String,
    name: String,
    username: String,
    new_password: Option<String>,
    website: String,
    notes: String,
    is_favorite: Option<bool>,
    category: Option<String>,
    color: Option<String>,
    emoji: Option<String>,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        validate_uuid(&entry_id)?;

        session
            .with_session_mut(|key_store, vault_store| {
                catch_sync_panic!({
                    let vk = key_store.vault_key_bytes();

                    let password = match new_password.as_deref() {
                        Some(p) if !p.is_empty() => p.to_string(),
                        _ => entry::get_entry_password(vk, &vault_store.data, &entry_id)?,
                    };

                    let input = crate::models::entry::EntryInput {
                        name,
                        username,
                        password,
                        website,
                        notes,
                        is_favorite,
                        category,
                        color,
                        emoji,
                    };

                    entry::update_entry(vk, &mut vault_store.data, &entry_id, input)
                })
            })
            .await
    })
}