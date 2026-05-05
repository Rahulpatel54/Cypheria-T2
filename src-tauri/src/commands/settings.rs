use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    models::settings::Settings,
    session::{manager::SessionManager, autolock::AutoLockTimer},
};

/// Get current settings (decrypted from vault using MK-derived subkey).
#[tauri::command]
pub async fn get_settings(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Settings, CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|key_store, vault_store| async move {
            let plaintext = crate::crypto::aes::decrypt(
                key_store.master_key_bytes(),
                &vault_store.data.settings.payload_encrypted,
            )?;
            serde_json::from_slice(&plaintext).or_else(|_| {
                // If deserialization fails (e.g. empty/new vault), return defaults
                Ok(Settings::default())
            })
        })
        .await
}

/// Save settings — re-encrypt with MK subkey and update auto-lock timer.
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    let new_timeout = settings.auto_lock_secs;

    session
        .with_session(|key_store, vault_store| async move {
            let json = serde_json::to_vec(&settings).map_err(|_| CypheriaError::SerdeError)?;
            vault_store.data.settings.payload_encrypted =
                crate::crypto::aes::encrypt(key_store.master_key_bytes(), &json)?;
            Ok(())
        })
        .await?;

    // Propagate new timeout to the autolock timer
    autolock.set_timeout(new_timeout);

    Ok(())
}
