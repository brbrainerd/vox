use async_trait::async_trait;
use sha3::Digest;
use vox_crypto::{SigningKey, VerifyingKey};
use vox_package::Sha3_512;

use crate::distributed_training::checkpoint::{CheckpointBundle, synthetic_weights_hash};
use crate::distributed_training::gradient::GradientShard;
use crate::distributed_training::session::{Batch, SessionId, StepResult, TrainingError, TrainingSession};

/// Single-node / rank-local data-parallel session (world_size = 1 implemented).
pub struct DataParallelSession {
    session_id: SessionId,
    rank: u32,
    world_size: u32,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    step: u64,
    state_digest: Sha3_512,
}

impl DataParallelSession {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        rank: u32,
        world_size: u32,
        signing_key: SigningKey,
        verifying_key: VerifyingKey,
    ) -> Self {
        Self {
            session_id,
            rank,
            world_size,
            signing_key,
            verifying_key,
            step: 0,
            state_digest: [0u8; 64],
        }
    }

    #[must_use]
    pub fn verifying_key_ref(&self) -> &VerifyingKey {
        &self.verifying_key
    }

    fn bump_state(&mut self, batch_id: u64) {
        let mut hasher = sha3::Sha3_512::new();
        hasher.update(b"vox.mens.train.state_digest:v1");
        hasher.update(self.state_digest);
        hasher.update(self.step.to_be_bytes());
        hasher.update(batch_id.to_be_bytes());
        self.state_digest = hasher.finalize().into();
    }
}

#[async_trait]
impl TrainingSession for DataParallelSession {
    fn rank(&self) -> u32 {
        self.rank
    }

    fn world_size(&self) -> u32 {
        self.world_size
    }

    fn session_id(&self) -> SessionId {
        self.session_id
    }

    fn step_index(&self) -> u64 {
        self.step
    }

    async fn step(&mut self, batch: Batch) -> Result<StepResult, TrainingError> {
        self.step = self.step.saturating_add(1);
        self.bump_state(batch.batch_id);
        Ok(StepResult {
            step: self.step,
            loss: 0.0,
        })
    }

    async fn all_reduce(&mut self, shard: GradientShard) -> Result<GradientShard, TrainingError> {
        if self.world_size != 1 {
            return Err(TrainingError::AllReduceUnsupported(self.world_size));
        }
        if shard.rank != self.rank {
            return Err(TrainingError::RankMismatch {
                expected: self.rank,
                got: shard.rank,
            });
        }
        if shard.step != self.step {
            return Err(TrainingError::StepMismatch {
                expected: self.step,
                got: shard.step,
            });
        }
        if !shard.verify(&self.verifying_key) {
            return Err(TrainingError::InvalidGradientSignature);
        }
        Ok(shard)
    }

    async fn checkpoint(&mut self) -> Result<CheckpointBundle, TrainingError> {
        let bundle_hash = synthetic_weights_hash(self.step, self.state_digest);
        Ok(CheckpointBundle::sign(
            &self.signing_key,
            self.session_id,
            self.step,
            bundle_hash,
            self.state_digest,
        ))
    }

    async fn resume(&mut self, bundle: &CheckpointBundle) -> Result<(), TrainingError> {
        if !bundle.verify(&self.verifying_key) {
            return Err(TrainingError::InvalidCheckpointSignature);
        }
        if bundle.session_id != self.session_id {
            // Allow resume only for the same logical session id.
            return Err(TrainingError::InvalidCheckpointSignature);
        }
        self.step = bundle.step;
        self.state_digest = bundle.optimizer_state_hash;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_crypto::{generate_signing_keypair, signing_key_from_bytes, signing_key_to_bytes};

    #[tokio::test]
    async fn single_rank_smoke() {
        let (sk, vk) = generate_signing_keypair();
        let sk_dup = signing_key_from_bytes(&signing_key_to_bytes(&sk));
        let sid = SessionId::new();
        let mut sess = DataParallelSession::new(sid, 0, 1, sk, vk);

        let out = sess.step(Batch { batch_id: 1 }).await.unwrap();
        assert_eq!(out.step, 1);

        let tensor_hash = {
            let mut h = sha3::Sha3_512::new();
            h.update(b"grad");
            h.finalize().into()
        };
        let shard = GradientShard::sign(&sk_dup, sid, sess.step_index(), 0, tensor_hash);
        let reduced = sess.all_reduce(shard).await.unwrap();
        assert!(reduced.verify(sess.verifying_key_ref()));

        let ckpt = sess.checkpoint().await.unwrap();
        assert!(ckpt.verify(sess.verifying_key_ref()));

        let sk_resume = signing_key_from_bytes(&signing_key_to_bytes(&sk_dup));
        let vk_resume = vox_crypto::to_verifying_key(&sk_resume);
        let mut sess2 = DataParallelSession::new(sid, 0, 1, sk_resume, vk_resume);
        sess2.resume(&ckpt).await.unwrap();
        assert_eq!(sess2.step_index(), ckpt.step);
    }
}
