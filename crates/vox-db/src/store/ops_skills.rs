//! Skill reliability reads/writes for [`VoxDb`], mirroring the `entity_type = 'agent'`
//! pattern established in `ops_agents.rs` for the consolidated `reliability_scores`
//! table (see `store/open.rs` migration notes: `agent_reliability`, `skill_reliability`,
//! `workflow_reliability`, and `repository_reliability` were retired in schema v51 and
//! consolidated here — do not read from the legacy `skill_reliability` table).
//!
//! Task 3.1: nothing currently writes `entity_type = 'skill'` rows (no caller yet
//! reports skill-invocation success/failure), so [`list_skill_reliability`] will
//! typically return an empty map. Callers (e.g. the chat-tools skill catalog
//! renderer) must treat a missing entry as "no data yet", not as a low score.

use turso::params;

use crate::store::types::StoreError;

impl crate::VoxDb {
    /// Return all `(skill_id, reliability)` pairs from `reliability_scores` where
    /// `entity_type = 'skill'`, as a map for O(1) lookup by skill id/name.
    ///
    /// Skills with no recorded observations simply have no entry in the returned
    /// map — callers must not default a missing entry to any particular score.
    pub async fn list_skill_reliability(
        &self,
    ) -> Result<std::collections::HashMap<String, f64>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT entity_id, reliability FROM reliability_scores WHERE entity_type = 'skill'",
                        (),
                    )
                    .await?;
                let mut out = std::collections::HashMap::new();
                while let Some(row) = rows.next().await? {
                    let id: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                    let r: f64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
                    out.insert(id, r);
                }
                Ok(out)
            })
            .await
    }

    /// Read `reliability` for one `skill_id`, or `None` if no row exists yet.
    pub async fn get_skill_reliability(&self, skill_id: &str) -> Result<Option<f64>, StoreError> {
        let skill_id = skill_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT reliability FROM reliability_scores WHERE entity_type = 'skill' AND entity_id = ?1 LIMIT 1",
                        params![skill_id.as_str()],
                    )
                    .await?;
                match rows.next().await? {
                    Some(row) => {
                        let r: f64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                        Ok(Some(r))
                    }
                    None => Ok(None),
                }
            })
            .await
    }

    /// Upsert a Laplace-smoothed reliability score for `skill_id`, same formula as
    /// `record_task_reliability_observation` for agents. Not yet called by any
    /// producer — a natural follow-up once something reports skill-invocation
    /// outcomes — but kept minimal here since the write path is cheap and keeps
    /// the read/write pair symmetric with the agent pattern.
    pub async fn record_skill_reliability_observation(
        &self,
        skill_id: &str,
        success: bool,
    ) -> Result<(), StoreError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let skill_id = skill_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                if success {
                    conn.execute(
                        "INSERT INTO reliability_scores (entity_type, entity_id, success_count, failure_count,
                             reliability, updated_at_ms)
                         VALUES ('skill', ?1, 1, 0,
                             CAST(2 AS REAL) / CAST(3 AS REAL),
                             ?2)
                         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                             success_count  = success_count + 1,
                             reliability    = CAST(success_count + 2 AS REAL)
                                            / CAST(success_count + failure_count + 3 AS REAL),
                             updated_at_ms  = ?2",
                        params![skill_id.as_str(), now_ms],
                    )
                    .await?;
                } else {
                    conn.execute(
                        "INSERT INTO reliability_scores (entity_type, entity_id, success_count, failure_count,
                             reliability, updated_at_ms)
                         VALUES ('skill', ?1, 0, 1,
                             CAST(1 AS REAL) / CAST(3 AS REAL),
                             ?2)
                         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                             failure_count  = failure_count + 1,
                             reliability    = CAST(success_count + 1 AS REAL)
                                            / CAST(success_count + failure_count + 3 AS REAL),
                             updated_at_ms  = ?2",
                        params![skill_id.as_str(), now_ms],
                    )
                    .await?;
                }
                Ok::<(), StoreError>(())
            })
            .await
    }
}
