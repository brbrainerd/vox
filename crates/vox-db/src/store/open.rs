//! Connection helpers and built-in schema migration.
//!
//! # Turso `execute_batch` constraint
//!
//! [`turso::Connection::execute_batch`] executes each statement with `execute`, which errors if a
//! statement returns rows. Assignment `PRAGMA` values are applied with `pragma_update` in
//! [`VoxDb::apply_pragmas`]. Migration bodies must be DDL/DML only; empty strings in a fragment
//! are skipped.

use crate::schema::{BASELINE_VERSION, baseline_sql};
use crate::store::types::StoreError;

impl crate::VoxDb {
    pub(crate) async fn apply_pragmas(conn: &turso::Connection) -> Result<(), StoreError> {
        // `execute_batch` runs each statement via `execute()`, which errors on any row-producing
        // statement. Assignment-style `PRAGMA x = y` returns the new value as a row; use
        // `pragma_update` so those rows are consumed (see turso `Connection::prepare_execute_batch`).
        let _ = conn.pragma_update("journal_mode", "WAL").await?;
        let _ = conn.pragma_update("busy_timeout", 5000).await?;
        // Do not set `wal_autocheckpoint` via `pragma_update`: libsql rejects assignment forms for
        // this pragma (parse → unsupported). SQLite default is 1000 pages, which matches intent.
        let _ = conn.pragma_update("synchronous", "NORMAL").await?;
        let _ = conn.pragma_update("foreign_keys", "ON").await?;
        let _ = conn.pragma_update("cache_size", -65536).await?;
        // `temp_store` / `mmap_size` are valid in stock SQLite but not in Turso/libSQL's supported
        // `PragmaName` set (`turso_parser`); `pragma_update` then fails with "Not a valid pragma name".

        if std::env::var("VOX_DB_MVCC")
            .map(|s| s == "1")
            .unwrap_or(false)
        {
            tracing::warn!("BETA: MVCC concurrent writes enabled for Turso.");
            let _ = conn.pragma_update("journal_mode", "mvcc").await?;
        }

        let mut rows = conn.query("PRAGMA journal_mode", ()).await?;
        if let Some(row) = rows.next().await? {
            let mode: String = row.get(0).unwrap_or_default();
            if mode.to_lowercase() != "wal"
                && mode.to_lowercase() != "mvcc"
                && mode.to_lowercase() != "memory"
            {
                tracing::warn!(mode = %mode, "Database connection did not apply WAL/MVCC journal_mode (likely remote Turso)");
            }
        }

        Ok(())
    }

