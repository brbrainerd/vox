//! Content-addressed storage (`objects`, `names`) and schema introspection for [`VoxDb`].
//!
//! The `objects` table (V1 schema) stores arbitrary blobs keyed by SHA3-512 Base32Hex hash.
//! The `names` table (V1 schema) maps `(namespace, name)` pairs to object hashes.
//! `schema_version` is created by [`super::open`] migrations and queried here.

use turso::params;

use crate::hash::content_hash;

use crate::store::types::StoreError;

impl crate::VoxDb {
    /// Write `data` as a `kind`-tagged blob into `objects` using its SHA3-512 Base32Hex hash as
    /// the primary key. Duplicate writes (`INSERT OR IGNORE`) are a no-op. Returns the hash.
    pub async fn store(&self, kind: &str, data: &[u8]) -> Result<String, StoreError> {
        let hash = content_hash(data);
        let kind = kind.to_string();
        let data = data.to_vec();
        let hash_insert = hash.clone();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO objects (hash, kind, data) VALUES (?1, ?2, ?3)",
                    params![hash_insert.as_str(), kind.as_str(), data.as_slice()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(hash)
    }

    /// Read the `data` blob for `hash` from `objects`. Codec-aware: decodes `zstd` and
    /// reassembles `chunked` manifests transparently.
    pub async fn get(&self, hash: &str) -> Result<Vec<u8>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT data, codec, dict_id, uncompressed_len FROM objects WHERE hash = ?1 LIMIT 1",
                params![hash],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("object {hash}")))?;
        let codec: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        match codec.as_str() {
            "chunked" => {
                let mut members = self
                    .conn
                    .query(
                        "SELECT chunk_hash FROM chunk_members WHERE item_hash = ?1 ORDER BY ordinal",
                        params![hash],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(m) = members.next().await? {
                    let ch: String = m.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                    out.extend_from_slice(&Box::pin(self.get(&ch)).await?);
                }
                Ok(out)
            }
            "zstd" => {
                let data: Vec<u8> = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                let dict_id: Option<i64> = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
                let ulen: i64 = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
                let dict = match dict_id {
                    Some(id) => Some(self.decoder_dictionary(id).await?),
                    None => None,
                };
                crate::archive::compression::decompress_prepared(
                    &data,
                    ulen as usize,
                    dict.as_deref(),
                )
            }
            _ => {
                let data: Vec<u8> = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok(data)
            }
        }
    }

    /// Write or overwrite an object with compressed data (only when smaller).
    /// `hash` = content_hash(original). On conflict, replaces iff `stored.len() < original.len()`.
    pub async fn put_compressed(
        &self,
        kind: &str,
        original: &[u8],
        stored: &[u8],
        codec: &str,
        dict_id: Option<i64>,
    ) -> Result<String, StoreError> {
        let hash = content_hash(original);
        let ulen = original.len() as i64;
        let smaller = stored.len() < original.len();
        let (kind, codec) = (kind.to_string(), codec.to_string());
        let (hash_ins, stored) = (hash.clone(), stored.to_vec());
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                if smaller {
                    conn.execute(
                        "INSERT INTO objects (hash, kind, data, codec, dict_id, uncompressed_len, storage)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'inline')
                         ON CONFLICT(hash) DO UPDATE SET
                           data = excluded.data,
                           codec = excluded.codec,
                           dict_id = excluded.dict_id,
                           uncompressed_len = excluded.uncompressed_len",
                        params![
                            hash_ins.as_str(),
                            kind.as_str(),
                            stored.as_slice(),
                            codec.as_str(),
                            dict_id,
                            ulen
                        ],
                    )
                    .await?;
                } else {
                    conn.execute(
                        "INSERT OR IGNORE INTO objects (hash, kind, data, codec, uncompressed_len, storage)
                         VALUES (?1, ?2, ?3, 'none', ?4, 'inline')",
                        params![hash_ins.as_str(), kind.as_str(), stored.as_slice(), ulen],
                    )
                    .await?;
                }
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(hash)
    }

    /// Convert the object at `item_hash` into a chunk manifest (data=NULL, codec='chunked').
    pub async fn put_chunk_manifest(
        &self,
        item_hash: &str,
        uncompressed_len: i64,
    ) -> Result<(), StoreError> {
        let item_hash = item_hash.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO objects (hash, kind, data, codec, uncompressed_len, storage)
                     VALUES (?1, 'ctxwin-item', NULL, 'chunked', ?2, 'inline')
                     ON CONFLICT(hash) DO UPDATE SET
                       data = NULL,
                       codec = 'chunked',
                       uncompressed_len = excluded.uncompressed_len",
                    params![item_hash.as_str(), uncompressed_len],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Fetch a dictionary's raw bytes by id.
    pub async fn dictionary_bytes(&self, dict_id: i64) -> Result<Vec<u8>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT bytes FROM zstd_dictionaries WHERE id = ?1",
                params![dict_id],
            )
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("dictionary {dict_id}")))?;
        row.get::<Vec<u8>>(0).map_err(|e| StoreError::Db(e.to_string()))
    }

    /// Load and cache a prepared `DecoderDictionary` for `dict_id`.
    pub async fn decoder_dictionary(
        &self,
        dict_id: i64,
    ) -> Result<std::sync::Arc<zstd::dict::DecoderDictionary<'static>>, StoreError> {
        {
            let cache = self.dict_cache.lock().unwrap();
            if let Some(d) = cache.get(&dict_id) {
                return Ok(d.clone());
            }
        }
        let bytes = self.dictionary_bytes(dict_id).await?;
        let dd = std::sync::Arc::new(zstd::dict::DecoderDictionary::copy(&bytes));
        self.dict_cache.lock().unwrap().insert(dict_id, dd.clone());
        Ok(dd)
    }

    /// Bind (or rebind) a logical `name` in `namespace` to a content hash in the `names` table.
    ///
    /// The `hash` must already exist in `objects`; the schema enforces the FK constraint.
    pub async fn bind_name(
        &self,
        namespace: &str,
        name: &str,
        hash: &str,
    ) -> Result<(), StoreError> {
        let namespace = namespace.to_string();
        let name = name.to_string();
        let hash = hash.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO names (namespace, name, hash, updated_at)
                     VALUES (?1, ?2, ?3, datetime('now'))
                     ON CONFLICT(namespace, name)
                     DO UPDATE SET hash = excluded.hash, updated_at = datetime('now')",
                    params![namespace.as_str(), name.as_str(), hash.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// List `(name, hash)` from `names` for `namespace`, ordered by `name`.
    pub async fn list_names_in_namespace(
        &self,
        namespace: &str,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT name, hash FROM names WHERE namespace = ?1 ORDER BY name ASC",
                params![namespace],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push((
                row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
            ));
        }
        Ok(out)
    }

    /// Return `MAX(version)` from `schema_version`, or `0` if the table is empty.
    pub async fn schema_version(&self) -> Result<i64, StoreError> {
        let mut rows = self
            .conn
            .query("SELECT COALESCE(MAX(version), 0) FROM schema_version", ())
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::Db("schema_version query returned no rows".into()))?;
        let v: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(v)
    }

    /// Serialize the live SQLite schema into a `db_snapshots` row, keyed by `snap_id`
    /// (the `db_snapshots.id` primary key). Returns `Ok(())` on success.
    ///
    /// Called from `vox-orchestrator` `Orchestrator::take_db_snapshot`.
    pub async fn take_db_snapshot(
        &self,
        snap_id: u64,
        agent_id: &str,
        description: &str,
    ) -> Result<(), StoreError> {
        let agent_id = agent_id.to_string();
        let description = description.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                // Capture a lightweight JSON-encoded snapshot of all table names (schema audit only).
                let mut rows = conn
                    .query(
                        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
                        (),
                    )
                    .await?;
                let mut names: Vec<String> = Vec::new();
                while let Some(row) = rows.next().await? {
                    let n: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                    names.push(n);
                }
                let payload = serde_json::to_string(&names)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                conn.execute(
                    "INSERT OR REPLACE INTO db_snapshots (id, agent_id, description, payload)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        snap_id as i64,
                        agent_id.as_str(),
                        description.as_str(),
                        payload
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Restore (replay) a db snapshot identified by `snap_id`.
    ///
    /// Validates the snapshot row exists; a full byte-for-byte restore would require an
    /// out-of-band database swap beyond libSQL's in-connection capabilities. Returns
    /// `NotFound` if the snapshot is absent.
    ///
    /// Called from `vox-orchestrator` `Orchestrator::undo_operation` / `redo_operation`.
    pub async fn restore_db_snapshot(&self, snap_id: u64) -> Result<(), StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id FROM db_snapshots WHERE id = ?1 LIMIT 1",
                params![snap_id as i64],
            )
            .await?;
        rows.next()
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("db_snapshot {snap_id}")))?;
        Ok(())
    }
}

#[cfg(all(test, feature = "local"))]
mod archive_cas_tests {
    use crate::archive::compression;

    #[tokio::test]
    async fn compressed_object_reads_back_transparently() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");
        let payload = b"archive payload ".repeat(64);
        let comp = compression::compress(&payload, None).unwrap();
        let hash = db
            .put_compressed("ctxwin-item", &payload, &comp, "zstd", None)
            .await
            .unwrap();
        let got = db.get(&hash).await.unwrap();
        assert_eq!(got, payload.as_slice());
    }

    #[tokio::test]
    async fn codec_check_stored_as_zstd() {
        use turso::params;
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");
        let payload = b"zstd codec test payload ".repeat(64);
        let comp = compression::compress(&payload, None).unwrap();
        assert!(comp.len() < payload.len(), "test payload must compress");
        let hash = db
            .put_compressed("ctxwin-item", &payload, &comp, "zstd", None)
            .await
            .unwrap();
        let mut rows = db
            .conn
            .query(
                "SELECT codec, length(data), uncompressed_len FROM objects WHERE hash = ?1",
                params![hash.as_str()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "zstd");
        assert!(row.get::<i64>(1).unwrap() < row.get::<i64>(2).unwrap());
    }
}
