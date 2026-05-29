//! VaultStore — in-memory decrypted vault state.
//!
//! Lives only inside SessionState::Unlocked.
//! Zeroized indirectly when the Unlocked variant is replaced with Locked
//! (EntryPayload and NotePayload inside VaultData implement ZeroizeOnDrop).

use std::path::Path;
use zeroize::Zeroize;
use crate::{
    crypto::{kdf, aes, keys::ActiveKeyStore},
    error::CypheriaError,
    vault::format::*,
};

/// In-memory vault state (decrypted).
pub struct VaultStore {
    pub data:   VaultData,
    pub header: VaultHeader,
}

/// Load a .qvault file and unlock it with the provided password.
///
/// Supports Version 1 (legacy) and Version 2 (full-file integrity).
pub async fn load_and_unlock(
    password: &[u8],
    path: &Path,
) -> Result<(ActiveKeyStore, VaultStore), CypheriaError> {
    if !tokio::fs::try_exists(&path)
        .await
        .map_err(|_| CypheriaError::VaultNotFound)?
    {
        return Err(CypheriaError::VaultNotFound);
    }

    let file_bytes = tokio::fs::read(path).await?;

    if !file_bytes.starts_with(MAGIC) {
        return Err(CypheriaError::VaultCorrupted);
    }

    // Parse Version (u16 LE at offset 9)
    let magic_len = MAGIC.len();
    if file_bytes.len() < magic_len + 2 {
        return Err(CypheriaError::VaultCorrupted);
    }
    let version = u16::from_le_bytes(
        file_bytes[magic_len..magic_len + 2]
            .try_into()
            .map_err(|_| CypheriaError::VaultCorrupted)?,
    );

    // Parse Header Length
    let header_len_offset = magic_len + 2;
    if file_bytes.len() < header_len_offset + 4 {
        return Err(CypheriaError::VaultCorrupted);
    }
    let header_len = u32::from_le_bytes(
        file_bytes[header_len_offset..header_len_offset + 4]
            .try_into()
            .map_err(|_| CypheriaError::VaultCorrupted)?,
    ) as usize;

    let header_start = header_len_offset + 4;
    let header_end   = header_start + header_len;

    if file_bytes.len() < header_end {
        return Err(CypheriaError::VaultCorrupted);
    }

    let header: VaultHeader = bincode::deserialize(&file_bytes[header_start..header_end])
        .map_err(|_| CypheriaError::VaultCorrupted)?;

    // Derive Master Key
    let password_vec: Vec<u8> = password.to_vec();
    let argon2_salt = header.argon2_salt;
    let kdf_memory_kb = header.kdf_memory_kb;
    let kdf_iterations = header.kdf_iterations;
    let kdf_parallelism = header.kdf_parallelism;
    let mk_bytes = tokio::task::spawn_blocking(move || {
        kdf::derive_master_key_with_params(
            &password_vec,
            &argon2_salt,
            kdf_memory_kb,
            kdf_iterations,
            kdf_parallelism,
        )
    })
    .await
    .map_err(|_| CypheriaError::InternalError("KDF thread panicked".into()))??;

    // Verify HMAC based on version
    let mut hmac_key = [0u8; 32];
    kdf::derive_subkey(&mk_bytes, b"HMAC_VAULT_INTEGRITY", &mut hmac_key);

    match version {
        1 => {
            // V1 HMAC covers only header — data section is unsigned. Auto-migrate to V2 on success.
            let hmac_start = header_end;
            let hmac_end   = hmac_start + 32;
            if file_bytes.len() < hmac_end + 4 {
                hmac_key.zeroize();
                return Err(CypheriaError::VaultCorrupted);
            }
            let covered_region = &file_bytes[..header_end];
            let expected_hmac  = &file_bytes[hmac_start..hmac_end];
            if let Err(e) = verify_vault_hmac(covered_region, expected_hmac, &hmac_key) {
                hmac_key.zeroize();
                return Err(e);
            }
            hmac_key.zeroize();

            let data_len_offset = hmac_end;
            if file_bytes.len() < data_len_offset + 4 {
                return Err(CypheriaError::VaultCorrupted);
            }
            let data_len = u32::from_le_bytes(
                file_bytes[data_len_offset..data_len_offset + 4]
                    .try_into()
                    .map_err(|_| CypheriaError::VaultCorrupted)?,
            ) as usize;
            let data_start = data_len_offset + 4;
            let data_end = data_start + data_len;
            if file_bytes.len() < data_end {
                return Err(CypheriaError::VaultCorrupted);
            }

            // Unwrap VK; zeroize mk_bytes on failure
            let vk_bytes = match aes::unwrap_key(&mk_bytes, &header.vk_wrapped_classical) {
                Ok(vk) => vk,
                Err(e) => {
                    let mut mk_mut = mk_bytes;
                    mk_mut.zeroize();
                    return Err(e);
                }
            };

            let vault_data = match decrypt_vault_data(&vk_bytes, &file_bytes[data_start..data_end]) {
                Ok(d) => d,
                Err(e) => {
                    let mut vk_mut = vk_bytes;
                    vk_mut.zeroize();
                    let mut mk_mut = mk_bytes;
                    mk_mut.zeroize();
                    return Err(e);
                }
            };

            let key_store = ActiveKeyStore::new(mk_bytes, vk_bytes);
            let vault_store = VaultStore { data: vault_data, header };

            // Migrate to V2 immediately so future opens get full-file HMAC coverage
            if let Err(e) = persist_vault(&key_store, &vault_store.data, &vault_store.header, path).await {
                eprintln!(
                    "[Cypheria] V1→V2 migration persist failed (non-fatal): {:?}", e
                );
            }

            Ok((key_store, vault_store))
        }
        2 => {
            // Version 2: HMAC(32) is at the VERY END. Covers everything before it.
            if file_bytes.len() < 32 {
                hmac_key.zeroize();
                return Err(CypheriaError::VaultCorrupted);
            }
            let hmac_start = file_bytes.len() - 32;
            let covered_region = &file_bytes[..hmac_start];
            let expected_hmac  = &file_bytes[hmac_start..];
            verify_vault_hmac(covered_region, expected_hmac, &hmac_key)?;

            // Data section is between header_end and hmac_start
            let data_len_offset = header_end;
            if file_bytes.len() < data_len_offset + 4 {
                hmac_key.zeroize();
                return Err(CypheriaError::VaultCorrupted);
            }
            let data_start = data_len_offset + 4;
            let vk_bytes = aes::unwrap_key(&mk_bytes, &header.vk_wrapped_classical)?;
            let vault_data = decrypt_vault_data(&vk_bytes, &file_bytes[data_start..hmac_start])?;
            hmac_key.zeroize();
            let key_store = ActiveKeyStore::new(mk_bytes, vk_bytes);
            Ok((key_store, VaultStore { data: vault_data, header }))
        }
        _ => {
            hmac_key.zeroize();
            Err(CypheriaError::InvalidInput(format!(
                "Unsupported vault format version {} (this build supports version 1 and 2).",
                version
            )))
        }
    }
}

