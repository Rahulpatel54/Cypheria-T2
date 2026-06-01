//! AES-256-GCM authenticated encryption.
//!
//! All encrypted blobs use the format: nonce(12 bytes) || ciphertext+tag(N+16 bytes)
//! The nonce is always randomly generated and prepended, making each blob self-contained.
//! AES-GCM's authentication tag provides built-in tamper detection — any byte flip
//! in the ciphertext or tag will cause decryption to fail with CryptoError.

use crate::{crypto::rng::aes_nonce, error::CypheriaError};
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use zeroize::Zeroize;

/// Encrypts plaintext using AES-256-GCM.
///
/// Returns: nonce(12) || ciphertext+tag(plaintext.len()+16)
/// The output is always 28 bytes longer than the input.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CypheriaError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CypheriaError::CryptoError)?;

    let nonce_bytes = aes_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let mut ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| CypheriaError::CryptoError)?;

    // Prepend nonce: output = nonce(12) || ciphertext+tag
    let mut output = nonce_bytes.to_vec();
    output.append(&mut ciphertext);

    Ok(output)
}

/// Decrypts an AES-256-GCM blob: nonce(12) || ciphertext+tag.
///
/// Returns plaintext on success.
/// Returns CryptoError if the authentication tag is invalid (tamper detected).
/// Returns VaultCorrupted if the blob is too short to be valid.
pub fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>, CypheriaError> {
    // Minimum: 12 (nonce) + 16 (tag) = 28 bytes; ciphertext may be 0 bytes
    if blob.len() < 12 + 16 {
        return Err(CypheriaError::VaultCorrupted);
    }

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CypheriaError::CryptoError)?;

    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CypheriaError::CryptoError)
}

/// Encrypts a 32-byte key using another 32-byte wrapping key.
/// Used to wrap the Vault Key with the Master Key.
pub fn wrap_key(wrapping_key: &[u8; 32], key_to_wrap: &[u8; 32]) -> Result<Vec<u8>, CypheriaError> {
    encrypt(wrapping_key, key_to_wrap.as_ref())
}

/// Decrypts a wrapped 32-byte key.
/// Returns CryptoError if the wrapping key is wrong (GCM tag mismatch).
pub fn unwrap_key(wrapping_key: &[u8; 32], wrapped_blob: &[u8]) -> Result<[u8; 32], CypheriaError> {
    let mut plaintext = decrypt(wrapping_key, wrapped_blob)?;
    if plaintext.len() != 32 {
        plaintext.zeroize();
        return Err(CypheriaError::VaultCorrupted);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&plaintext);
    plaintext.zeroize();
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42_u8; 32]
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = b"Hello, Cypheria!";
        let ciphertext = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty_plaintext() {
        let key = test_key();
        let ciphertext = encrypt(&key, b"").unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();
        assert_eq!(decrypted, b"");
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = test_key();
        let plaintext = b"Secret data";
        let mut ciphertext = encrypt(&key, plaintext).unwrap();
        // Flip a byte in the ciphertext portion (after nonce)
        ciphertext[15] ^= 0xFF;
        assert!(
            decrypt(&key, &ciphertext).is_err(),
            "Tampered ciphertext must fail"
        );
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = [0x11_u8; 32];
        let key2 = [0x22_u8; 32];
        let ciphertext = encrypt(&key1, b"secret").unwrap();
        assert!(decrypt(&key2, &ciphertext).is_err(), "Wrong key must fail");
    }

    #[test]
    fn test_wrap_unwrap_key() {
        let wrapping_key = [0xAA_u8; 32];
        let key_to_wrap = [0xBB_u8; 32];
        let wrapped = wrap_key(&wrapping_key, &key_to_wrap).unwrap();
        let unwrapped = unwrap_key(&wrapping_key, &wrapped).unwrap();
        assert_eq!(unwrapped, key_to_wrap);
    }

    #[test]
    fn test_nonces_are_unique() {
        let key = test_key();
        let ct1 = encrypt(&key, b"same plaintext").unwrap();
        let ct2 = encrypt(&key, b"same plaintext").unwrap();
        // Nonces (first 12 bytes) should differ with overwhelming probability
        assert_ne!(
            &ct1[..12],
            &ct2[..12],
            "Each encryption must use a unique nonce"
        );
    }

    #[test]
    fn test_blob_too_short_rejected() {
        let key = test_key();
        let short_blob = [0u8; 10]; // less than 28 bytes minimum
        assert!(decrypt(&key, &short_blob).is_err());
    }
}
