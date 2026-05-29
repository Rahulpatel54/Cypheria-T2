use serde::{Serialize, Deserialize};

/// Input from the frontend for add/update operations.
#[derive(Debug, Deserialize)]
pub struct EntryInput {
    pub name:        String,
    pub username:    String,
    pub password:    String,
    pub website:     String,
    pub notes:       String,
    pub is_favorite: Option<bool>,
    pub category:    Option<String>,
    pub color:       Option<String>,
    pub emoji:       Option<String>,
}

/// Decrypted entry data sent TO the frontend.
///
/// CRITICAL: This struct intentionally does NOT have a `password` field.
/// Passwords are retrieved via the separate `get_entry_password` command.
/// This ensures the full entry list never carries password bytes.
#[derive(Debug, Serialize)]
pub struct EntryView {
    pub id:              String,
    pub created_at:      String,
    pub updated_at:      String,
    pub is_favorite:     bool,
    pub category:        String,
    pub color:           String,
    pub emoji:           String,
    pub name:            String,
    pub username:        String,
    pub website:         String,
    pub notes:           String,
    /// Always true — signals to the frontend that password must be fetched separately.
    pub password_masked: bool,
}

/// Password generator options from the frontend.
#[derive(Debug, Deserialize)]
pub struct GenOptions {
    pub length:  usize,
    pub upper:   bool,
    pub lower:   bool,
    pub numbers: bool,
    pub symbols: bool,
}

#[derive(Debug, Serialize)]
pub struct EntryScoreView {
    pub id:                String,
    pub score:             u8,
    pub has_password:      bool,
    /// Truncated HMAC tag for duplicate detection (32-bit, not reversible to password).
    pub dup_tag:           String,
    pub days_since_update: u64,
}
