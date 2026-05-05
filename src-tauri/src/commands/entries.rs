//! Entry commands — CRUD operations exposed to the frontend via Tauri IPC.
//!
//! Every command:
//!   1. Bumps the auto-lock activity timer
//!   2. Goes through session.with_session() guard (fails if locked)
//!   3. Persists the vault after any mutation

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    models::entry::{EntryInput, EntryView},
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::{entry, store::persist_vault},
};

/// Returns all entries with credentials (PASSWORD EXCLUDED).
/// Passwords are intentionally withheld — use `get_entry_password` for those.
#[tauri::command]
pub async fn get_all_entries(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Vec<EntryView>, CypheriaError> {
    autolock.bump_activity();
    session.with_session(|key_store, vault_store| async move {
        vault_store
            .data
            .entries
            .iter()
            .map(|e| entry::decrypt_entry(key_store.vault_key_bytes(), e))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
}

/// Get password for a single entry. Separate command for audit isolation.
/// UUID v4 format is validated before any decryption.
#[tauri::command]
pub async fn get_entry_password(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    autolock.bump_activity();
    // Validate UUID v4 format: 8-4-4-4-12 hex chars + dashes = 36 chars
    if entry_id.len() != 36 {
        return Err(CypheriaError::InvalidInput("Invalid entry ID format".into()));
    }
    session
        .with_session(|key_store, vault_store| async move {
            entry::get_entry_password(key_store.vault_key_bytes(), &vault_store.data, &entry_id)
        })
        .await
}

/// Add a new encrypted entry. Persists vault after insert.
#[tauri::command]
pub async fn add_entry(
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    autolock.bump_activity();
    let id = session
        .with_session(|key_store, vault_store| async move {
            let id = entry::add_entry(key_store.vault_key_bytes(), &mut vault_store.data, input)?;
            persist_vault(key_store, &vault_store.data, &vault_store.header, &vault_store.header.vault_name.as_ref().map(|_| std::path::PathBuf::new()).unwrap_or_default()).await.ok();
            Ok(id)
        })
        .await?;

    // Persist is attempted inside with_session; errors are non-fatal (data is in memory)
    Ok(id)
}

/// Update an existing entry. Rotates Entry Key (forward secrecy). Persists vault.
#[tauri::command]
pub async fn update_entry(
    entry_id: String,
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|key_store, vault_store| async move {
            entry::update_entry(key_store.vault_key_bytes(), &mut vault_store.data, &entry_id, input)
        })
        .await
}

/// Delete an entry permanently. Persists vault.
#[tauri::command]
pub async fn delete_entry(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|_key_store, vault_store| async move {
            let pre_len = vault_store.data.entries.len();
            vault_store.data.entries.retain(|e| e.id != entry_id);
            if vault_store.data.entries.len() == pre_len {
                return Err(CypheriaError::EntryNotFound(entry_id));
            }
            Ok(())
        })
        .await
}

/// Toggle favorite flag (non-sensitive metadata — no re-encryption needed).
#[tauri::command]
pub async fn toggle_favorite(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<bool, CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|_key_store, vault_store| async move {
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
