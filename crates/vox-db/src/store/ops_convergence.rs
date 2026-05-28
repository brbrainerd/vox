//! Convergence op-log CRUD for [`crate::VoxDb`] (P3-T1).

use turso::params;

use crate::store::types::StoreError;

/// A row reconstructed from `convergence_op_log` for warm-tier loading.
#[derive(Debug, Clone)]
pub struct ConvergenceOpRow {
    pub op_id: u64,
    pub agent_id: u64,
    pub daemon_id: String,
    pub kind_json: String,
    pub description: String,
    pub predecessor_hash: Option<String>,
    pub signature: Option<String>,
    pub signing_key_id: Option<String>,
    pub parent_op_ids_json: String,
    pub produced_at: u64,
    pub change_id: Option<u64>,
    pub model_id: Option<String>,
    pub undone: bool,
}

impl crate::VoxDb {
    /// Insert a row into `convergence_op_log`. Duplicate op_ids are ignored (idempotent).
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_convergence_op_log(
        &self,
        op_id: i64,
        set_id: &str,
        parent_op_ids_json: &str,
        kind_json: &str,
        payload_blake3_hex: &str,
        predecessor_hash: Option<&str>,
        signature: Option<&str>,
        signing_key_id: Option<&str>,
        agent_id: i64,
        daemon_id: &str,
        produced_at: i64,
        description: &str,
        change_id: Option<i64>,
        model_id: Option<&str>,
    ) -> Result<(), StoreError> {
        let set_id = set_id.to_string();
        let parent_op_ids_json = parent_op_ids_json.to_string();
        let kind_json = kind_json.to_string();
        let payload_blake3_hex = payload_blake3_hex.to_string();
        let predecessor_hash = predecessor_hash.map(str::to_string);
        let signature = signature.map(str::to_string);
        let signing_key_id = signing_key_id.map(str::to_string);
        let daemon_id = daemon_id.to_string();
        let description = description.to_string();
        let model_id = model_id.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO convergence_op_log \
                     (op_id, set_id, parent_op_ids, kind_json, payload, payload_blake3, \
                      predecessor_hash, signature, signing_key_id, agent_id, daemon_id, \
                      produced_at, description, change_id, model_id) \
                     VALUES (?,?,?,?,X'',?,?,?,?,?,?,?,?,?,?)",
                    params![
                        op_id,
                        set_id.as_str(),
                        parent_op_ids_json.as_str(),
                        kind_json.as_str(),
                        payload_blake3_hex.as_str(),
                        predecessor_hash.as_deref(),
                        signature.as_deref(),
                        signing_key_id.as_deref(),
                        agent_id,
                        daemon_id.as_str(),
                        produced_at,
                        description.as_str(),
                        change_id,
                        model_id.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Insert a row into `convergence_op_log_backfill_dlq` (best-effort, upserts retry_count).
    pub async fn insert_backfill_dlq(
        &self,
        op_id: i64,
        payload: Vec<u8>,
        parent_op_ids_json: String,
        first_seen_at: i64,
        last_error: String,
    ) -> Result<(), StoreError> {
        let conn = self.conn.clone();
        conn.execute(
            "INSERT INTO convergence_op_log_backfill_dlq \
                (op_id, payload, parent_op_ids, first_seen_at, retry_count, last_error) \
             VALUES (?,?,?,?,0,?) \
             ON CONFLICT(op_id) DO UPDATE SET \
                retry_count = retry_count + 1, \
                last_error  = excluded.last_error",
            turso::params![
                op_id,
                payload,
                parent_op_ids_json.as_str(),
                first_seen_at,
                last_error.as_str(),
            ],
        )
        .await
        .map(|_| ())
        .map_err(StoreError::Turso)
    }

    /// Load the most recent `limit` rows from `convergence_op_log`, ordered newest-first.
    pub async fn load_recent_convergence_op_log(
        &self,
        limit: i64,
    ) -> Result<Vec<ConvergenceOpRow>, StoreError> {
        let conn = self.conn.clone();
        let mut rows = conn
            .query(
                "SELECT op_id, agent_id, daemon_id, kind_json, description, \
                        predecessor_hash, signature, signing_key_id, parent_op_ids, \
                        produced_at, change_id, model_id, undone \
                 FROM convergence_op_log \
                 ORDER BY op_id DESC LIMIT ?",
                params![limit],
            )
            .await
            .map_err(StoreError::Turso)?;

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let op_id: i64 = row.get(0).unwrap_or(0);
            let agent_id: i64 = row.get(1).unwrap_or(0);
            let daemon_id: String = row.get(2).unwrap_or_default();
            let kind_json: String = row.get(3).unwrap_or_default();
            let description: String = row.get(4).unwrap_or_default();
            let predecessor_hash: Option<String> = row.get(5).ok().flatten();
            let signature: Option<String> = row.get(6).ok().flatten();
            let signing_key_id: Option<String> = row.get(7).ok().flatten();
            let parent_op_ids_json: String = row.get(8).unwrap_or_else(|_| "[]".to_string());
            let produced_at: i64 = row.get(9).unwrap_or(0);
            let change_id: Option<i64> = row.get(10).ok().flatten();
            let model_id: Option<String> = row.get(11).ok().flatten();
            let undone: i64 = row.get(12).unwrap_or(0);

            out.push(ConvergenceOpRow {
                op_id: op_id as u64,
                agent_id: agent_id as u64,
                daemon_id,
                kind_json,
                description,
                predecessor_hash,
                signature,
                signing_key_id,
                parent_op_ids_json,
                produced_at: produced_at as u64,
                change_id: change_id.map(|c| c as u64),
                model_id,
                undone: undone != 0,
            });
        }
        Ok(out)
    }
}
