//! All random bytes in Cypheria flow through this module.
//! Never call rand or getrandom directly elsewhere.

use rand::rngs::OsRng;
use rand::RngCore;

/// Generate cryptographically secure random bytes via OS entropy (getrandom).
pub fn secure_random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    OsRng.fill_bytes(&mut buf);
    buf
}

/// Generate a random nonce for AES-GCM (96 bits = 12 bytes).
pub fn aes_nonce() -> [u8; 12] {
    secure_random_bytes::<12>()
}

/// Generate a salt for Argon2id (256 bits = 32 bytes).
pub fn argon2_salt() -> [u8; 32] {
    secure_random_bytes::<32>()
}

/// Generate a fresh Entry Key or Vault Key (256 bits = 32 bytes).
pub fn entry_key() -> [u8; 32] {
    secure_random_bytes::<32>()
}
