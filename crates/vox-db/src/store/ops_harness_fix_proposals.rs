//! Store ops for `scientia_harness_fix_proposals`.
//!
//! `proposed_content` is the full replacement file content and is the sole
//! source of truth for what gets written on approval. `proposed_diff` is
//! computed once at proposal time purely for human-readable display and is
//! never parsed back — a unified diff with context lines cannot be
//! losslessly reconstructed into full content by filtering `+` lines (an
//! earlier draft of this plan tried that and would have silently truncated
//! approved files to just their changed lines).

use crate::VoxDb;
use crate::store::types::StoreError;
use serde::Serialize;
use turso::params;

pub const VALID_STATUSES: &[&str] = &["pending_approval", "applied", "rejected"];

/// One row of `scientia_harness_fix_proposals`. Derives `Serialize` because
/// Tauri commands (Task 13) return this type directly to the frontend as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HarnessFixProposalRow {
    pub id: i64,
    pub issue_id: i64,
    pub target_path: String,
    pub proposed_content: String,
    pub proposed_diff: String,
    pub status: String,
    pub proposed_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
}

impl HarnessFixProposalRow {
    fn from_row(row: &turso::Row) -> Result<Self, StoreError> {
        Ok(Self {
            id: row.get(0).map_err(StoreError::Turso)?,
            issue_id: row.get(1).map_err(StoreError::Turso)?,
            target_path: row.get(2).map_err(StoreError::Turso)?,
            proposed_content: row.get(3).map_err(StoreError::Turso)?,
            proposed_diff: row.get(4).map_err(StoreError::Turso)?,
            status: row.get(5).map_err(StoreError::Turso)?,
            proposed_at_ms: row.get(6).map_err(StoreError::Turso)?,
            resolved_at_ms: row.get(7).map_err(StoreError::Turso)?,
        })
    }
}

pub struct NewFixProposal<'a> {
    pub issue_id: i64,
    pub target_path: &'a str,
    pub proposed_content: &'a str,
    pub proposed_diff: &'a str,
    pub proposed_at_ms: i64,
}

impl VoxDb {
    /// Insert a new pending-approval fix proposal; returns its `id`.
    pub async fn insert_harness_fix_proposal(
        &self,
        new: NewFixProposal<'_>,
    ) -> Result<i64, StoreError> {
        self.conn
            .execute(
                "INSERT INTO scientia_harness_fix_proposals \
                 (issue_id, target_path, proposed_content, proposed_diff, status, proposed_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, 'pending_approval', ?5)",
                params![
                    new.issue_id,
                    new.target_path.to_string(),
                    new.proposed_content.to_string(),
                    new.proposed_diff.to_string(),
                    new.proposed_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        let mut rows = self
            .conn
            .query("SELECT last_insert_rowid()", ())
            .await
            .map_err(StoreError::Turso)?;
        let id: i64 = rows
            .next()
            .await
            .map_err(StoreError::Turso)?
            .ok_or_else(|| {
                StoreError::Db(
                    "scientia_harness_fix_proposals: last_insert_rowid() returned no row".into(),
                )
            })?
            .get(0)
            .map_err(StoreError::Turso)?;
        Ok(id)
    }

    /// List fix proposals, optionally filtered by status, newest first.
    pub async fn list_harness_fix_proposals(
        &self,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<HarnessFixProposalRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, issue_id, target_path, proposed_content, proposed_diff, status, proposed_at_ms, resolved_at_ms \
                 FROM scientia_harness_fix_proposals WHERE (?1 IS NULL OR status = ?1) \
                 ORDER BY id DESC LIMIT ?2",
                params![status.map(str::to_string), limit],
            )
            .await
            .map_err(StoreError::Turso)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            out.push(HarnessFixProposalRow::from_row(&row)?);
        }
        Ok(out)
    }

    /// Fetch one proposal by id.
    pub async fn get_harness_fix_proposal(
        &self,
        id: i64,
    ) -> Result<Option<HarnessFixProposalRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, issue_id, target_path, proposed_content, proposed_diff, status, proposed_at_ms, resolved_at_ms \
                 FROM scientia_harness_fix_proposals WHERE id = ?1",
                params![id],
            )
            .await
            .map_err(StoreError::Turso)?;
        match rows.next().await.map_err(StoreError::Turso)? {
            Some(row) => Ok(Some(HarnessFixProposalRow::from_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Resolve a proposal to `applied` or `rejected`. Does not touch the filesystem —
    /// callers apply `proposed_content` themselves before calling this with `applied`.
    pub async fn resolve_harness_fix_proposal(
        &self,
        id: i64,
        status: &str,
        resolved_at_ms: i64,
    ) -> Result<(), StoreError> {
        if !VALID_STATUSES.contains(&status) || status == "pending_approval" {
            return Err(StoreError::Db(format!(
                "resolve_harness_fix_proposal: status must be 'applied' or 'rejected', got {status:?}"
            )));
        }
        self.conn
            .execute(
                "UPDATE scientia_harness_fix_proposals SET status = ?2, resolved_at_ms = ?3 WHERE id = ?1",
                params![id, status.to_string(), resolved_at_ms],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::NewFixProposal;
    use crate::store::ops_harness_issues::NewHarnessIssue;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn insert_list_resolve_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some("examples/golden/hello.vox"),
                detected_at_ms: 1_000,
                category: "stale_frontmatter",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");
        let proposal_id = db
            .insert_harness_fix_proposal(NewFixProposal {
                issue_id,
                target_path: "examples/golden/hello.vox",
                proposed_content: "// last_validated: 2026-08-02\nfn main() {}\n",
                proposed_diff: "--- a\n+++ b\n",
                proposed_at_ms: 1_500,
            })
            .await
            .expect("insert proposal");

        let pending = db
            .list_harness_fix_proposals(Some("pending_approval"), 10)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, proposal_id);
        assert!(pending[0].proposed_content.contains("2026-08-02"));

        db.resolve_harness_fix_proposal(proposal_id, "applied", 2_000)
            .await
            .expect("resolve");

        let fetched = db
            .get_harness_fix_proposal(proposal_id)
            .await
            .expect("get")
            .expect("row exists");
        assert_eq!(fetched.status, "applied");
        assert_eq!(fetched.resolved_at_ms, Some(2_000));
    }

    #[tokio::test]
    async fn resolve_rejects_pending_approval_as_a_target_status() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some("x"),
                detected_at_ms: 1,
                category: "c",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");
        let proposal_id = db
            .insert_harness_fix_proposal(NewFixProposal {
                issue_id,
                target_path: "x",
                proposed_content: "y",
                proposed_diff: "z",
                proposed_at_ms: 1,
            })
            .await
            .expect("insert proposal");
        let err = db
            .resolve_harness_fix_proposal(proposal_id, "pending_approval", 2)
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("must be 'applied' or 'rejected'"));
    }
}
