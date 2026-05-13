/// Tauri command layer — every public #[tauri::command] lives in a sub-module here.
///
/// Sub-modules:
///   auth        — unlock_vault, lock_vault, create_vault, change_master_password
///   entries     — get_all_entries, get_entry_password, add_entry, update_entry,
///                 delete_entry, toggle_favorite
///   notes       — get_all_notes, save_note, delete_note
///   generator   — generate_password (server-side CSPRNG with rejection sampling)
///   settings    — get_settings, save_settings
///   vault_mgmt  — open_vault, export_vault
///   vault_path  — get_last_vault_path, set_last_vault_path, clear_last_vault_path
///                 (persist last-used vault path in OS app-data; no localStorage used)
///
/// SECURITY CONTRACT:
///   Every command in this layer MUST:
///     1. Call `autolock.bump_activity()` as its first action (where session-guarded).
///     2. Route all vault access through `session.with_session()`.
///     3. Validate all untrusted inputs before any processing.
///     4. Never log passwords, key bytes, or plaintext credential data.
#[macro_export]
macro_rules! safe_command {
    ($body:block) => {
        $body
    };
}

#[macro_export]
macro_rules! catch_sync_panic {
    ($body:block) => {{
        use std::panic::{self, AssertUnwindSafe};
        use $crate::error::CypheriaError;

        panic::catch_unwind(AssertUnwindSafe(|| $body)).unwrap_or_else(|payload| {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic payload".to_string()
            };
            eprintln!("[Cypheria] caught sync panic in vault closure: {msg}");
            Err(CypheriaError::InternalError(
                "An unexpected internal error occurred".to_string(),
            ))
        })
    }};
}

pub mod auth;
pub mod entries;
pub mod notes;
pub mod generator;
pub mod settings;
pub mod vault_mgmt;
pub mod vault_path;
pub mod clipboard;