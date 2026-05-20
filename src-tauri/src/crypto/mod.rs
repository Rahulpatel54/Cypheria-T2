//! Cryptographic primitives for Cypheria.
//!
//! ALL cryptographic operations in Cypheria must go through this module.
//! Direct use of underlying crates outside of these sub-modules is forbidden.
//!
//! Sub-modules:
//!   rng    — All randomness (OsRng only, no direct rand/getrandom calls)
//!   kdf    — Argon2id key derivation + domain-separated subkeys
//!   aes    — AES-256-GCM authenticated encryption / key wrapping
//!   kyber  — CRYSTALS-Kyber-1024 post-quantum key encapsulation
//!   keys   — In-memory key hierarchy types (all ZeroizeOnDrop)

pub mod rng;
pub mod kdf;
pub mod aes;
pub mod kyber;
pub mod keys;
