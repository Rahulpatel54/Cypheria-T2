//! VaultStore — in-memory decrypted vault state.
//!
//! Lives only inside SessionState::Unlocked.
//! Zeroized indirectly when the Unlocked variant is replaced with Locked
//! (EntryPayload and NotePayload inside VaultData implement ZeroizeOnDrop).

use std::path::Path;
use zeroize::Zeroize;
use crate::{
    crypto::{kdf, aes, keys::ActiveKeyStore, rng},
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
/// Security steps (in order):
///   1. Verify magic bytes
///   2. Parse and deserialize the header
///   3. Derive Master Key from password + salt
///   4. Derive HMAC subkey; verify file HMAC (tamper detection — before any decrypt)
///   5. Unwrap Vault Key (classical path — AES-GCM with MK)
///   6. Decrypt VaultData with VK
///   7. Return (ActiveKeyStore, VaultStore)
pub async fn load_and_unlock(
    password: &[u8],
    path: &Path,
) -> Result<(ActiveKeyStore, VaultStore), CypheriaError> {
    if !path.exists() {
        return Err(CypheriaError::VaultNotFound);
    }

    let file_bytes = tokio::fs::read(path).await?;

    // Step 1: Verify magic prefix
    if !file_bytes.starts_with(MAGIC) {
        return Err(CypheriaError::VaultCorrupted);
    }

    // Step 2: Parse header length and deserialize header
    // Layout: MAGIC(9) + VERSION(2) + HEADER_LEN(4) + HEADER(n) + HMAC(32) + DATA_LEN(4) + DATA(m)
    let magic_len = MAGIC.len();         // 9
    let version_offset = magic_len;      // 9
    let header_len_offset = version_offset + 2; // 11

    if file_bytes.len() < header_len_offset + 4 {
        return Err(CypheriaError::VaultCorrupted);
    }

    let header_len = u32::from_le_bytes(
        file_bytes[header_len_offset..header_len_offset + 4]
            .try_into()
            .map_err(|_| CypheriaError::VaultCorrupted)?,
    ) as usize;

    let header_start = header_len_offset + 4; // 15
    let header_end   = header_start + header_len;
    let hmac_end     = header_end + 32;

    if file_bytes.len() < hmac_end + 4 {
        return Err(CypheriaError::VaultCorrupted);
    }

    let header: VaultHeader = bincode::deserialize(&file_bytes[header_start..header_end])
        .map_err(|_| CypheriaError::VaultCorrupted)?;

    // Step 3: Derive Master Key
    let mut mk_bytes = kdf::derive_master_key(password, &header.argon2_salt)?;

    // Step 4: Verify HMAC over the header section (tamper detection before decryption)
    let mut hmac_key = [0u8; 32];
    kdf::derive_subkey(&mk_bytes, b"HMAC_VAULT_INTEGRITY", &mut hmac_key);
    verify_vault_hmac(
        &file_bytes[..header_end],
        &file_bytes[header_end..hmac_end],
        &hmac_key,
    )?;
    hmac_key.zeroize();

    // Step 5: Unwrap Vault Key (classical path)
    let vk_bytes = aes::unwrap_key(&mk_bytes, &header.vk_wrapped_classical)
        .map_err(|_| CypheriaError::AuthFailed)?;

    // Step 6: Build ActiveKeyStore (owns both keys; ZeroizeOnDrop)
    let key_store = ActiveKeyStore::new(mk_bytes, vk_bytes);

    // Step 7: Decrypt VaultData
    let data_len_offset = hmac_end;
    let data_start = data_len_offset + 4;

    if file_bytes.len() < data_start {
        return Err(CypheriaError::VaultCorrupted);
    }

    let vault_data = decrypt_vault_data(key_store.vault_key_bytes(), &file_bytes[data_start..])?;

    Ok((key_store, VaultStore { data: vault_data, header }))
}

/// Constant-time HMAC verification.
/// Prevents timing attacks that would reveal partial HMAC matches.
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

/// Persist the current vault state to disk.
///
/// SECURITY: Uses atomic write — writes to a .tmp file then renames.
/// A crash or power loss mid-write leaves the original file intact.
/// The OS-level rename() is atomic, so the vault file is never partially written.
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

    // Compute HMAC over: MAGIC + VERSION + HEADER_LEN + HEADER
    let mut hmac_key = [0u8; 32];
    kdf::derive_subkey(key_store.master_key_bytes(), b"HMAC_VAULT_INTEGRITY", &mut hmac_key);

    let mut covered = Vec::new();
    covered.extend_from_slice(MAGIC);
    covered.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    covered.extend_from_slice(&header_len);
    covered.extend_from_slice(&header_bytes);

    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256>>::new_from_slice(&hmac_key)
        .map_err(|_| CypheriaError::CryptoError)?;
    mac.update(&covered);
    let hmac_bytes = mac.finalize().into_bytes();
    hmac_key.zeroize();

    // Assemble final file
    let mut file = covered; // reuse the already-built prefix
    file.extend_from_slice(&hmac_bytes);
    file.extend_from_slice(&data_len);
    file.extend_from_slice(&encrypted_data);

     if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp_path = path.with_extension("qvault.tmp");
    tokio::fs::write(&tmp_path, &file).await?;
    tokio::fs::rename(&tmp_path, path).await?;

    Ok(())
}
