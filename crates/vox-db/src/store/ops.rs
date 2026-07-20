//! Core connection and async bridge helpers for [`VoxDb`].
//!
//! Domain-specific CRUD lives in the sibling submodules:
//! - [`super::ops_cas`]: content-addressed storage (`objects`, `names`, `schema_version`)
//! - [`super::ops_agents`]: agent sessions and LLM interactions
//! - [`super::ops_memory`]: memories, knowledge nodes, embeddings, components
//! - [`super::ops_learning`]: behavioral learning, patterns, training data
//! - [`super::ops_codex`]: codex reactivity, research graph, reliability, eval, corpus

use crate::store::types::StoreError;

impl crate::VoxDb {
    /// Borrow the underlying libSQL connection (wrapped in [`crate::GuardedConnection`], which
    /// serializes concurrent `query`/`execute`/`execute_batch` calls to avoid
    /// `turso::Error::Misuse("concurrent use forbidden")`; see its docs for why this is
    /// necessary even though `turso::Connection` is itself `Clone`).
    ///
    /// Prefer typed `VoxDb` methods over calling SQL directly; use this only for
    /// one-off queries that do not belong to any domain module or for test verification.
    #[inline]
    #[must_use]
    pub fn connection(&self) -> &crate::GuardedConnection {
        &self.conn
    }

    /// Cached PRAGMA snapshot (WAL, FK, compile options / FTS hints). Safe to call repeatedly.
    pub async fn sqlite_capabilities_snapshot(
        &self,
    ) -> Result<crate::capabilities::SqliteProbeSnapshot, turso::Error> {
        {
            let cache = self.sqlite_probe_cache.read().await;
            if let Some(s) = cache.as_ref() {
                return Ok(s.clone());
            }
        }
        let snap = crate::capabilities::probe_sqlite_capabilities(self.connection()).await?;
        let mut cache = self.sqlite_probe_cache.write().await;
        if let Some(s) = cache.as_ref() {
            return Ok(s.clone());
        }
        *cache = Some(snap.clone());
        Ok(snap)
    }

    /// Alias for [`Self::sqlite_capabilities_snapshot`] (kept for call sites that name “probe”).
    pub async fn probe_sqlite_capabilities(
        &self,
    ) -> Result<crate::capabilities::SqliteProbeSnapshot, turso::Error> {
        self.sqlite_capabilities_snapshot().await
    }

    /// Run an async future from a synchronous call site (e.g. a `std::thread` worker).
    ///
    /// Uses `block_in_place` when a Tokio runtime is active; falls back to a fresh
    /// single-threaded runtime otherwise. Panics if the runtime cannot be created.
    pub fn block_on<R: Send>(&self, fut: impl std::future::Future<Output = R> + Send) -> R {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
            Err(_) => panic!("VoxDb::block_on requires an active Tokio runtime"),
        }
    }

    /// Run SQLite `VACUUM` (compact / reclaim space).
    pub async fn run_sqlite_vacuum(&self) -> Result<(), StoreError> {
        self.conn.execute("VACUUM", ()).await?;
        Ok(())
    }
}
