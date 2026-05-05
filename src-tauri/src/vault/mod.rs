/// Vault layer — on-disk format, in-memory state, and CRUD operations.
///
/// Sub-modules:
///   format  — .qvault binary format constants, VaultHeader, VaultData structs
///   store   — VaultStore (in-memory decrypted state), load_and_unlock, persist_vault
///   entry   — Entry CRUD: add, decrypt, get_password, update (with key rotation), delete
///   notes   — Notes CRUD: add, decrypt, update (with key rotation), delete

pub mod format;
pub mod store;
pub mod entry;
pub mod notes;
