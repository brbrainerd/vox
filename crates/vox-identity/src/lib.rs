//! Vox identity: per-user keypair, signing challenges, trust ledger.

pub mod challenge;
/// Per-job ephemeral Ed25519 subkey for result attestation (P5-T6).
pub mod ephemeral;
pub mod identity;
/// Per-pairing X25519 key derivation for JWE encryption (P5-T10).
pub mod pairing_x25519;
pub mod storage;
pub mod trust;

pub use ephemeral::EphemeralSigner;
pub use identity::NodeIdentity;
pub use pairing_x25519::PairingKey;
pub use trust::{TrustedNode, TrustedNodeRegistry};
