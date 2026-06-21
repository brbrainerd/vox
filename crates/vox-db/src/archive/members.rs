//! Reassembly map from a large item's hash to its ordered chunk hashes (design §4.2).

use crate::VoxDb;
use crate::store::types::StoreError;
use turso::params;

/// Record the ordered chunk hashes for `item_hash`.
/// Use only when an item was actually split into >1 chunk.
pub async fn set_members(
    db: &VoxDb,
    item_hash: &str,
    chunk_hashes: &[String],
) -> Result<(), StoreError> {
    for (ordinal, chunk_hash) in chunk_hashes.iter().enumerate() {
        let (item_hash, chunk_hash) = (item_hash.to_string(), chunk_hash.clone());
        let ordinal = ordinal as i64;
        let breaker = db.breaker.clone();
        let conn = db.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO chunk_members (item_hash, ordinal, chunk_hash)
                     VALUES (?1, ?2, ?3)",
                    params![item_hash.as_str(), ordinal, chunk_hash.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
    }
    Ok(())
}

/// Ordered chunk hashes for `item_hash`; empty when the item is a whole-message object.
pub async fn members_of(db: &VoxDb, item_hash: &str) -> Result<Vec<String>, StoreError> {
    let mut rows = db
        .conn
        .query(
            "SELECT chunk_hash FROM chunk_members WHERE item_hash = ?1 ORDER BY ordinal",
            params![item_hash],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        out.push(
            r.get::<String>(0)
                .map_err(|e| StoreError::Db(e.to_string()))?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn members_round_trip_in_order() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");
        // chunk_members FK requires the objects to exist.
        for b in [b"a".as_slice(), b"b", b"c"] {
            db.store("chunk", b).await.unwrap();
        }
        let (ha, hb, hc) = (
            crate::hash::content_hash(b"a"),
            crate::hash::content_hash(b"b"),
            crate::hash::content_hash(b"c"),
        );
        db.store("item", b"abc").await.unwrap();
        let item = crate::hash::content_hash(b"abc");
        set_members(&db, &item, &[ha.clone(), hb.clone(), hc.clone()])
            .await
            .unwrap();
        assert_eq!(members_of(&db, &item).await.unwrap(), vec![ha, hb, hc]);
    }
}
