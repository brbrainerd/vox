//! Actor KV state persistence through the Codex pipeline.

use crate::store::types::StoreError;
use serde::{Deserialize, Serialize};

impl crate::VoxDb {
    /// Save an actor state value (JSON-serialized) under a key.
    pub async fn save_actor_state_generic<T: Serialize>(
        &self,
        key: &str,
        value: &T,
    ) -> Result<(), StoreError> {
        let data = serde_json::to_string(value).map_err(|e| StoreError::Db(e.to_string()))?;
        let key = key.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO actor_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
                    turso::params![key.as_str(), data],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Load an actor state value by key.
    pub async fn load_actor_state_generic<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Result<Option<T>, StoreError> {
        let mut rows: crate::GuardedRows = self
            .connection()
            .query(
                "SELECT value FROM actor_state WHERE key = ?1",
                turso::params![key],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let data: String = row.get(0)?;
            let parsed = serde_json::from_str(&data).map_err(|e| StoreError::Db(e.to_string()))?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }
}
