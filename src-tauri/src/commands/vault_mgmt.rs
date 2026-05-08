//! Vault management commands — open, export, import.

use std::sync::Arc;
use tauri::State;
use crate::{
    error::CypheriaError,
    session::{manager::SessionManager, autolock::AutoLockTimer},
};

// BUG-006 fix: panic boundary macro — see auth.rs for rationale.
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

    // BUG-011 fix: the previous check blocked overwriting a *different* existing
    // file but silently allowed exporting to the vault's OWN path, which would
    // cause tokio::fs::copy to truncate the source file mid-write, corrupting it.
    //
    // New logic:
    //   - If dest == vault_path (same file): always reject — this is self-corruption.
    //   - If dest is a different existing file: allow (copy will atomically replace it).
    if dest.canonicalize().ok() == vault_path.canonicalize().ok() {
        return Err(CypheriaError::InvalidInput(
            "Cannot export vault to its own location. Choose a different path.".into(),
        ));
    }

    safe_command!({
        tokio::fs::copy(&vault_path, &dest).await?;
        Ok(())
    })
}