    /// Drop every user-defined table (not `sqlite_%`). Caller usually follows with `Self::migrate`.
    #[cfg(feature = "local")]
    pub async fn drop_all_user_tables(conn: &turso::Connection) -> Result<(), StoreError> {
        conn.execute("PRAGMA foreign_keys = OFF", ())
            .await
            .map_err(StoreError::from)?;
        let mut rows = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                (),
            )
            .await?;
        let mut names = Vec::new();
        while let Some(row) = rows.next().await? {
            names.push(row.get::<String>(0)?);
        }
        for name in names {
            let quoted = sqlite_quote_ident(&name);
            conn.execute(&format!("DROP TABLE IF EXISTS {quoted}"), ())
                .await
                .map_err(StoreError::from)?;
        }
        conn.execute("PRAGMA foreign_keys = ON", ())
            .await
            .map_err(StoreError::from)?;
        Ok(())
    }

    /// Open a local file, wipe all user tables, and apply baseline + cutover (same as fresh migrate).
    #[cfg(feature = "local")]
    pub async fn open_local_reset_to_baseline(path: &str) -> Result<Self, StoreError> {
        let db = turso::Builder::new_local(path).build().await?;
        let conn = db.connect()?;
        Self::apply_pragmas(&conn).await?;
        Self::drop_all_user_tables(&conn).await?;
        Self::migrate(&conn).await?;
        Ok(Self {
            conn,
            sync_db: None,
            writer: None,
            breaker: std::sync::Arc::new(crate::DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    /// Apply the manifest **baseline** DDL and advance `schema_version` to [`BASELINE_VERSION`].
    ///
    /// DDL in each fragment is run through Turso `execute_batch`. Existing databases with
    /// version ≤ [`BASELINE_VERSION`] are automatically advanced (DDL is idempotent).
    pub(crate) async fn migrate(conn: &turso::Connection) -> Result<(), StoreError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .await?;

        let mut rows = conn
            .query("SELECT COALESCE(MAX(version), 0) FROM schema_version", ())
            .await?;
        let current_version: i64 = if let Some(row) = rows.next().await? {
            row.get(0)?
        } else {
            0
        };

        // Single maintained baseline: only `BASELINE_VERSION` (or 0 pre-bootstrap) is valid.
        // `MAX(schema_version)` from ad-hoc `apply_migrations` rows also trips this guard.
        if current_version > BASELINE_VERSION {
            tracing::warn!(
                target: "vox_db::legacy_schema",
                schema_max = current_version,
                baseline = BASELINE_VERSION,
                "database schema is newer than this binary; upgrade vox for compatibility."
            );
            return Err(StoreError::LegacySchemaChain {
                max_version: current_version,
            });
        }

        if current_version < BASELINE_VERSION {
            let sql = baseline_sql().trim();
            if !sql.is_empty() {
                conn.execute_batch(sql).await?;
            }

            // v51 Migration: Flatten legacy reliability tables into the consolidated SSOT
            if current_version > 0 && current_version < 51 {
                let _ = conn.execute_batch(r#"
                    INSERT OR REPLACE INTO reliability_scores (entity_type, entity_id, reliability, success_count, failure_count, updated_at_ms)
                    SELECT 'agent', agent_id, reliability, success_count, failure_count, updated_at_ms FROM agent_reliability;
                    
                    INSERT OR REPLACE INTO reliability_scores (entity_type, entity_id, reliability, success_count, failure_count, updated_at_ms)
                    SELECT 'skill', skill_id, reliability, success_count, failure_count, updated_at_ms FROM skill_reliability;
                    
                    INSERT OR REPLACE INTO reliability_scores (entity_type, entity_id, reliability, success_count, failure_count, updated_at_ms)
                    SELECT 'workflow', workflow_id, reliability, success_count, failure_count, updated_at_ms FROM workflow_reliability;
                    
                    INSERT OR REPLACE INTO reliability_scores (entity_type, entity_id, reliability, success_count, failure_count, updated_at_ms)
                    SELECT 'repository', repository_id, reliability, success_count, failure_count, updated_at_ms FROM repository_reliability;
                "#).await;
            }

            crate::schema_extensions::apply_schema_extensions(conn).await?;

            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)
                 ON CONFLICT(version) DO UPDATE SET applied_at = datetime('now')",
                (BASELINE_VERSION,),
            )
            .await?;
        }

        Ok(())
    }

    /// Open local file **without** running `migrate` (export tools only — reads legacy multi-version DBs).
    #[cfg(feature = "local")]
    pub async fn open_local_legacy_export(path: &str) -> Result<Self, StoreError> {
        let db = turso::Builder::new_local(path).build().await?;
        let conn = db.connect()?;
        Self::apply_pragmas(&conn).await?;
        Ok(Self {
            conn,
            sync_db: None,
            writer: None,
            breaker: std::sync::Arc::new(crate::DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    /// Open or create a local database file (requires `local` feature on `vox-db`).
    #[cfg(feature = "local")]
    pub async fn open(path: &str) -> Result<Self, StoreError> {
        let db = turso::Builder::new_local(path).build().await?;
        let conn = db.connect()?;
        Self::apply_pragmas(&conn).await?;
        Self::migrate(&conn).await?;
        Ok(Self {
            conn,
            sync_db: None,
            writer: None,
            breaker: std::sync::Arc::new(crate::DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    /// In-memory store (requires `local` feature on `vox-db`).
    #[cfg(feature = "local")]
    pub async fn open_memory() -> Result<Self, StoreError> {
        Self::open(":memory:").await
    }

    /// Blocking constructor for in-memory store (tests and `std::thread` call sites).
    #[cfg(feature = "local")]
    pub fn new_memory() -> Result<Self, StoreError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| StoreError::Db(format!("VoxDb::new_memory tokio runtime: {e}")))?
            .block_on(Self::open_memory())
    }

    /// Remote Turso / libSQL (sync client).
    pub async fn open_remote(url: &str, auth_token: &str) -> Result<Self, StoreError> {
        let db = turso::sync::Builder::new_remote(":memory:")
            .with_remote_url(url)
            .with_auth_token(auth_token)
            .build()
            .await?;
        let conn = db.connect().await?;
        Self::apply_pragmas(&conn).await?;
        Self::migrate(&conn).await?;
        Ok(Self {
            conn,
            sync_db: Some(db),
            writer: None,
            breaker: std::sync::Arc::new(crate::DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    /// Embedded replica: local path with remote sync.
    #[cfg(feature = "replication")]
    pub async fn open_embedded_replica(
        local_path: &str,
        url: &str,
        auth_token: &str,
    ) -> Result<Self, StoreError> {
        let db = turso::sync::Builder::new_remote(local_path)
            .with_remote_url(url)
            .with_auth_token(auth_token)
            .build()
            .await?;
        let conn = db.connect().await?;
        Self::apply_pragmas(&conn).await?;
        Self::migrate(&conn).await?;
        Ok(Self {
            conn,
            sync_db: Some(db),
            writer: None,
            breaker: std::sync::Arc::new(crate::DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    /// Default local project store at [`super::DEFAULT_PROJECT_STORE_PATH`].
    #[cfg(feature = "local")]
    pub async fn open_default() -> Result<Self, StoreError> {
        let path = super::DEFAULT_PROJECT_STORE_PATH;
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::Db(e.to_string()))?;
        }
        Self::open(path).await
    }

    /// Pull remote changes when using a sync-backed store; no-op for pure local files.
    pub async fn sync(&self) -> Result<(), StoreError> {
        if let Some(db) = &self.sync_db {
            let _ = db.pull().await?;
        }
        Ok(())
    }

    /// Explicit read boundary for replica-backed handles (see [`crate::ReadConsistency`]).
    pub async fn sync_for(&self, consistency: crate::ReadConsistency) -> Result<(), StoreError> {
        match consistency {
            crate::ReadConsistency::Local => Ok(()),
            crate::ReadConsistency::ReplicaLatest => self.sync().await,
        }
    }
}

fn sqlite_quote_ident(name: &str) -> String {
    let mut s = String::with_capacity(name.len() + 2);
    s.push('"');
    for c in name.chars() {
        if c == '"' {
            s.push_str("\"\"");
        } else {
            s.push(c);
        }
    }
    s.push('"');
    s
}

#[cfg(all(test, feature = "local"))]
mod tests {
    use crate::{ReadConsistency, VoxDb};
    use turso::Builder;

    #[tokio::test]
    async fn sync_for_local_skips_pull() {
        let db = VoxDb::open_memory().await.expect("open");
        db.sync_for(ReadConsistency::Local).await.expect("sync");
    }

    #[tokio::test]
    async fn fresh_db_schema_version_is_baseline_latest() {
        let db = Builder::new_local(":memory:").build().await.expect("mem");
        let conn = db.connect().expect("conn");
        VoxDb::apply_pragmas(&conn).await.expect("pragmas");
        VoxDb::migrate(&conn).await.expect("migrate");
        let mut rows = conn
            .query("SELECT MAX(version) FROM schema_version", ())
            .await
            .expect("q");
        let v: i64 = rows
            .next()
            .await
            .expect("next")
            .expect("row")
            .get(0)
            .expect("v");
        assert_eq!(v, crate::schema::BASELINE_VERSION);
    }
}
