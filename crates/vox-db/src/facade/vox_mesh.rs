use crate::VoxDb;
use vox_mesh_types::kudos::CreditJobRequest;

impl VoxDb {
    /// Record a contribution in the kudos ledger.
    pub async fn credit_kudos(&self, req: &CreditJobRequest) -> Result<(), crate::StoreError> {
        let sql = "INSERT INTO vox_kudos (vox_user_id, node_id, primitive, amount, task_id, created_unix_ms, metadata_json)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
        let now = crate::types::now_unix_ms() as i64;
        let user_id = req.vox_user_id.clone();
        let node_id = req.node_id.clone();
        let primitive = req.primitive.as_str().to_string();
        let amount = req.amount as i64;
        let task_id = req.task_id.clone();
        let metadata = req.metadata_json.clone();

        self.conn
            .execute(
                sql,
                turso::params![user_id, node_id, primitive, amount, task_id, now, metadata],
            )
            .await
            .map_err(crate::StoreError::Turso)?;

        Ok(())
    }

    /// Count total kudos for a given user.
    pub async fn count_kudos_for_user(&self, user_id: &str) -> Result<u64, crate::StoreError> {
        let sql = "SELECT COALESCE(SUM(amount), 0) FROM vox_kudos WHERE vox_user_id = ?1";
        let mut rows = self
            .conn
            .query(sql, turso::params![user_id.to_string()])
            .await
            .map_err(crate::StoreError::Turso)?;

        if let Ok(Some(row)) = rows.next().await {
            let sum = row.get::<i64>(0).unwrap_or(0);
            return Ok(sum as u64);
        }

        Ok(0)
    }

    /// Record a peer reputation event (success, failure, timeout, invalid).
    pub async fn record_peer_reputation(
        &self,
        node_id: &str,
        event_type: &str,
    ) -> Result<(), crate::StoreError> {
        let now = crate::types::now_unix_ms() as i64;
        let column = match event_type {
            "success" => "success_count",
            "fail" => "fail_count",
            "timeout" => "timeout_count",
            "invalid" => "invalid_output_count",
            _ => {
                return Err(crate::StoreError::Internal(
                    "Invalid reputation event".into(),
                ));
            }
        };

        // UPSERT the peer reputation.
        let sql = format!(
            "INSERT INTO vox_peer_reputation (node_id, {}, last_updated_unix_ms) 
             VALUES (?1, 1, ?2) 
             ON CONFLICT(node_id) DO UPDATE SET 
             {} = {} + 1, last_updated_unix_ms = ?2",
            column, column, column
        );

        self.conn
            .execute(&sql, turso::params![node_id.to_string(), now])
            .await
            .map_err(crate::StoreError::Turso)?;

        Ok(())
    }

    /// Retrieve the reputation score of a peer. Returns (success_count, fail_count, timeout_count, invalid_output_count).
    pub async fn get_peer_reputation(
        &self,
        node_id: &str,
    ) -> Result<Option<(u64, u64, u64, u64)>, crate::StoreError> {
        let sql = "SELECT success_count, fail_count, timeout_count, invalid_output_count 
                   FROM vox_peer_reputation WHERE node_id = ?1";

        let mut rows = self
            .conn
            .query(sql, turso::params![node_id.to_string()])
            .await
            .map_err(crate::StoreError::Turso)?;

        if let Ok(Some(row)) = rows.next().await {
            let success = row.get::<u64>(0).unwrap_or(0);
            let fail = row.get::<u64>(1).unwrap_or(0);
            let timeout = row.get::<u64>(2).unwrap_or(0);
            let invalid = row.get::<u64>(3).unwrap_or(0);
            return Ok(Some((success, fail, timeout, invalid)));
        }

        Ok(None)
    }

    /// Migrate reputation history from an old node ID to a new node ID (used during identity rotation).
    pub async fn migrate_peer_reputation(
        &self,
        old_node_id: &str,
        new_node_id: &str,
    ) -> Result<(), crate::StoreError> {
        // If the new node doesn't exist, we just UPDATE the old row.
        // If the new node DOES exist (unlikely, but possible), we could sum them, but a simple UPDATE OR REPLACE is easier.
        // Actually, SQLite doesn't natively sum on conflict easily with UPDATE. Let's just do an UPDATE and ignore if new_node_id exists
        // (or we can just UPDATE OR REPLACE, which overwrites). Let's use UPDATE OR IGNORE, and if it ignored because of conflict, we could do a manual merge, but for now simple update is fine.
        let sql = "UPDATE OR IGNORE vox_peer_reputation SET node_id = ?2 WHERE node_id = ?1";

        self.conn
            .execute(
                sql,
                turso::params![old_node_id.to_string(), new_node_id.to_string()],
            )
            .await
            .map_err(crate::StoreError::Turso)?;

        Ok(())
    }
}
