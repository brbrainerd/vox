//! Locks-as-projection: hard locks on file paths held by daemons.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::oplog::{OperationEntry, OperationKind};
use crate::projection::{Projection, ProjectionError};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LockOwner {
    pub daemon: [u8; 16],
    pub agent_id: u64,
    pub lease_expires_ms: u64,
}

#[derive(Default)]
pub struct LocksProjection {
    state: Mutex<BTreeMap<String, LockOwner>>,
}

impl Projection for LocksProjection {
    fn name(&self) -> &'static str {
        "locks"
    }

    fn apply(&self, e: &OperationEntry) {
        match &e.kind {
            OperationKind::LockAcquire { path, agent_id } => {
                self.state.lock().unwrap().insert(
                    path.clone(),
                    LockOwner {
                        daemon: e.daemon_id,
                        agent_id: *agent_id,
                        lease_expires_ms: e.timestamp_ms.saturating_add(60_000),
                    },
                );
            }
            OperationKind::LockRelease { path, .. } => {
                self.state.lock().unwrap().remove(path);
            }
            OperationKind::Custom { label } if label.starts_with("lock.acquire:") => {
                let path = label.trim_start_matches("lock.acquire:").to_string();
                self.state.lock().unwrap().insert(
                    path,
                    LockOwner {
                        daemon: e.daemon_id,
                        agent_id: e.agent_id.0,
                        lease_expires_ms: e.timestamp_ms.saturating_add(60_000),
                    },
                );
            }
            OperationKind::Custom { label } if label.starts_with("lock.release:") => {
                let path = label.trim_start_matches("lock.release:").to_string();
                self.state.lock().unwrap().remove(&path);
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        // BTreeMap iteration is deterministic — same keys always in same order.
        serde_json::to_vec(&*self.state.lock().unwrap()).expect("locks snapshot")
    }

    fn restore(&self, b: &[u8]) -> Result<(), ProjectionError> {
        let parsed: BTreeMap<String, LockOwner> =
            serde_json::from_slice(b).map_err(|e| ProjectionError::Decode(e.to_string()))?;
        *self.state.lock().unwrap() = parsed;
        Ok(())
    }
}
