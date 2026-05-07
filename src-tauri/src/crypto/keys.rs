use zeroize::{Zeroize, ZeroizeOnDrop};
/// Key hierarchy types — the nerve center of Cypheria's security model.
///
/// Every type that holds key material implements Zeroize and ZeroizeOnDrop.
/// When any of these structs are dropped, their memory is overwritten with zeros
/// before being released. This prevents key material from lingering in freed heap pages.
///
/// Key hierarchy:
///   MasterKey  — derived from master password via Argon2id; never stored on disk
///   VaultKey   — random 32-byte key; stored wrapped with MK in vault header
///   EntryKey   — random 32-byte key per entry; stored wrapped with VK in vault


/// Master Key — derived from the user's master password via Argon2id.
/// Lives ONLY in memory while the vault is unlocked.
/// Never serialized. Zeroized on lock (via ActiveKeyStore drop).
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MasterKey(pub(crate) [u8; 32]);

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
///
/// SECURITY: ZeroizeOnDrop ensures that when this struct is dropped
/// (on vault lock), all key bytes are overwritten with zeros before
/// the memory is released. A `*state = SessionState::Locked` assignment
/// drops the previous Unlocked variant, which drops this struct.
#[derive(ZeroizeOnDrop)]
pub struct ActiveKeyStore {
    pub master_key: MasterKey,
    pub vault_key:  VaultKey,
}

impl ActiveKeyStore {
    pub fn new(mk: [u8; 32], vk: [u8; 32]) -> Self {
        Self {
            master_key: MasterKey(mk),
            vault_key:  VaultKey(vk),
        }
    }

    #[inline]
    pub fn vault_key_bytes(&self) -> &[u8; 32] {
        &self.vault_key.0
    }

    #[inline]
    pub fn master_key_bytes(&self) -> &[u8; 32] {
        &self.master_key.0
    }
}
