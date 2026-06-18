use turso::params;
use crate::store::types::StoreError;

impl crate::VoxDb {
    /// Delete `memories` rows for `agent_id` with `created_at` strictly before `created_before`
    /// (ISO-like timestamp string compared as SQLite TEXT).
    pub async fn delete_memories_created_before(
        &self,
        agent_id: &str,
        created_before: &str,
    ) -> Result<u64, StoreError> {
        let agent_id = agent_id.to_string();
        let created_before = created_before.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let n = conn
                    .execute(
                        "DELETE FROM memories WHERE agent_id = ?1 AND created_at < ?2",
                        params![agent_id.as_str(), created_before.as_str()],
                    )
                    .await?;
                Ok::<_, StoreError>(n)
            })
            .await
    }
}
