//! Inactivity auto-lock timer.
//!
//! A background async task polls last-activity every 10 seconds.
//! Every vault command resets the timer via bump_activity().
//! When inactivity exceeds the configured timeout, the vault is locked
//! and a "vault-auto-locked" Tauri event is emitted to the frontend.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use crate::session::manager::SessionManager;

pub struct AutoLockTimer {
    last_activity_unix_secs: Arc<AtomicU64>,
    timeout_secs:            Arc<AtomicU64>,
}

impl AutoLockTimer {
    pub fn new(timeout_secs: u64) -> Self {
        let now = now_secs();
        Self {
            last_activity_unix_secs: Arc::new(AtomicU64::new(now)),
            timeout_secs:            Arc::new(AtomicU64::new(timeout_secs)),
        }
    }

    /// Reset the inactivity timer. Call on every vault command.
    pub fn bump_activity(&self) {
        self.last_activity_unix_secs.store(now_secs(), Ordering::Relaxed);
    }

    /// Update the timeout (called from save_settings).
    pub fn set_timeout(&self, secs: u64) {
        self.timeout_secs.store(secs, Ordering::Relaxed);
    }

    pub fn get_timeout(&self) -> u64 {
        self.timeout_secs.load(Ordering::Relaxed)
    }

    /// Spawn the background auto-lock polling task.
    /// Takes an AppHandle so it can emit the "vault-auto-locked" event.
    pub fn start(self: Arc<Self>, session: Arc<SessionManager>, app: tauri::AppHandle) {
        let timer_ref = self.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(10)).await; // Poll every 10 seconds

                let now     = now_secs();
                let last    = timer_ref.last_activity_unix_secs.load(Ordering::Relaxed);
                let timeout = timer_ref.timeout_secs.load(Ordering::Relaxed);

                // Skip if timeout is 0 (disabled)
                if timeout == 0 { continue; }

                if now.saturating_sub(last) >= timeout {
                    if session.is_unlocked().await {
                        session.lock().await;
                        // Notify the frontend so it can show the lock screen
                        use tauri::Emitter;
                        let _ = app.emit("vault-auto-locked", ());
                    }
                }
            }
        });
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
