//! Session management — vault lock/unlock state machine and inactivity timer.
//!
//! Sub-modules:
//!   manager   — SessionManager: Locked / Unlocked / RateLimited state machine.
//!               Exposes `with_session()` guard so no command can touch key
//!               material unless the vault is unlocked.
//!   autolock  — AutoLockTimer: background async task that locks the vault
//!               after a configurable period of inactivity and emits the
//!               `vault-auto-locked` Tauri event to the frontend.

pub mod autolock;
pub mod manager;
