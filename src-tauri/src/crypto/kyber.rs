//! CRYSTALS-Kyber-1024 post-quantum key encapsulation.
//!
//! ARCHITECTURE — Hybrid dual-path for the Vault Key (VK):
//!   Classical path:  AES-256-GCM(MasterKey, VK)    → vk_wrapped_classical
//!   Post-quantum:    Kyber1024.Encapsulate(kyber_pk) → shared_secret + ciphertext
//!                    AES-256-GCM(shared_secret[..32], VK) → vk_wrapped_pq
//!
//! The PQ path allows a future quantum-capable operator to recover the VK from
//! a recorded ciphertext using the Kyber secret key — without needing the master
//! password. This defends against "harvest now, decrypt later" attacks.
//!
//! Kyber-1024 is used (highest security level, ~AES-256 equivalent quantum security).

use pqcrypto_kyber::kyber1024;
use pqcrypto_traits::kem::{PublicKey, SecretKey, SharedSecret, Ciphertext};
use zeroize::Zeroize;
use crate::{crypto::aes, error::CypheriaError};

/// A freshly generated Kyber-1024 keypair.
/// The secret key must be encrypted before being stored in the vault header.
pub struct KyberKeypair {
    pub public_key: Vec<u8>,
    pub secret_key: Vec<u8>, // Encrypt with MK before storing!
}

impl Drop for KyberKeypair {
    fn drop(&mut self) {
        self.secret_key.zeroize();
        self.public_key.zeroize();
    }
}

/// Generate a fresh Kyber-1024 keypair.
/// Call once at vault creation; store pk plaintext, sk encrypted with MK.
pub fn generate_keypair() -> KyberKeypair {
    let (pk, sk) = kyber1024::keypair();
    KyberKeypair {
        public_key: pk.as_bytes().to_vec(),
        secret_key: sk.as_bytes().to_vec(),
    }
}

/// Encapsulate the Vault Key using the Kyber public key.
///
/// Returns: (kyber_ciphertext, vk_wrapped_under_shared_secret)
///
/// The Kyber ciphertext is stored in the vault header (plaintext — it is not secret).
/// The wrapped VK is also stored in the vault header.
/// Together they allow recovering VK using only the Kyber secret key.
pub fn encapsulate_vault_key(
    kyber_pk_bytes: &[u8],
    vault_key: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), CypheriaError> {
    let pk = kyber1024::PublicKey::from_bytes(kyber_pk_bytes)
        .map_err(|_| CypheriaError::CryptoError)?;

    let (shared_secret, ciphertext) = kyber1024::encapsulate(&pk);

    // Kyber-1024 shared secret is 32 bytes — use directly as AES-256 key
    let mut aes_key = [0u8; 32];
    aes_key.copy_from_slice(&shared_secret.as_bytes()[..32]);

    let wrapped_vk = aes::wrap_key(&aes_key, vault_key)
        .map_err(|_| CypheriaError::CryptoError)?;

    aes_key.zeroize();

    Ok((ciphertext.as_bytes().to_vec(), wrapped_vk))
}

/// Decapsulate the Vault Key using the Kyber secret key.
///
/// This is the post-quantum recovery path. Used when:
///   - The classical (password-based) path is unavailable, OR
///   - A quantum attacker has broken classical encryption
///
/// Requires the Kyber secret key (stored encrypted in vault header with MK).
pub fn decapsulate_vault_key(
    kyber_sk_bytes: &[u8],
    kyber_ciphertext: &[u8],
    wrapped_vk: &[u8],
) -> Result<[u8; 32], CypheriaError> {
    let sk = kyber1024::SecretKey::from_bytes(kyber_sk_bytes)
        .map_err(|_| CypheriaError::CryptoError)?;
    let ct = kyber1024::Ciphertext::from_bytes(kyber_ciphertext)
        .map_err(|_| CypheriaError::CryptoError)?;

    let shared_secret = kyber1024::decapsulate(&ct, &sk);

    let mut aes_key = [0u8; 32];
    aes_key.copy_from_slice(&shared_secret.as_bytes()[..32]);

    let vault_key = aes::unwrap_key(&aes_key, wrapped_vk)?;
    aes_key.zeroize();

    Ok(vault_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encapsulate_decapsulate_roundtrip() {
        let kp = generate_keypair();
        let vault_key = [0xDE_u8; 32];

        let (ciphertext, wrapped_vk) =
            encapsulate_vault_key(&kp.public_key, &vault_key).unwrap();

        let recovered = decapsulate_vault_key(&kp.secret_key, &ciphertext, &wrapped_vk).unwrap();
        assert_eq!(recovered, vault_key, "Decapsulated VK must match original");
    }

    #[test]
    fn test_wrong_secret_key_fails() {
        let kp1 = generate_keypair();
        let kp2 = generate_keypair();
        let vault_key = [0xAB_u8; 32];

        let (ciphertext, wrapped_vk) =
            encapsulate_vault_key(&kp1.public_key, &vault_key).unwrap();

        // Using kp2's secret key to decapsulate kp1's ciphertext should fail or return garbage
        // (Kyber decapsulation always succeeds but produces a wrong shared secret)
        let result = decapsulate_vault_key(&kp2.secret_key, &ciphertext, &wrapped_vk);
        // The AES unwrap will fail because the shared secret is different
        assert!(result.is_err(), "Wrong secret key must not recover the vault key");
    }
}
