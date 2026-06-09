use crate::{
    error::CypheriaError,
    models::settings::Settings,
    session::{autolock::AutoLockTimer, manager::SessionManager},
};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use zeroize::Zeroize;

#[tauri::command]
pub async fn get_settings(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Settings, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        // DRY: delegate to shared read_settings helper
        session
            .with_session(|key_store, vault_store| {
                Ok(crate::vault::store::read_settings(
                    key_store.vault_key_bytes(),
                    &vault_store.data,
                ))
            })
            .await
    })
}

#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
    app: AppHandle,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        let new_timeout = settings.auto_lock_secs;
        let new_screenshot_protection = settings.screenshot_protection;

        session
            .with_session_mut(|key_store, vault_store| {
                catch_sync_panic!({
                    let json =
                        serde_json::to_vec(&settings).map_err(|_| CypheriaError::SerdeError)?;
                    let mut settings_key = [0u8; 32];
                    crate::crypto::kdf::derive_subkey(
                        key_store.vault_key_bytes(),
                        b"SETTINGS_ENCRYPTION_VK",
                        &mut settings_key,
                    );
                    let result = crate::crypto::aes::encrypt(&settings_key, &json);
                    settings_key.zeroize();
                    vault_store.data.settings.payload_encrypted = result?;
                    Ok(())
                })
            })
            .await?;

        autolock.set_timeout(new_timeout);

        // Apply content protection to the main window immediately when the setting changes.
        // This avoids requiring a restart for the change to take effect.
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.set_content_protected(new_screenshot_protection);
        }

        Ok(())
    })
}

#[tauri::command]
pub async fn apply_screenshot_protection(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<(), crate::error::CypheriaError> {
    if let Some(win) = app.get_webview_window("main") {
        win.set_content_protected(enabled)
            .map_err(|e| crate::error::CypheriaError::InternalError(e.to_string()))?;
    }
    Ok(())
}