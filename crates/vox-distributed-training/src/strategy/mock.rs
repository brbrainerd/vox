use crate::checkpoint::{CheckpointBundle, synthetic_weights_hash};
use crate::gradient::GradientShard;
use crate::session::{Batch, SessionId, StepResult, TrainingError, TrainingSession};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use vox_crypto::{SigningKey, VerifyingKey};

/// Thread-safe coordinator for in-process multi-rank simulation.
struct MockCoordinator {
    ranks_at_step: Mutex<HashMap<u64, Vec<GradientShard>>>,
}

impl MockCoordinator {
    fn new(_world_size: u32) -> Self {
        Self {
            ranks_at_step: Mutex::new(HashMap::new()),
        }
    }
}

/// Simulated distributed session for testing Mn-T6 rank-aware coordination.
pub struct MockDistributedSession {
    session_id: SessionId,
    rank: u32,
    world_size: u32,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    step: u64,
    state_digest: [u8; 64],
    coordinator: Arc<MockCoordinator>,
}

impl MockDistributedSession {
    pub fn new_cluster(
        session_id: SessionId,
        world_size: u32,
        keypairs: Vec<(SigningKey, VerifyingKey)>,
    ) -> Vec<Self> {
        let coordinator = Arc::new(MockCoordinator::new(world_size));
        keypairs
            .into_iter()
            .enumerate()
            .map(|(i, (sk, vk))| Self {
                session_id,
                rank: i as u32,
                world_size,
                signing_key: sk,
                verifying_key: vk,
                step: 0,
                state_digest: [0u8; 64],
                coordinator: coordinator.clone(),
            })
            .collect()
    }
}

#[async_trait]
impl TrainingSession for MockDistributedSession {
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

    async fn step(&mut self, _batch: Batch) -> Result<StepResult, TrainingError> {
        self.step += 1;
        // Simplified state bump
        self.state_digest[0] = self.state_digest[0].wrapping_add(1);
        Ok(StepResult {
            step: self.step,
            loss: 0.1,
        })
    }

    async fn all_reduce(&mut self, shard: GradientShard) -> Result<GradientShard, TrainingError> {
        // 1. Validate shard
        if shard.rank != self.rank || shard.step != self.step {
            return Err(TrainingError::RankMismatch {
                expected: self.rank,
                got: shard.rank,
            });
        }
        if !shard.verify(&self.verifying_key) {
            return Err(TrainingError::InvalidGradientSignature);
        }

        // 2. Register shard with coordinator
        let is_ready = {
            let mut lock = self.coordinator.ranks_at_step.lock().unwrap();
            let shards = lock.entry(self.step).or_default();
            shards.push(shard.clone());
            shards.len() >= self.world_size as usize
        };

        // 3. Wait for all ranks (spin-wait for mock, in reality this is a rendezvous)
        if !is_ready {
            loop {
                {
                    let lock = self.coordinator.ranks_at_step.lock().unwrap();
                    if lock.get(&self.step).unwrap().len() >= self.world_size as usize {
                        break;
                    }
                }
                tokio::task::yield_now().await;
            }
        }

        // 4. "Sum" the gradients (mock sum = first shard's hash for simplicity)
        let final_lock = self.coordinator.ranks_at_step.lock().unwrap();
        let all_shards = final_lock.get(&self.step).unwrap();

        // Return a new shard that represents the reduced result
        // In a mock, we just return the first shard but maybe with a special rank marker
        Ok(all_shards[0].clone())
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
        self.step = bundle.step;
        self.state_digest = bundle.optimizer_state_hash;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::task::JoinSet;
    use vox_crypto::generate_signing_keypair;

    #[tokio::test]
    async fn multi_rank_all_reduce() {
        let sid = SessionId::new();
        let world_size = 3;
        let mut keypairs = Vec::new();
        let mut vks = Vec::new();
        for _ in 0..world_size {
            let (sk, vk) = generate_signing_keypair();
            vks.push(vk.clone());
            keypairs.push((sk, vk));
        }

        let sessions = MockDistributedSession::new_cluster(sid, world_size, keypairs);
        let mut set = JoinSet::new();

        for (i, mut sess) in sessions.into_iter().enumerate() {
            let _vk = vks[i].clone();
            set.spawn(async move {
                sess.step(Batch { batch_id: 100 }).await.unwrap();
                let hash = [i as u8; 64];
                let shard = GradientShard::sign(&sess.signing_key, sid, 1, i as u32, hash);
                sess.all_reduce(shard).await.unwrap();
                sess.step_index()
            });
        }

        while let Some(res) = set.join_next().await {
            assert_eq!(res.unwrap(), 1);
        }
    }
}
