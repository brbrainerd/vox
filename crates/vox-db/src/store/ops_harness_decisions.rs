//! Store ops for `scientia_harness_decisions` (append-only human decision ledger).

use crate::VoxDb;
use crate::store::types::StoreError;
use turso::params;

pub const VALID_DECISIONS: &[&str] = &["confirmed", "dismissed"];

/// One row of `scientia_harness_decisions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessIssueDecisionRow {
    pub issue_id: i64,
    pub decision: String,
    pub actor: String,
    pub reason: Option<String>,
    pub decided_at_ms: i64,
}

impl VoxDb {
    /// Append a human decision for `row.issue_id` and flip the issue's status to match.
    pub async fn record_harness_issue_decision(
        &self,
        row: &HarnessIssueDecisionRow,
    ) -> Result<(), StoreError> {
        if !VALID_DECISIONS.contains(&row.decision.as_str()) {
            return Err(StoreError::Db(format!(
                "scientia_harness_decisions.decision must be one of {VALID_DECISIONS:?}, got {:?}",
                row.decision
            )));
        }
        if row.actor.trim().is_empty() {
            return Err(StoreError::Db(
                "scientia_harness_decisions.actor must be non-empty".to_string(),
            ));
        }
        self.conn
            .execute(
                "INSERT INTO scientia_harness_decisions \
                 (issue_id, decision, actor, reason, decided_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    row.issue_id,
                    row.decision.clone(),
                    row.actor.clone(),
                    row.reason.clone(),
                    row.decided_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        self.set_harness_issue_status(row.issue_id, &row.decision)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::HarnessIssueDecisionRow;
    use crate::store::ops_harness_issues::NewHarnessIssue;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn recording_decision_flips_issue_status() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some("examples/golden/x.vox"),
                detected_at_ms: 1_000,
                category: "stale_frontmatter",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");

        db.record_harness_issue_decision(&HarnessIssueDecisionRow {
            issue_id,
            decision: "confirmed".to_string(),
            actor: "local_user".to_string(),
            reason: None,
            decided_at_ms: 2_000,
        })
        .await
        .expect("record decision");

        let rows = db
            .list_harness_issues(Some("confirmed"), None, 10)
            .await
            .expect("list confirmed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, issue_id);
    }

    #[tokio::test]
    async fn rejects_empty_actor() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: None,
                detected_at_ms: 1_000,
                category: "c",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");
        let err = db
            .record_harness_issue_decision(&HarnessIssueDecisionRow {
                issue_id,
                decision: "confirmed".to_string(),
                actor: String::new(),
                reason: None,
                decided_at_ms: 2_000,
            })
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("actor must be non-empty"));
    }
}