/// Constant-time HMAC verification.
/// Prevents timing attacks that would reveal partial HMAC matches.
// FIX: IMPROVE-005 — #[must_use] ensures callers cannot silently ignore the Result.
#[must_use = "HMAC verification result must be checked — ignoring it bypasses tamper detection"]
fn verify_vault_hmac(
    covered_data: &[u8],
    expected_hmac: &[u8],
    key: &[u8; 32],
) -> Result<(), CypheriaError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use subtle::ConstantTimeEq;

    let mut mac = <Hmac<Sha256>>::new_from_slice(key)
        .map_err(|_| CypheriaError::CryptoError)?;
    mac.update(covered_data);
    let computed = mac.finalize().into_bytes();

    // ConstantTimeEq from the `subtle` crate — prevents timing oracle attacks
    if computed.as_slice().ct_eq(expected_hmac).into() {
        Ok(())
    } else {
        Err(CypheriaError::VaultCorrupted)
    }
}

fn decrypt_vault_data(vault_key: &[u8; 32], blob: &[u8]) -> Result<VaultData, CypheriaError> {
    let plaintext = aes::decrypt(vault_key, blob)?;
    bincode::deserialize(&plaintext).map_err(|_| CypheriaError::VaultCorrupted)
}

pub fn read_settings(
    vault_key: &[u8; 32],
    vault_data: &crate::vault::format::VaultData,
) -> crate::models::settings::Settings {
    use zeroize::Zeroize;
    let mut settings_key = [0u8; 32];
    crate::crypto::kdf::derive_subkey(vault_key, b"SETTINGS_ENCRYPTION_VK", &mut settings_key);
    let result = crate::crypto::aes::decrypt(&settings_key, &vault_data.settings.payload_encrypted);
    settings_key.zeroize();
    match result {
        Ok(json) => serde_json::from_slice::<crate::models::settings::Settings>(&json)
            .unwrap_or_default(),
        Err(_) => crate::models::settings::Settings::default(),
    }
}

/// Persist the current vault state to disk.
///
/// Writes in Version 2 format:
///   MAGIC(9) | VERSION(2) | HEADER_LEN(4) | HEADER(n) | DATA_LEN(4) | DATA(m) | HMAC(32)
///
/// HMAC covers EVERYTHING before the HMAC itself.
pub async fn persist_vault(
    key_store: &ActiveKeyStore,
    vault_data: &VaultData,
    header: &VaultHeader,
    path: &Path,
) -> Result<(), CypheriaError> {
    // Serialize and encrypt VaultData with Vault Key
    let data_plaintext = bincode::serialize(vault_data).map_err(|_| CypheriaError::SerdeError)?;
    let encrypted_data = aes::encrypt(key_store.vault_key_bytes(), &data_plaintext)?;

    // Serialize VaultHeader
    let header_bytes = bincode::serialize(header).map_err(|_| CypheriaError::SerdeError)?;
    let header_len   = (header_bytes.len() as u32).to_le_bytes();
    let data_len     = (encrypted_data.len() as u32).to_le_bytes();

    // Assemble the region to be HMAC-signed (Version 2 layout):
    // MAGIC + VERSION + HEADER_LEN + HEADER + DATA_LEN + DATA
    let mut file = Vec::new();
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    file.extend_from_slice(&header_len);
    file.extend_from_slice(&header_bytes);
    file.extend_from_slice(&data_len);
    file.extend_from_slice(&encrypted_data);

    // Derive HMAC subkey and sign the entire region
    let mut hmac_key = [0u8; 32];
    kdf::derive_subkey(key_store.master_key_bytes(), b"HMAC_VAULT_INTEGRITY", &mut hmac_key);

    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256>>::new_from_slice(&hmac_key)
        .map_err(|_| CypheriaError::CryptoError)?;
    mac.update(&file);
    let hmac_bytes = mac.finalize().into_bytes();
    hmac_key.zeroize();

    // Append HMAC to the end
    file.extend_from_slice(&hmac_bytes);

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp_path = {
        let mut name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        name.push_str(".tmp");
        path.with_file_name(name)
    };
    tokio::fs::write(&tmp_path, &file).await?;
    let rename_result = tokio::fs::rename(&tmp_path, path).await;
    if rename_result.is_err() {
        let _ = tokio::fs::remove_file(&tmp_path).await; // best-effort cleanup
        rename_result?;
    }

    Ok(())
}