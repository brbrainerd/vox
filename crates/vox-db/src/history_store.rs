//! History and Clip Manager DB store accessors.
//! Placed under the `history_entries` table.

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub repo_id: String,
    pub kind: String,
    pub text: String,
    pub redacted_text: String,
    pub created_at: i64,
    pub pinned: bool,
    pub source: Option<String>,
    pub token_estimate: i64,
}

/// Estimate tokens for a text string (chars/4 approximation).
fn estimate_tokens(text: &str) -> i64 {
    (text.chars().count() / 4) as i64
}

/// Add an entry with explicitly provided caps for eviction.
pub async fn add_entry_with_caps(
    db: &VoxDb,
    repo_id: &str,
    kind: &str,
    text: &str,
    _redacted_text: &str,
    created_at: i64,
    source: &str,
    caps: &HistoryCaps,
) -> Result<i64, StoreError> {
    let repo_id = repo_id.to_string();
    let kind = kind.to_string();
    let text = text.to_string();
    let (redacted_text, _) = crate::redact::redact(&text);
    let source = source.to_string();
    let token_estimate = estimate_tokens(&text);

    let limit_val = match kind.as_str() {
        "clip" => caps.clip,
        "command" => caps.command,
        _ => caps.chat,
    };

    let breaker = db.breaker.clone();
    let conn = db.conn.clone();

    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO history_entries (repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    repo_id.as_str(),
                    kind.as_str(),
                    text.as_str(),
                    redacted_text.as_str(),
                    created_at,
                    0i64, // pinned = false
                    source.as_str(),
                    token_estimate,
                ],
            )
            .await?;
            let id = conn.last_insert_rowid();

            conn.execute(
                "DELETE FROM history_entries
                 WHERE repo_id = ?1 AND kind = ?2 AND pinned = 0
                   AND id NOT IN (
                       SELECT id FROM history_entries
                       WHERE repo_id = ?1 AND kind = ?2 AND pinned = 0
                       ORDER BY created_at DESC, id DESC
                       LIMIT ?3
                   )",
                params![repo_id.as_str(), kind.as_str(), limit_val],
            )
            .await?;

            Ok::<i64, StoreError>(id)
        })
        .await
}

/// Add an entry to the history store using the default retention caps.
pub async fn add_entry(
    db: &VoxDb,
    repo_id: &str,
    kind: &str,
    text: &str,
    redacted_text: &str,
    created_at: i64,
    source: &str,
) -> Result<i64, StoreError> {
    add_entry_with_caps(
        db,
        repo_id,
        kind,
        text,
        redacted_text,
        created_at,
        source,
        &HistoryCaps::default(),
    )
    .await
}

/// List entries from the history store, ordered by newest first.
pub async fn list_entries(
    db: &VoxDb,
    repo_id: &str,
    kind: Option<&str>,
    limit: u32,
) -> Result<Vec<HistoryEntry>, StoreError> {
    let repo_id = repo_id.to_string();
    let kind = kind.map(str::to_string);

    let mut rows = if let Some(k) = kind {
        db.connection()
            .query(
                "SELECT id, repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate
                 FROM history_entries
                 WHERE repo_id = ?1 AND kind = ?2
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?3",
                params![repo_id.as_str(), k.as_str(), limit as i64],
            )
            .await?
    } else {
        db.connection()
            .query(
                "SELECT id, repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate
                 FROM history_entries
                 WHERE repo_id = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?2",
                params![repo_id.as_str(), limit as i64],
            )
            .await?
    };

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(row_to_entry(&row)?);
    }
    Ok(entries)
}

/// Search history entries matching the query, ordered by newest first.
/// Metacharacters (`%`, `_`, `\`) in `query` are escaped so they match literally.
pub async fn search_entries(
    db: &VoxDb,
    repo_id: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<HistoryEntry>, StoreError> {
    let repo_id = repo_id.to_string();
    // Escape LIKE metacharacters so `%` and `_` in the query match literally.
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{escaped}%");

    let mut rows = db
        .connection()
        .query(
            "SELECT id, repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate
             FROM history_entries
             WHERE repo_id = ?1 AND (text LIKE ?2 ESCAPE '\\' OR redacted_text LIKE ?2 ESCAPE '\\')
             ORDER BY created_at DESC, id DESC
             LIMIT ?3",
            params![repo_id.as_str(), pattern.as_str(), limit as i64],
        )
        .await?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(row_to_entry(&row)?);
    }
    Ok(entries)
}

