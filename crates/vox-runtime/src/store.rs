use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;

/// Resolve state directory where the SQLite DB will live.
pub(crate) fn state_dir() -> Option<PathBuf> {
    let base = std::env::var("VOX_DATA_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(platform_data_dir)
        .map(|d| d.join("vox"));
    base.map(|d| {
        let p = d.join("state");
        std::fs::create_dir_all(&p).ok();
        p
    })
}

#[cfg(target_os = "windows")]
fn platform_data_dir() -> Option<PathBuf> {
    std::env::var("APPDATA").ok().map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn platform_data_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .map(|h| h.join("Library").join("Application Support"))
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn platform_data_dir() -> Option<PathBuf> {
    std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local").join("share"))
        })
}

/// A SQLite-backed KV store for Actor state.
pub struct StateStore;

impl StateStore {
    async fn get_conn() -> Result<turso::Connection, std::io::Error> {
        let dir = state_dir().unwrap_or_else(|| PathBuf::from(".vox_state"));
        std::fs::create_dir_all(&dir).unwrap_or_default();
        let db_path = dir.join("state.db");

        let conn = turso::Builder::new_local(db_path.to_str().unwrap())
            .build()
            .await
            .map_err(|e: turso::Error| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?
            .connect()
            .map_err(|e: turso::Error| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS actor_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            turso::params![],
        )
        .await
        .map_err(|e: turso::Error| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        Ok(conn)
    }

    pub async fn save<T: Serialize>(key: &str, value: &T) -> Result<(), std::io::Error> {
        let conn = Self::get_conn().await?;
        let data = serde_json::to_string(value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        conn.execute(
            "INSERT INTO actor_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            turso::params![key, data],
        )
        .await
        .map_err(|e: turso::Error| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        Ok(())
    }

    pub async fn load<T: DeserializeOwned>(key: &str) -> Result<Option<T>, std::io::Error> {
        let conn = Self::get_conn().await?;
        let mut rows = conn
            .query(
                "SELECT value FROM actor_state WHERE key = ?1",
                turso::params![key],
            )
            .await
            .map_err(|e: turso::Error| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;

        if let Some(row) = rows.next().await.map_err(|e: turso::Error| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })? {
            let data: String = row.get(0).map_err(|e: turso::Error| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;
            let parsed = serde_json::from_str(&data)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }
}
