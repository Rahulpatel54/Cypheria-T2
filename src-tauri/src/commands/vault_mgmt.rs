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

        if magic_buf != MAGIC {
            return Err(CypheriaError::VaultCorrupted);
        }

        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.len() < 200 {
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

        let tmp_dest = dest.with_extension("qvault.tmp");
        match tokio::fs::copy(&vault_path, &tmp_dest).await {
            Ok(_) => {},
            Err(e) => return Err(CypheriaError::IoError(e)),
        }
        let rename_result = tokio::fs::rename(&tmp_dest, &dest).await;
        if rename_result.is_err() {
            let _ = tokio::fs::remove_file(&tmp_dest).await;
            rename_result?;
        }
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
    safe_command!({
        use crate::vault::format::MAGIC;
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncSeekExt;

        let path = std::path::PathBuf::from(&vault_path);

        if !path.exists() {
            return Err(CypheriaError::VaultNotFound);
        }

        let mut file = tokio::fs::File::open(&path).await?;
        
        // Step 1: Verify Magic
        let mut magic_buf = [0u8; 9];
        file.read_exact(&mut magic_buf).await.map_err(|_| CypheriaError::VaultCorrupted)?;
        if magic_buf != MAGIC {
            return Err(CypheriaError::VaultCorrupted);
        }

        // Step 2: Skip Version (2 bytes)
        file.seek(std::io::SeekFrom::Current(2)).await?;

        // Step 3: Read Header Len
        let mut len_buf = [0u8; 4];
        file.read_exact(&mut len_buf).await.map_err(|_| CypheriaError::VaultCorrupted)?;
        let header_len = u32::from_le_bytes(len_buf) as usize;

        // Step 4: Read Header
        let mut header_bytes = vec![0u8; header_len];
        file.read_exact(&mut header_bytes).await.map_err(|_| CypheriaError::VaultCorrupted)?;

        let header: VaultHeader = bincode::deserialize(&header_bytes)
            .map_err(|_| CypheriaError::VaultCorrupted)?;

        Ok(VaultMetaView {
            vault_name:     header.vault_name.clone(),
            created_at:     header.created_at.to_rfc3339(),
            format_version: header.format_version,
        })
    })
}