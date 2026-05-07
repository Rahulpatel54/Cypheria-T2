use std::sync::Arc;
use tauri::State;
use zeroize::Zeroize;
use crate::{
    error::CypheriaError,
    models::settings::Settings,
    session::{manager::SessionManager, autolock::AutoLockTimer},
};

#[tauri::command]
pub async fn get_settings(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Settings, CypheriaError> {
    autolock.bump_activity();
    session
        .with_session(|key_store, vault_store| {
            // BUG-004 fix: use a domain-separated subkey instead of the raw master key.
            // This prevents cross-purpose key reuse between settings and other domains.
            let mut settings_key = [0u8; 32];
            crate::crypto::kdf::derive_subkey(
                key_store.master_key_bytes(),
                b"SETTINGS_ENCRYPTION",
                &mut settings_key,
            );
            let result = crate::crypto::aes::decrypt(
                &settings_key,
                &vault_store.data.settings.payload_encrypted,
            );
            settings_key.zeroize();
            let plaintext = result?;
            serde_json::from_slice(&plaintext).or_else(|_| Ok(Settings::default()))
        })
        .await
}

#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    autolock.bump_activity();
    let new_timeout = settings.auto_lock_secs;

    session
        .with_session_mut(|key_store, vault_store| {
            let json = serde_json::to_vec(&settings).map_err(|_| CypheriaError::SerdeError)?;
            // BUG-004 fix: use a domain-separated subkey instead of the raw master key.
            let mut settings_key = [0u8; 32];
            crate::crypto::kdf::derive_subkey(
                key_store.master_key_bytes(),
                b"SETTINGS_ENCRYPTION",
                &mut settings_key,
            );
            let result = crate::crypto::aes::encrypt(&settings_key, &json);
            settings_key.zeroize();
            vault_store.data.settings.payload_encrypted = result?;
            Ok(())
        })
        .await?;

    autolock.set_timeout(new_timeout);
    Ok(())
}
