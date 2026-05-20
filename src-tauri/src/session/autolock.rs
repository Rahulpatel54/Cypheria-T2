//! Inactivity auto-lock timer.
//!
//! A background async task polls last-activity every 10 seconds.
//! Every vault command resets the timer via bump_activity().
//! When inactivity exceeds the configured timeout, the vault is locked
//! and a "vault-auto-locked" Tauri event is emitted to the frontend.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use crate::session::manager::SessionManager;

pub struct AutoLockTimer {
    last_activity_unix_secs: Arc<AtomicU64>,
    timeout_secs:            Arc<AtomicU64>,
    running:                 Arc<AtomicBool>,
}

impl AutoLockTimer {
    pub fn new(timeout_secs: u64) -> Self {
        let now = now_secs();
        Self {
            last_activity_unix_secs: Arc::new(AtomicU64::new(now)),
            timeout_secs:            Arc::new(AtomicU64::new(timeout_secs)),
            running:                 Arc::new(AtomicBool::new(false)),
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

    /// Stop the background polling loop. Safe to call multiple times.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Spawn the background auto-lock polling task.
    /// Takes an AppHandle so it can emit the "vault-auto-locked" event.
    pub fn start(self: Arc<Self>, session: Arc<SessionManager>, app: tauri::AppHandle) {
        // Prevent double-start
        if self.running.swap(true, Ordering::Relaxed) {
            eprintln!("[Cypheria] AutoLockTimer::start() called while already running — ignoring");
            return;
        }

        let timer_ref = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                sleep(Duration::from_secs(10)).await; // Poll every 10 seconds

                // Exit if stopped
                if !timer_ref.running.load(Ordering::Relaxed) {
                    timer_ref.running.store(false, Ordering::Relaxed);
                    break;
                }

                let now     = now_secs();
                let last    = timer_ref.last_activity_unix_secs.load(Ordering::Relaxed);
                let timeout = timer_ref.timeout_secs.load(Ordering::Relaxed);

                // Skip if timeout is 0 (disabled)
                if timeout == 0 { continue; }

                if now.saturating_sub(last) >= timeout && session.is_unlocked().await {
                    session.lock().await;
                    use tauri::Emitter;
                    let _ = app.emit("vault-auto-locked", ());
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
