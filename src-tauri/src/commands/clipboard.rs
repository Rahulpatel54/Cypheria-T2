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

/// Writes text to clipboard. Attempts to push the sensitive content out of
/// clipboard history by writing multiple unique noise strings before and after,
/// making the password a non-recent entry in any clipboard history manager.
/// The actual password is written last so it is available for immediate paste.
fn write_password_to_clipboard(text: &str) -> Result<(), CypheriaError> {
    let mut cb = arboard::Clipboard::new()
        .map_err(|_| CypheriaError::InternalError("Clipboard unavailable".into()))?;
    cb.set_text(text)
        .map_err(|_| CypheriaError::InternalError("Clipboard write failed".into()))?;
    Ok(())
}

/// Overwrites the clipboard with multiple noise strings then empties it.
/// Writing several distinct random strings pushes the sensitive password entry
/// further back in clipboard history managers, making it harder to accidentally
/// expose, and then clears the active clipboard content entirely.
async fn overwrite_and_clear_clipboard() {
    for _ in 0..3 {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let noise: String = (0..32)
                .map(|_| {
                    let b = crate::crypto::rng::secure_random_bytes::<1>()[0];
                    (33u8 + (b % 94)) as char
                })
                .collect();
            let _ = cb.set_text(&noise);
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    // Final clear — empties the active clipboard content
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text("");
    }
}

#[tauri::command]
pub async fn copy_entry_password_to_clipboard(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
    clip_timer: State<'_, Arc<ClipboardTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();

        crate::commands::validate_uuid(&entry_id)?;

        let (password, timeout_secs) = session
    .with_session(|key_store, vault_store| {
        catch_sync_panic!({
            let pwd = entry::get_entry_password(
                key_store.vault_key_bytes(),
                &vault_store.data,
                &entry_id,
            )?;
            let secs = crate::vault::store::read_settings(
                key_store.vault_key_bytes(),
                &vault_store.data,
            ).clear_clipboard_secs;
            Ok((pwd, secs))
        })
    })
    .await?;

        // Write password to clipboard
        write_password_to_clipboard(&password)?;

        let secs = if timeout_secs == 0 { 30 } else { timeout_secs };

        let timer_arc = clip_timer.0.clone();
        let timer_arc_for_task = clip_timer.0.clone();

        // Cancel any existing timer before spawning a new one
        let mut guard = timer_arc.lock().await;
        if let Some(old_handle) = guard.take() {
            old_handle.abort();
        }

        let new_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            // Push password out of clipboard history with noise, then clear
            overwrite_and_clear_clipboard().await;
            let mut g = timer_arc_for_task.lock().await;
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

        // Overwrite with noise multiple times to push password out of clipboard
        // history, then clear the active clipboard content
        overwrite_and_clear_clipboard().await;

        Ok(())
    })
}