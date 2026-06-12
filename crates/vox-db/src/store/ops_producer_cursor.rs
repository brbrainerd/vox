//! Store ops for `scientia_producer_cursor`.
//!
//! A single durable row per automated discovery producer recording the last
//! position it scanned (`last_seen`, producer-defined — e.g. a commit sha).
//! Rows are upserted with INSERT OR REPLACE so the latest write wins. The
//! cursor is advanced by the producer ONLY after a batch's draft inserts
//! succeed, so a crash mid-batch re-scans rather than skipping work.

use crate::VoxDb;
use crate::store::types::StoreError;
use turso::params;

impl VoxDb {
    /// Return the `last_seen` cursor for `producer`, or `None` if never set.
    pub async fn get_producer_cursor(&self, producer: &str) -> Result<Option<String>, StoreError> {
        let p = producer.to_string();
        let mut rows = self
            .conn
            .query(
                "SELECT last_seen FROM scientia_producer_cursor WHERE producer = ?1",
                params![p],
            )
            .await
            .map_err(StoreError::Turso)?;

        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let last_seen: String = row.get(0).map_err(StoreError::Turso)?;
            Ok(Some(last_seen))
        } else {
            Ok(None)
        }
    }

    /// Upsert the `last_seen` cursor for `producer` (latest write wins).
    pub async fn set_producer_cursor(
        &self,
        producer: &str,
        last_seen: &str,
    ) -> Result<(), StoreError> {
        let p = producer.to_string();
        let ls = last_seen.to_string();
        let now = crate::now_unix_ms() as i64;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO scientia_producer_cursor \
                 (producer, last_seen, updated_at_ms) VALUES (?1, ?2, ?3)",
                params![p, ls, now],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn missing_cursor_returns_none() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let got = db.get_producer_cursor("commit_watcher").await.expect("get");
        assert!(got.is_none(), "unset producer must return None");
    }

    #[tokio::test]
    async fn set_then_get_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        db.set_producer_cursor("commit_watcher", "deadbeef")
            .await
            .expect("set");
        let got = db
            .get_producer_cursor("commit_watcher")
            .await
            .expect("get")
            .expect("row present");
        assert_eq!(got, "deadbeef");

        // Upsert replaces.
        db.set_producer_cursor("commit_watcher", "cafef00d")
            .await
            .expect("set2");
        let got2 = db
            .get_producer_cursor("commit_watcher")
            .await
            .expect("get2")
            .expect("row present");
        assert_eq!(got2, "cafef00d", "upsert must replace previous cursor");
    }
}
