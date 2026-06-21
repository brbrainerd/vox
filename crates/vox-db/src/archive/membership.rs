//! Idempotent edge table: tracks which windows reference which CAS objects (Rev 2 Correction 3).
//!
//! Replaces the materialized `cas_refcount` counter. Refcount = COUNT(*), re-archiving = no-op,
//! unarchive/trim correctly decrements via `drop_window_edges`.

use crate::VoxDb;
use crate::store::types::StoreError;
use turso::params;

/// Idempotently record that `window_id` references `ref_hash`. Re-archiving = no-op.
pub async fn add_edge(db: &VoxDb, window_id: &str, ref_hash: &str) -> Result<(), StoreError> {
    let (window_id, ref_hash) = (window_id.to_string(), ref_hash.to_string());
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT OR IGNORE INTO archive_membership (window_id, ref_hash) VALUES (?1, ?2)",
                params![window_id.as_str(), ref_hash.as_str()],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

/// Remove all membership edges for `window_id` (on unarchive-delete / window hard-delete).
pub async fn drop_window_edges(db: &VoxDb, window_id: &str) -> Result<(), StoreError> {
    let window_id = window_id.to_string();
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "DELETE FROM archive_membership WHERE window_id = ?1",
                params![window_id.as_str()],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

/// Live reference count = distinct windows referencing this object.
pub async fn refs_of(db: &VoxDb, ref_hash: &str) -> Result<i64, StoreError> {
    let mut rows = db
        .conn
        .query(
            "SELECT COUNT(*) FROM archive_membership WHERE ref_hash = ?1",
            params![ref_hash],
        )
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| StoreError::Db("COUNT query returned no rows".into()))?;
    row.get::<i64>(0).map_err(|e| StoreError::Db(e.to_string()))
}

/// Frequency mining: most-referenced objects (for dictionary training sample selection).
pub async fn top_referenced(db: &VoxDb, limit: i64) -> Result<Vec<String>, StoreError> {
    let mut rows = db
        .conn
        .query(
            "SELECT ref_hash FROM archive_membership GROUP BY ref_hash ORDER BY COUNT(*) DESC LIMIT ?1",
            params![limit],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        out.push(r.get::<String>(0).map_err(|e| StoreError::Db(e.to_string()))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_edge_idempotent_and_refs_count() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        // Need a real object for the FK constraint.
        db.store("k", b"abc").await.unwrap();
        let h = crate::hash::content_hash(b"abc");

        add_edge(&db, "w1", &h).await.unwrap();
        add_edge(&db, "w2", &h).await.unwrap();
        assert_eq!(refs_of(&db, &h).await.unwrap(), 2);

        // Re-archiving w1 must not inflate refs (idempotent).
        add_edge(&db, "w1", &h).await.unwrap();
        assert_eq!(refs_of(&db, &h).await.unwrap(), 2, "re-archive must not inflate refs");

        drop_window_edges(&db, "w1").await.unwrap();
        assert_eq!(refs_of(&db, &h).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn top_referenced_orders_by_frequency() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        // Two objects: "popular" referenced 3×, "rare" referenced 1×.
        db.store("k", b"popular").await.unwrap();
        db.store("k", b"rare").await.unwrap();
        let hp = crate::hash::content_hash(b"popular");
        let hr = crate::hash::content_hash(b"rare");

        for win in ["w1", "w2", "w3"] {
            add_edge(&db, win, &hp).await.unwrap();
        }
        add_edge(&db, "w1", &hr).await.unwrap();

        let top = top_referenced(&db, 2).await.unwrap();
        assert_eq!(top[0], hp, "most-referenced must be first");
        assert_eq!(top[1], hr);
    }
}
