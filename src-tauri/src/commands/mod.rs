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
///
/// SECURITY CONTRACT:
///   Every command in this layer MUST:
///     1. Call `autolock.bump_activity()` as its first action.
///     2. Route all vault access through `session.with_session()`.
///     3. Validate all untrusted inputs before any processing.
///     4. Never log passwords, key bytes, or plaintext credential data.

pub mod auth;
pub mod entries;
pub mod notes;
pub mod generator;
pub mod settings;
pub mod vault_mgmt;
