//! Shared workspace database handle for GUI commands that would otherwise open
//! a fresh `VoxDb` connection per invoke (causing SQLITE_BUSY / os error 33).

use std::sync::Arc;

#[cfg(test)]
use vox_db::DbConfig;
use vox_db::{DbConnectSurface, VoxDb, connect_workspace_journey_optional};

#[derive(Clone)]
pub struct GuiDbPool {
    db: Option<Arc<VoxDb>>,
}

impl GuiDbPool {
    /// Best-effort workspace connect; pool may be empty when Turso is unavailable.
    pub async fn connect_workspace() -> Self {
        let db = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
            .await
            .map(Arc::new);
        Self { db }
    }

    #[cfg(test)]
    pub async fn connect_memory() -> Result<Self, String> {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self {
            db: Some(Arc::new(db)),
        })
    }

    pub fn handle(&self) -> Result<Arc<VoxDb>, String> {
        self.db
            .clone()
            .ok_or_else(|| "workspace database unavailable".to_string())
    }
}

pub fn map_db_err(e: impl std::fmt::Display) -> String {
    let s = e.to_string();
    if s.contains("Locking error")
        || s.contains("SQLITE_BUSY")
        || s.contains("os error 33")
        || s.contains("database is locked")
    {
        "Database busy — another process is writing. Retry in a moment.".into()
    } else if s.contains("concurrent use forbidden") {
        // Defense-in-depth: the primary fix serializes access inside `vox_db::GuardedConnection`
        // so this should no longer be reachable from `VoxDb` methods. Keep this mapping in case
        // some other unguarded connection path still surfaces the raw Turso error, so a future
        // regression degrades to a retry hint instead of a scary raw Misuse string in the toast.
        "Database busy — another process is writing. Retry in a moment.".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pool_reuses_same_connection() {
        let pool = GuiDbPool::connect_memory().await.unwrap();
        let a = pool.handle().unwrap();
        let b = pool.handle().unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn map_db_err_gives_friendly_message_for_concurrent_use_forbidden() {
        let raw = "Misuse(\"concurrent use forbidden\")";
        let mapped = map_db_err(raw);
        assert_eq!(
            mapped,
            "Database busy — another process is writing. Retry in a moment."
        );
        assert!(
            !mapped.contains("concurrent use forbidden"),
            "raw Turso Misuse string must not leak into the user-facing toast"
        );
    }
}
