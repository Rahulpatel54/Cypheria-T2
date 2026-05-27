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
/// Wraps a Tauri command body with synchronous panic catching.
/// Any panic inside the block is caught, logged, and converted to
/// `CypheriaError::InternalError` rather than crashing the process.
///
/// All #[tauri::command] functions MUST use this macro as their outer wrapper.
#[macro_export]
macro_rules! safe_command {
    ($body:block) => {{
        #[cfg(debug_assertions)]
        let _cmd_start = std::time::Instant::now();

        let result = { $body };

        #[cfg(debug_assertions)]
        eprintln!("[Cypheria] command completed in {:.2}ms", _cmd_start.elapsed().as_secs_f64() * 1000.0);

        result
    }};
}

#[macro_export]
macro_rules! catch_sync_panic {
    ($body:block) => {{
        use std::panic::{self, AssertUnwindSafe};
        use $crate::error::CypheriaError;

        // SAFETY: This macro only catches SYNCHRONOUS panics.
        // Do NOT use .await inside $body — catch_unwind cannot protect against
        // panics in async code. AssertUnwindSafe is required because &mut VaultStore
        // does not implement UnwindSafe.
        let sync_result = panic::catch_unwind(AssertUnwindSafe(|| $body));
        sync_result.unwrap_or_else(|payload| {
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
pub mod reveal;


/// Validate that `id` is a well-formed UUID string.
/// Returns `Err(InvalidInput)` if parsing fails.
#[must_use = "UUID validation result must be checked with ?"]
pub(crate) fn validate_uuid(id: &str) -> Result<(), crate::error::CypheriaError> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| crate::error::CypheriaError::InvalidInput("Invalid ID format".into()))
}