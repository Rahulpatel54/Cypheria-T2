//! Authentication commands — vault unlock, lock, create, change password.

use std::sync::Arc;
use tauri::{AppHandle, State, Emitter};
use zeroize::Zeroize;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
};

// BUG-006 fix: macro that wraps any command body in catch_unwind so a panic
// cannot bypass ZeroizeOnDrop and leave key material live in freed memory.
macro_rules! safe_command {
    ($body:block) => {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)) {
            Ok(result) => result,
            Err(_) => Err(CypheriaError::InternalError(
                "Unexpected internal error".into(),
            )),
        }
    };
}

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

    // BUG-006 fix: wrap the entire async body in safe_command! via a sync
    // closure passed to with_session_mut (which is already sync inside).
    let result = session.with_session_mut(|key_store, vault_store| {
        safe_command!({
            use crate::crypto::{kdf, aes, kyber, rng};
            use subtle::ConstantTimeEq;

            // 1. Verify old password matches current MK
            let derived_mk = kdf::derive_master_key(&old_bytes, &vault_store.header.argon2_salt)?;
            let matches: bool = derived_mk.ct_eq(key_store.master_key_bytes()).into();
            if !matches {
                return Err(CypheriaError::AuthFailed);
            }

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

            key_store.master_key = crate::crypto::keys::MasterKey(new_mk);

            // BUG-005 fix: sync KDF params in the header to current constants.
            // Without this, if ARGON2_* constants change between releases, a
            // vault whose password was changed under the old constants will
            // store stale params, causing unlock to derive a different key and
            // fail authentication.
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
    let salt     = rng::argon2_salt();
    let mk_bytes = kdf::derive_master_key(&pwd_bytes, &salt)?;
    pwd_bytes.zeroize();

    let vk_bytes: [u8; 32] = rng::entry_key();
    let vk_wrapped_classical = aes::wrap_key(&mk_bytes, &vk_bytes)?;

    let kp = kyber::generate_keypair();
    let pub_key = kp.public_key.clone();
    let sec_key = kp.secret_key.clone();
    drop(kp);

    let (kyber_ciphertext, vk_wrapped_pq) =
        kyber::encapsulate_vault_key(&pub_key, &vk_bytes)?;
    let kyber_sk_encrypted = aes::encrypt(&mk_bytes, &sec_key)?;

    let default_settings = Settings::default();
    let settings_json = serde_json::to_vec(&default_settings).map_err(|_| CypheriaError::SerdeError)?;
    let settings_encrypted = aes::encrypt(&mk_bytes, &settings_json)?;

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
        let key_store = ActiveKeyStore::new(mk_bytes, vk_bytes);
        persist_vault(&key_store, &vault_data, &header, &path).await?;
    }

    Ok(())
}
