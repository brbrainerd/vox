//! Store ops for `scientia_review_decisions` (design §5.1).
//!
//! Append-only per-claim human review decisions. Latest by `decided_at_ms` wins
//! (tie-break: highest `id`). The `decision` field is validated in Rust; Turso/libSQL
//! does not support SQL CHECK constraints.

use crate::VoxDb;
use crate::store::types::StoreError;
use serde::{Deserialize, Serialize};
use turso::params;

/// Allowed values for [`ReviewDecisionRow::decision`].
pub const VALID_DECISIONS: &[&str] = &["approved", "rejected", "deferred", "edited"];

/// One row of `scientia_review_decisions`. The autoincrement `id` is not surfaced;
/// rows are keyed by `(claim_id, decided_at_ms)`. Field order matches the DDL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewDecisionRow {
    pub claim_id: i64,
    pub publication_id: Option<String>,
    /// SHA3-256 of the publication content at decision time.
    pub bound_digest: String,
    /// One of: approved | rejected | deferred | edited. Validated in Rust.
    pub decision: String,
    /// Human user_id (local_user_id()).
    pub actor: String,
    pub reason: Option<String>,
    /// JSON array of model fingerprints present in the artifact (for AI disclosure).
    pub model_fingerprints_json: Option<String>,
    /// Epoch-millis supplied by the caller; do NOT call a clock inside the lib.
    pub decided_at_ms: i64,
}

impl VoxDb {
    /// Append a human review decision for `row.claim_id`.
    ///
    /// `decision` must be one of `approved|rejected|deferred|edited`, and
    /// `bound_digest` and `actor` must be non-empty. These constraints are
    /// enforced in Rust because Turso/libSQL does not support CHECK constraints.
    pub async fn record_review_decision(&self, row: &ReviewDecisionRow) -> Result<(), StoreError> {
        if !VALID_DECISIONS.contains(&row.decision.as_str()) {
            return Err(StoreError::Db(format!(
                "scientia_review_decisions.decision must be one of {:?}, got {:?}",
                VALID_DECISIONS, row.decision,
            )));
        }
        if row.bound_digest.trim().is_empty() {
            return Err(StoreError::Db(
                "scientia_review_decisions.bound_digest must be non-empty".to_string(),
            ));
        }
        if row.actor.trim().is_empty() {
            return Err(StoreError::Db(
                "scientia_review_decisions.actor must be non-empty".to_string(),
            ));
        }
        self.conn
            .execute(
                "INSERT INTO scientia_review_decisions(\
                    claim_id, publication_id, bound_digest, decision, actor, \
                    reason, model_fingerprints_json, decided_at_ms\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    row.claim_id,
                    row.publication_id.clone(),
                    row.bound_digest.clone(),
                    row.decision.clone(),
                    row.actor.clone(),
                    row.reason.clone(),
                    row.model_fingerprints_json.clone(),
                    row.decided_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }

    /// Return the latest review decision for `claim_id` (highest `decided_at_ms`,
    /// tie-break by highest `id`), or `None` if no decision has been recorded.
    pub async fn latest_decision_for_claim(
        &self,
        claim_id: i64,
    ) -> Result<Option<ReviewDecisionRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT claim_id, publication_id, bound_digest, decision, actor, \
                        reason, model_fingerprints_json, decided_at_ms \
                 FROM scientia_review_decisions \
                 WHERE claim_id = ?1 \
                 ORDER BY decided_at_ms DESC, id DESC \
                 LIMIT 1",
                params![claim_id],
            )
            .await
            .map_err(StoreError::Turso)?;
        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            Ok(Some(ReviewDecisionRow {
                claim_id: row.get(0).map_err(StoreError::Turso)?,
                publication_id: row.get(1).map_err(StoreError::Turso)?,
                bound_digest: row.get(2).map_err(StoreError::Turso)?,
                decision: row.get(3).map_err(StoreError::Turso)?,
                actor: row.get(4).map_err(StoreError::Turso)?,
                reason: row.get(5).map_err(StoreError::Turso)?,
                model_fingerprints_json: row.get(6).map_err(StoreError::Turso)?,
                decided_at_ms: row.get(7).map_err(StoreError::Turso)?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn record_and_latest_decision_round_trips() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 42,
            publication_id: Some("pub-001".into()),
            bound_digest: "abc123sha3".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1_748_000_000_000,
        };
        db.record_review_decision(&row).await.expect("record");

        let got = db
            .latest_decision_for_claim(42)
            .await
            .expect("latest")
            .expect("row present");
        assert_eq!(got, row);
    }

    #[tokio::test]
    async fn latest_decision_supersedes_earlier_by_decided_at_ms() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");

        let first = ReviewDecisionRow {
            claim_id: 99,
            publication_id: Some("pub-002".into()),
            bound_digest: "digest-v1".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: Some("looks good".into()),
            model_fingerprints_json: None,
            decided_at_ms: 1_748_000_000_000,
        };
        db.record_review_decision(&first)
            .await
            .expect("record first");

        let second = ReviewDecisionRow {
            claim_id: 99,
            publication_id: Some("pub-002".into()),
            bound_digest: "digest-v2".into(),
            decision: "rejected".into(),
            actor: "bob".into(),
            reason: Some("on reflection, not novel".into()),
            model_fingerprints_json: Some(r#"["fp-model-a"]"#.into()),
            decided_at_ms: 1_748_000_001_000, // 1 second later
        };
        db.record_review_decision(&second)
            .await
            .expect("record second");

        let got = db
            .latest_decision_for_claim(99)
            .await
            .expect("latest")
            .expect("row present");
        // The later (rejected) decision must win.
        assert_eq!(
            got.decision, "rejected",
            "later decision must supersede earlier"
        );
        assert_eq!(got.actor, "bob");
        assert_eq!(got.bound_digest, "digest-v2");
    }

    #[tokio::test]
    async fn latest_decision_returns_none_for_unknown_claim() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let got = db.latest_decision_for_claim(9999).await.expect("latest");
        assert!(got.is_none(), "unknown claim must return None");
    }

    #[tokio::test]
    async fn record_review_decision_rejects_invalid_decision_value() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: None,
            bound_digest: "some-digest".into(),
            decision: "maybe".into(), // invalid
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        };
        let err = db
            .record_review_decision(&row)
            .await
            .expect_err("invalid decision must error");
        assert!(
            err.to_string().contains("decision"),
            "error must mention decision, got: {err}"
        );
    }

    #[tokio::test]
    async fn record_review_decision_rejects_empty_bound_digest() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: None,
            bound_digest: "  ".into(), // empty after trim
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        };
        let err = db
            .record_review_decision(&row)
            .await
            .expect_err("empty bound_digest must error");
        assert!(
            err.to_string().contains("bound_digest"),
            "error must mention bound_digest, got: {err}"
        );
    }

    #[tokio::test]
    async fn record_review_decision_rejects_empty_actor() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: None,
            bound_digest: "some-digest".into(),
            decision: "approved".into(),
            actor: "  ".into(), // empty after trim
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        };
        let err = db
            .record_review_decision(&row)
            .await
            .expect_err("empty actor must error");
        assert!(
            err.to_string().contains("actor"),
            "error must mention actor, got: {err}"
        );
    }
}
