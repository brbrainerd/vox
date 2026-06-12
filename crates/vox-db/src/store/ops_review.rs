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
    /// The publication this decision is scoped to. Non-empty; enforced in Rust
    /// (the DDL column stays nullable `TEXT` — Turso/libSQL has no CHECK).
    pub publication_id: String,
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
        // Rust-side non-empty guard is the enforcement (the DDL column is
        // nullable `TEXT`; Turso/libSQL has no CHECK constraint).
        if row.publication_id.trim().is_empty() {
            return Err(StoreError::Db(
                "scientia_review_decisions.publication_id must be non-empty".to_string(),
            ));
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
        // `model_fingerprints_json`, when present, is documented as a JSON array
        // (AI-disclosure fingerprints). Validate shape in Rust (no CHECK in Turso)
        // so malformed disclosure data cannot be persisted.
        if let Some(raw) = &row.model_fingerprints_json {
            let parsed: serde_json::Value = serde_json::from_str(raw).map_err(|e| {
                StoreError::Db(format!(
                    "scientia_review_decisions.model_fingerprints_json must be valid JSON: {e}"
                ))
            })?;
            if !parsed.is_array() {
                return Err(StoreError::Db(
                    "scientia_review_decisions.model_fingerprints_json must be a JSON array"
                        .to_string(),
                ));
            }
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

    /// Return the latest review decision for `claim_id` **within `publication_id`**
    /// (highest `decided_at_ms`, tie-break by highest `id`), or `None` if none has
    /// been recorded. Scoping by publication is load-bearing: `claim_id` is an
    /// FNV-1a hash of the claim text, so the same claim text in two publications
    /// shares an id — an unscoped lookup would let a decision in one publication
    /// leak into another. Approval is per-claim *within* a publication.
    pub async fn latest_decision_for_claim(
        &self,
        claim_id: i64,
        publication_id: &str,
    ) -> Result<Option<ReviewDecisionRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT claim_id, publication_id, bound_digest, decision, actor, \
                        reason, model_fingerprints_json, decided_at_ms \
                 FROM scientia_review_decisions \
                 WHERE claim_id = ?1 AND publication_id = ?2 \
                 ORDER BY decided_at_ms DESC, id DESC \
                 LIMIT 1",
                params![claim_id, publication_id],
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

    async fn memory_db() -> Result<VoxDb, StoreError> {
        VoxDb::connect(DbConfig::Memory).await
    }

    #[tokio::test]
    async fn record_and_latest_decision_round_trips() -> Result<(), StoreError> {
        let db = memory_db().await?;
        let row = ReviewDecisionRow {
            claim_id: 42,
            publication_id: "pub-001".into(),
            bound_digest: "abc123sha3".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1_748_000_000_000,
        };
        db.record_review_decision(&row).await?;

        let got = db
            .latest_decision_for_claim(42, "pub-001")
            .await?
            .ok_or_else(|| StoreError::Db("expected review row".into()))?;
        assert_eq!(got, row);
        Ok(())
    }

    #[tokio::test]
    async fn latest_decision_supersedes_earlier_by_decided_at_ms() -> Result<(), StoreError> {
        let db = memory_db().await?;

        let first = ReviewDecisionRow {
            claim_id: 99,
            publication_id: "pub-002".into(),
            bound_digest: "digest-v1".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: Some("looks good".into()),
            model_fingerprints_json: None,
            decided_at_ms: 1_748_000_000_000,
        };
        db.record_review_decision(&first).await?;

        let second = ReviewDecisionRow {
            claim_id: 99,
            publication_id: "pub-002".into(),
            bound_digest: "digest-v2".into(),
            decision: "rejected".into(),
            actor: "bob".into(),
            reason: Some("on reflection, not novel".into()),
            model_fingerprints_json: Some(r#"["fp-model-a"]"#.into()),
            decided_at_ms: 1_748_000_001_000, // 1 second later
        };
        db.record_review_decision(&second).await?;

        let got = db
            .latest_decision_for_claim(99, "pub-002")
            .await?
            .ok_or_else(|| StoreError::Db("expected review row".into()))?;
        // The later (rejected) decision must win.
        assert_eq!(
            got.decision, "rejected",
            "later decision must supersede earlier"
        );
        assert_eq!(got.actor, "bob");
        assert_eq!(got.bound_digest, "digest-v2");
        Ok(())
    }

    #[tokio::test]
    async fn latest_decision_returns_none_for_unknown_claim() -> Result<(), StoreError> {
        let db = memory_db().await?;
        let got = db.latest_decision_for_claim(9999, "pub-x").await?;
        assert!(got.is_none(), "unknown claim must return None");
        Ok(())
    }

    #[tokio::test]
    async fn latest_decision_scoped_per_publication() {
        // Same claim_id (claim text hashes are publication-independent) decided
        // differently in two publications: each lookup must see only its own.
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        db.record_review_decision(&ReviewDecisionRow {
            claim_id: 7,
            publication_id: "pub-A".into(),
            bound_digest: "dig-A".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        })
        .await
        .expect("record A");
        db.record_review_decision(&ReviewDecisionRow {
            claim_id: 7,
            publication_id: "pub-B".into(),
            bound_digest: "dig-B".into(),
            decision: "rejected".into(),
            actor: "bob".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 2,
        })
        .await
        .expect("record B");

        let a = db
            .latest_decision_for_claim(7, "pub-A")
            .await
            .expect("latest A")
            .expect("row A");
        assert_eq!(a.decision, "approved");
        assert_eq!(a.bound_digest, "dig-A");
        let b = db
            .latest_decision_for_claim(7, "pub-B")
            .await
            .expect("latest B")
            .expect("row B");
        assert_eq!(b.decision, "rejected");
        assert_eq!(b.bound_digest, "dig-B");
    }

    #[tokio::test]
    async fn latest_decision_tiebreak_by_id_desc() {
        // Equal decided_at_ms: the later-inserted row (higher id) must win.
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let base = ReviewDecisionRow {
            claim_id: 5,
            publication_id: "pub-tb".into(),
            bound_digest: "d1".into(),
            decision: "approved".into(),
            actor: "first".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 100,
        };
        db.record_review_decision(&base).await.expect("record 1");
        db.record_review_decision(&ReviewDecisionRow {
            bound_digest: "d2".into(),
            decision: "rejected".into(),
            actor: "second".into(),
            ..base.clone()
        })
        .await
        .expect("record 2");
        let got = db
            .latest_decision_for_claim(5, "pub-tb")
            .await
            .expect("latest")
            .expect("row");
        assert_eq!(got.actor, "second", "higher id must win on equal timestamp");
        assert_eq!(got.decision, "rejected");
    }

    #[tokio::test]
    async fn record_review_decision_rejects_invalid_decision_value() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: "pub-none".into(),
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
    async fn record_review_decision_rejects_non_array_model_fingerprints() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let base = ReviewDecisionRow {
            claim_id: 1,
            publication_id: "pub-fp".into(),
            bound_digest: "d".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: Some("not json".into()),
            decided_at_ms: 1,
        };
        // Invalid JSON → error.
        let err = db
            .record_review_decision(&base)
            .await
            .expect_err("invalid JSON must error");
        assert!(err.to_string().contains("model_fingerprints_json"));
        // Valid JSON but not an array → error.
        let err = db
            .record_review_decision(&ReviewDecisionRow {
                model_fingerprints_json: Some(r#"{"a":1}"#.into()),
                ..base.clone()
            })
            .await
            .expect_err("non-array JSON must error");
        assert!(err.to_string().contains("array"));
        // Valid JSON array → accepted.
        db.record_review_decision(&ReviewDecisionRow {
            model_fingerprints_json: Some(r#"["fp-a","fp-b"]"#.into()),
            ..base.clone()
        })
        .await
        .expect("valid JSON array must be accepted");
    }

    #[tokio::test]
    async fn record_review_decision_rejects_empty_bound_digest() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: "pub-none".into(),
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
    async fn record_review_decision_rejects_empty_publication_id() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: "  ".into(), // empty after trim
            bound_digest: "some-digest".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        };
        let err = db
            .record_review_decision(&row)
            .await
            .expect_err("empty publication_id must error");
        assert!(
            err.to_string().contains("publication_id"),
            "error must mention publication_id, got: {err}"
        );
    }

    #[tokio::test]
    async fn record_review_decision_rejects_empty_actor() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = ReviewDecisionRow {
            claim_id: 1,
            publication_id: "pub-none".into(),
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
