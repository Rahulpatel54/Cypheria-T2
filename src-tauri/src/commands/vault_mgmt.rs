//! Vault management commands — open, export, import.

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
};

/// Open an existing vault by setting the vault path.
/// The actual unlock (key derivation) is done via unlock_vault().
/// This command just validates the file exists and looks like a .qvault file.
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
    file.read_exact(&mut magic_buf).await
        .map_err(|_| CypheriaError::VaultCorrupted)?;

    if &magic_buf != MAGIC {
        return Err(CypheriaError::VaultCorrupted);
    }

    // Return the canonical path for the frontend to store
    let canonical = path.canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or(vault_path);

    Ok(canonical)
}

/// Export (copy) the current vault file to a destination path.
/// Useful for USB backup. The file is already encrypted — no re-encryption needed.
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

    // Prevent overwrite of different file without confirmation
    if dest.exists() && dest.canonicalize().ok() != vault_path.canonicalize().ok() {
        return Err(CypheriaError::InvalidInput(
            "Destination file already exists. Choose a different path.".into(),
        ));
    }

    tokio::fs::copy(&vault_path, &dest).await?;
    Ok(())
}
