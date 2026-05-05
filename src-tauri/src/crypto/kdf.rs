//! Key derivation using Argon2id (OWASP-compliant parameters).

use argon2::{Argon2, Algorithm, Version, Params};
use zeroize::Zeroize;
use crate::error::CypheriaError;

/// Argon2id parameters — OWASP recommended minimum for high-security use.
pub const ARGON2_MEMORY_KB:   u32   = 65536; // 64 MB — defeats GPU brute force
pub const ARGON2_ITERATIONS:  u32   = 3;
pub const ARGON2_PARALLELISM: u32   = 4;
pub const ARGON2_OUTPUT_LEN:  usize = 32;    // 256-bit master key

/// Derives a 32-byte Master Key from the user's master password + salt.
///
/// SECURITY:
///   - memory=64MB: defeats GPU-parallel brute force (GPU VRAM limited)
///   - iterations=3: adds sequential compute cost
///   - parallelism=4: uses multi-core while limiting GPU
///   - algorithm=Argon2id: hybrid of Argon2i (side-channel) + Argon2d (GPU)
///
/// The returned key MUST be stored in an ActiveKeyStore (ZeroizeOnDrop) and
/// zeroized immediately on vault lock. NEVER log or print this value.
pub fn derive_master_key(
    password: &[u8],
    salt: &[u8; 32],
) -> Result<[u8; ARGON2_OUTPUT_LEN], CypheriaError> {
    let params = Params::new(
        ARGON2_MEMORY_KB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(ARGON2_OUTPUT_LEN),
    )
    .map_err(|_| CypheriaError::KdfError)?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut output_key = [0u8; ARGON2_OUTPUT_LEN];

    argon2
        .hash_password_into(password, salt, &mut output_key)
        .map_err(|_| CypheriaError::KdfError)?;

    Ok(output_key)
}

/// Domain-separated subkey derivation.
///
/// Derives different 32-byte subkeys from the same Master Key for different
/// cryptographic purposes (AES encryption vs. HMAC authentication vs. settings
/// encryption). This prevents cross-purpose key reuse.
///
/// Scheme: SHA-256("CYPHERIA_SUBKEY_v1|" || domain || "|" || master_key)
pub fn derive_subkey(master_key: &[u8; 32], domain: &[u8], out: &mut [u8; 32]) {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(b"CYPHERIA_SUBKEY_v1|");
    hasher.update(domain);
    hasher.update(b"|");
    hasher.update(master_key);
    let result = hasher.finalize();
    out.copy_from_slice(&result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_master_key_deterministic() {
        let password = b"test_password_42";
        let salt = [0xAB_u8; 32];
        let key1 = derive_master_key(password, &salt).unwrap();
        let key2 = derive_master_key(password, &salt).unwrap();
        assert_eq!(key1, key2, "Same password+salt must produce same key");
    }

    #[test]
    fn test_derive_master_key_different_salts() {
        let password = b"test_password";
        let salt1 = [0x01_u8; 32];
        let salt2 = [0x02_u8; 32];
        let key1 = derive_master_key(password, &salt1).unwrap();
        let key2 = derive_master_key(password, &salt2).unwrap();
        assert_ne!(key1, key2, "Different salts must produce different keys");
    }

    #[test]
    fn test_derive_subkey_domain_separation() {
        let mk = [0x42_u8; 32];
        let mut out1 = [0u8; 32];
        let mut out2 = [0u8; 32];
        derive_subkey(&mk, b"DOMAIN_A", &mut out1);
        derive_subkey(&mk, b"DOMAIN_B", &mut out2);
        assert_ne!(out1, out2, "Different domains must produce different subkeys");
    }

    #[test]
    fn test_derive_subkey_deterministic() {
        let mk = [0x99_u8; 32];
        let mut out1 = [0u8; 32];
        let mut out2 = [0u8; 32];
        derive_subkey(&mk, b"HMAC_VAULT_INTEGRITY", &mut out1);
        derive_subkey(&mk, b"HMAC_VAULT_INTEGRITY", &mut out2);
        assert_eq!(out1, out2);
    }
}
