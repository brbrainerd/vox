//! Versioned zstd dictionaries trained from the corpus (design §4.4, §5.5).

use crate::VoxDb;
use crate::archive::membership;
use crate::store::types::StoreError;
use turso::params;

/// Insert a new dictionary version; returns its `id`.
pub async fn insert_dictionary(db: &VoxDb, bytes: &[u8], sample_count: i64) -> Result<i64, StoreError> {
    let bytes = bytes.to_vec();
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO zstd_dictionaries (version, bytes, sample_count)
                 VALUES ((SELECT COALESCE(MAX(version), 0) + 1 FROM zstd_dictionaries), ?1, ?2)",
                params![bytes.as_slice(), sample_count],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await?;
    let mut rows = db
        .conn
        .query("SELECT MAX(id) FROM zstd_dictionaries", ())
        .await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| StoreError::Db("no dict after insert".into()))?;
    row.get::<i64>(0).map_err(|e| StoreError::Db(e.to_string()))
}

/// The newest dictionary as `(id, bytes)`, or `None` if none trained yet.
pub async fn latest_dictionary(db: &VoxDb) -> Result<Option<(i64, Vec<u8>)>, StoreError> {
    let mut rows = db
        .conn
        .query(
            "SELECT id, bytes FROM zstd_dictionaries ORDER BY version DESC LIMIT 1",
            (),
        )
        .await?;
    match rows.next().await? {
        Some(r) => Ok(Some((
            r.get::<i64>(0).map_err(|e| StoreError::Db(e.to_string()))?,
            r.get::<Vec<u8>>(1).map_err(|e| StoreError::Db(e.to_string()))?,
        ))),
        None => Ok(None),
    }
}

/// Train a new zstd dictionary from the highest-frequency objects and persist it as a new version.
/// Samples up to `max_samples` objects by membership frequency (most-referenced first).
/// Returns the new dict id, or `Ok(None)` if there are fewer than 8 samples (zstd minimum).
pub async fn train_from_corpus(db: &VoxDb, max_samples: usize) -> Result<Option<i64>, StoreError> {
    let hashes = membership::top_referenced(db, max_samples as i64).await?;
    if hashes.len() < 8 {
        return Ok(None);
    }
    let mut samples: Vec<Vec<u8>> = Vec::with_capacity(hashes.len());
    for h in &hashes {
        samples.push(db.get(h).await?);
    }
    let dict = zstd::dict::from_samples(&samples, 112 * 1024)
        .map_err(|e| StoreError::Db(format!("zstd train: {e}")))?;
    let id = insert_dictionary(db, &dict, samples.len() as i64).await?;
    Ok(Some(id))
}

#[cfg(all(test, feature = "local"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_then_latest() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");
        assert!(latest_dictionary(&db).await.unwrap().is_none());
        let id = insert_dictionary(&db, b"dict-bytes-v1", 10)
            .await
            .unwrap();
        let (lid, bytes) = latest_dictionary(&db).await.unwrap().unwrap();
        assert_eq!(lid, id);
        assert_eq!(bytes, b"dict-bytes-v1");
    }

    #[tokio::test]
    async fn trains_when_enough_samples() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        // Insert 16 objects and create membership edges so they appear in top_referenced.
        for i in 0..16u64 {
            let body = format!("context window archive sample number {i} ").repeat(20);
            db.store("s", body.as_bytes()).await.unwrap();
            let h = crate::hash::content_hash(body.as_bytes());
            // Add an edge so it appears in top_referenced.
            crate::archive::membership::add_edge(&db, &format!("w{i}"), &h).await.unwrap();
        }
        let id = train_from_corpus(&db, 64).await.unwrap();
        assert!(id.is_some(), "should train a dictionary from 16 samples");
        assert!(latest_dictionary(&db).await.unwrap().is_some());
    }
}
