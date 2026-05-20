//! Cypheria — Quantum-Resistant Offline Password Manager
//! Rust/Tauri backend entry point.
//!
//! Registers all Tauri commands and initializes shared application state:
//!   - SessionManager: vault lock/unlock state machine
//!   - AutoLockTimer: inactivity timer background task

use std::sync::Arc;

pub mod error;
pub mod crypto;
pub mod vault;
pub mod session;
pub mod commands;
pub mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session  = Arc::new(session::manager::SessionManager::new());
    let autolock = Arc::new(session::autolock::AutoLockTimer::new(300)); // 5-minute default
    let clipboard_timer = Arc::new(crate::commands::clipboard::ClipboardTimer(
        Arc::new(tokio::sync::Mutex::new(None)),
));

    tauri::Builder::default()
        // ── Plugins ────────────────────────────────────────────────────────
        // The dialog plugin MUST be registered here before any frontend call
        // to `plugin:dialog|save` / `plugin:dialog|open` will work.
        .plugin(tauri_plugin_dialog::init())
        // ── Managed state ─────────────────────────────────────────────────
        .manage(session.clone())
        .manage(autolock.clone())
        .manage(clipboard_timer.clone())
        .invoke_handler(tauri::generate_handler![
            // Auth & vault lifecycle
            commands::auth::unlock_vault,
            commands::auth::lock_vault,
            commands::auth::create_vault,
            commands::auth::change_master_password,
            // Entry CRUD
            commands::entries::get_all_entries,
            commands::entries::get_entry_password,
            commands::entries::add_entry,
            commands::entries::update_entry,
            commands::entries::delete_entry,
            commands::entries::toggle_favorite,
            commands::entries::update_entry_keep_password,
            // Notes CRUD
            commands::notes::get_all_notes,
            commands::notes::save_note,
            commands::notes::delete_note,
            commands::notes::get_note_content,
            // Password generator
            commands::generator::generate_password,
            // Settings
            commands::settings::get_settings,
            commands::settings::save_settings,
            // Vault management
            commands::vault_mgmt::open_vault,
            commands::vault_mgmt::export_vault,
            commands::vault_mgmt::get_vault_meta,
            // Vault path persistence (BUG-002 fix — replaces localStorage)
            commands::vault_path::get_last_vault_path,
            commands::vault_path::set_last_vault_path,
            commands::vault_path::clear_last_vault_path,
            commands::clipboard::copy_entry_password_to_clipboard,
            commands::clipboard::clear_clipboard,
        ])
        .setup(move |app| {
            // Start auto-lock background task (needs AppHandle for event emission)
            let autolock_clone  = autolock.clone();
            let session_clone   = session.clone();
            let app_handle      = app.handle().clone();
            autolock_clone.start(session_clone, app_handle);

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
