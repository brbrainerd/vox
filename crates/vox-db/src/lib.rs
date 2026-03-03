//! # vox-db — High-level database facade for Vox
//!
//! Provides a unified `VoxDb` interface that wraps `vox_pm::CodeStore` and
//! supports multiple connection modes:
//!
//! - **Remote** (Turso cloud) — always available
//! - **Local** (file-based Turso) — requires `local` feature
//! - **Embedded replica** (local + cloud sync) — requires `replication` feature
//!
//! ```no_run
//! use vox_db::{VoxDb, DbConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let db = VoxDb::connect(DbConfig::Remote {
//!         url: "turso://my-db.turso.io".to_string(),
//!         token: "my-token".to_string(),
//!     }).await?;
//!
//!     let hash = db.store().store("fn", b"fn hello(): ret 42").await?;
//!     println!("Stored: {hash}");
//!     Ok(())
//! }
//! ```

pub mod auto_migrate;
pub mod collection;
mod config;
pub mod data_flow;
pub mod ddl;
pub mod error_enrichment;
pub mod learning;
pub mod migration;
pub mod paths;
pub mod retrieval;
pub mod schema_digest;
use crate::paths::local_user_id;

pub use auto_migrate::AutoMigrator;
pub use collection::Collection;
pub use config::DbConfig;
pub use data_flow::{build_data_flow, DataFlowMap};
pub use ddl::{diff_schemas, table_to_ddl, tables_to_ddl, SchemaDiff};
pub use error_enrichment::{enrich_error, EnrichedDbError};
pub use migration::{builtin_migrations, validate_migrations, Migration};
pub use retrieval::{fuse_hybrid_results, RetrievalMode, RetrievalQuery, RetrievalResult};
pub use schema_digest::{digest_to_json, format_llm_context, generate_schema_digest, SchemaDigest};
pub use vox_pm::store::{
    // V6 entry types
    AgentDefEntry,
    ArtifactEntry,
    // V9 Populi Foundry
    BuilderSessionEntry,
    // Core types
    CodeStore,
    ComponentEntry,
    EmbeddingEntry,
    // V1/V2 entry types
    ExecutionEntry,
    LogExecutionParams,
    MemoryEntry,
    ReviewEntry,
    ScheduledEntry,
    // V10 session turns
    SessionTurnEntry,
    // V12 skill marketplace
    SkillManifestEntry,
    SnippetEntry,
    StoreError,
    TrainingPair,
    TypedStreamEventEntry,
    UserEntry,
};

/// High-level database facade for the Vox ecosystem.
///
/// Wraps `CodeStore` and provides convenience methods for common operations.
pub struct VoxDb {
    store: CodeStore,
}

/// Default maximum number of connection retry attempts.
const DEFAULT_MAX_RETRIES: u64 = 3;
/// Default base delay between retries in milliseconds.
const DEFAULT_RETRY_BASE_MS: u64 = 500;

impl VoxDb {
    /// Connect to a database using the given configuration, with retry logic.
    pub async fn connect(config: DbConfig) -> Result<Self, StoreError> {
        Self::connect_with_retries(config, DEFAULT_MAX_RETRIES, DEFAULT_RETRY_BASE_MS).await
    }

    /// Connect using the platform-aware default local path.
    ///
    /// Uses `paths::default_db_path()` to determine the DB file location.
    /// Falls back to `DbConfig::from_env()` if the platform path cannot be
    /// determined.
    #[cfg(feature = "local")]
    pub async fn connect_default() -> Result<Self, StoreError> {
        let config = if let Some(path) = paths::default_db_path() {
            DbConfig::Local {
                path: path.to_string_lossy().to_string(),
            }
        } else {
            DbConfig::from_env().map_err(StoreError::NotFound)?
        };
        Self::connect(config).await
    }

    /// Connect with configurable retry parameters.
    pub async fn connect_with_retries(
        config: DbConfig,
        max_retries: u64,
        retry_base_ms: u64,
    ) -> Result<Self, StoreError> {
        let mut attempts = 0u64;
        let store = loop {
            attempts += 1;
            let result = match &config {
                DbConfig::Remote { url, token } => CodeStore::open_remote(url, token).await,
                #[cfg(feature = "local")]
                DbConfig::Local { path } => CodeStore::open(path).await,
                #[cfg(feature = "local")]
                DbConfig::Memory => CodeStore::open_memory().await,
                #[cfg(feature = "replication")]
                DbConfig::EmbeddedReplica {
                    local_path,
                    url,
                    token,
                } => CodeStore::open_embedded_replica(local_path, url, token).await,
            };

            match result {
                Ok(store) => break store,
                Err(e) if attempts < max_retries => {
                    eprintln!(
                        "Failed to connect to VoxDB, retrying ({}/{})... Error: {}",
                        attempts, max_retries, e
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(retry_base_ms * attempts))
                        .await;
                }
                Err(e) => return Err(e),
            }
        };
        Ok(Self { store })
    }

    /// Access the underlying `CodeStore` for all CRUD operations.
    pub fn store(&self) -> &CodeStore {
        &self.store
    }

    /// Sync the database schema using an auto-migration plan based on the given digest.
    pub async fn sync_schema_from_digest(&self, digest: &SchemaDigest) -> Result<(), StoreError> {
        let migrator = AutoMigrator::new(&self.store.conn);
        migrator.sync_from_digest(digest).await?;
        Ok(())
    }


    /// Return the platform-specific data directory (if resolvable).
    pub fn data_dir() -> Option<std::path::PathBuf> {
        paths::data_dir()
    }
    /// Sync the embedded replica with the remote primary (no-op for other modes).
    pub async fn sync(&self) -> Result<(), StoreError> {
        self.store.sync().await
    }

