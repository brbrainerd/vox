use crate::store::types::StoreError;
use crate::{DbCircuitBreaker, DbConfig, VoxDb};
use std::sync::Arc;

/// Underlying database factory for parallel execution.
pub enum DbBackend {
    #[cfg(feature = "local")]
    Local(Arc<turso::Database>),
    Sync(Arc<turso::sync::Database>),
}

/// A lightweight pool/factory for [`VoxDb`] connections.
///
/// `VoxDbPool` holds the core libSQL `Database` engine and can vend multiple independent
/// `Connection` handles for parallel task execution. This avoids blocking connections
/// in highly concurrent systems (like the orchestrator MCP loops).
#[derive(Clone)]
pub struct VoxDbPool {
    backend: Arc<DbBackend>,
    breaker: Arc<DbCircuitBreaker>,
    // Shared sqlite probe cache across all handles from this pool.
    sqlite_probe_cache: Arc<tokio::sync::RwLock<Option<crate::capabilities::SqliteProbeSnapshot>>>,
    writer: Arc<tokio::sync::OnceCell<crate::VoxWriteHandle>>,
}

impl VoxDbPool {
    /// Create a new pool using the project's canonical store configuration.
    #[cfg(all(feature = "local", feature = "host-integration"))]
    pub async fn new_canonical() -> Result<Self, StoreError> {
        let config = DbConfig::resolve_canonical().map_err(StoreError::NotFound)?;
        Self::new(config).await
    }

    /// Instantiate the database pool from the specified config.
    /// This runs migrations automatically on the first connection established.
    pub async fn new(config: DbConfig) -> Result<Self, StoreError> {
        let backend = match &config {
            #[cfg(feature = "local")]
            DbConfig::Local { path } => {
                let db = turso::Builder::new_local(path)
                    .build()
                    .await
                    .map_err(StoreError::from)?;
                Arc::new(DbBackend::Local(Arc::new(db)))
            }
            #[cfg(feature = "local")]
            DbConfig::Memory => {
                let db = turso::Builder::new_local(":memory:")
                    .build()
                    .await
                    .map_err(StoreError::from)?;
                Arc::new(DbBackend::Local(Arc::new(db)))
            }
            DbConfig::Remote { url, token } => {
                let db = turso::sync::Builder::new_remote(":memory:")
                    .with_remote_url(url)
                    .with_auth_token(token)
                    .build()
                    .await
                    .map_err(StoreError::from)?;
                Arc::new(DbBackend::Sync(Arc::new(db)))
            }
            #[cfg(feature = "replication")]
            DbConfig::EmbeddedReplica {
                local_path,
                url,
                token,
            } => {
                let db = turso::sync::Builder::new_remote(local_path.as_str())
                    .with_remote_url(url)
                    .with_auth_token(token)
                    .build()
                    .await
                    .map_err(StoreError::from)?;
                Arc::new(DbBackend::Sync(Arc::new(db)))
            }
        };

        let pool = Self {
            backend,
            breaker: Arc::new(DbCircuitBreaker::from_env()),
            sqlite_probe_cache: Arc::new(tokio::sync::RwLock::new(None)),
            writer: Arc::new(tokio::sync::OnceCell::new()),
        };

        // Bootstrap: perform schema check / migrations via a temporary connection
        let conn = pool.get().await?;
        crate::VoxDb::apply_pragmas(&conn.conn).await?;
        crate::VoxDb::migrate(&conn.conn).await?;
        Ok(pool)
    }

    /// Obtain a new, independent [`VoxDb`] handle.
    /// Connections are very cheap to construct from an initialized local database.
    pub async fn get(&self) -> Result<VoxDb, StoreError> {
        let conn = match &*self.backend {
            #[cfg(feature = "local")]
            DbBackend::Local(db) => db.connect().map_err(StoreError::from)?,
            DbBackend::Sync(db) => db.connect().await.map_err(StoreError::from)?,
        };

        crate::VoxDb::apply_pragmas(&conn).await?;

        // Extract raw turso sync database if any.
        // VoxDb expects an Option<turso::sync::Database>, but we hold Arc<turso::sync::Database>.
        // Since turso doesn't allow extracting it cheaply if it doesn't implement Clone,
        // we might actually have a slight discrepancy, but VoxDb sync calls can use it.
        // Wait, turso::sync::Database doesn't implement Clone?
        // Let's just create the exact type `VoxDb` expects.

        let sync_db_out = match &*self.backend {
            DbBackend::Sync(sdb) => Some((**sdb).clone()),
            #[cfg(feature = "local")]
            _ => None,
        };

        Ok(VoxDb {
            conn: crate::GuardedConnection::new(conn),
            sync_db: sync_db_out,
            local_db: None,
            writer: self.writer.get().cloned(),
            breaker: Arc::clone(&self.breaker),
            sqlite_probe_cache: Arc::clone(&self.sqlite_probe_cache),
        })
    }

    /// Obtain a handle to the dedicated database writer task.
    /// Spawns the actor on first access if not already running.
    pub async fn writer(&self) -> Result<crate::VoxWriteHandle, StoreError> {
        self.writer
            .get_or_try_init(|| async {
                let db = self.get().await?;
                Ok(crate::writer_actor::spawn_writer(db))
            })
            .await
            .cloned()
    }
}
