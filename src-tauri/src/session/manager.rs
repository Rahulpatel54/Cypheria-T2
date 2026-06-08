//! Session state machine.

use crate::{crypto::keys::ActiveKeyStore, error::CypheriaError, vault::store::VaultStore};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// NOTE: This key is derived from username + static string and is intentionally
// tamper-EVIDENT, not tamper-PROOF. A local attacker with filesystem access can
// compute the same key and forge the attempts file. The persistent counter is
// a defence-in-depth measure against casual restarts, not against a determined
// local attacker. For stronger protection, store the key in the OS keychain.
fn attempts_hmac_key() -> [u8; 32] {
    let mut buf = String::new();
    if let Ok(u) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        buf.push_str(&u);
    }
    buf.push_str("cypheria.attempts.v1");
    let mut key = [0u8; 32];
    let bytes = buf.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        key[i % 32] ^= *b;
    }
    key
}

fn sign_attempts(payload: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let key = attempts_hmac_key();
    let mut mac = <Hmac<Sha256>>::new_from_slice(&key).expect("hmac accepts any key");
    mac.update(payload.as_bytes());
    let tag = mac.finalize().into_bytes();
    tag.iter().map(|b| format!("{:02x}", b)).collect()
}

fn verify_attempts(payload: &str, tag: &str) -> bool {
    sign_attempts(payload) == tag
}

fn attempts_file_path() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|d| d.join("cypheria").join(".attempts"))
}

fn read_persisted_attempts() -> (u32, u64) {
    let path = match attempts_file_path() {
        Some(p) => p,
        None => return (0, 0),
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return (0, 0);
    };
    let Ok(s) = std::str::from_utf8(&bytes) else {
        return (0, 0);
    };
    let mut outer = s.trim().splitn(2, '|');
    let payload = outer.next().unwrap_or("");
    let tag = outer.next().unwrap_or("");
    if tag.is_empty() || !verify_attempts(payload, tag) {
        eprintln!("[Cypheria] attempts file failed HMAC verification — resetting to max");
        return (MAX_UNLOCK_ATTEMPTS, 0);
    }
    let parts: Vec<&str> = payload.splitn(2, ':').collect();
    if parts.len() != 2 {
        return (0, 0);
    }
    let count = parts[0].parse::<u32>().unwrap_or(0);
    let until = parts[1].parse::<u64>().unwrap_or(0);
    (count, until)
}

fn write_persisted_attempts(count: u32, lockout_until_unix_secs: u64) {
    let Some(path) = attempts_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let payload = format!("{}:{}", count, lockout_until_unix_secs);
    let tag = sign_attempts(&payload);
    let _ = std::fs::write(&path, format!("{}|{}", payload, tag));
}

