use thiserror::Error;

#[derive(Debug, Error)]
pub enum CypheriaError {
    #[error("Authentication failed")]
    AuthFailed,

    #[error("Vault is locked")]
    VaultLocked,

    #[error("Vault already exists at path")]
    VaultExists,

    #[error("Vault file not found")]
    VaultNotFound,

    #[error("Vault file is corrupted or tampered")]
    VaultCorrupted,

    #[error("Cryptographic operation failed")]
    CryptoError,

    #[error("Key derivation failed")]
    KdfError,

    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    #[error("Note not found: {0}")]
    NoteNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error")]
    SerdeError,

    #[error("Maximum unlock attempts exceeded. Try again in {0} seconds")]
    RateLimited(u64),

    #[error("Session expired")]
    SessionExpired,
}

// CRITICAL: Never expose internal crypto details to the frontend.
impl serde::Serialize for CypheriaError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
