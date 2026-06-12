//! Store ops for `scientia_embedding_cache`.
//!
//! Provides a typed cache for LLM embedding vectors keyed by `sha256(model+text)`.
//! Rows are upserted with INSERT OR REPLACE so the latest write always wins.
//!
//! `vector_json` is a JSON array of `f32` values serialised/deserialised by these
//! ops; callers deal only in `Vec<f32>`.

use crate::VoxDb;
use crate::store::types::StoreError;
use turso::params;

impl VoxDb {
    /// Return the cached embedding vector for `text_sha256`, or `None` if absent.
    ///
    /// `vector_json` is deserialised from a JSON array; a malformed value in the
    /// DB is surfaced as a [`StoreError::Db`] rather than silently returning
    /// `None`.
    pub async fn get_cached_embedding(
        &self,
        text_sha256: &str,
    ) -> Result<Option<Vec<f32>>, StoreError> {
        let sha = text_sha256.to_string();
        let mut rows = self
            .conn
            .query(
                "SELECT vector_json FROM scientia_embedding_cache WHERE text_sha256 = ?1",
                params![sha],
            )
            .await
            .map_err(StoreError::Turso)?;

        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let json: String = row.get(0).map_err(StoreError::Turso)?;
            let vec: Vec<f32> = serde_json::from_str(&json).map_err(|e| {
                StoreError::Db(format!(
                    "scientia_embedding_cache: malformed vector_json for sha256 {text_sha256}: {e}"
                ))
            })?;
            Ok(Some(vec))
        } else {
            Ok(None)
        }
    }

    /// Upsert an embedding vector for `text_sha256`.
    ///
    /// Uses `INSERT OR REPLACE` so calling this twice with the same key stores
    /// the latest vector (the previous row is atomically replaced).
    ///
    /// `created_at_ms` is set to the current wall-clock time at call time.
    pub async fn put_cached_embedding(
        &self,
        text_sha256: &str,
        model: &str,
        vector: &[f32],
    ) -> Result<(), StoreError> {
        let sha = text_sha256.to_string();
        let mdl = model.to_string();
        let json = serde_json::to_string(vector).map_err(|e| {
            StoreError::Db(format!(
                "scientia_embedding_cache: failed to serialise vector for sha256 {text_sha256}: {e}"
            ))
        })?;
        let now = crate::now_unix_ms() as i64;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO scientia_embedding_cache \
                 (text_sha256, model, vector_json, created_at_ms) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![sha, mdl, json, now],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{DbConfig, VoxDb};

    fn approx_eq_vecs(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    /// Round-trip: `put_cached_embedding` then `get_cached_embedding` returns
    /// the same vector (same length and values within float precision).
    #[tokio::test]
    async fn round_trip_put_then_get() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let sha = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
        let model = "text-embedding-3-small";
        let vec: Vec<f32> = vec![0.1, -0.5, 0.9, 1.0, 0.0];

        db.put_cached_embedding(sha, model, &vec)
            .await
            .expect("put");

        let got = db
            .get_cached_embedding(sha)
            .await
            .expect("get")
            .expect("row present");

        assert_eq!(got.len(), vec.len(), "vector length must match");
        assert!(
            approx_eq_vecs(&got, &vec),
            "round-tripped values must match: got {got:?}, want {vec:?}"
        );
    }

    /// `get_cached_embedding` returns `None` for an unknown sha256.
    #[tokio::test]
    async fn get_missing_key_returns_none() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let result = db
            .get_cached_embedding(
                "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
            )
            .await
            .expect("get");
        assert!(result.is_none(), "missing sha256 must return None");
    }

    /// Calling `put_cached_embedding` twice with the same key stores the
    /// SECOND vector (upsert / INSERT OR REPLACE semantics).
    #[tokio::test]
    async fn upsert_replaces_previous_vector() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let sha = "1111111111111111111111111111111111111111111111111111111111111111";
        let model = "text-embedding-3-small";
        let first: Vec<f32> = vec![1.0, 2.0, 3.0];
        let second: Vec<f32> = vec![9.0, 8.0, 7.0];

        db.put_cached_embedding(sha, model, &first)
            .await
            .expect("first put");
        db.put_cached_embedding(sha, model, &second)
            .await
            .expect("second put");

        let got = db
            .get_cached_embedding(sha)
            .await
            .expect("get")
            .expect("row present after upsert");

        assert!(
            approx_eq_vecs(&got, &second),
            "upsert must return the latest vector: got {got:?}, want {second:?}"
        );
    }
}
