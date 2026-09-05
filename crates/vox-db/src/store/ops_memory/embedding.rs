use crate::store::types::{EmbeddingEntry, StoreError};
use turso::params;

impl crate::VoxDb {
    /// Store a raw embedding vector.
    pub async fn store_embedding(
        &self,
        source_type: &str,
        source_id: &str,
        _model: &str,
        vector: &[f32],
        metadata: Option<&str>,
        _vcs_snapshot_id: Option<&str>,
    ) -> Result<i64, StoreError> {
        let source_type = source_type.to_string();
        let source_id = source_id.to_string();
        let metadata = metadata.map(str::to_string);
        let mut blob = Vec::with_capacity(vector.len() * 4);
        for &v in vector {
            blob.extend_from_slice(&v.to_le_bytes());
        }
        let dim = vector.len() as i64;
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO embeddings (source_type, source_id, dim, vector, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        source_type.as_str(),
                        source_id.as_str(),
                        dim,
                        blob,
                        metadata.as_deref()
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Brute-force cosine similarity search over the `embeddings` table.
    ///
    /// Fetches up to `limit * 10` candidate rows, scores each, and returns the top `limit`
    /// sorted by similarity descending. Suitable for small tables (< 10 k rows).
    ///
    /// Called from `vox-db/src/lib.rs` `VoxDb::search_embeddings`.
    pub async fn search_similar_embeddings(
        &self,
        vector: &[f32],
        source_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<(EmbeddingEntry, f32)>, StoreError> {
        let lim = limit.clamp(1, 500);
        let probe = self.sqlite_capabilities_snapshot().await.ok();
        let candidate_cap = crate::capabilities::embedding_candidate_cap(lim, 10, probe.as_ref());
        let mut rows = match source_type {
            Some(st) => {
                self.conn
                    .query(
                        "SELECT id, source_type, source_id, dim, vector, metadata
                         FROM embeddings WHERE source_type = ?1
                         ORDER BY created_at DESC LIMIT ?2",
                        params![st, candidate_cap],
                    )
                    .await?
            }
            None => {
                self.conn
                    .query(
                        "SELECT id, source_type, source_id, dim, vector, metadata
                         FROM embeddings ORDER BY created_at DESC LIMIT ?1",
                        params![candidate_cap],
                    )
                    .await?
            }
        };

        let mut scored: Vec<(EmbeddingEntry, f32)> = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let st: Option<String> = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let source_id: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let dim: i64 = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            let blob: Vec<u8> = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
            let metadata: Option<String> = row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
            // Deserialise little-endian f32 bytes
            let stored: Vec<f32> = blob
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_le_bytes(*c))
                .collect();
            let dot: f32 = vector.iter().zip(stored.iter()).map(|(a, b)| a * b).sum();
            let mag_a: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            let mag_b: f32 = stored.iter().map(|x| x * x).sum::<f32>().sqrt();
            let sim = if mag_a > 0.0 && mag_b > 0.0 {
                dot / (mag_a * mag_b)
            } else {
                0.0
            };
            scored.push((
                EmbeddingEntry {
                    id,
                    source_type: st,
                    source_id,
                    dim,
                    metadata,
                },
                sim,
            ));
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(lim as usize);
        Ok(scored)
    }
}
