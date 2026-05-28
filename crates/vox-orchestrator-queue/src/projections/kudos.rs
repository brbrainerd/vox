//! Kudos-as-projection: accumulated reputation points per (agent, primitive).

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::oplog::{OperationEntry, OperationKind};
use crate::projection::{Projection, ProjectionError};

type KudosKey = (u64, String); // (agent_id, primitive)

#[derive(Default)]
pub struct KudosProjection {
    /// (agent_id, primitive) → total kudos
    state: Mutex<BTreeMap<KudosKey, i64>>,
}

impl Projection for KudosProjection {
    fn name(&self) -> &'static str {
        "kudos"
    }

    fn apply(&self, e: &OperationEntry) {
        match &e.kind {
            OperationKind::Custom { label } if label.starts_with("kudos.add:") => {
                // Format: "kudos.add:<primitive>:<amount>"
                let rest = label.trim_start_matches("kudos.add:");
                let mut parts = rest.splitn(2, ':');
                if let (Some(primitive), Some(amount_str)) = (parts.next(), parts.next())
                    && let Ok(amount) = amount_str.parse::<i64>()
                {
                    let key = (e.agent_id.0, primitive.to_string());
                    *self.state.lock().unwrap().entry(key).or_default() += amount;
                }
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        // BTreeMap is deterministic; serialize as sorted (agent_id, primitive) → amount.
        let s = self.state.lock().unwrap();
        let entries: Vec<_> = s.iter().map(|((a, p), v)| (a, p, v)).collect();
        serde_json::to_vec(&entries).expect("kudos snapshot")
    }

    fn restore(&self, b: &[u8]) -> Result<(), ProjectionError> {
        let entries: Vec<(u64, String, i64)> =
            serde_json::from_slice(b).map_err(|e| ProjectionError::Decode(e.to_string()))?;
        let mut map = self.state.lock().unwrap();
        map.clear();
        for (agent, prim, amount) in entries {
            map.insert((agent, prim), amount);
        }
        Ok(())
    }
}
