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
                        "SELECT id, candidate_name, source, raw_json, status, created_at_ms
                         FROM skill_candidates
                         WHERE status = 'pending'
                         ORDER BY created_at_ms DESC, id DESC",
                        (),
                    )
                    .await
                    .map_err(StoreError::Turso)?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
                    out.push(SkillCandidateRow {
                        id: row.get(0).map_err(StoreError::Turso)?,
                        candidate_name: row.get(1).map_err(StoreError::Turso)?,
                        source: row.get(2).map_err(StoreError::Turso)?,
                        raw_json: row.get(3).map_err(StoreError::Turso)?,
                        status: row.get(4).map_err(StoreError::Turso)?,
                        created_at_ms: row.get(5).map_err(StoreError::Turso)?,
                    });
                }
                Ok::<Vec<SkillCandidateRow>, StoreError>(out)
            })
            .await
    }
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
}
