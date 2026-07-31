//! Store ops for the `skill_candidates` table.
//!
//! Task 3.2: the skill-mining pipeline (`code_miner`/`op_miner` in
//! `vox-skill-discovery`) previously only printed advisory `Candidate`s to
//! stdout via `vox skill suggest` / `vox-discover`. These ops give the CLI
//! caller a place to persist what a mining run found, so later work (Task
//! 3.3: promotion gate, `vox-similarity` dedup, verification-by-different-
//! model, shadow-period state machine) has something durable to read.
//!
//! `status` is a plain TEXT column (not a Rust enum) matching the codebase's
//! looser SQLite convention where the value set is expected to grow — see
//! `reliability_scores.entity_type` for a sibling example of a free-text
//! discriminator. Valid values today: `pending`, `reviewed`, `promoted`,
//! `rejected`; new callers should stick to that set even though it isn't a
//! DB-enforced constraint (Turso does not support CHECK constraints).

use crate::VoxDb;
use crate::store::types::StoreError;
use serde::{Deserialize, Serialize};
use turso::params;

/// One row of `skill_candidates`. Field order matches the DDL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillCandidateRow {
    pub id: i64,
    /// Proposed skill name/id (e.g. `DraftFrontmatter::name` from
    /// `vox-skill-discovery::Candidate`).
    pub candidate_name: String,
    /// Which miner found it / what trajectory, e.g. `"op_miner"` or
    /// `"code_miner"`.
    pub source: String,
    /// Raw JSON of the abstracted trajectory — the serialized
    /// `vox_skill_discovery::Candidate` this row was derived from.
    pub raw_json: String,
    pub status: String,
    /// Provisional/confirmed/deprecated shadow-period state (Task 3.3).
    /// Only meaningful once `status = 'promoted'`; defaults to
    /// `"provisional"` for rows still pending review.
    pub lifecycle_state: String,
    /// Keccak/blake3 hash of `raw_json` at promotion time, used to detect a
    /// later mining run producing a materially different trajectory under
    /// the same `candidate_name` (Task 3.3 gate 8). `None` until promoted.
    pub source_hash: Option<String>,
    pub created_at_ms: i64,
}

/// Fields needed to insert a new candidate; `id` is assigned by the DB.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSkillCandidate {
    pub candidate_name: String,
    pub source: String,
    pub raw_json: String,
}

