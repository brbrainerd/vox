//! File-affinity-as-projection: tracks which (daemon, agent) claims each path.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::oplog::{OperationEntry, OperationKind};
use crate::projection::{Projection, ProjectionError};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AffinityOwner {
    pub daemon: [u8; 16],
    pub agent_id: u64,
    /// Lamport clock of last claim — last-write-wins across daemons (P3-T4 extends this).
    pub lamport: u64,
}

#[derive(Default)]
pub struct AffinityProjection {
    state: Mutex<BTreeMap<String, AffinityOwner>>,
    lamport: Mutex<u64>,
}

impl Projection for AffinityProjection {
    fn name(&self) -> &'static str {
        "affinity"
    }

    fn apply(&self, e: &OperationEntry) {
        let tick = {
            let mut l = self.lamport.lock().unwrap();
            *l += 1;
            *l
        };
        match &e.kind {
            OperationKind::WorkspaceCreate { agent_id } => {
                // Claim affinity for all paths in the workspace on behalf of this agent.
                self.state.lock().unwrap().insert(
                    format!("workspace:{agent_id}"),
                    AffinityOwner {
                        daemon: e.daemon_id,
                        agent_id: *agent_id,
                        lamport: tick,
                    },
                );
            }
            OperationKind::WorkspaceMerge { agent_id } => {
                self.state
                    .lock()
                    .unwrap()
                    .remove(&format!("workspace:{agent_id}"));
            }
            OperationKind::Custom { label } if label.starts_with("affinity.claim:") => {
                let path = label.trim_start_matches("affinity.claim:").to_string();
                self.state.lock().unwrap().insert(
                    path,
                    AffinityOwner {
                        daemon: e.daemon_id,
                        agent_id: e.agent_id.0,
                        lamport: tick,
                    },
                );
            }
            OperationKind::Custom { label } if label.starts_with("affinity.release:") => {
                let path = label.trim_start_matches("affinity.release:").to_string();
                self.state.lock().unwrap().remove(&path);
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        serde_json::to_vec(&*self.state.lock().unwrap()).expect("affinity snapshot")
    }

    fn restore(&self, b: &[u8]) -> Result<(), ProjectionError> {
        let parsed: BTreeMap<String, AffinityOwner> =
            serde_json::from_slice(b).map_err(|e| ProjectionError::Decode(e.to_string()))?;
        *self.state.lock().unwrap() = parsed;
        Ok(())
    }
}
