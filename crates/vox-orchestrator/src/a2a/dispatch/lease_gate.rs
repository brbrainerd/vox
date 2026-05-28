//! P0-T3 (ADR-017, W1): authoritative-lease check before local fallback.

use thiserror::Error;
use vox_db::VoxDb;

#[derive(Debug, Error)]
pub enum LeaseGateError {
    #[error("scope `{scope_key}` is held by remote node `{holder_node_id}` until {expires_at}ms")]
    HeldByRemote {
        scope_key: String,
        holder_node_id: String,
        expires_at: i64,
    },
    #[error("vox-db error: {0}")]
    Db(String),
}

/// Returns `Ok(())` when local fallback is permitted. Returns
/// `Err(HeldByRemote)` when an unexpired lease exists on a different node —
/// the caller must surface this as a routing decision (queue for retry,
/// proxy via mesh) rather than duplicate-execute.
pub async fn check_before_local_fallback(
    db: &VoxDb,
    scope_key: &str,
    self_node_id: &str,
    now_ms: i64,
) -> Result<(), LeaseGateError> {
    let lease = db
        .mesh_exec_lease_for_scope(scope_key)
        .await
        .map_err(|e| LeaseGateError::Db(e.to_string()))?;
    let Some(lease) = lease else {
        return Ok(());
    };
    if lease.expires_at < now_ms {
        return Ok(());
    }
    if lease.holder_node_id == self_node_id {
        return Ok(());
    }
    Err(LeaseGateError::HeldByRemote {
        scope_key: scope_key.to_string(),
        holder_node_id: lease.holder_node_id,
        expires_at: lease.expires_at,
    })
}
