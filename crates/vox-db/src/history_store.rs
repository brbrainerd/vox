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

/// Add an entry to the history store.
pub async fn add_entry(
    db: &VoxDb,
    repo_id: &str,
    kind: &str,
    text: &str,
    redacted_text: &str,
    created_at: i64,
    source: &str,
) -> Result<i64, StoreError> {
    let repo_id = repo_id.to_string();
    let kind = kind.to_string();
    let text = text.to_string();
    let redacted_text = redacted_text.to_string();
    let source = source.to_string();

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
                    0i64, // token_estimate = 0 (placeholder)
                ],
            )
            .await?;
            let id = conn.last_insert_rowid();
            Ok::<i64, StoreError>(id)
        })
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
        let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let repo_id: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        let kind: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
        let text: String = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
        let redacted_text: String = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
        let created_at: i64 = row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
        let pinned_val: i64 = row.get(6).map_err(|e| StoreError::Db(e.to_string()))?;
        let source: Option<String> = row.get(7).map_err(|e| StoreError::Db(e.to_string()))?;
        let token_estimate: i64 = row.get(8).map_err(|e| StoreError::Db(e.to_string()))?;

        entries.push(HistoryEntry {
            id,
            repo_id,
            kind,
            text,
            redacted_text,
            created_at,
            pinned: pinned_val == 1,
            source,
            token_estimate,
        });
    }

    Ok(entries)
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
}
