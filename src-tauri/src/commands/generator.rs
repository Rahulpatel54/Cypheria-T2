//! Server-side CSPRNG password generator.
//!
//! IMPROVEMENTS over a naive implementation:
//!   1. Rejection sampling — prevents modulo bias for non-power-of-2 charsets
//!   2. Real entropy calculation — returns log2(charset^length) for accurate strength meter
//!   3. All randomness from OsRng (OS entropy source, same as the rest of the crypto stack)

use rand::RngCore;
use rand::rngs::OsRng;
use serde::Serialize;
use crate::{error::CypheriaError, models::entry::GenOptions};

/// Result returned to the frontend.
#[derive(Serialize)]
pub struct PasswordGenResult {
    pub password:     String,
    pub entropy_bits: u32,
    pub strength:     String,
    pub charset_size: u32,
}

/// Generate a cryptographically secure password.
///
/// Uses rejection sampling to eliminate modulo bias — for any charset that is
/// not a power of 2, naive `byte % charset_len` over-represents low-index chars.
/// We reject bytes above `floor(256 / charset_len) * charset_len` so the
/// distribution across the charset is perfectly uniform.
#[tauri::command]
pub fn generate_password(options: GenOptions) -> Result<PasswordGenResult, CypheriaError> {
    // Validate length bounds
    if options.length < 4 || options.length > 256 {
        return Err(CypheriaError::InvalidInput("Password length must be between 4 and 256".into()));
    }

    // Build character set from enabled classes
    let mut charset = String::new();
    if options.upper   { charset.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ"); }
    if options.lower   { charset.push_str("abcdefghijklmnopqrstuvwxyz"); }
    if options.numbers { charset.push_str("0123456789"); }
    if options.symbols { charset.push_str("!@#$%^&*()_+-=[]{}|;:,.<>?"); }

    if charset.is_empty() {
        return Err(CypheriaError::InvalidInput(
            "At least one character class must be selected".into(),
        ));
    }

    let charset_bytes: Vec<u8> = charset.bytes().collect();
    let charset_len = charset_bytes.len();

    // Rejection sampling threshold:
    // Accept bytes in [0, max_valid) — this range divides evenly into `charset_len` groups
    let max_valid = (256 / charset_len) * charset_len;

    let mut password = Vec::with_capacity(options.length);
    let mut buf = [0u8; 1];
    let mut accepted = 0usize;

    while accepted < options.length {
        OsRng.fill_bytes(&mut buf);
        let byte = buf[0] as usize;
        if byte < max_valid {
            password.push(charset_bytes[byte % charset_len]);
            accepted += 1;
        }
        // Rejected bytes are discarded — no modulo bias in accepted bytes
    }

    let pwd_str = String::from_utf8(password).map_err(|_| CypheriaError::CryptoError)?;

    // True entropy: log2(charset_size ^ length) = length * log2(charset_size)
    let entropy_bits = (options.length as f64) * (charset_len as f64).log2();

    let strength = match entropy_bits as u32 {
        0..=35  => "Weak",
        36..=59 => "Moderate",
        60..=79 => "Strong",
        _       => "Very Strong",
    };

    Ok(PasswordGenResult {
        password:     pwd_str,
        entropy_bits: entropy_bits as u32,
        strength:     strength.to_string(),
        charset_size: charset_len as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(length: usize, upper: bool, lower: bool, numbers: bool, symbols: bool) -> GenOptions {
        GenOptions { length, upper, lower, numbers, symbols }
    }

    #[test]
    fn test_basic_generation() {
        let result = generate_password(opts(16, true, true, true, false)).unwrap();
        assert_eq!(result.password.len(), 16);
        assert!(result.entropy_bits > 0);
    }

    #[test]
    fn test_entropy_calculation() {
        // 16 chars from 26 lowercase = 16 * log2(26) ≈ 75 bits
        let result = generate_password(opts(16, false, true, false, false)).unwrap();
        assert!(result.entropy_bits >= 70, "Expected ~75 bits, got {}", result.entropy_bits);
        assert_eq!(result.strength, "Strong");
    }

    #[test]
    fn test_length_bounds_rejected() {
        assert!(generate_password(opts(3, true, true, true, false)).is_err());
        assert!(generate_password(opts(257, true, true, true, false)).is_err());
    }

    #[test]
    fn test_empty_charset_rejected() {
        assert!(generate_password(opts(16, false, false, false, false)).is_err());
    }

    #[test]
    fn test_charset_respected() {
        let result = generate_password(opts(100, false, true, false, false)).unwrap();
        assert!(result.password.chars().all(|c| c.is_ascii_lowercase()),
            "Only lowercase expected");
    }

    #[test]
    fn test_passwords_are_random() {
        let r1 = generate_password(opts(24, true, true, true, true)).unwrap();
        let r2 = generate_password(opts(24, true, true, true, true)).unwrap();
        assert_ne!(r1.password, r2.password, "Two generated passwords should differ");
    }
}
