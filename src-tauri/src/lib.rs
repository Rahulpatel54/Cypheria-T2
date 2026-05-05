//! Cypheria — Quantum-Resistant Offline Password Manager
//! Rust/Tauri backend entry point.
//!
//! Registers all Tauri commands and initializes shared application state:
//!   - SessionManager: vault lock/unlock state machine
//!   - AutoLockTimer: inactivity timer background task

use std::sync::Arc;
use tauri::Manager;

pub mod error;
pub mod crypto;
pub mod vault;
pub mod session;
pub mod commands;
pub mod models;

/// Application state shared across all Tauri commands via State<'_, Arc<T>>.
pub struct AppState {
    pub session:  Arc<session::manager::SessionManager>,
    pub autolock: Arc<session::autolock::AutoLockTimer>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session  = Arc::new(session::manager::SessionManager::new());
    let autolock = Arc::new(session::autolock::AutoLockTimer::new(300)); // 5-minute default

    tauri::Builder::default()
        .manage(session.clone())
        .manage(autolock.clone())
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
            // Notes CRUD
            commands::notes::get_all_notes,
            commands::notes::save_note,
            commands::notes::delete_note,
            // Password generator
            commands::generator::generate_password,
            // Settings
            commands::settings::get_settings,
            commands::settings::save_settings,
            // Vault management
            commands::vault_mgmt::open_vault,
            commands::vault_mgmt::export_vault,
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
