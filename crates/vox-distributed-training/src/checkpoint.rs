use sha3::Digest;
use vox_crypto::{SigningKey, VerifyingKey, sign, verify};
use vox_orchestrator_queue::oplog::OperationKind;
use vox_package::Sha3_512;

use crate::session::SessionId;

/// Signed checkpoint marker referencing CAS bundles (Mn-T6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointBundle {
    pub session_id: SessionId,
    pub step: u64,
    pub bundle_hash: Sha3_512,
    pub optimizer_state_hash: Sha3_512,
    pub signature: [u8; 64],
}

impl CheckpointBundle {
    fn message_bytes(
        session_id: SessionId,
        step: u64,
        bundle_hash: Sha3_512,
        optimizer_state_hash: Sha3_512,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 8 + 64 + 64);
        out.extend_from_slice(session_id.0.as_bytes());
        out.extend_from_slice(&step.to_be_bytes());
        out.extend_from_slice(&bundle_hash);
        out.extend_from_slice(&optimizer_state_hash);
        out
    }

    #[must_use]
    pub fn sign(
        signing_key: &SigningKey,
        session_id: SessionId,
        step: u64,
        bundle_hash: Sha3_512,
        optimizer_state_hash: Sha3_512,
    ) -> Self {
        let msg = Self::message_bytes(session_id, step, bundle_hash, optimizer_state_hash);
        let signature = sign(signing_key, &msg);
        Self {
            session_id,
            step,
            bundle_hash,
            optimizer_state_hash,
            signature,
        }
    }

    #[must_use]
    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        let msg = Self::message_bytes(
            self.session_id,
            self.step,
            self.bundle_hash,
            self.optimizer_state_hash,
        );
        verify(verifying_key, &msg, &self.signature)
    }

    /// Maps this checkpoint into a durable [`OperationKind`] for `vox-orchestrator-queue`.
    #[must_use]
    pub fn to_operation_kind(&self) -> OperationKind {
        OperationKind::TrainingCheckpoint {
            session_id: self.session_id.to_string(),
            bundle_hash: hex::encode(self.bundle_hash),
            optimizer_state_hash: hex::encode(self.optimizer_state_hash),
            step: self.step,
        }
    }
}

#[must_use]
pub fn synthetic_weights_hash(step: u64, state_digest: Sha3_512) -> Sha3_512 {
    let mut hasher = sha3::Sha3_512::new();
    hasher.update(b"vox.mens.checkpoint.weights:v1");
    hasher.update(step.to_be_bytes());
    hasher.update(state_digest);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_crypto::generate_signing_keypair;

    #[test]
    fn checkpoint_sig_round_trip() {
        let (sk, vk) = generate_signing_keypair();
        let sid = SessionId::new();
        let bundle_hash = [1u8; 64];
        let opt_hash = [2u8; 64];
        let c = CheckpointBundle::sign(&sk, sid, 9, bundle_hash, opt_hash);
        assert!(c.verify(&vk));
    }

    #[test]
    fn checkpoint_maps_to_operation_kind() {
        let (sk, _) = generate_signing_keypair();
        let sid = SessionId::new();
        let c = CheckpointBundle::sign(&sk, sid, 4, [3u8; 64], [4u8; 64]);
        let json = serde_json::to_string(&c.to_operation_kind()).unwrap();
        let back: OperationKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c.to_operation_kind());
    }
}
