//! HITL approval audit log ops (B3).
//!
//! Durable audit trail for the in-memory `PendingApprovals` gate: one row per
//! dangerous-tool approval request, written `pending` at the gate and updated to
//! its outcome when resolved. Survives restarts (audit/visibility); the live
//! await itself is in-memory and not resumable across restarts.
//!
//! Writes (`hitl_approval_record`, `hitl_approval_resolve`) run through
//! `self.breaker.call` for circuit-breaker protection; the reads
//! (`hitl_approval_get`, `hitl_approvals_recent`) query the connection directly,
//! consistent with sibling facades like `agent_runs.rs`.

use crate::StoreError;
use crate::VoxDb;

/// One `hitl_approvals` row.
#[derive(Debug, Clone)]
pub struct HitlApprovalRow {
    /// Approval id (primary key; matches the in-memory registry id).
    pub approval_id: String,
    /// Canonical tool name the approval gated.
    pub tool: String,
    /// Short human-readable summary of the gated action.
    pub summary: String,
    /// Lifecycle status: `pending` | `approved` | `rejected` | `modified` | `timed_out`.
    pub status: String,
    /// When the approval was requested (unix-ms).
    pub requested_at_ms: i64,
    /// When it was resolved (unix-ms); `None` while pending.
    pub resolved_at_ms: Option<i64>,
}

const SELECT_COLS: &str = "approval_id, tool, summary, status, requested_at_ms, resolved_at_ms";

fn map_row(row: &turso::Row) -> Result<HitlApprovalRow, StoreError> {
    let e = |err: turso::Error| StoreError::Db(err.to_string());
    Ok(HitlApprovalRow {
        approval_id: row.get(0).map_err(e)?,
        tool: row.get(1).map_err(e)?,
        summary: row.get(2).map_err(e)?,
        status: row.get(3).map_err(e)?,
        requested_at_ms: row.get(4).map_err(e)?,
        resolved_at_ms: row.get(5).map_err(e)?,
    })
}

impl VoxDb {
    /// Record a newly-requested approval (status `pending`).
    // toestub-ignore(skeleton/untested-pub-api) — DB facade methods exercised by tests/hitl_approvals_tests.rs integration tests
    pub async fn hitl_approval_record(
        &self,
        approval_id: &str,
        tool: &str,
        summary: &str,
        requested_at_ms: i64,
    ) -> Result<(), StoreError> {
        let (approval_id, tool, summary) = (
            approval_id.to_string(),
            tool.to_string(),
            summary.to_string(),
        );
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO hitl_approvals
                       (approval_id, tool, summary, status, requested_at_ms, resolved_at_ms)
                     VALUES (?1, ?2, ?3, 'pending', ?4, NULL)
                     ON CONFLICT(approval_id) DO UPDATE SET
                        tool = excluded.tool,
                        summary = excluded.summary",
                    turso::params![approval_id, tool, summary, requested_at_ms],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Update an approval to its terminal outcome.
    // toestub-ignore(skeleton/untested-pub-api) — DB facade methods exercised by tests/hitl_approvals_tests.rs integration tests
    pub async fn hitl_approval_resolve(
        &self,
        approval_id: &str,
        status: &str,
        resolved_at_ms: i64,
    ) -> Result<(), StoreError> {
        let (approval_id, status) = (approval_id.to_string(), status.to_string());
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE hitl_approvals SET status = ?2, resolved_at_ms = ?3 WHERE approval_id = ?1",
                    turso::params![approval_id, status, resolved_at_ms],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Fetch one approval by id.
    // toestub-ignore(skeleton/untested-pub-api) — DB facade methods exercised by tests/hitl_approvals_tests.rs integration tests
    pub async fn hitl_approval_get(
        &self,
        approval_id: &str,
    ) -> Result<Option<HitlApprovalRow>, StoreError> {
        let conn = self.conn.clone();
        let sql = format!("SELECT {SELECT_COLS} FROM hitl_approvals WHERE approval_id = ?1");
        let mut cursor = conn
            .query(&sql, turso::params![approval_id.to_string()])
            .await?;
        match cursor.next().await? {
            Some(row) => Ok(Some(map_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Most-recently-requested approvals, newest first.
    // toestub-ignore(skeleton/untested-pub-api) — DB facade methods exercised by tests/hitl_approvals_tests.rs integration tests
    pub async fn hitl_approvals_recent(
        &self,
        limit: i64,
    ) -> Result<Vec<HitlApprovalRow>, StoreError> {
        let conn = self.conn.clone();
        let sql = format!(
            "SELECT {SELECT_COLS} FROM hitl_approvals ORDER BY requested_at_ms DESC LIMIT ?1"
        );
        let mut cursor = conn.query(&sql, turso::params![limit]).await?;
        let mut out = Vec::new();
        while let Some(row) = cursor.next().await? {
            out.push(map_row(&row)?);
        }
        Ok(out)
    }
}
