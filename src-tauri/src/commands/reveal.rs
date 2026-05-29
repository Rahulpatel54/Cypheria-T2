// One-use, TTL-bounded token store for password reveal.
// Tokens expire after 10 seconds even if not consumed, limiting the window
// during which a stolen token could be replayed.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Instant, Duration};
use uuid::Uuid;

const TOKEN_TTL: Duration = Duration::from_secs(10);

pub struct RevealEntry {
    password:   String,
    created_at: Instant,
}

pub struct RevealStore(pub Mutex<HashMap<String, RevealEntry>>);

impl RevealStore {
    pub fn new() -> Self {
        RevealStore(Mutex::new(HashMap::new()))
    }

    /// Store a password and return a single-use, TTL-bounded token.
    pub fn store(&self, password: String) -> String {
        let token = Uuid::new_v4().to_string();
        if let Ok(mut map) = self.0.lock() {
            // Evict any expired entries to avoid unbounded growth
            map.retain(|_, v| v.created_at.elapsed() < TOKEN_TTL);
            map.insert(token.clone(), RevealEntry {
                password,
                created_at: Instant::now(),
            });
        }
        token
    }

    /// Consume a token — returns the password and deletes the entry immediately.
    /// Returns None if the token is unknown or has expired.
    pub fn consume(&self, token: &str) -> Option<String> {
        if let Ok(mut map) = self.0.lock() {
            if let Some(entry) = map.remove(token) {
                if entry.created_at.elapsed() < TOKEN_TTL {
                    return Some(entry.password);
                }
                // Token was valid but expired — already removed, return None
            }
        }
        None
    }
}

impl Default for RevealStore {
    fn default() -> Self { Self::new() }
}