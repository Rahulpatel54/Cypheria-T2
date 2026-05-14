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
/// Security steps (in order):
///   1. Verify magic bytes
///   2. Parse and deserialize the header
///   3. Verify format version — reject files from incompatible future versions
///   4. Derive Master Key from password + salt using header-stored KDF params
///   5. Derive HMAC subkey; verify HMAC over MAGIC+VERSION+HEADER_LEN+HEADER
///   6. Unwrap Vault Key (classical path — AES-GCM with MK)
///   7. Decrypt VaultData with VK
///   8. Return (ActiveKeyStore, VaultStore)
///
/// File layout on disk:
///   MAGIC(9) | VERSION(2) | HEADER_LEN(4) | HEADER(n) | HMAC(32) | DATA_LEN(4) | DATA(m)
///
/// The HMAC covers only: MAGIC + VERSION + HEADER_LEN + HEADER
/// (i.e. file_bytes[0..header_end])
pub async fn load_and_unlock(
    password: &[u8],
    path: &Path,
) -> Result<(ActiveKeyStore, VaultStore), CypheriaError> {
    // ERR-005 fix: use the async variant so the Tokio thread is not blocked
    // during the filesystem round-trip.
    if !tokio::fs::try_exists(&path)
        .await
        .map_err(|_| CypheriaError::VaultNotFound)?
    {
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

    // FIX: IMPROVE-007 — reject vault files written by an incompatible format version.
    // A vault with format_version > FORMAT_VERSION was written by a newer build and
    // may use layout changes we do not understand. Reject it with a clear message.
    // A vault with format_version < FORMAT_VERSION could be migrated; for now we
    // require an exact match and surface a descriptive error.
    if header.format_version != FORMAT_VERSION {
        return Err(CypheriaError::InvalidInput(format!(
            "Unsupported vault format version {} (this build supports version {}). \
             Please use a compatible version of Cypheria.",
            header.format_version, FORMAT_VERSION
        )));
    }

    // Step 3: Derive Master Key
    // ERR-003 fix: use the KDF params stored in the header, not the compile-time
    // constants, so vaults created under old params remain unlockable.
    let mk_bytes = kdf::derive_master_key_with_params(
        password,
        &header.argon2_salt,
        header.kdf_memory_kb,
        header.kdf_iterations,
        header.kdf_parallelism,
    )?;

    // Step 4: Verify HMAC over exactly MAGIC + VERSION + HEADER_LEN + HEADER
    // This must match what persist_vault() signed.
    let mut hmac_key = [0u8; 32];
    kdf::derive_subkey(&mk_bytes, b"HMAC_VAULT_INTEGRITY", &mut hmac_key);

    // covered_region = file_bytes[0..header_end] = MAGIC+VERSION+HEADER_LEN+HEADER
    let covered_region = &file_bytes[..header_end];

    verify_vault_hmac(
        covered_region,
        &file_bytes[header_end..hmac_end],
        &hmac_key,
    )?;
    hmac_key.zeroize();

    // Step 5: Unwrap Vault Key (classical path)
    let vk_bytes = aes::unwrap_key(&mk_bytes, &header.vk_wrapped_classical)
        .map_err(|_| CypheriaError::AuthFailed)?;

    // Step 6: Build ActiveKeyStore (owns both keys; zeroized on drop)
    let key_store = ActiveKeyStore::new(mk_bytes, vk_bytes);

    // Step 7: Decrypt VaultData
    // Layout after header_end: HMAC(32) | DATA_LEN(4) | DATA(m)
    let data_start = hmac_end + 4; // skip DATA_LEN u32

    if file_bytes.len() < data_start {
        return Err(CypheriaError::VaultCorrupted);
    }

    let vault_data = decrypt_vault_data(key_store.vault_key_bytes(), &file_bytes[data_start..])?;

    Ok((key_store, VaultStore { data: vault_data, header }))
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

/// Persist the current vault state to disk.
///
/// File layout written:
///   MAGIC(9) | VERSION(2) | HEADER_LEN(4) | HEADER(n) | HMAC(32) | DATA_LEN(4) | DATA(m)
///
/// HMAC is computed over exactly: MAGIC + VERSION + HEADER_LEN + HEADER
/// (i.e. the first header_end bytes of the output file).
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

    // Build the region to be HMAC-signed:
    // MAGIC + VERSION + HEADER_LEN + HEADER
    // This is exactly what load_and_unlock() reads as file_bytes[0..header_end].
    let mut covered_for_hmac = Vec::new();
    covered_for_hmac.extend_from_slice(MAGIC);
    covered_for_hmac.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    covered_for_hmac.extend_from_slice(&header_len);
    covered_for_hmac.extend_from_slice(&header_bytes);

    // Derive HMAC subkey and sign
    let mut hmac_key = [0u8; 32];
    kdf::derive_subkey(key_store.master_key_bytes(), b"HMAC_VAULT_INTEGRITY", &mut hmac_key);

    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = <Hmac<Sha256>>::new_from_slice(&hmac_key)
        .map_err(|_| CypheriaError::CryptoError)?;
    mac.update(&covered_for_hmac);
    let hmac_bytes = mac.finalize().into_bytes();
    hmac_key.zeroize();

    // Assemble final file:
    // MAGIC + VERSION + HEADER_LEN + HEADER | HMAC | DATA_LEN | DATA
    let mut file = covered_for_hmac; // already contains the signed header region
    file.extend_from_slice(&hmac_bytes);
    file.extend_from_slice(&data_len);
    file.extend_from_slice(&encrypted_data);

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
    tokio::fs::rename(&tmp_path, path).await?;

    Ok(())
}