fn clear_persisted_attempts() {
    if let Some(path) = attempts_file_path() {
        let _ = std::fs::remove_file(path);
    }
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub const MAX_UNLOCK_ATTEMPTS: u32 = 5;
pub const LOCKOUT_DURATION_SECS: u64 = 30;

pub enum SessionState {
    Locked,
    Unlocked {
        key_store: ActiveKeyStore,
        vault_store: Box<VaultStore>,
        vault_path: std::path::PathBuf,
        unlocked_at: Instant,
    },
    RateLimited {
        locked_until: Instant,
        attempts: u32,
    },
}

pub struct SessionManager {
    state: Arc<RwLock<SessionState>>,
    attempts: Arc<AtomicU32>,
    /// Tracks password reveal calls in the current window for rate limiting
    reveal_count: Arc<AtomicU32>,
    /// Unix timestamp of when the current reveal window started
    reveal_window_start: Arc<AtomicU64>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(SessionState::Locked)),
            attempts: Arc::new(AtomicU32::new(0)),
            reveal_count: Arc::new(AtomicU32::new(0)),
            reveal_window_start: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn check_reveal_rate_limit(&self) -> Result<(), CypheriaError> {
        const MAX_REVEALS_PER_WINDOW: u32 = 10;
        const WINDOW_SECS: u64 = 60;

        let now = now_unix_secs();
        let window_start = self.reveal_window_start.load(Ordering::SeqCst);

        if now.saturating_sub(window_start) >= WINDOW_SECS {
            self.reveal_count.store(1, Ordering::SeqCst);
            self.reveal_window_start.store(now, Ordering::SeqCst);
            return Ok(());
        }

        let count = self.reveal_count.fetch_add(1, Ordering::SeqCst) + 1;
        if count > MAX_REVEALS_PER_WINDOW {
            let remaining = WINDOW_SECS.saturating_sub(now.saturating_sub(window_start));
            return Err(CypheriaError::RateLimited(remaining));
        }
        Ok(())
    }

    /// Reset reveal rate limit counter (called on vault lock)
    pub fn reset_reveal_rate_limit(&self) {
        self.reveal_count.store(0, Ordering::SeqCst);
        self.reveal_window_start.store(0, Ordering::SeqCst);
    }

    pub async fn unlock(
        &self,
        password: &[u8],
        vault_path: &std::path::Path,
    ) -> Result<(), CypheriaError> {
        // Check in-memory rate-limit state first
        {
            let state = self.state.read().await;
            if let SessionState::RateLimited { locked_until, .. } = &*state {
                if Instant::now() < *locked_until {
                    let remaining = locked_until.duration_since(Instant::now()).as_secs();
                    return Err(CypheriaError::RateLimited(remaining));
                }
            }
        }

        // Also check persisted lockout — this catches restarts mid-lockout
        {
            let (_, lockout_until) = read_persisted_attempts();
            let now = now_unix_secs();
            if lockout_until > now {
                let remaining = lockout_until - now;
                // Re-apply in-memory state so the rest of the session is consistent
                let mut state = self.state.write().await;
                *state = SessionState::RateLimited {
                    locked_until: Instant::now() + Duration::from_secs(remaining),
                    attempts: MAX_UNLOCK_ATTEMPTS,
                };
                return Err(CypheriaError::RateLimited(remaining));
            }
        }

        match crate::vault::store::load_and_unlock(password, vault_path).await {
            Ok((key_store, vault_store)) => {
                // Success — clear both in-memory and persisted attempt counters
                self.attempts.store(0, Ordering::SeqCst);
                clear_persisted_attempts();
                let mut state = self.state.write().await;
                *state = SessionState::Unlocked {
                    key_store,
                    vault_store: Box::new(vault_store),
                    vault_path: vault_path.to_path_buf(),
                    unlocked_at: Instant::now(),
                };
                Ok(())
            }
            Err(e) => {
                let attempts = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempts >= MAX_UNLOCK_ATTEMPTS {
                    self.attempts.store(0, Ordering::SeqCst);
                    let lockout_until = now_unix_secs() + LOCKOUT_DURATION_SECS;
                    // Persist lockout timestamp so restart doesn't reset it
                    write_persisted_attempts(0, lockout_until);
                    let mut state = self.state.write().await;
                    *state = SessionState::RateLimited {
                        locked_until: Instant::now() + Duration::from_secs(LOCKOUT_DURATION_SECS),
                        attempts,
                    };
                    return Err(CypheriaError::RateLimited(LOCKOUT_DURATION_SECS));
                }
                // Persist partial attempt count so restarts don't reset the counter
                write_persisted_attempts(attempts, 0);
                Err(e)
            }
        }
    }

    pub async fn lock(&self) {
        let mut state = self.state.write().await;
        *state = SessionState::Locked;
        self.reset_reveal_rate_limit();
    }

    pub async fn is_unlocked(&self) -> bool {
        matches!(&*self.state.read().await, SessionState::Unlocked { .. })
    }

    /// Synchronous guard — closure runs while the write-lock is held.
    /// No async inside the closure; all crypto operations are synchronous anyway.
    pub async fn with_session<T, F>(&self, f: F) -> Result<T, CypheriaError>
    where
        F: FnOnce(&ActiveKeyStore, &VaultStore) -> Result<T, CypheriaError>,
    {
        // First check with read lock
        {
            let state = self.state.read().await;
            match &*state {
                SessionState::Unlocked {
                    key_store,
                    vault_store,
                    ..
                } => return f(key_store, vault_store),
                SessionState::RateLimited { locked_until, .. } => {
                    if Instant::now() < *locked_until {
                        let remaining = locked_until.duration_since(Instant::now()).as_secs();
                        return Err(CypheriaError::RateLimited(remaining));
                    }
                }
                SessionState::Locked => return Err(CypheriaError::VaultLocked),
            }
        }

        // If we reached here, it means we were RateLimited but the time has expired.
        // Transition back to Locked state.
        let mut state = self.state.write().await;
        if let SessionState::RateLimited { locked_until, .. } = &*state {
            if Instant::now() >= *locked_until {
                *state = SessionState::Locked;
            }
        }
        Err(CypheriaError::VaultLocked)
    }

    pub async fn with_session_mut<T, F>(&self, f: F) -> Result<T, CypheriaError>
    where
        F: FnOnce(&mut ActiveKeyStore, &mut VaultStore) -> Result<T, CypheriaError>,
    {
        let mut state = self.state.write().await;
        match &mut *state {
            SessionState::Unlocked {
                ref mut key_store,
                vault_store,
                vault_path,
                ..
            } => {
                let result = f(key_store, vault_store)?;
                // Persist AFTER successful mutation. If persist fails, surface a clear error.
                if let Err(persist_err) = crate::vault::store::persist_vault(
                    key_store,
                    &vault_store.data,
                    &vault_store.header,
                    vault_path,
                )
                .await
                {
                    eprintln!(
                        "[Cypheria] CRITICAL: persist_vault failed: {:?}",
                        persist_err
                    );
                    return Err(CypheriaError::PersistFailed(format!(
                        "Changes were applied in memory but could not be saved to disk. \
                            WARNING: locking the vault will discard these in-memory changes. \
                            Error: {}",
                        persist_err
                    )));
                }
                Ok(result)
            }
            SessionState::RateLimited { locked_until, .. } => {
                if Instant::now() < *locked_until {
                    let remaining = locked_until.duration_since(Instant::now()).as_secs();
                    Err(CypheriaError::RateLimited(remaining))
                } else {
                    // Time expired, transition to Locked
                    *state = SessionState::Locked;
                    Err(CypheriaError::VaultLocked)
                }
            }
            SessionState::Locked => Err(CypheriaError::VaultLocked),
        }
    }

    pub async fn vault_path(&self) -> Option<std::path::PathBuf> {
        let state = self.state.read().await;
        if let SessionState::Unlocked { vault_path, .. } = &*state {
            Some(vault_path.clone())
        } else {
            None
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
