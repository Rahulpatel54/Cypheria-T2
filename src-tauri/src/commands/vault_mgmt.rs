//! Vault management commands — open, export.

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
};

/// Open an existing vault by validating the file and returning its canonical path.
/// The actual unlock (key derivation) is done via unlock_vault().
#[tauri::command]
pub async fn open_vault(vault_path: String) -> Result<String, CypheriaError> {
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
}

/// Export (copy) the current vault file to a destination path.
/// The file is already encrypted — no re-encryption needed.
#[tauri::command]
pub async fn export_vault(
    destination_path: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
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
}