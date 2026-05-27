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
    safe_command!({
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
    })
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
    // Validate lengths before any crypto work
    let mut old_bytes = old_password.into_bytes();
    let mut new_bytes = new_password.into_bytes();

    if new_bytes.len() < 8 {
        old_bytes.zeroize();
        new_bytes.zeroize();
        return Err(CypheriaError::InvalidInput("New password must be at least 8 characters".into()));
    }

    // Derive new salt and new MK from new password on a blocking thread
    let new_salt = crate::crypto::rng::argon2_salt();
    let new_bytes_for_kdf = new_bytes.clone();
    let new_salt_for_kdf = new_salt;
    let new_mk_raw = tokio::task::spawn_blocking(move || {
        crate::crypto::kdf::derive_master_key(&new_bytes_for_kdf, &new_salt_for_kdf)
    })
    .await
    .map_err(|_| CypheriaError::InternalError("KDF thread panicked".into()))??;
    let new_mk = zeroize::Zeroizing::new(new_mk_raw);
    new_bytes.zeroize();

    // Keep old_bytes alive so the closure can use it for verification
    let old_bytes_capture = old_bytes.clone();
    old_bytes.zeroize();

    let result = session.with_session_mut(|key_store, vault_store| {
        catch_sync_panic!({
            use crate::crypto::{aes, kyber, kdf};

            // Re-derive MK from the provided old password and compare to the session key.
            // This is the authoritative check — without it, any authenticated caller could
            // change the password to anything without knowing the current one.
            let old_mk_derived = kdf::derive_master_key_with_params(
                old_bytes_capture.as_slice(),
                &vault_store.header.argon2_salt,
                vault_store.header.kdf_memory_kb,
                vault_store.header.kdf_iterations,
                vault_store.header.kdf_parallelism,
            ).map_err(|_| CypheriaError::KdfError)?;

            let keys_match = old_mk_derived == *key_store.master_key_bytes();
            let mut z = old_mk_derived;
            z.zeroize();

            if !keys_match {
                return Err(CypheriaError::AuthFailed);
            }

            // Re-wrap VK with new MK
            let new_vk_wrapped = aes::wrap_key(&new_mk, key_store.vault_key_bytes())?;

            // Re-generate Kyber keypair and re-wrap VK post-quantum
            let kp = kyber::generate_keypair();
            let pub_key = kp.public_key.clone();
            let sec_key = kp.secret_key.clone();
            drop(kp);

            let (kyber_ciphertext, vk_wrapped_pq) =
                kyber::encapsulate_vault_key(&pub_key, key_store.vault_key_bytes())?;
            let kyber_sk_encrypted = aes::encrypt(&new_mk, &sec_key)?;

            // Update header with new key material
            vault_store.header.argon2_salt          = new_salt;
            vault_store.header.vk_wrapped_classical = new_vk_wrapped;
            vault_store.header.kyber_public_key     = pub_key;
            vault_store.header.kyber_sk_encrypted   = kyber_sk_encrypted;
            vault_store.header.kyber_ciphertext     = kyber_ciphertext;
            vault_store.header.vk_wrapped_pq        = vk_wrapped_pq;

            // Update in-memory MK so subsequent operations use the new key
            key_store.master_key = crate::crypto::keys::MasterKey::new(*new_mk);

            // Sync KDF params to current constants
            vault_store.header.kdf_memory_kb   = kdf::ARGON2_MEMORY_KB;
            vault_store.header.kdf_iterations  = kdf::ARGON2_ITERATIONS;
            vault_store.header.kdf_parallelism = kdf::ARGON2_PARALLELISM;

            Ok(())
        })
    }).await;

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

    // Offload Argon2id KDF to a blocking thread pool thread
    let pwd_for_kdf = pwd_bytes.clone();
    let salt_for_kdf = salt;
    let mk_raw = tokio::task::spawn_blocking(move || {
        kdf::derive_master_key(&pwd_for_kdf, &salt_for_kdf)
    })
    .await
    .map_err(|_| CypheriaError::InternalError("KDF thread panicked".into()))??;
    let mk_bytes = Zeroizing::new(mk_raw);
    pwd_bytes.zeroize();

    let vk_bytes = Zeroizing::new(rng::entry_key());
    let vk_wrapped_classical = aes::wrap_key(&mk_bytes, &vk_bytes)?;

    let kp = kyber::generate_keypair();
    let pub_key = kp.public_key.clone();
    let sec_key = zeroize::Zeroizing::new(kp.secret_key.clone());
    drop(kp);

    let (kyber_ciphertext, vk_wrapped_pq) =
        kyber::encapsulate_vault_key(&pub_key, &vk_bytes)?;
    let kyber_sk_encrypted = aes::encrypt(&mk_bytes, &sec_key)?;

    let default_settings = Settings::default();
    let settings_json = serde_json::to_vec(&default_settings).map_err(|_| CypheriaError::SerdeError)?;
    // CRIT-002 fix: use mk_bytes through Zeroizing wrapper (already in scope), no extra key needed
    // Encrypt settings with a VK-derived subkey (not MK) so settings survive
    // master password changes without re-encryption.
    let mut settings_subkey = [0u8; 32];
    kdf::derive_subkey(&vk_bytes, b"SETTINGS_ENCRYPTION_VK", &mut settings_subkey);
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