//! In-memory [`MeshStore`] — for unit tests and migration staging.
//!
//! No persistence; behaviourally identical to `VoxDbMeshStore` except:
//! - `schema_version()` always returns 1.
//! - `integrity_check()` validates dedupe but never returns sqlite-level findings.
//! - `LockContention` is never returned.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use super::{A2AAck, A2APage, IntegrityFinding, IntegrityReport, MeshStore, MeshStoreError};
use crate::transport::{A2AStoredMessage, DispatchResponse, RemoteExecLeaseRow};

/// In-memory [`MeshStore`]; suitable for unit tests and migration staging.
#[derive(Debug, Default)]
pub struct InMemoryMeshStore {
    a2a: Mutex<Vec<A2AStoredMessage>>,
    leases: Mutex<Vec<RemoteExecLeaseRow>>,
    dispatch: Mutex<HashMap<String, DispatchResponse>>,
}

impl InMemoryMeshStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populate from legacy data (used by the migration path).
    #[must_use]
    pub fn with_data(
        a2a: Vec<A2AStoredMessage>,
        leases: Vec<RemoteExecLeaseRow>,
        dispatch: HashMap<String, DispatchResponse>,
    ) -> Self {
        Self {
            a2a: Mutex::new(a2a),
            leases: Mutex::new(leases),
            dispatch: Mutex::new(dispatch),
        }
    }
}

#[async_trait]
impl MeshStore for InMemoryMeshStore {
    async fn put_a2a(&self, msg: &A2AStoredMessage) -> Result<(), MeshStoreError> {
        let mut v = self
            .a2a
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        if let Some(existing) = v.iter_mut().find(|m| m.id == msg.id) {
            *existing = msg.clone();
        } else {
            v.push(msg.clone());
        }
        Ok(())
    }

    async fn list_a2a(&self, page: A2APage) -> Result<Vec<A2AStoredMessage>, MeshStoreError> {
        let v = self
            .a2a
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        let mut result: Vec<A2AStoredMessage> = v
            .iter()
            .filter(|m| page.include_acked || !m.acknowledged)
            .filter(|m| {
                page.receiver_agent_id
                    .as_deref()
                    .is_none_or(|r| m.receiver_agent_id == r)
            })
            .filter(|m| page.since_id.is_none_or(|s| m.id > s))
            .cloned()
            .collect();
        result.sort_by_key(|m| m.id);
        if let Some(lim) = page.limit {
            result.truncate(lim);
        }
        Ok(result)
    }

    async fn ack_a2a(&self, message_id: u64, ack: A2AAck) -> Result<(), MeshStoreError> {
        let mut v = self
            .a2a
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        if let Some(m) = v.iter_mut().find(|m| m.id == message_id) {
            m.acknowledged = ack.acknowledged;
        }
        Ok(())
    }

    async fn load_all_a2a(&self) -> Result<Vec<A2AStoredMessage>, MeshStoreError> {
        self.list_a2a(A2APage {
            include_acked: true,
            ..Default::default()
        })
        .await
    }

    async fn put_exec_lease(&self, row: &RemoteExecLeaseRow) -> Result<(), MeshStoreError> {
        let mut v = self
            .leases
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        if let Some(e) = v.iter_mut().find(|r| r.lease_id == row.lease_id) {
            *e = row.clone();
        } else {
            v.push(row.clone());
        }
        Ok(())
    }

    async fn list_exec_leases(&self) -> Result<Vec<RemoteExecLeaseRow>, MeshStoreError> {
        let v = self
            .leases
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        Ok(v.clone())
    }

    async fn revoke_exec_lease(&self, lease_id: &str) -> Result<(), MeshStoreError> {
        let mut v = self
            .leases
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        v.retain(|r| r.lease_id != lease_id);
        Ok(())
    }

    async fn delete_exec_lease(&self, lease_id: &str) -> Result<(), MeshStoreError> {
        let mut v = self
            .leases
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        v.retain(|r| r.lease_id != lease_id);
        Ok(())
    }

    async fn put_dispatch_result(
        &self,
        key: &str,
        value: &DispatchResponse,
    ) -> Result<(), MeshStoreError> {
        let mut m = self
            .dispatch
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        m.insert(key.to_string(), value.clone());
        Ok(())
    }

    async fn get_dispatch_result(
        &self,
        key: &str,
    ) -> Result<Option<DispatchResponse>, MeshStoreError> {
        let m = self
            .dispatch
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        Ok(m.get(key).cloned())
    }

    async fn load_all_dispatch_results(
        &self,
    ) -> Result<HashMap<String, DispatchResponse>, MeshStoreError> {
        let m = self
            .dispatch
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        Ok(m.clone())
    }

    fn schema_version(&self) -> u32 {
        1
    }

    async fn integrity_check(&self) -> Result<IntegrityReport, MeshStoreError> {
        let v = self
            .a2a
            .lock()
            .map_err(|e| MeshStoreError::Other(e.to_string()))?;
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for m in v.iter().filter(|m| !m.acknowledged) {
            if let Some(k) = m.idempotency_dedupe_key.as_deref() {
                *counts.entry(k).or_insert(0) += 1;
            }
        }
        let findings: Vec<IntegrityFinding> = counts
            .into_iter()
            .filter(|(_, cnt)| *cnt > 1)
            .map(|(k, cnt)| IntegrityFinding {
                code: "dedupe_violation".into(),
                detail: format!(
                    "idempotency_dedupe_key '{k}' has {cnt} unacked rows (expected ≤1)"
                ),
            })
            .collect();
        Ok(IntegrityReport {
            ok: findings.is_empty(),
            findings,
            schema_version: 1,
        })
    }
}