impl VoxDb {
    /// Insert a mined candidate skill with `status = 'pending'`. Returns the
    /// assigned row id.
    pub async fn insert_skill_candidate(
        &self,
        candidate: &NewSkillCandidate,
    ) -> Result<i64, StoreError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let candidate_name = candidate.candidate_name.clone();
        let source = candidate.source.clone();
        let raw_json = candidate.raw_json.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO skill_candidates
                         (candidate_name, source, raw_json, status, created_at_ms)
                     VALUES (?1, ?2, ?3, 'pending', ?4)",
                    params![
                        candidate_name.as_str(),
                        source.as_str(),
                        raw_json.as_str(),
                        now_ms
                    ],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// List candidates with `status = 'pending'`, newest first.
    pub async fn list_pending_skill_candidates(
        &self,
    ) -> Result<Vec<SkillCandidateRow>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, candidate_name, source, raw_json, status, lifecycle_state, source_hash, created_at_ms
                         FROM skill_candidates
                         WHERE status = 'pending'
                         ORDER BY created_at_ms DESC, id DESC",
                        (),
                    )
                    .await
                    .map_err(StoreError::Turso)?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
                    out.push(row_from_turso(&row)?);
                }
                Ok::<Vec<SkillCandidateRow>, StoreError>(out)
            })
            .await
    }

    /// Fetch a single candidate row by id.
    pub async fn get_skill_candidate(
        &self,
        id: i64,
    ) -> Result<Option<SkillCandidateRow>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(move || async move {
                let mut rows = conn
                    .query(
                        "SELECT id, candidate_name, source, raw_json, status, lifecycle_state, source_hash, created_at_ms
                         FROM skill_candidates WHERE id = ?1",
                        params![id],
                    )
                    .await
                    .map_err(StoreError::Turso)?;
                match rows.next().await.map_err(StoreError::Turso)? {
                    Some(row) => Ok::<Option<SkillCandidateRow>, StoreError>(Some(row_from_turso(&row)?)),
                    None => Ok(None),
                }
            })
            .await
    }

    /// Count all rows (any status) sharing `candidate_name` — the observed-
    /// trajectory count used by Task 3.3's generality gate (gate 5: promotion
    /// requires >= N independently mined trajectories for the same
    /// candidate, not just one).
    pub async fn count_skill_candidates_by_name(
        &self,
        candidate_name: &str,
    ) -> Result<i64, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let candidate_name = candidate_name.to_string();
        breaker
            .call(move || async move {
                let mut rows = conn
                    .query(
                        "SELECT COUNT(*) FROM skill_candidates WHERE candidate_name = ?1",
                        params![candidate_name.as_str()],
                    )
                    .await
                    .map_err(StoreError::Turso)?;
                let count: i64 = match rows.next().await.map_err(StoreError::Turso)? {
                    Some(row) => row.get(0).map_err(StoreError::Turso)?,
                    None => 0,
                };
                Ok::<i64, StoreError>(count)
            })
            .await
    }

    /// Update `status` for a candidate row (e.g. `pending` -> `promoted` /
    /// `rejected` / `reviewed`).
    pub async fn update_skill_candidate_status(
        &self,
        id: i64,
        status: &str,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let status = status.to_string();
        breaker
            .call(move || async move {
                conn.execute(
                    "UPDATE skill_candidates SET status = ?1 WHERE id = ?2",
                    params![status.as_str(), id],
                )
                .await
                .map_err(StoreError::Turso)?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Update the provisional/confirmed/deprecated shadow-period state, and
    /// optionally bind `source_hash` (set at first promotion, or refreshed
    /// when a re-verification succeeds after a provenance mismatch).
    pub async fn update_skill_candidate_lifecycle(
        &self,
        id: i64,
        lifecycle_state: &str,
        source_hash: Option<&str>,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let lifecycle_state = lifecycle_state.to_string();
        let source_hash = source_hash.map(|s| s.to_string());
        breaker
            .call(move || async move {
                conn.execute(
                    "UPDATE skill_candidates SET lifecycle_state = ?1, source_hash = COALESCE(?2, source_hash) WHERE id = ?3",
                    params![lifecycle_state.as_str(), source_hash.as_deref(), id],
                )
                .await
                .map_err(StoreError::Turso)?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}

fn row_from_turso(row: &turso::Row) -> Result<SkillCandidateRow, StoreError> {
    Ok(SkillCandidateRow {
        id: row.get(0).map_err(StoreError::Turso)?,
        candidate_name: row.get(1).map_err(StoreError::Turso)?,
        source: row.get(2).map_err(StoreError::Turso)?,
        raw_json: row.get(3).map_err(StoreError::Turso)?,
        status: row.get(4).map_err(StoreError::Turso)?,
        lifecycle_state: row.get(5).map_err(StoreError::Turso)?,
        source_hash: row.get(6).map_err(StoreError::Turso)?,
        created_at_ms: row.get(7).map_err(StoreError::Turso)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn insert_and_list_pending_roundtrip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");

        let id = db
            .insert_skill_candidate(&NewSkillCandidate {
                candidate_name: "a-b-c".to_string(),
                source: "op_miner".to_string(),
                raw_json: r#"{"kind":"RepeatedOperations"}"#.to_string(),
            })
            .await
            .expect("insert");
        assert!(id > 0);

        let pending = db.list_pending_skill_candidates().await.expect("list");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].candidate_name, "a-b-c");
        assert_eq!(pending[0].source, "op_miner");
        assert_eq!(pending[0].status, "pending");
        assert!(pending[0].raw_json.contains("RepeatedOperations"));
    }

    #[tokio::test]
    async fn list_pending_excludes_non_pending_status() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");

        db.insert_skill_candidate(&NewSkillCandidate {
            candidate_name: "keep-me".to_string(),
            source: "op_miner".to_string(),
            raw_json: "{}".to_string(),
        })
        .await
        .expect("insert pending");

        let id = db
            .insert_skill_candidate(&NewSkillCandidate {
                candidate_name: "promote-me".to_string(),
                source: "code_miner".to_string(),
                raw_json: "{}".to_string(),
            })
            .await
            .expect("insert to-be-promoted");
        db.conn
            .execute(
                "UPDATE skill_candidates SET status = 'promoted' WHERE id = ?1",
                params![id],
            )
            .await
            .expect("update status");

        let pending = db.list_pending_skill_candidates().await.expect("list");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].candidate_name, "keep-me");
    }

    #[tokio::test]
    async fn list_pending_orders_newest_first() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        for name in ["first", "second", "third"] {
            db.insert_skill_candidate(&NewSkillCandidate {
                candidate_name: name.to_string(),
                source: "op_miner".to_string(),
                raw_json: "{}".to_string(),
            })
            .await
            .expect("insert");
        }
        let pending = db.list_pending_skill_candidates().await.expect("list");
        assert_eq!(pending.len(), 3);
        assert_eq!(pending[0].candidate_name, "third");
        assert_eq!(pending[2].candidate_name, "first");
    }

    #[tokio::test]
    async fn new_rows_default_to_provisional_lifecycle_and_no_hash() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        db.insert_skill_candidate(&NewSkillCandidate {
            candidate_name: "a".to_string(),
            source: "op_miner".to_string(),
            raw_json: "{}".to_string(),
        })
        .await
        .expect("insert");
        let pending = db.list_pending_skill_candidates().await.expect("list");
        assert_eq!(pending[0].lifecycle_state, "provisional");
        assert_eq!(pending[0].source_hash, None);
    }

    #[tokio::test]
    async fn count_by_name_counts_across_all_statuses() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        for _ in 0..2 {
            db.insert_skill_candidate(&NewSkillCandidate {
                candidate_name: "dup".to_string(),
                source: "op_miner".to_string(),
                raw_json: "{}".to_string(),
            })
            .await
            .expect("insert");
        }
        db.insert_skill_candidate(&NewSkillCandidate {
            candidate_name: "other".to_string(),
            source: "op_miner".to_string(),
            raw_json: "{}".to_string(),
        })
        .await
        .expect("insert");

        assert_eq!(db.count_skill_candidates_by_name("dup").await.unwrap(), 2);
        assert_eq!(db.count_skill_candidates_by_name("other").await.unwrap(), 1);
        assert_eq!(db.count_skill_candidates_by_name("missing").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn update_status_and_lifecycle_roundtrip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let id = db
            .insert_skill_candidate(&NewSkillCandidate {
                candidate_name: "a".to_string(),
                source: "op_miner".to_string(),
                raw_json: "{}".to_string(),
            })
            .await
            .expect("insert");

        db.update_skill_candidate_status(id, "promoted")
            .await
            .expect("status update");
        db.update_skill_candidate_lifecycle(id, "confirmed", Some("abc123"))
            .await
            .expect("lifecycle update");

        let row = db.get_skill_candidate(id).await.unwrap().expect("row exists");
        assert_eq!(row.status, "promoted");
        assert_eq!(row.lifecycle_state, "confirmed");
        assert_eq!(row.source_hash.as_deref(), Some("abc123"));

        // A later lifecycle update with source_hash = None must not clobber
        // the previously bound hash (COALESCE keeps provenance sticky).
        db.update_skill_candidate_lifecycle(id, "deprecated", None)
            .await
            .expect("lifecycle update 2");
        let row = db.get_skill_candidate(id).await.unwrap().expect("row exists");
        assert_eq!(row.lifecycle_state, "deprecated");
        assert_eq!(row.source_hash.as_deref(), Some("abc123"));
    }

    #[tokio::test]
    async fn get_skill_candidate_returns_none_for_missing_id() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        assert_eq!(db.get_skill_candidate(999).await.unwrap(), None);
    }
}
