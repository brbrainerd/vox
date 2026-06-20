//! Store accessors for context_windows + context_window_items (design 2026-06-20 §4).

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

/// Create a new context window record.
pub async fn create_window(
    db: &VoxDb,
    id: &str,
    repo_id: &str,
    kind: &str,
    root_window_id: &str,
    now: i64,
) -> Result<(), StoreError> {
    let id = id.to_string();
    let repo_id = repo_id.to_string();
    let kind = kind.to_string();
    let root_window_id = root_window_id.to_string();

    let breaker = db.breaker.clone();
    let conn = db.conn.clone();

    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO context_windows (id, repo_id, kind, root_window_id, token_estimate, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
                params![
                    id.as_str(),
                    repo_id.as_str(),
                    kind.as_str(),
                    root_window_id.as_str(),
                    now,
                ],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

/// Add an item to a window, storing the content in CAS.
/// Returns the content_hash (SHA3-512 Base32Hex) from `db.store()`.
/// `byte_len` is stored as `content.len()` — exact, model-invariant (design §6.1).
/// Does NOT compute or store any `token_estimate` here (that's done at the API boundary).
pub async fn add_item(
    db: &VoxDb,
    item_id: &str,
    window_id: &str,
    ordinal: i64,
    role: &str,
    item_kind: &str,
    content: &[u8],
    now: i64,
) -> Result<String, StoreError> {
    let hash = db.store("ctxwin-item", content).await?;
    let byte_len = content.len() as i64;

    let item_id = item_id.to_string();
    let window_id = window_id.to_string();
    let role = role.to_string();
    let item_kind = item_kind.to_string();
    let hash_copy = hash.clone();

    let breaker = db.breaker.clone();
    let conn = db.conn.clone();

    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO context_window_items (id, window_id, ordinal, role, item_kind, content_hash, byte_len, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    item_id.as_str(),
                    window_id.as_str(),
                    ordinal,
                    role.as_str(),
                    item_kind.as_str(),
                    hash_copy.as_str(),
                    byte_len,
                    now,
                ],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await?;

    Ok(hash)
}

/// Count live (non-trimmed) references to a content hash across all items.
pub async fn count_hash_references(db: &VoxDb, hash: &str) -> Result<i64, StoreError> {
    let mut rows = db
        .connection()
        .query(
            "SELECT COUNT(*) FROM context_window_items WHERE content_hash = ?1 AND trimmed_at IS NULL",
            params![hash],
        )
        .await?;

    let row = rows
        .next()
        .await?
        .ok_or_else(|| StoreError::Db("no row returned from COUNT(*)".into()))?;
    let count: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(count)
}

/// Mark an item as trimmed (soft delete).
pub async fn mark_item_trimmed(db: &VoxDb, item_id: &str, now: i64) -> Result<(), StoreError> {
    let item_id = item_id.to_string();

    let breaker = db.breaker.clone();
    let conn = db.conn.clone();

    breaker
        .call(|| async move {
            conn.execute(
                "UPDATE context_window_items SET trimmed_at = ?1 WHERE id = ?2",
                params![now, item_id.as_str()],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dedup_and_refcount() {
        let db = VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        create_window(&db, "w1", "repo1", "chat", "w1", 1000)
            .await
            .expect("window");
        let content = b"hello world";
        let h1 = add_item(&db, "i1", "w1", 0, "user", "message", content, 1001)
            .await
            .expect("i1");
        let h2 = add_item(&db, "i2", "w1", 1, "user", "message", content, 1002)
            .await
            .expect("i2");
        // same content → same hash (CAS dedup)
        assert_eq!(h1, h2, "duplicate content must produce the same hash");
        // refcount = 2 (both items live)
        let count = count_hash_references(&db, &h1).await.expect("count");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn cas_roundtrip() {
        let db = VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        create_window(&db, "w1", "repo1", "chat", "w1", 1000)
            .await
            .expect("window");
        let content = b"roundtrip content";
        let hash = add_item(&db, "i1", "w1", 0, "user", "message", content, 1001)
            .await
            .expect("item");
        let retrieved = db.get(&hash).await.expect("get");
        assert_eq!(retrieved, content, "content must survive CAS roundtrip");
    }

    #[tokio::test]
    async fn byte_len_is_exact() {
        let db = VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        create_window(&db, "w1", "repo1", "chat", "w1", 1000)
            .await
            .expect("window");
        let content = b"exact byte count";
        let hash = add_item(&db, "i1", "w1", 0, "user", "message", content, 1001)
            .await
            .expect("item");
        let mut q = db
            .connection()
            .query(
                "SELECT byte_len FROM context_window_items WHERE id='i1'",
                (),
            )
            .await
            .expect("query");
        let row = q.next().await.expect("next").expect("row");
        let stored_len: i64 = row
            .get(0)
            .map_err(|e| crate::store::StoreError::Db(e.to_string()))
            .expect("len");
        assert_eq!(
            stored_len,
            content.len() as i64,
            "byte_len must equal content.len()"
        );
        let _ = hash;
    }

    #[tokio::test]
    async fn trim_reduces_refcount() {
        let db = VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        create_window(&db, "w1", "repo1", "chat", "w1", 1000)
            .await
            .expect("window");
        let content = b"trim me";
        let hash = add_item(&db, "i1", "w1", 0, "user", "message", content, 1001)
            .await
            .expect("i1");
        add_item(&db, "i2", "w1", 1, "user", "message", content, 1002)
            .await
            .expect("i2");
        // before trim: 2 references
        assert_eq!(count_hash_references(&db, &hash).await.expect("count"), 2);
        mark_item_trimmed(&db, "i1", 2000).await.expect("trim");
        // after trim: 1 reference (trimmed_at IS NOT NULL excludes i1)
        assert_eq!(count_hash_references(&db, &hash).await.expect("count"), 1);
    }
}
