//! Clipboard commands — passwords never cross IPC.

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::entry,
};

#[tauri::command]
pub async fn copy_entry_password_to_clipboard(
    entry_id: String,
    timeout_secs: u64,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();

        // Reuse existing private validate_uuid by duplicating its logic here
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
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text("");
            }
        });

        Ok(())
    })
}

#[tauri::command]
pub async fn clear_clipboard() -> Result<(), CypheriaError> {
    safe_command!({
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|_| CypheriaError::InternalError("Clipboard unavailable".into()))?;
        clipboard
            .set_text("")
            .map_err(|_| CypheriaError::InternalError("Clipboard clear failed".into()))?;
        Ok(())
    })
}