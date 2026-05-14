//! .qvault binary file format — version 1.
//!
//! File layout:
//! ┌─────────────────────────────────────────────────┐
//! │ MAGIC        "CYPHERIA\x01"   (9 bytes)         │
//! │ VERSION      u16 little-endian (2 bytes)        │
//! │ HEADER_LEN   u32 little-endian (4 bytes)        │
//! │ HEADER       bincode(VaultHeader) bytes         │
//! │ HMAC         HMAC-SHA256 of all bytes above     │
//! │               (32 bytes)                        │
//! │ DATA_LEN     u32 little-endian (4 bytes)        │
//! │ DATA         AES-256-GCM encrypted VaultData    │
//! └─────────────────────────────────────────────────┘
//!
//! The HMAC covers everything from MAGIC through the last byte of HEADER.
//! It is verified with a subkey derived from the Master Key BEFORE any
//! decryption is attempted, providing early tamper detection.

use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

pub const MAGIC: &[u8] = b"CYPHERIA\x01";
pub const FORMAT_VERSION: u16 = 1;

/// Stored in plaintext in the header section.
/// Contains everything needed to re-derive keys and verify integrity.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VaultHeader {
    /// Random 32-byte salt used for Argon2id key derivation.
    pub argon2_salt: [u8; 32],

    /// KDF parameters snapshot — allows future clients to use the correct params.
    pub kdf_memory_kb:   u32,
    pub kdf_iterations:  u32,
    pub kdf_parallelism: u32,

    /// Vault Key wrapped with Master Key (AES-256-GCM).
    /// Format: nonce(12) || ciphertext+tag(48)  = 60 bytes
    pub vk_wrapped_classical: Vec<u8>,

    /// Kyber-1024 public key — stored plaintext (public).
    pub kyber_public_key: Vec<u8>,

    /// Kyber-1024 secret key encrypted with Master Key (AES-256-GCM).
    pub kyber_sk_encrypted: Vec<u8>,

    /// Kyber ciphertext produced during VK encapsulation.
    pub kyber_ciphertext: Vec<u8>,

    /// VK wrapped under the Kyber shared secret (post-quantum recovery path).
    pub vk_wrapped_pq: Vec<u8>,

    /// Vault creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Human-readable vault name (low-sensitivity metadata, plaintext).
    pub vault_name: String,

    /// Format version that wrote this file (for future migration).
    pub format_version: u16,
}

/// The encrypted data section — decrypted by the Vault Key.
/// This entire struct is serialized with bincode then AES-GCM encrypted.
/// It never appears on disk in plaintext.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VaultData {
    pub entries:    Vec<EncryptedEntry>,
    pub notes:      Vec<EncryptedNote>,
    pub settings:   EncryptedSettings,
    pub updated_at: DateTime<Utc>,
}

/// An entry stored in the vault.
///
/// Non-sensitive metadata (id, timestamps, category, color, emoji, is_favorite)
/// is stored in plaintext for fast listing without decryption.
///
/// All credential fields (name, username, password, website, notes) are
/// individually encrypted together as a JSON blob using a unique Entry Key.
/// The Entry Key itself is wrapped with the Vault Key.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptedEntry {
    /// UUID v4 — non-sensitive, used for lookups.
    pub id: String,

    pub created_at:  DateTime<Utc>,
    pub updated_at:  DateTime<Utc>,
    pub is_favorite: bool,

    /// UI metadata — plaintext for fast rendering without decryption.
    pub category: String,
    pub color:    String,
    pub emoji:    String,

    /// Entry Key wrapped with Vault Key.
    /// Format: nonce(12) || AES-GCM(VK, ek_bytes)(32+16) = 60 bytes
    pub ek_wrapped: Vec<u8>,

    /// JSON blob of { name, username, password, website, notes } encrypted with Entry Key.
    /// Format: nonce(12) || AES-GCM(EK, json_bytes) || tag(16)
    pub payload_encrypted: Vec<u8>,
}

/// A note stored in the vault. Same encryption pattern as entries.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptedNote {
    pub id:         String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// Note Key wrapped with Vault Key.
    pub ek_wrapped: Vec<u8>,

    /// JSON blob of { title, content } encrypted with Note Key.
    pub payload_encrypted: Vec<u8>,
}

/// Settings encrypted as a JSON blob with a subkey derived from the Master Key.
/// Separate from the VK-encrypted data so settings survive a VK rotation.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptedSettings {
    pub payload_encrypted: Vec<u8>,
}

/// Plaintext credential payload — NEVER written to disk.
/// Zeroized automatically when dropped (ZeroizeOnDrop).
#[derive(Debug, Serialize, Deserialize)]
#[derive(zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct EntryPayload {
    pub name:     String,
    pub username: String,
    pub password: String,
    pub website:  String,
    pub notes:    String,
}

/// Plaintext note payload — NEVER written to disk.
#[derive(Debug, Serialize, Deserialize)]
#[derive(zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct NotePayload {
    pub title:   String,
    pub content: String,
}
