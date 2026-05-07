//! Vault path persistence — stores the last opened vault path in the OS app-data directory.
//!
//! SECURITY:
//!   - The path is stored in `{app_data_dir}/cypheria/last_vault.json`, NOT in localStorage.
//!   - localStorage is accessible to injected JS; app-data is OS-protected and not web-accessible.
//!   - The file contains only the filesystem path string — no key material, no credentials.
//!
//! These commands are intentionally NOT session-guarded (no `with_session` call) because
//! they are called before the vault is unlocked, during startup path resolution.

use tauri::{AppHandle, Manager};
use crate::error::CypheriaError;

/// Returns the path to the persistence file: `{app_data_dir}/cypheria/last_vault.json`
fn persistence_path(app: &AppHandle) -> Result<std::path::PathBuf, CypheriaError> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|_| CypheriaError::InvalidInput("Cannot resolve app data directory".into()))?;
    Ok(app_data.join("last_vault.json"))
}

/// Read the last-used vault path from the OS app-data directory.
///
/// Returns `None` if no path has been stored or if the file is missing/corrupt.
/// Never panics — all errors return `None` gracefully.
#[tauri::command]
pub async fn get_last_vault_path(app: AppHandle) -> Result<Option<String>, CypheriaError> {
    let path = match persistence_path(&app) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    if !path.exists() {
        return Ok(None);
    }

    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    // File contains a JSON string: `"path/to/vault.qvault"`
    let parsed: Option<String> = serde_json::from_str(&contents).ok().flatten();
    Ok(parsed)
}

/// Write the last-used vault path to the OS app-data directory.
///
/// Creates the directory if it doesn't exist.
/// The path argument must be a non-empty string.
#[tauri::command]
pub async fn set_last_vault_path(
    path: String,
    app: AppHandle,
) -> Result<(), CypheriaError> {
    if path.trim().is_empty() {
        return Err(CypheriaError::InvalidInput("Vault path cannot be empty".into()));
    }

    let file_path = persistence_path(&app)?;

    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let json = serde_json::to_string(&Some(path.trim()))
        .map_err(|_| CypheriaError::SerdeError)?;

    tokio::fs::write(&file_path, json.as_bytes()).await?;
    Ok(())
}

/// Delete the persisted vault path (e.g., when the user wants to start fresh
/// or after the vault file is detected as missing on startup).
#[tauri::command]
pub async fn clear_last_vault_path(app: AppHandle) -> Result<(), CypheriaError> {
    let file_path = match persistence_path(&app) {
        Ok(p) => p,
        Err(_) => return Ok(()), // Nothing to clear
    };

    if file_path.exists() {
        tokio::fs::remove_file(&file_path).await?;
    }
    Ok(())
}
