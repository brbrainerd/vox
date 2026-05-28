//! P0-T2: lock-leader election with heartbeat refresh.
//!
//! Backed by the `lock_leader` table in vox-db. The leader is the only node
//! that writes to `vcs_lock`. Followers proxy mutations via A2A to the leader;
//! reads are served locally from the cached snapshot.
//!
//! Heartbeat: spawn_heartbeat fires at TTL/3 intervals. If the heartbeat
//! returns `Ok(false)` (preempted), the watch channel sends `false` and the
//! background task exits — callers observe the demotion.

use std::sync::Arc;

use vox_db::VoxDb;

const DEFAULT_LEADER_TTL_MS: i64 = 9_000; // 9 s; heartbeat at 3 s.

/// Role returned by `try_become_leader`.
#[derive(Debug, Clone)]
pub enum LeaderRole {
    /// This node now owns the lock-leader row.
    Leader { ttl_ms: i64 },
    /// Another node holds an unexpired claim.
    Follower { leader_node_id: String },
}

/// A2A proxy used by followers to forward lock mutations to the leader.
///
/// Implementors live in `vox-orchestrator` (which carries the populi client);
/// this trait is a layer-clean injection point so `vox-orchestrator-queue` does
/// not depend on the network layer.
#[async_trait::async_trait]
pub trait LockMutationProxy: Send + Sync {
    async fn proxy_acquire(
        &self,
        leader_node_id: &str,
        path: &std::path::Path,
        agent: vox_orchestrator_types::AgentId,
        kind: super::LockKind,
    ) -> Result<(), String>;

    async fn proxy_release(
        &self,
        leader_node_id: &str,
        path: &std::path::Path,
        agent: vox_orchestrator_types::AgentId,
    ) -> Result<(), String>;
}

/// Manages the lock-leader lifecycle for one repository.
pub struct LockLeaderElection {
    db: Arc<VoxDb>,
    node_id: String,
    repository_id: String,
    ttl_ms: i64,
}

impl LockLeaderElection {
    /// Create with default 9 s TTL (heartbeat every 3 s).
    pub fn new(db: VoxDb, node_id: impl Into<String>, repository_id: impl Into<String>) -> Self {
        Self {
            db: Arc::new(db),
            node_id: node_id.into(),
            repository_id: repository_id.into(),
            ttl_ms: DEFAULT_LEADER_TTL_MS,
        }
    }

    /// Create with a custom TTL (used in tests with short expiry).
    pub fn with_ttl_ms(
        db: VoxDb,
        node_id: impl Into<String>,
        repository_id: impl Into<String>,
        ttl_ms: i64,
    ) -> Self {
        Self {
            db: Arc::new(db),
            node_id: node_id.into(),
            repository_id: repository_id.into(),
            ttl_ms,
        }
    }

    /// Attempt a compare-and-swap leadership claim.
    ///
    /// Returns `Leader` if this node now owns the row; `Follower` if another
    /// node holds an unexpired claim.
    pub async fn try_become_leader(&self) -> Result<LeaderRole, String> {
        let now = super::persisted::now_ms();
        let claimed = self
            .db
            .lock_leader_try_claim(&self.repository_id, &self.node_id, now, self.ttl_ms)
            .await
            .map_err(|e| e.to_string())?;

        if claimed {
            Ok(LeaderRole::Leader {
                ttl_ms: self.ttl_ms,
            })
        } else {
            let row = self
                .db
                .lock_leader_get(&self.repository_id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "leader row absent after CAS failure".to_string())?;
            Ok(LeaderRole::Follower {
                leader_node_id: row.leader_node_id,
            })
        }
    }

    /// Refresh this node's heartbeat. Returns `Ok(true)` while still leader,
    /// `Ok(false)` when preempted (row's leader_node_id changed).
    pub async fn heartbeat(&self) -> Result<bool, String> {
        let now = super::persisted::now_ms();
        self.db
            .lock_leader_heartbeat(&self.repository_id, &self.node_id, now, self.ttl_ms)
            .await
            .map_err(|e| e.to_string())
    }

    /// Spawn a background heartbeat task. The returned watch channel carries
    /// `true` while leader and `false` after demotion. Drop the JoinHandle to
    /// abort the task.
    pub fn spawn_heartbeat(
        self: Arc<Self>,
    ) -> (
        tokio::task::JoinHandle<()>,
        tokio::sync::watch::Receiver<bool>,
    ) {
        let (tx, rx) = tokio::sync::watch::channel(true);
        let interval = std::time::Duration::from_millis((self.ttl_ms / 3).max(1) as u64);
        let me = Arc::clone(&self);
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                match me.heartbeat().await {
                    Ok(true) => {}
                    Ok(false) => {
                        let _ = tx.send(false);
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "lock_leader heartbeat error; will retry");
                    }
                }
            }
        });
        (handle, rx)
    }
}
