//! Session state machine.
//!
//! States:
//!   Locked       — no key material in memory; vault file may or may not be set
//!   Unlocked     — ActiveKeyStore + VaultStore live here; all commands operate here
//!   RateLimited  — temporary lockout after MAX_UNLOCK_ATTEMPTS failed attempts
//!
//! Transitions:
//!   Locked ──[unlock(correct_pwd)]──▶ Unlocked
//!   Locked ──[unlock(wrong_pwd) × MAX_ATTEMPTS]──▶ RateLimited
//!   Unlocked ──[lock() / timeout / panic]──▶ Locked
//!   RateLimited ──[after LOCKOUT_DURATION]──▶ Locked (next unlock attempt resets)
//!
//! SECURITY: The `with_session` guard pattern ensures that no command can operate
//! on vault data unless the session is in the Unlocked state. The RwLock prevents
//! concurrent state mutations during sensitive transitions.

use std::sync::Arc;
use std::time::{Instant, Duration};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;
use crate::{
    crypto::keys::ActiveKeyStore,
    vault::{format::VaultData, store::VaultStore},
    error::CypheriaError,
};

pub const MAX_UNLOCK_ATTEMPTS:  u32 = 5;
pub const LOCKOUT_DURATION_SECS: u64 = 30;

pub enum SessionState {
    /// No key material. Vault file is not loaded.
    Locked,

    /// Vault is decrypted and fully accessible.
    Unlocked {
        key_store:   ActiveKeyStore, // ZeroizeOnDrop — zeroed when dropped
        vault_store: VaultStore,
        vault_path:  std::path::PathBuf,
        unlocked_at: Instant,
    },

    /// Temporary lockout after too many failed password attempts.
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

    /// Attempt to unlock the vault. Enforces rate limiting.
    ///
    /// SECURITY: Password bytes must be zeroized by the CALLER immediately
    /// after this function returns (whether Ok or Err). The caller (unlock_vault
    /// command) converts the String to bytes and zeroizes them.
    pub async fn unlock(
        &self,
        password: &[u8],
        vault_path: &std::path::Path,
    ) -> Result<(), CypheriaError> {
        // Check rate limit FIRST (read lock — cheap)
        {
            let state = self.state.read().await;
            if let SessionState::RateLimited { locked_until, .. } = &*state {
                if Instant::now() < *locked_until {
                    let remaining = locked_until.duration_since(Instant::now()).as_secs();
                    return Err(CypheriaError::RateLimited(remaining));
                }
                // Lockout period has elapsed — allow next attempt
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

    /// Lock the vault — drops ActiveKeyStore, which triggers ZeroizeOnDrop.
    ///
    /// After this returns, no key material remains in memory.
    pub async fn lock(&self) {
        let mut state = self.state.write().await;
        // Replacing Unlocked with Locked drops ActiveKeyStore → ZeroizeOnDrop fires
        *state = SessionState::Locked;
    }

    pub async fn is_unlocked(&self) -> bool {
        matches!(&*self.state.read().await, SessionState::Unlocked { .. })
    }

    /// Guard pattern: run a closure only if the session is Unlocked.
    ///
    /// Returns VaultLocked if locked or rate limited.
    /// All vault commands go through this — it is impossible to access key_store
    /// or vault_store without the session being in Unlocked state.
    pub async fn with_session<T, F, Fut>(&self, f: F) -> Result<T, CypheriaError>
    where
        F: FnOnce(&ActiveKeyStore, &mut VaultStore) -> Fut,
        Fut: std::future::Future<Output = Result<T, CypheriaError>>,
    {
        let mut state = self.state.write().await;
        match &mut *state {
            SessionState::Unlocked { key_store, vault_store, .. } => f(key_store, vault_store).await,
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

    /// Access the vault path from an unlocked session (used for persist after mutations).
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
