use std::sync::Arc;
use tokio::sync::OnceCell;
use vox_db::{DbConfig, VoxDb, SchemaDigest};
pub use vox_db::StoreError;

static DB: OnceCell<Arc<VoxDb>> = OnceCell::const_new();

/// Initialize the global database instance using environment variables.
///
/// Looks for:
/// - `VOX_DB_PATH`: Local SQLite file path (default: `vox_state/vox.db`)
/// - `TURSO_URL`: Remote Turso URL
/// - `TURSO_AUTH_TOKEN`: Remote Turso auth token
pub async fn get_db() -> Result<Arc<VoxDb>, StoreError> {
    DB.get_or_try_init(|| async {
        let config = if let (Ok(url), Ok(token)) = (std::env::var("TURSO_URL"), std::env::var("TURSO_AUTH_TOKEN")) {
            DbConfig::Remote { url, token }
        } else {
            let path = std::env::var("VOX_DB_PATH").unwrap_or_else(|_| {
                let mut p = crate::store::state_dir().unwrap_or_else(|| std::path::PathBuf::from(".vox_state"));
                p.push("vox.db");
                p.to_string_lossy().to_string()
            });
            DbConfig::Local { path }
        };

        let db = VoxDb::connect(config).await?;
        Ok(Arc::new(db))
    }).await.map(|db| db.clone())
}

/// Ensure the database schema matches the provided digest.
///
/// This should be called during application startup (usually in the generated `main()`).
pub async fn ensure_schema(digest: &SchemaDigest) -> Result<(), StoreError> {
    let db = get_db().await?;
    db.sync_schema_from_digest(digest).await?;
    Ok(())
}
