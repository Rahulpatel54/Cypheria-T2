//! Vault management commands — open, export, meta.
use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
    vault::format::VaultHeader,
};

/// Open an existing vault by validating the file and returning its canonical path.
/// The actual unlock (key derivation) is done via unlock_vault().
#[tauri::command]
pub async fn open_vault(vault_path: String) -> Result<String, CypheriaError> {
    safe_command!({
        use crate::vault::format::MAGIC;

        let path = std::path::PathBuf::from(&vault_path);

        if !path.exists() {
            return Err(CypheriaError::VaultNotFound);
        }

        // Peek at magic bytes to validate file type
        let mut file = tokio::fs::File::open(&path).await?;
        use tokio::io::AsyncReadExt;
        let mut magic_buf = [0u8; 9];
        file.read_exact(&mut magic_buf)
            .await
            .map_err(|_| CypheriaError::VaultCorrupted)?;

        if &magic_buf != MAGIC {
            return Err(CypheriaError::VaultCorrupted);
        }

        let canonical = path
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(vault_path);

        Ok(canonical)
    })
}

/// Export (copy) the current vault file to a destination path.
/// The file is already encrypted — no re-encryption needed.
#[tauri::command]
pub async fn export_vault(
    destination_path: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();

        let vault_path = session
            .vault_path()
            .await
            .ok_or(CypheriaError::VaultLocked)?;

        let dest = std::path::PathBuf::from(&destination_path);

        // BUG-011 fix: reject export to the vault's own path to prevent
        // tokio::fs::copy from truncating the source file mid-write.
        if dest.canonicalize().ok() == vault_path.canonicalize().ok() {
            return Err(CypheriaError::InvalidInput(
                "Cannot export vault to its own location. Choose a different path.".into(),
            ));
        }

        tokio::fs::copy(&vault_path, &dest).await?;
        Ok(())
    })
}

/// View model returned to the frontend — only non-sensitive header metadata.
#[derive(serde::Serialize)]
pub struct VaultMetaView {
    pub vault_name:     String,
    pub created_at:     String,
    pub format_version: u16,
}

/// Read vault header metadata WITHOUT unlocking (no key derivation, no HMAC check).
/// Used to display the vault name on the lock screen before the user enters a password.
///
/// SECURITY NOTE: This reads the plaintext header only. No secret material is accessed.
/// vault_name and created_at are stored in plaintext in VaultHeader by design.
#[tauri::command]
pub async fn get_vault_meta(vault_path: String) -> Result<VaultMetaView, CypheriaError> {
    use crate::vault::format::MAGIC;

    let path = std::path::PathBuf::from(&vault_path);

    if !path.exists() {
        return Err(CypheriaError::VaultNotFound);
    }

    let file_bytes = tokio::fs::read(&path).await?;

    if !file_bytes.starts_with(MAGIC) {
        return Err(CypheriaError::VaultCorrupted);
    }

    // Layout: MAGIC(9) + VERSION(2) + HEADER_LEN(4) + HEADER(n) + ...
    let magic_len = MAGIC.len();           // 9
    let header_len_offset = magic_len + 2; // 11

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

    Ok(VaultMetaView {
        vault_name:     header.vault_name.clone(),
        created_at:     header.created_at.to_rfc3339(),
        format_version: header.format_version,
    })
}