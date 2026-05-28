//! Ed25519 signing for op-log entries and capability mints.
//!
//! Signature payload (canonical):
//!   blake3( op_id_be(8) || predecessor_hash_bytes(32, zero-padded) || description_blake3(32) )
//!
//! The verifier looks up the signing daemon's pubkey from the [`KeyRing`], which is
//! seeded from `Vox.toml [mesh.trust]` at startup. Phase 5 hardens this to a
//! gossiped trust ledger; Phase 3 trusts the static config.

use std::collections::HashMap;

use vox_crypto::{
    SigningKey, VerifyingKey, generate_signing_keypair, secure_hash, sign, to_verifying_key,
    verify, verifying_key_from_bytes, verifying_key_to_bytes,
};

use super::{OperationEntry, OperationId};

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("no local signing key available")]
    NoLocalKey,
    #[error("unknown signing key id {0:?}")]
    UnknownKey(Vec<u8>),
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("invalid signature length (expected 64)")]
    BadSigLen,
    #[error("invalid key id length (expected 32)")]
    BadKeyIdLen,
}

/// Runtime key store: one optional local signing key + a map of peer pubkeys.
pub struct KeyRing {
    local_signing: Option<SigningKey>,
    local_verifying: Option<VerifyingKey>,
    /// key_id (blake3 of 32-byte verifying key bytes) → VerifyingKey
    peers: HashMap<Vec<u8>, VerifyingKey>,
}

impl KeyRing {
    /// Generate an ephemeral keypair — suitable for unit tests.
    pub fn ephemeral_for_tests() -> Self {
        let (sk, vk) = generate_signing_keypair();
        let id = key_id_for(&vk);
        let mut peers = HashMap::new();
        peers.insert(id, vk.clone());
        Self {
            local_signing: Some(sk),
            local_verifying: Some(vk),
            peers,
        }
    }

    /// 32-byte id (blake3 of verifying key bytes) for the local daemon key.
    pub fn local_daemon_id(&self) -> Option<[u8; 32]> {
        self.local_verifying
            .as_ref()
            .map(|vk| secure_hash(&verifying_key_to_bytes(vk)))
    }

    /// Register a peer's verifying key.
    pub fn add_peer(&mut self, vk: VerifyingKey) {
        peers_insert(&mut self.peers, &vk);
    }

    /// Register a peer's verifying key from its raw 32-byte representation.
    pub fn add_peer_bytes(&mut self, bytes: &[u8; 32]) -> Result<(), SignError> {
        let vk = verifying_key_from_bytes(bytes).map_err(|_| SignError::BadKeyIdLen)?;
        peers_insert(&mut self.peers, &vk);
        Ok(())
    }
}

fn key_id_for(vk: &VerifyingKey) -> Vec<u8> {
    secure_hash(&verifying_key_to_bytes(vk)).to_vec()
}

fn peers_insert(map: &mut HashMap<Vec<u8>, VerifyingKey>, vk: &VerifyingKey) {
    map.insert(key_id_for(vk), vk.clone());
}

/// Canonical payload:
///   blake3( op_id_be(8) || pred_hash_bytes(32, zero-padded) || description_blake3(32) )
fn canonical_payload(entry: &OperationEntry) -> [u8; 32] {
    let pred = entry.predecessor_hash.as_deref().unwrap_or("");
    let pred_bytes = hex::decode(pred).unwrap_or_default();
    let mut padded = [0u8; 32];
    let n = pred_bytes.len().min(32);
    padded[..n].copy_from_slice(&pred_bytes[..n]);

    let mut buf = Vec::with_capacity(8 + 32 + 32);
    buf.extend_from_slice(&entry.id.0.to_be_bytes());
    buf.extend_from_slice(&padded);
    buf.extend_from_slice(&secure_hash(entry.description.as_bytes()));
    secure_hash(&buf)
}

/// Sign `entry` with the local daemon key, filling in `signature` and `signing_key_id`.
pub fn sign_entry(ring: &KeyRing, entry: &mut OperationEntry) -> Result<(), SignError> {
    let sk = ring.local_signing.as_ref().ok_or(SignError::NoLocalKey)?;
    let vk = ring.local_verifying.as_ref().ok_or(SignError::NoLocalKey)?;
    let payload = canonical_payload(entry);
    let sig_bytes = sign(sk, &payload);
    entry.signature = Some(sig_bytes.to_vec());
    entry.signing_key_id = Some(key_id_for(vk));
    Ok(())
}

/// Verify `entry`'s signature against the key in `ring`.
pub fn verify_entry(ring: &KeyRing, entry: &OperationEntry) -> Result<(), SignError> {
    let kid = entry.signing_key_id.as_ref().ok_or(SignError::NoLocalKey)?;
    let vk = ring
        .peers
        .get(kid)
        .ok_or_else(|| SignError::UnknownKey(kid.clone()))?;

    let sig_vec = entry
        .signature
        .as_ref()
        .ok_or(SignError::SignatureMismatch)?;
    if sig_vec.len() != 64 {
        return Err(SignError::BadSigLen);
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(sig_vec);

    let payload = canonical_payload(entry);
    if verify(vk, &payload, &sig_arr) {
        Ok(())
    } else {
        Err(SignError::SignatureMismatch)
    }
}

/// Sign an arbitrary capability-mint blob. Returns the 64-byte signature.
pub fn sign_capability(
    ring: &KeyRing,
    op_id: OperationId,
    capability_blob: &[u8],
) -> Result<[u8; 64], SignError> {
    let sk = ring.local_signing.as_ref().ok_or(SignError::NoLocalKey)?;
    let mut buf = Vec::with_capacity(8 + 32);
    buf.extend_from_slice(&op_id.0.to_be_bytes());
    buf.extend_from_slice(&secure_hash(capability_blob));
    let payload = secure_hash(&buf);
    Ok(sign(sk, &payload))
}

/// Load the signing key for this daemon from a 32-byte seed (from `vox-secrets`).
pub fn key_ring_from_seed(seed: &[u8; 32]) -> KeyRing {
    let sk = vox_crypto::signing_key_from_bytes(seed);
    let vk = to_verifying_key(&sk);
    let id = key_id_for(&vk);
    let mut peers = HashMap::new();
    peers.insert(id, vk.clone());
    KeyRing {
        local_signing: Some(sk),
        local_verifying: Some(vk),
        peers,
    }
}