fn row_to_entry(row: &turso::Row) -> Result<HistoryEntry, StoreError> {
    let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
    let repo_id: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
    let kind: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
    let text: String = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
    let redacted_text: String = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
    let created_at: i64 = row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
    let pinned_val: i64 = row.get(6).map_err(|e| StoreError::Db(e.to_string()))?;
    let source: Option<String> = row.get(7).map_err(|e| StoreError::Db(e.to_string()))?;
    let token_estimate: i64 = row.get(8).map_err(|e| StoreError::Db(e.to_string()))?;

    Ok(HistoryEntry {
        id,
        repo_id,
        kind,
        text,
        redacted_text,
        created_at,
        pinned: pinned_val == 1,
        source,
        token_estimate,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryCaps {
    pub clip: i64,
    pub command: i64,
    pub chat: i64,
}

impl Default for HistoryCaps {
    fn default() -> Self {
        Self {
            clip: 1000,
            command: 500,
            chat: 500,
        }
    }
}

pub async fn pin_entry(db: &VoxDb, id: i64, pinned: bool) -> Result<(), StoreError> {
    let pinned_val = if pinned { 1i64 } else { 0i64 };
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "UPDATE history_entries SET pinned = ?1 WHERE id = ?2",
                params![pinned_val, id],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

pub async fn delete_entry(db: &VoxDb, id: i64) -> Result<(), StoreError> {
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute("DELETE FROM history_entries WHERE id = ?1", params![id])
                .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DbConfig;

    #[tokio::test]
    async fn add_then_list_by_kind() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        add_entry(&db, "r1", "clip", "snippet A", "snippet A", 1000, "cli")
            .await
            .expect("add");
        add_entry(
            &db,
            "r1",
            "command",
            "cargo test",
            "cargo test",
            1001,
            "osc633",
        )
        .await
        .expect("add");

        let clips = list_entries(&db, "r1", Some("clip"), 50)
            .await
            .expect("list");
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].text, "snippet A");
        assert_eq!(clips[0].kind, "clip");

        let all = list_entries(&db, "r1", None, 50).await.expect("list");
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn add_entry_honors_injected_caps() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let caps = HistoryCaps {
            clip: 1,
            command: 50,
            chat: 50,
        };
        for i in 0..3i64 {
            add_entry_with_caps(
                &db,
                "r1",
                "clip",
                &format!("c{i}"),
                &format!("c{i}"),
                1000 + i,
                "cli",
                &caps,
            )
            .await
            .expect("add");
        }
        let clips = list_entries(&db, "r1", Some("clip"), 50).await.unwrap();
        assert_eq!(clips.len(), 1, "cap of 1 should evict down to 1 entry");
    }

    #[tokio::test]
    async fn evict_respects_caps_and_pins() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        let caps = HistoryCaps {
            clip: 2,
            command: 50,
            chat: 50,
        };
        // Add c0 first, then pin it before adding more
        add_entry_with_caps(&db, "r1", "clip", "c0", "c0", 1000, "cli", &caps)
            .await
            .unwrap();
        // Pin c0 (first inserted row has id=1)
        let first_entries = list_entries(&db, "r1", Some("clip"), 10).await.unwrap();
        let pinned_id = first_entries[0].id;
        pin_entry(&db, pinned_id, true).await.unwrap();

        for i in 1..5i64 {
            add_entry_with_caps(
                &db,
                "r1",
                "clip",
                &format!("c{i}"),
                &format!("c{i}"),
                1000 + i,
                "cli",
                &caps,
            )
            .await
            .unwrap();
        }

        let clips = list_entries(&db, "r1", Some("clip"), 50).await.unwrap();
        // pinned c0 + 2 newest unpinned (c4, c3) = 3
        assert_eq!(clips.len(), 3);
        assert!(clips.iter().any(|c| c.text == "c0"));
        assert!(clips.iter().any(|c| c.text == "c4"));
        assert!(clips.iter().any(|c| c.text == "c3"));
    }

    #[tokio::test]
    async fn token_estimate_is_nonzero_for_nonempty_text() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        // "hello world" = 11 chars → 11/4 = 2
        add_entry(&db, "r1", "clip", "hello world", "hello world", 1000, "cli")
            .await
            .expect("add");
        let clips = list_entries(&db, "r1", Some("clip"), 10).await.unwrap();
        assert_eq!(clips.len(), 1);
        assert!(
            clips[0].token_estimate > 0,
            "token_estimate should be non-zero for non-empty text"
        );
    }

    #[tokio::test]
    async fn search_treats_percent_as_literal() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        add_entry(&db, "r1", "clip", "100%", "100%", 1000, "cli")
            .await
            .unwrap();
        add_entry(
            &db,
            "r1",
            "clip",
            "no-percent-here",
            "no-percent-here",
            1001,
            "cli",
        )
        .await
        .unwrap();

        let hits = search_entries(&db, "r1", "%", 50).await.unwrap();
        assert_eq!(
            hits.len(),
            1,
            "'%' must match the literal percent, not all rows"
        );
        assert!(hits[0].text.contains("100%"));
    }
}
