//! Store ops for `scientia_discovery_inbox`.
//!
//! A surfacing index over draft publication manifests: when an automated
//! discovery producer lands a new candidate, it inserts one inbox row so the
//! GUI can show a "new research candidate" alert. The draft manifest remains the
//! source of truth; this table is a regenerable derived index (hence it is in
//! `LEGACY_EXPORT_SKIP_TABLES`). `signal_codes` is stored as a JSON array string.

use crate::VoxDb;
use crate::store::types::StoreError;
use turso::params;

/// One row of the discovery inbox.
///
/// `signal_codes` is parsed from the stored JSON array on read; a malformed
/// value is surfaced as a [`StoreError::Db`] rather than silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryInboxRow {
    pub id: i64,
    pub publication_id: String,
    pub surfaced_at_ms: i64,
    pub intake_tier: String,
    pub signal_codes: Vec<String>,
    pub acknowledged_at_ms: Option<i64>,
}

impl DiscoveryInboxRow {
    /// Build a row from a SELECT in the canonical column order
    /// `(id, publication_id, surfaced_at_ms, intake_tier, signal_codes, acknowledged_at_ms)`.
    fn from_row(row: &turso::Row) -> Result<Self, StoreError> {
        let id: i64 = row.get(0).map_err(StoreError::Turso)?;
        let publication_id: String = row.get(1).map_err(StoreError::Turso)?;
        let surfaced_at_ms: i64 = row.get(2).map_err(StoreError::Turso)?;
        let intake_tier: String = row.get(3).map_err(StoreError::Turso)?;
        let signal_codes_json: String = row.get(4).map_err(StoreError::Turso)?;
        let acknowledged_at_ms: Option<i64> = row.get(5).map_err(StoreError::Turso)?;
        let signal_codes: Vec<String> = serde_json::from_str(&signal_codes_json).map_err(|e| {
            StoreError::Db(format!(
                "scientia_discovery_inbox: malformed signal_codes JSON for id {id}: {e}"
            ))
        })?;
        Ok(Self {
            id,
            publication_id,
            surfaced_at_ms,
            intake_tier,
            signal_codes,
            acknowledged_at_ms,
        })
    }
}

impl VoxDb {
    /// Insert a new (unacknowledged) discovery inbox row; returns its `id`.
    ///
    /// `signal_codes_json` MUST be a JSON array string (e.g. produced by
    /// `serde_json::to_string(&Vec<String>)`); it is read back via `from_row`.
    pub async fn insert_discovery_inbox(
        &self,
        publication_id: &str,
        surfaced_at_ms: i64,
        intake_tier: &str,
        signal_codes_json: &str,
    ) -> Result<i64, StoreError> {
        let pid = publication_id.to_string();
        let tier = intake_tier.to_string();
        let codes = signal_codes_json.to_string();
        self.conn
            .execute(
                "INSERT INTO scientia_discovery_inbox \
                 (publication_id, surfaced_at_ms, intake_tier, signal_codes) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![pid, surfaced_at_ms, tier, codes],
            )
            .await
            .map_err(StoreError::Turso)?;

        let mut rows = self
            .conn
            .query("SELECT last_insert_rowid()", ())
            .await
            .map_err(StoreError::Turso)?;
        let id: i64 = rows
            .next()
            .await
            .map_err(StoreError::Turso)?
            .ok_or_else(|| {
                StoreError::Db(
                    "scientia_discovery_inbox: last_insert_rowid() returned no row".into(),
                )
            })?
            .get(0)
            .map_err(StoreError::Turso)?;
        Ok(id)
    }

