//! Authentication commands — vault unlock, lock, create, change password.

use std::sync::Arc;
use tauri::{AppHandle, State, Emitter};
use zeroize::Zeroize;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
};


#[tauri::command]
pub async fn unlock_vault(
    password: String,
    vault_path: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<bool, CypheriaError> {
    let mut pwd_bytes = password.into_bytes();
    let path = std::path::PathBuf::from(&vault_path);

    let result = session.unlock(&pwd_bytes, &path).await;
    pwd_bytes.zeroize();

    match result {
        Ok(()) => {
            autolock.bump_activity();
            Ok(true)
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn lock_vault(
    session: State<'_, Arc<SessionManager>>,
    app: AppHandle,
) -> Result<(), CypheriaError> {
    session.lock().await;
    let _ = app.emit("vault-locked", ());
    Ok(())
}

#[tauri::command]
pub async fn change_master_password(
    old_password: String,
    new_password: String,
    session: State<'_, Arc<SessionManager>>,
) -> Result<(), CypheriaError> {
    let mut old_bytes = old_password.into_bytes();
    let mut new_bytes = new_password.into_bytes();

    if new_bytes.len() < 8 {
        old_bytes.zeroize();
        new_bytes.zeroize();
        return Err(CypheriaError::InvalidInput("New password must be at least 8 characters".into()));
    }

    // BUG-006 fix: wrap the entire sync closure body in catch_sync_panic!
    // to prevent a panic inside the password-change logic from bypassing
    // ZeroizeOnDrop on ActiveKeyStore.
    let result = session.with_session_mut(|key_store, vault_store| {
        catch_sync_panic!({
            use crate::crypto::{kdf, aes, kyber, rng};
            // 1. Verify old password cryptographically via AES-GCM unwrap
            let mut derived_mk = kdf::derive_master_key(&old_bytes, &vault_store.header.argon2_salt)?;
            let unwrap_result = aes::unwrap_key(&derived_mk, &vault_store.header.vk_wrapped_classical);
            derived_mk.zeroize();
            unwrap_result.map_err(|_| CypheriaError::AuthFailed)?;

            // 2. Generate new salt and new Master Key
            let new_salt = rng::argon2_salt();
            let new_mk   = kdf::derive_master_key(&new_bytes, &new_salt)?;

            // 3. Re-wrap VK with new MK
            let new_vk_wrapped = aes::wrap_key(&new_mk, key_store.vault_key_bytes())?;

            // 4. Re-generate Kyber keypair and re-wrap VK post-quantum
            let kp = kyber::generate_keypair();
            let pub_key = kp.public_key.clone();
            let sec_key = kp.secret_key.clone();
            drop(kp);

            let (kyber_ciphertext, vk_wrapped_pq) =
                kyber::encapsulate_vault_key(&pub_key, key_store.vault_key_bytes())?;
            let kyber_sk_encrypted = aes::encrypt(&new_mk, &sec_key)?;

            // 5. Update header
            vault_store.header.argon2_salt          = new_salt;
            vault_store.header.vk_wrapped_classical = new_vk_wrapped;
            vault_store.header.kyber_public_key     = pub_key;
            vault_store.header.kyber_sk_encrypted   = kyber_sk_encrypted;
            vault_store.header.kyber_ciphertext     = kyber_ciphertext;
            vault_store.header.vk_wrapped_pq        = vk_wrapped_pq;

            // HIGH-003 fix: re-encrypt settings blob from old MK subkey → new MK subkey.
            // Must happen BEFORE key_store.master_key is updated, so master_key_bytes()
            // still returns the old key during the decrypt step.
            {
                let mut old_settings_key = [0u8; 32];
                let mut new_settings_key = [0u8; 32];

                crate::crypto::kdf::derive_subkey(
                    key_store.vault_key_bytes(),
                    b"SETTINGS_ENCRYPTION_VK",
                    &mut old_settings_key,
                );
                crate::crypto::kdf::derive_subkey(
                    key_store.vault_key_bytes(),
                    b"SETTINGS_ENCRYPTION_VK",
                    &mut new_settings_key,
                );

                let old_plaintext = aes::decrypt(
                    &old_settings_key,
                    &vault_store.data.settings.payload_encrypted,
                );
                old_settings_key.zeroize();

                match old_plaintext {
                    Ok(json_bytes) => {
                        let new_encrypted = aes::encrypt(&new_settings_key, &json_bytes)
                            .map_err(|e| { new_settings_key.zeroize(); e })?;
                        new_settings_key.zeroize();
                        vault_store.data.settings.payload_encrypted = new_encrypted;
                    }
                    Err(_) => {
                        new_settings_key.zeroize();
                        let default_json = serde_json::to_vec(&crate::models::settings::Settings::default())
                            .map_err(|_| CypheriaError::SerdeError)?;
                        let mut fresh_key = [0u8; 32];
                        kdf::derive_subkey(key_store.vault_key_bytes(), b"SETTINGS_ENCRYPTION_VK", &mut fresh_key);
                        vault_store.data.settings.payload_encrypted =
                            aes::encrypt(&fresh_key, &default_json)
                                .map_err(|e| { fresh_key.zeroize(); e })?;
                        fresh_key.zeroize();
                    }
                }
            }

            // FIX: IMPROVE-001 — MasterKey::new() replaces direct tuple construction.
            key_store.master_key = crate::crypto::keys::MasterKey::new(new_mk);

            // BUG-005 fix: sync KDF params in the header to current constants.
            vault_store.header.kdf_memory_kb   = kdf::ARGON2_MEMORY_KB;
            vault_store.header.kdf_iterations  = kdf::ARGON2_ITERATIONS;
            vault_store.header.kdf_parallelism = kdf::ARGON2_PARALLELISM;

            Ok(())
        })
    }).await;

    old_bytes.zeroize();
    new_bytes.zeroize();

    result
}

#[tauri::command]
pub async fn create_vault(
    password: String,
    vault_path: String,
    vault_name: String,
) -> Result<(), CypheriaError> {
    use crate::crypto::{kdf, aes, kyber, rng, keys::ActiveKeyStore};
    use crate::vault::format::*;
    use crate::vault::store::persist_vault;
    use crate::models::settings::Settings;
    use chrono::Utc;
    use zeroize::Zeroizing;

    if password.len() < 8 {
        return Err(CypheriaError::InvalidInput("Password must be at least 8 characters".into()));
    }
    if vault_name.trim().is_empty() {
        return Err(CypheriaError::InvalidInput("Vault name cannot be empty".into()));
    }

    let path = std::path::PathBuf::from(&vault_path);
    if path.exists() {
        return Err(CypheriaError::VaultExists);
    }

    let mut pwd_bytes = password.into_bytes();
    let salt = rng::argon2_salt();
    let mk_bytes = Zeroizing::new(kdf::derive_master_key(&pwd_bytes, &salt)?);
    pwd_bytes.zeroize();

    let vk_bytes = Zeroizing::new(rng::entry_key());
    let vk_wrapped_classical = aes::wrap_key(&*mk_bytes, &*vk_bytes)?;

    let kp = kyber::generate_keypair();
    let pub_key = kp.public_key.clone();
    let sec_key = zeroize::Zeroizing::new(kp.secret_key.clone());
    drop(kp);

    let (kyber_ciphertext, vk_wrapped_pq) =
        kyber::encapsulate_vault_key(&pub_key, &vk_bytes)?;
    let kyber_sk_encrypted = aes::encrypt(&*mk_bytes, &sec_key)?;

    let default_settings = Settings::default();
    let settings_json = serde_json::to_vec(&default_settings).map_err(|_| CypheriaError::SerdeError)?;
    // CRIT-002 fix: use mk_bytes through Zeroizing wrapper (already in scope), no extra key needed
    // Encrypt settings with a VK-derived subkey (not MK) so settings survive
    // master password changes without re-encryption.
    let mut settings_subkey = [0u8; 32];
    kdf::derive_subkey(&*vk_bytes, b"SETTINGS_ENCRYPTION_VK", &mut settings_subkey);
    let settings_encrypted = aes::encrypt(&settings_subkey, &settings_json)?;
    settings_subkey.zeroize();
    let mut settings_json = settings_json;
    settings_json.zeroize();

    let vault_data = VaultData {
        entries:    vec![],
        notes:      vec![],
        settings:   EncryptedSettings { payload_encrypted: settings_encrypted },
        updated_at: Utc::now(),
    };

    let header = VaultHeader {
        argon2_salt:         salt,
        kdf_memory_kb:       kdf::ARGON2_MEMORY_KB,
        kdf_iterations:      kdf::ARGON2_ITERATIONS,
        kdf_parallelism:     kdf::ARGON2_PARALLELISM,
        vk_wrapped_classical,
        kyber_public_key:    pub_key,
        kyber_sk_encrypted,
        kyber_ciphertext,
        vk_wrapped_pq,
        created_at:          Utc::now(),
        vault_name,
        format_version:      FORMAT_VERSION,
    };

    {
        let key_store = ActiveKeyStore::new(*mk_bytes, *vk_bytes);
        persist_vault(&key_store, &vault_data, &header, &path).await?;
    }

    Ok(())
}