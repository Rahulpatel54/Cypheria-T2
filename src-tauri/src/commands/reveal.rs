// One-use token store for password reveal — password never returned directly over IPC
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;
use zeroize::Zeroize;

pub struct RevealStore(pub Mutex<HashMap<String, String>>);

impl RevealStore {
    pub fn new() -> Self {
        RevealStore(Mutex::new(HashMap::new()))
    }

    /// Store a password and return a single-use token
    pub fn store(&self, password: String) -> String {
        let token = Uuid::new_v4().to_string();
        if let Ok(mut map) = self.0.lock() {
            map.insert(token.clone(), password);
        }
        token
    }

    /// Consume a token — returns the password and deletes the entry immediately
    pub fn consume(&self, token: &str) -> Option<String> {
        if let Ok(mut map) = self.0.lock() {
            map.remove(token)
        } else {
            None
        }
    }
}

impl Default for RevealStore {
    fn default() -> Self { Self::new() }
}