    // ── Collection & Schema Methods ─────────────────────

    /// Get a handle to a schemaless document collection.
    ///
    /// The collection stores JSON documents in a SQLite table with `json_extract`
    /// based querying. Call `ensure_table()` on the returned handle to create the
    /// backing table if it doesn't exist.
    pub fn collection(&self, name: impl Into<String>) -> collection::Collection<'_> {
        collection::Collection::new(name, &self.store.conn)
    }

    /// Create an auto-migrator for schema synchronization.
    ///
    /// Use this to introspect the live database schema and diff it against your
    /// desired `@table` declarations, then apply non-destructive migrations.
    pub fn auto_migrator(&self) -> auto_migrate::AutoMigrator<'_> {
        auto_migrate::AutoMigrator::new(&self.store.conn)
    }

    /// Automatically sync the database schema derived from AST declarations.
    pub async fn sync_schema_ast(
        &self,
        tables: &[&vox_ast::decl::TableDecl],
        collections: &[&vox_ast::decl::CollectionDecl],
        indexes: &[&vox_ast::decl::IndexDecl],
    ) -> Result<auto_migrate::MigrationPlan, StoreError> {
        let plan = self
            .auto_migrator()
            .sync_schema(tables, collections, indexes)
            .await?;
        Ok(plan)
    }

    // ── Memory Convenience Methods ──────────────────────

    pub async fn store_memory(
        &self,
        agent_id: &str,
        session_id: &str,
        memory_type: &str,
        content: &str,
        metadata: Option<&str>,
        importance: f64,
    ) -> Result<i64, StoreError> {
        self.store
            .save_memory(
                agent_id,
                session_id,
                memory_type,
                content,
                metadata,
                importance,
                None, // vcs_snapshot_id
            )
            .await
    }

    pub async fn recall_memory(
        &self,
        agent_id: &str,
        memory_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<MemoryEntry>, StoreError> {
        self.store
            .recall_memory(agent_id, memory_type, limit, None)
            .await
    }

    pub async fn search_memories(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        self.store.query_knowledge_nodes(query, limit).await
    }

    pub async fn search_embeddings(
        &self,
        vector: &[f32],
        source_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<(EmbeddingEntry, f32)>, StoreError> {
        self.store.search_similar_embeddings(vector, source_type, limit).await
    }

    /// Return a behavioral learner for this database.
    pub fn learner(&self) -> learning::BehavioralLearner<'_> {
        learning::BehavioralLearner::new(&self.store)
    }

    /// Get the current schema version.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        self.store.schema_version().await
    }

    /// Apply ordered migrations that have not yet been executed.
    ///
    /// Returns versions that were newly applied.
    pub async fn apply_migrations(&self, migrations: &[Migration]) -> Result<Vec<i64>, StoreError> {
        validate_migrations(migrations)?;
        self.store
            .conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_version (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                );",
            )
            .await?;

        let current = self.schema_version().await?;
        let mut applied = Vec::new();
        for migration in migrations {
            if migration.version <= current {
                continue;
            }
            self.store.conn.execute_batch(&migration.up_sql).await?;
            self.store
                .conn
                .execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    (migration.version,),
                )
                .await?;
            applied.push(migration.version);
        }
        Ok(applied)
    }

    /// Record an eval run for regression tracking (V11).
    pub async fn record_eval_run(
        &self,
        run_id: &str,
        model_path: Option<&str>,
        format_validity: Option<f64>,
        safety_rejection_rate: Option<f64>,
        quality_proxy: Option<f64>,
        metadata_json: Option<&str>,
    ) -> Result<i64, StoreError> {
        self.store
            .record_eval_run(
                run_id,
                model_path,
                format_validity,
                safety_rejection_rate,
                quality_proxy,
                metadata_json,
            )
            .await
    }

    /// Add a closure within a database transaction.
    /// Note: Currently, we simulate a transaction via BEGIN/COMMIT/ROLLBACK on the single connection.
    pub async fn transaction<F, T>(&self, f: F) -> Result<T, StoreError>
    where
        F: std::future::Future<Output = Result<T, StoreError>>,
    {
        self.store.conn.execute("BEGIN", ()).await?;
        match f.await {
            Ok(val) => {
                self.store.conn.execute("COMMIT", ()).await?;
                Ok(val)
            }
            Err(e) => {
                self.store.conn.execute("ROLLBACK", ()).await.ok();
                Err(e)
            }
        }
    }

    /// Register the current local directory as a known Vox project in the global database.
    /// This stores the absolute path and package name as an 'artifact' of type 'project'.
    pub async fn register_local_project(&self, name: &str, path: &std::path::Path) -> Result<(), StoreError> {
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = abs_path.to_string_lossy();

        self.store.register_component(
            name,
            "local", // namespace for local projects
            None,   // schema_hash not needed for projects
            Some(&format!("Local project at {}", path_str)),
            "1.0.0"
        ).await?;

        // Also store the path in user_preferences as a 'known_project'
        let _ = self.store.conn.execute(
            "INSERT OR REPLACE INTO user_preferences (user_id, key, value) VALUES (?1, ?2, ?3)",
            (local_user_id(), format!("project.{}.path", name), path_str.to_string()),
        ).await;

        Ok(())
    }
}

#[cfg(all(test, feature = "local"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_memory() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("Failed to connect to memory DB");
        let hash = db
            .store()
            .store("test_kind", b"test_data")
            .await
            .expect("Store failed");
        assert!(!hash.is_empty());
    }
}
