use crate::{DbCircuitBreaker, DbConfig, StoreError};

const DEFAULT_MAX_RETRIES: u64 = 3;
const DEFAULT_RETRY_BASE_MS: u64 = 500;

impl crate::VoxDb {
    /// Wrap an already-open `VoxDb` (e.g. after custom Turso setup).
    pub fn from_store(conn: turso::Connection, sync_db: Option<turso::sync::Database>) -> Self {
        Self {
            conn,
            sync_db,
            writer: None,
            breaker: std::sync::Arc::new(DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Connect to a database using the given configuration, with retry logic.
    pub async fn connect(config: DbConfig) -> Result<Self, StoreError> {
        Self::connect_with_retries(config, DEFAULT_MAX_RETRIES, DEFAULT_RETRY_BASE_MS).await
    }

    /// Connect using the **canonical** user-global Codex configuration.
    ///
    /// Same as [`Self::connect_canonical`]. Uses [`DbConfig::resolve_canonical`] so `VOX_DB_PATH`,
    /// remote/replica env, and platform `vox.db` behave consistently with [`DbConfig::resolve_standalone`].
    #[cfg(feature = "local")]
    pub async fn connect_default() -> Result<Self, StoreError> {
        Self::connect_canonical().await
    }

    /// Connect to the canonical user-global Codex store ([`DbConfig::resolve_canonical`]).
    pub async fn connect_canonical() -> Result<Self, StoreError> {
        let config = DbConfig::resolve_canonical().map_err(StoreError::NotFound)?;
        Self::connect(config).await
    }

    /// Blocking [`Self::connect_default`] for `std::thread` workers without a Tokio handle.
    #[cfg(feature = "local")]
    pub fn connect_default_sync() -> Result<Self, StoreError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| StoreError::Db(format!("tokio runtime: {e}")))?
            .block_on(Self::connect_default())
    }

    /// Optional hook before dropping a database handle (flush/sync); currently a no-op.
    pub fn shutdown_for_drop(&self) {
        let _ = self;
    }

    /// Connect with configurable retry parameters.
    ///
    /// Logs retries to stderr with the product name **Codex** (alias for this type).
    pub async fn connect_with_retries(
        config: DbConfig,
        max_retries: u64,
        retry_base_ms: u64,
    ) -> Result<Self, StoreError> {
        let mut attempts = 0u64;
        loop {
            attempts += 1;
            let result = match &config {
                #[cfg(feature = "local")]
                DbConfig::Local { path } => {
                    let db = turso::Builder::new_local(path)
                        .build()
                        .await
                        .map_err(StoreError::from)?;
                    let conn = db.connect().map_err(StoreError::from)?;
                    Ok((conn, None))
                }
                #[cfg(feature = "local")]
                DbConfig::Memory => {
                    let db = turso::Builder::new_local(":memory:")
                        .build()
                        .await
                        .map_err(StoreError::from)?;
                    let conn = db.connect().map_err(StoreError::from)?;
                    Ok((conn, None))
                }
                DbConfig::Remote { url, token } => {
                    let db = turso::sync::Builder::new_remote(":memory:")
                        .with_remote_url(url)
                        .with_auth_token(token)
                        .build()
                        .await
                        .map_err(StoreError::from)?;
                    let conn = db.connect().await.map_err(StoreError::from)?;
                    Ok((conn, Some(db)))
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
                    let conn = db.connect().await.map_err(StoreError::from)?;
                    Ok((conn, Some(db)))
                }
            };

            match result {
                Ok((conn, sync_db)) => {
                    Self::apply_pragmas(&conn).await?;
                    Self::migrate(&conn).await?;
                    return Ok(Self {
                        conn,
                        sync_db,
                        writer: None,
                        breaker: std::sync::Arc::new(DbCircuitBreaker::from_env()),
                        sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
                    });
                }
                Err(e) if attempts < max_retries => {
                    eprintln!(
                        "Failed to connect to Codex, retrying ({}/{})... Error: {}",
                        attempts, max_retries, e
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(retry_base_ms * attempts))
                        .await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Open the configured store **without** running the Arca baseline migration.
    ///
    /// Used by `vox codex export-legacy` so databases that still have the historical multi-version
    /// `schema_version` chain can be read. Normal apps should use [`Self::connect`].
    pub async fn connect_legacy_export_only(config: DbConfig) -> Result<Self, StoreError> {
        let conn = match config {
            DbConfig::Remote { url, token } => {
                turso::sync::Builder::new_remote(":memory:")
                    .with_remote_url(url)
                    .with_auth_token(token)
                    .build()
                    .await?
                    .connect()
                    .await?
            }
            #[cfg(feature = "local")]
            DbConfig::Local { path } => {
                let db = turso::Builder::new_local(&path).build().await?;
                db.connect()?
            }
            #[cfg(feature = "local")]
            DbConfig::Memory => {
                return Err(StoreError::NotFound(
                    "legacy export requires VOX_DB_PATH or remote URL (not memory)".into(),
                ));
            }
            #[cfg(feature = "replication")]
            DbConfig::EmbeddedReplica {
                local_path,
                url,
                token,
            } => {
                turso::sync::Builder::new_remote(local_path.as_str())
                    .with_remote_url(url)
                    .with_auth_token(token)
                    .build()
                    .await?
                    .connect()
                    .await?
            }
        };
        Self::apply_pragmas(&conn).await?;
        Ok(Self {
            conn,
            sync_db: None,
            writer: None,
            breaker: std::sync::Arc::new(DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    /// Access the circuit breaker for this database.
    pub fn breaker(&self) -> &DbCircuitBreaker {
        &self.breaker
    }
}