    /// List unacknowledged discoveries, newest first, capped at `limit`.
    pub async fn list_unacknowledged_discoveries(
        &self,
        limit: i64,
    ) -> Result<Vec<DiscoveryInboxRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, publication_id, surfaced_at_ms, intake_tier, signal_codes, acknowledged_at_ms \
                 FROM scientia_discovery_inbox \
                 WHERE acknowledged_at_ms IS NULL \
                 ORDER BY id DESC LIMIT ?1",
                params![limit],
            )
            .await
            .map_err(StoreError::Turso)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            out.push(DiscoveryInboxRow::from_row(&row)?);
        }
        Ok(out)
    }

    /// Mark a discovery row acknowledged. No-op (0 rows) if the id is unknown.
    pub async fn acknowledge_discovery(
        &self,
        id: i64,
        acknowledged_at_ms: i64,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE scientia_discovery_inbox SET acknowledged_at_ms = ?2 WHERE id = ?1",
                params![id, acknowledged_at_ms],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }

    /// Return `MAX(id)` from the inbox, or `0` when empty (WS poller diff anchor).
    pub async fn max_discovery_inbox_id(&self) -> Result<i64, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT COALESCE(MAX(id), 0) FROM scientia_discovery_inbox",
                (),
            )
            .await
            .map_err(StoreError::Turso)?;
        let max: i64 = rows
            .next()
            .await
            .map_err(StoreError::Turso)?
            .ok_or_else(|| {
                StoreError::Db("scientia_discovery_inbox: MAX(id) returned no row".into())
            })?
            .get(0)
            .map_err(StoreError::Turso)?;
        Ok(max)
    }

    /// Return rows with `id > after_id`, oldest first, capped at `limit` (WS
    /// poller payload — broadcasts only the genuinely-new rows).
    pub async fn discoveries_since(
        &self,
        after_id: i64,
        limit: i64,
    ) -> Result<Vec<DiscoveryInboxRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, publication_id, surfaced_at_ms, intake_tier, signal_codes, acknowledged_at_ms \
                 FROM scientia_discovery_inbox \
                 WHERE id > ?1 \
                 ORDER BY id ASC LIMIT ?2",
                params![after_id, limit],
            )
            .await
            .map_err(StoreError::Turso)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            out.push(DiscoveryInboxRow::from_row(&row)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn insert_then_list_shows_unacknowledged() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let id = db
            .insert_discovery_inbox(
                "commit-abc123",
                1_000,
                "review_suggested",
                r#"["perf_claim"]"#,
            )
            .await
            .expect("insert");
        assert!(id >= 1, "insert must return a positive rowid");

        let rows = db.list_unacknowledged_discoveries(10).await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].publication_id, "commit-abc123");
        assert_eq!(rows[0].intake_tier, "review_suggested");
        assert_eq!(rows[0].signal_codes, vec!["perf_claim".to_string()]);
        assert_eq!(rows[0].acknowledged_at_ms, None);
    }

    #[tokio::test]
    async fn acknowledge_removes_from_unacknowledged_list() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let id = db
            .insert_discovery_inbox("commit-xyz", 2_000, "auto_intake", "[]")
            .await
            .expect("insert");

        db.acknowledge_discovery(id, 3_000).await.expect("ack");

        let rows = db.list_unacknowledged_discoveries(10).await.expect("list");
        assert!(
            rows.is_empty(),
            "acknowledged rows must not appear in the unacknowledged list"
        );
    }

    #[tokio::test]
    async fn discoveries_since_and_max_id_track_inserts() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        assert_eq!(db.max_discovery_inbox_id().await.expect("max empty"), 0);

        let id1 = db
            .insert_discovery_inbox("commit-1", 10, "review_suggested", "[]")
            .await
            .expect("insert 1");
        let id2 = db
            .insert_discovery_inbox("commit-2", 20, "review_suggested", "[]")
            .await
            .expect("insert 2");

        assert_eq!(db.max_discovery_inbox_id().await.expect("max"), id2);

        // since(0) returns both, oldest first.
        let all = db.discoveries_since(0, 64).await.expect("since 0");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, id1);
        assert_eq!(all[1].id, id2);

        // since(id1) returns only the newer row.
        let newer = db.discoveries_since(id1, 64).await.expect("since id1");
        assert_eq!(newer.len(), 1);
        assert_eq!(newer[0].id, id2);
    }
}
