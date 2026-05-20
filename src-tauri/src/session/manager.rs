//! Session state machine.

use std::sync::Arc;
use std::time::{Instant, Duration};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;
use crate::{
    crypto::keys::ActiveKeyStore,
    vault::store::VaultStore,
    error::CypheriaError,
};

pub const MAX_UNLOCK_ATTEMPTS:  u32 = 5;
pub const LOCKOUT_DURATION_SECS: u64 = 30;

pub enum SessionState {
    Locked,
    Unlocked {
        key_store:   ActiveKeyStore,
        vault_store: VaultStore,
        vault_path:  std::path::PathBuf,
        unlocked_at: Instant,
    },
    RateLimited {
        locked_until: Instant,
        attempts:     u32,
    },
}

pub struct SessionManager {
    state:    Arc<RwLock<SessionState>>,
    attempts: Arc<AtomicU32>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            state:    Arc::new(RwLock::new(SessionState::Locked)),
            attempts: Arc::new(AtomicU32::new(0)),
        }
    }

    pub async fn unlock(
        &self,
        password: &[u8],
        vault_path: &std::path::Path,
    ) -> Result<(), CypheriaError> {
        {
            let state = self.state.read().await;
            if let SessionState::RateLimited { locked_until, .. } = &*state {
                if Instant::now() < *locked_until {
                    let remaining = locked_until.duration_since(Instant::now()).as_secs();
                    return Err(CypheriaError::RateLimited(remaining));
                }
            }
        }

        match crate::vault::store::load_and_unlock(password, vault_path).await {
            Ok((key_store, vault_store)) => {
                self.attempts.store(0, Ordering::SeqCst);
                let mut state = self.state.write().await;
                *state = SessionState::Unlocked {
                    key_store,
                    vault_store,
                    vault_path: vault_path.to_path_buf(),
                    unlocked_at: Instant::now(),
                };
                Ok(())
            }
            Err(e) => {
                let attempts = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempts >= MAX_UNLOCK_ATTEMPTS {
                    self.attempts.store(0, Ordering::SeqCst);
                    let mut state = self.state.write().await;
                    *state = SessionState::RateLimited {
                        locked_until: Instant::now() + Duration::from_secs(LOCKOUT_DURATION_SECS),
                        attempts,
                    };
                    return Err(CypheriaError::RateLimited(LOCKOUT_DURATION_SECS));
                }
                Err(e)
            }
        }
    }

    pub async fn lock(&self) {
        let mut state = self.state.write().await;
        *state = SessionState::Locked;
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
        let state = self.state.read().await;
        match &*state {
            SessionState::Unlocked { key_store, vault_store, .. } => f(key_store, vault_store),
            SessionState::RateLimited { locked_until, .. } => {
                if Instant::now() < *locked_until {
                    let remaining = locked_until.duration_since(Instant::now()).as_secs();
                    Err(CypheriaError::RateLimited(remaining))
                } else {
                    Err(CypheriaError::VaultLocked)
                }
            }
            SessionState::Locked => Err(CypheriaError::VaultLocked),
        }
    }

    pub async fn with_session_mut<T, F>(&self, f: F) -> Result<T, CypheriaError>
    where
        F: FnOnce(&mut ActiveKeyStore, &mut VaultStore) -> Result<T, CypheriaError>,
    {
        let mut state = self.state.write().await;
        match &mut *state {
            SessionState::Unlocked { ref mut key_store, vault_store, vault_path, .. } => {
            let result = f(key_store, vault_store)?;
            // Persist AFTER successful mutation. If persist fails, surface a clear error.
            if let Err(persist_err) = crate::vault::store::persist_vault(
                key_store,
                &vault_store.data,
                &vault_store.header,
                vault_path,
            ).await {
                eprintln!("[Cypheria] CRITICAL: persist_vault failed: {:?}", persist_err);
                return Err(CypheriaError::PersistFailed(
                    format!("Changes were applied in memory but could not be saved to disk. \
                        WARNING: locking the vault will discard these in-memory changes. \
                        Error: {}", persist_err)
                ));
            }
            Ok(result)
        }
            SessionState::RateLimited { locked_until, .. } => {
                if Instant::now() < *locked_until {
                    let remaining = locked_until.duration_since(Instant::now()).as_secs();
                    Err(CypheriaError::RateLimited(remaining))
                } else {
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
    fn default() -> Self { Self::new() }
}
