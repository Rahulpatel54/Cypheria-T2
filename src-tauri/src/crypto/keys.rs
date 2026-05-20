// FIX: IMPROVE-001 — MasterKey now uses secrecy::Secret<[u8; 32]>.
// secrecy v0.8 requires the inner type S to implement Zeroize.
// [u8; 32] implements Zeroize via DefaultIsZeroes, so Secret<[u8; 32]> compiles.
// All access goes through expose_secret() via the expose() helper method.
// This prevents accidental Debug-printing of key material and ensures
// the memory is zeroized when Secret is dropped.

use zeroize::ZeroizeOnDrop;
use secrecy::{Secret, ExposeSecret};

/// Key hierarchy types — the nerve center of Cypheria's security model.
/// Every type that holds key material implements ZeroizeOnDrop.
/// When any of these structs are dropped, their memory is overwritten with zeros
/// before being released. This prevents key material from lingering in freed heap pages.
/// Key hierarchy:
///   MasterKey  — derived from master password via Argon2id; never stored on disk
///   VaultKey   — random 32-byte key; stored wrapped with MK in vault header
///   EntryKey   — random 32-byte key per entry; stored wrapped with VK in vault
/// Master Key — derived from the user's master password via Argon2id.
/// Lives ONLY in memory while the vault is unlocked.
/// Never serialized. Zeroized on lock (via ActiveKeyStore drop) via Secret<[u8;32]>.
/// Secret<[u8;32]> zeroizes the inner array on drop and prevents Debug output.
pub struct MasterKey(pub(crate) Secret<[u8; 32]>);

impl MasterKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        MasterKey(Secret::new(bytes))
    }

    #[inline]
    pub fn expose(&self) -> &[u8; 32] {
        self.0.expose_secret()
    }
}

// Redacting Debug impl — never print key bytes
impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey([REDACTED])")
    }
}

/// Vault Key — encrypts all entry keys.
/// Stored encrypted (wrapped with MK) in the vault header.
/// Decrypted into memory on unlock; zeroized on lock.
#[derive(ZeroizeOnDrop)]
pub struct VaultKey(pub(crate) [u8; 32]);

/// Entry Key — unique per entry; encrypts the entry's credential payload.
/// Stored encrypted (wrapped with VK) per entry in the vault.
/// Decrypted on demand for a single operation, then immediately zeroized.
#[derive(ZeroizeOnDrop)]
pub struct EntryKey(pub(crate) [u8; 32]);

/// In-memory key store, active ONLY while vault is unlocked.
/// SECURITY: When this struct is dropped (on vault lock), all key bytes are
/// overwritten with zeros before the memory is released.
///   - MasterKey: zeroed by Secret<[u8;32]>'s Drop impl
///   - VaultKey:  zeroed by ZeroizeOnDrop derive
///
/// A `*state = SessionState::Locked` assignment drops the previous Unlocked
/// variant, which drops this struct and triggers both cleanup paths.
pub struct ActiveKeyStore {
    pub master_key: MasterKey,
    pub vault_key:  VaultKey,
}

impl ActiveKeyStore {
    pub fn new(mk: [u8; 32], vk: [u8; 32]) -> Self {
        Self {
            master_key: MasterKey::new(mk),
            vault_key:  VaultKey(vk),
        }
    }

    #[inline]
    pub fn vault_key_bytes(&self) -> &[u8; 32] {
        &self.vault_key.0
    }

    #[inline]
    pub fn master_key_bytes(&self) -> &[u8; 32] {
        self.master_key.expose()
    }
}