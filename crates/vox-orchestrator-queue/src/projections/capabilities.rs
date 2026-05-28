//! Capability-log-as-projection: tracks minted capabilities by op-id.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::oplog::{OperationEntry, OperationKind};
use crate::projection::{Projection, ProjectionError};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CapabilityRecord {
    pub op_id: u64,
    pub agent_id: u64,
    pub kind: String,
    pub minted_at_ms: u64,
}

#[derive(Default)]
pub struct CapabilityProjection {
    /// op_id → record
    state: Mutex<BTreeMap<u64, CapabilityRecord>>,
}

impl Projection for CapabilityProjection {
    fn name(&self) -> &'static str {
        "capabilities"
    }

    fn apply(&self, e: &OperationEntry) {
        match &e.kind {
            OperationKind::Custom { label } if label.starts_with("cap.mint:") => {
                let kind = label.trim_start_matches("cap.mint:").to_string();
                self.state.lock().unwrap().insert(
                    e.id.0,
                    CapabilityRecord {
                        op_id: e.id.0,
                        agent_id: e.agent_id.0,
                        kind,
                        minted_at_ms: e.timestamp_ms,
                    },
                );
            }
            OperationKind::Custom { label } if label.starts_with("cap.revoke:") => {
                let op_id_str = label.trim_start_matches("cap.revoke:");
                if let Ok(op_id) = op_id_str.parse::<u64>() {
                    self.state.lock().unwrap().remove(&op_id);
                }
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        serde_json::to_vec(&*self.state.lock().unwrap()).expect("capabilities snapshot")
    }

    fn restore(&self, b: &[u8]) -> Result<(), ProjectionError> {
        let parsed: BTreeMap<u64, CapabilityRecord> =
            serde_json::from_slice(b).map_err(|e| ProjectionError::Decode(e.to_string()))?;
        *self.state.lock().unwrap() = parsed;
        Ok(())
    }
}
