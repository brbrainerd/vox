use vox_crypto::{SigningKey, VerifyingKey, sign, verify};
use vox_package::Sha3_512;

use crate::distributed_training::session::SessionId;

/// Signed gradient shard journaled to the op-log in multi-rank mode (Mn-T1 sketch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradientShard {
    pub session_id: SessionId,
    pub step: u64,
    pub rank: u32,
    pub tensor_blob_hash: Sha3_512,
    pub signature: [u8; 64],
}

impl GradientShard {
    fn message_bytes(
        session_id: SessionId,
        step: u64,
        rank: u32,
        tensor_blob_hash: Sha3_512,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 8 + 4 + 64);
        out.extend_from_slice(session_id.0.as_bytes());
        out.extend_from_slice(&step.to_be_bytes());
        out.extend_from_slice(&rank.to_be_bytes());
        out.extend_from_slice(&tensor_blob_hash);
        out
    }

    #[must_use]
    pub fn sign(
        signing_key: &SigningKey,
        session_id: SessionId,
        step: u64,
        rank: u32,
        tensor_blob_hash: Sha3_512,
    ) -> Self {
        let msg = Self::message_bytes(session_id, step, rank, tensor_blob_hash);
        let signature = sign(signing_key, &msg);
        Self {
            session_id,
            step,
            rank,
            tensor_blob_hash,
            signature,
        }
    }

    #[must_use]
    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        let msg = Self::message_bytes(self.session_id, self.step, self.rank, self.tensor_blob_hash);
        verify(verifying_key, &msg, &self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed_training::session::SessionId;
    use vox_crypto::generate_signing_keypair;

    #[test]
    fn gradient_round_trip_sig() {
        let (sk, vk) = generate_signing_keypair();
        let sid = SessionId::new();
        let hash = [7u8; 64];
        let g = GradientShard::sign(&sk, sid, 3, 0, hash);
        assert!(g.verify(&vk));
    }
}
