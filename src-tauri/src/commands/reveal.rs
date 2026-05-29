use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;
use zeroize::Zeroize;

const TOKEN_TTL_SECS: u64 = 30;

struct RevealEntry {
    password: String,
    expires:  Instant,
}

impl Drop for RevealEntry {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

pub struct RevealStore {
    tokens: Mutex<HashMap<String, RevealEntry>>,
}

impl RevealStore {
    pub fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Store a password and return a one-use token.
    pub fn store(&self, password: String) -> String {
        let token = Uuid::new_v4().to_string();
        let expires = Instant::now() + Duration::from_secs(TOKEN_TTL_SECS);
        let mut map = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        // Evict expired tokens on each store to prevent unbounded growth
        map.retain(|_, v| v.expires > Instant::now());
        map.insert(token.clone(), RevealEntry { password, expires });
        token
    }

    /// Consume a token and return its password. Returns None if missing or expired.
    pub fn consume(&self, token: &str) -> Option<String> {
        let mut map = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        match map.remove(token) {
            Some(entry) if entry.expires > Instant::now() => Some(entry.password.clone()),
            Some(_) => None, // expired — entry dropped and zeroized here
            None => None,
        }
    }
}

impl Default for RevealStore {
    fn default() -> Self { Self::new() }
}