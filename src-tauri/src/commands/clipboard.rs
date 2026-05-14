//! Clipboard commands — passwords never cross IPC.

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tauri::State;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::entry,
};

/// Holds a handle to the active clipboard-clear timer so it can be cancelled
/// when a new password is copied before the previous timer fires.
pub struct ClipboardTimer(pub Arc<Mutex<Option<JoinHandle<()>>>>);

#[tauri::command]
pub async fn copy_entry_password_to_clipboard(
    entry_id: String,
    timeout_secs: u64,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
    clip_timer: State<'_, Arc<ClipboardTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();

        uuid::Uuid::parse_str(&entry_id)
            .map_err(|_| CypheriaError::InvalidInput("Invalid ID format".into()))?;

        let password = session
            .with_session(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::get_entry_password(
                        key_store.vault_key_bytes(),
                        &vault_store.data,
                        &entry_id,
                    )
                })
            })
            .await?;

        {
            let mut clipboard = arboard::Clipboard::new()
                .map_err(|_| CypheriaError::InternalError("Clipboard unavailable".into()))?;
            clipboard
                .set_text(&password)
                .map_err(|_| CypheriaError::InternalError("Clipboard write failed".into()))?;
        }

        let secs = if timeout_secs == 0 { 30 } else { timeout_secs };
        let timer_arc = clip_timer.0.clone();

        // Cancel any existing timer before spawning a new one
        let mut guard = timer_arc.lock().await;
        if let Some(old_handle) = guard.take() {
            old_handle.abort();
        }

        let new_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text("");
            }
            // Clear the handle slot after firing
            let mut g = timer_arc.lock().await;
            *g = None;
        });

        *guard = Some(new_handle);
        drop(guard);

        Ok(())
    })
}

#[tauri::command]
pub async fn clear_clipboard(
    clip_timer: State<'_, Arc<ClipboardTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        // Cancel pending auto-clear timer if one exists
        let mut guard = clip_timer.0.lock().await;
        if let Some(handle) = guard.take() {
            handle.abort();
        }
        drop(guard);

        let mut clipboard = arboard::Clipboard::new()
            .map_err(|_| CypheriaError::InternalError("Clipboard unavailable".into()))?;
        clipboard
            .set_text("")
            .map_err(|_| CypheriaError::InternalError("Clipboard clear failed".into()))?;
        Ok(())
    })
}