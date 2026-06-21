//! Archive pipeline: enumerate items → hybrid chunk → dedup+compress → membership edge → mark cold.

use crate::VoxDb;
use crate::store::types::StoreError;
use crate::archive::{chunking, compression, dictionary, members, membership};
use turso::params;

/// Archive a context window: dedup + compress all items into the shared CAS, record membership
/// edges (idempotent), and mark the window `tier='cold'`. Safe to call twice (idempotent).
pub async fn archive_window(db: &VoxDb, window_id: &str, now: i64) -> Result<(), StoreError> {
    // Resolve the latest dictionary once (None until first training run).
    let dict = dictionary::latest_dictionary(db).await?;
    let (dict_id, dict_bytes_owned): (Option<i64>, Option<Vec<u8>>) = match dict {
        Some((id, b)) => (Some(id), Some(b)),
        None => (None, None),
    };
    let dict_bytes: Option<&[u8]> = dict_bytes_owned.as_deref();

    // Enumerate item content hashes in ordinal order.
    let mut rows = db
        .conn
        .query(
            "SELECT content_hash FROM context_window_items WHERE window_id = ?1 ORDER BY ordinal",
            params![window_id],
        )
        .await?;
    let mut item_hashes = Vec::new();
    while let Some(r) = rows.next().await? {
        item_hashes.push(r.get::<String>(0).map_err(|e| StoreError::Db(e.to_string()))?);
    }

    // Process each item: chunk → compress → dedup → membership edge.
    for item_hash in &item_hashes {
        let content = db.get(item_hash).await?;
        let chunks = chunking::chunk_content(&content);

        if chunks.len() == 1 {
            // Whole-message item: compress in-place under the same content hash.
            let comp = compression::compress(&content, dict_bytes)?;
            db.put_compressed("ctxwin-item", &content, &comp, "zstd", dict_id).await?;
            membership::add_edge(db, window_id, item_hash).await?;
        } else {
            // Large item: store each chunk, record reassembly, convert original to manifest.
            let mut chunk_hashes = Vec::with_capacity(chunks.len());
            for chunk in &chunks {
                let comp = compression::compress(chunk, dict_bytes)?;
                let ch = db.put_compressed("ctxwin-chunk", chunk, &comp, "zstd", dict_id).await?;
                membership::add_edge(db, window_id, &ch).await?;
                chunk_hashes.push(ch);
            }
            members::set_members(db, item_hash, &chunk_hashes).await?;
            db.put_chunk_manifest(item_hash, content.len() as i64).await?;
        }
    }

    // Mark window cold — only after all items processed successfully.
    let window_id_str = window_id.to_string();
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "UPDATE context_windows SET tier = 'cold', updated_at = ?2 WHERE id = ?1",
                params![window_id_str.as_str(), now],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::membership;

    #[tokio::test]
    async fn identical_content_across_windows_dedups() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        let payload = b"shared system prompt ".repeat(8); // < 4KB, whole-message path

        crate::context_window_store::create_window(&db, "w1", "repo", "chat", "w1", 1).await.unwrap();
        crate::context_window_store::add_item(&db, "i1", "w1", 0, "user", "msg", &payload, 1).await.unwrap();
        crate::context_window_store::create_window(&db, "w2", "repo", "chat", "w2", 1).await.unwrap();
        crate::context_window_store::add_item(&db, "i2", "w2", 0, "user", "msg", &payload, 1).await.unwrap();

        archive_window(&db, "w1", 10).await.unwrap();
        archive_window(&db, "w2", 11).await.unwrap();

        let h = crate::hash::content_hash(&payload);

        // Ref count = 2 (one per window).
        assert_eq!(membership::refs_of(&db, &h).await.unwrap(), 2);

        // Re-archiving w1 must NOT inflate refs (idempotency check).
        archive_window(&db, "w1", 12).await.unwrap();
        assert_eq!(membership::refs_of(&db, &h).await.unwrap(), 2, "re-archive must not inflate refs");

        // Reads back losslessly.
        assert_eq!(db.get(&h).await.unwrap(), payload.as_slice());

        // Stored with codec='zstd' and compressed bytes < uncompressed.
        let mut rows = db.conn.query(
            "SELECT codec, length(data), uncompressed_len FROM objects WHERE hash = ?1",
            params![h.as_str()],
        ).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "zstd", "must be stored compressed");
        assert!(row.get::<i64>(1).unwrap() < row.get::<i64>(2).unwrap(), "stored < uncompressed");
    }

    #[tokio::test]
    async fn large_item_creates_chunked_manifest() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        // 120KB of varied content → forces FastCDC split.
        let big: Vec<u8> = (0..120_000).map(|i| (i * 2654435761usize) as u8).collect();

        crate::context_window_store::create_window(&db, "wl", "repo", "chat", "wl", 1).await.unwrap();
        crate::context_window_store::add_item(&db, "il", "wl", 0, "user", "paste", &big, 1).await.unwrap();

        archive_window(&db, "wl", 10).await.unwrap();

        let item_hash = crate::hash::content_hash(&big);

        // Original item hash should now be a 'chunked' manifest.
        let mut rows = db.conn.query(
            "SELECT codec, data FROM objects WHERE hash = ?1",
            params![item_hash.as_str()],
        ).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "chunked", "large item must become a manifest");

        // get() must reassemble and return byte-identical content.
        let got = db.get(&item_hash).await.unwrap();
        assert_eq!(got, big, "reassembled content must be byte-identical");
    }
}
