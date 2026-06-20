# Archive / Dedup / Compression Engine (vox-db) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the vox-db engine that, on manual archive, deduplicates a context window's items into the shared CAS (whole-message + FastCDC), zstd-compresses new objects under a corpus-trained dictionary, refcounts for safe GC, and reads them back losslessly.

**Architecture:** Extends the existing content-addressed `objects` store and the `context_windows`/`context_window_items` spine (from the context-window-spine branch). Compression and chunking are pure, unit-testable modules; CAS `get()` becomes codec-aware so all existing readers transparently decompress; an `archive_store` module orchestrates the pipeline using the breaker pattern.

**Tech Stack:** Rust, Turso/libSQL, `zstd` (bulk + dictionary API), `fastcdc` (v2020), SHA3-512 Base32Hex content hashing.

**BASE BRANCH (REQUIRED):** Create the working branch from `claude/context-window-spine`, NOT `main`. That branch has `context_windows`/`context_window_items` + `context_window_store.rs` + `BASELINE_VERSION = 80`. This plan bumps the baseline **80 → 81**. The design spec lives at `docs/superpowers/specs/2026-06-20-archive-dedup-compression-design.md`.

**SCOPE:** This plan is the engine only (vox-db). Out of scope here, tracked as follow-on plans:
- **Plan B** — GUI chat-tab archive/unarchive control, Tauri commands + `vox://context-archived`, `SearchCorpus::ContextArchive` + the ~3 match sites, background archive via `processing_runs`.
- **Phase 2** — embeddings over deduped chunks, the `vox ci db-hygiene` standing gate, and the full dead-table sweep (spec §6.2/§6.4).

**Verification command (every task):** `cargo test -p vox-db <test_name> -- --nocapture`. Full crate gate at the end: `cargo test -p vox-db`.

---

### Task 1: Add `zstd` and `fastcdc` dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root `[workspace.dependencies]`)
- Modify: `crates/vox-db/Cargo.toml:8-34`

- [ ] **Step 1: Add to workspace dependencies**

In the root `Cargo.toml` under `[workspace.dependencies]`, add (keep alphabetical where the file is sorted):

```toml
zstd = "0.13"
fastcdc = "3"
```

- [ ] **Step 2: Reference them in vox-db**

In `crates/vox-db/Cargo.toml`, add under `[dependencies]` (after line 28 `blake3`):

```toml
zstd = { workspace = true }
fastcdc = { workspace = true }
```

- [ ] **Step 3: Verify it builds and licenses pass**

Run: `cargo build -p vox-db`
Expected: compiles. Then run: `cargo deny check licenses 2>&1 | tail -20`
Expected: no new license violations for `zstd`/`fastcdc` (both MIT/BSD-family). If `cargo deny` is unavailable, note it and continue.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/vox-db/Cargo.toml Cargo.lock
git commit -m "build(vox-db): add zstd + fastcdc for archive compression/chunking"
```

---

### Task 2: Schema — `objects` columns + archive tables + baseline 80→81

**Files:**
- Modify: `crates/vox-db/src/schema/domains/cas_codex.rs:3-8` (objects table columns)
- Create: `crates/vox-db/src/schema/domains/context_archive.rs`
- Modify: `crates/vox-db/src/schema/domains/mod.rs`
- Modify: `crates/vox-db/src/schema/manifest.rs:12-15` and `:118-126` (version + fragment list)
- Modify: `contracts/db/baseline-version-policy.yaml`

- [ ] **Step 1: Add compression columns to `objects`**

In `crates/vox-db/src/schema/domains/cas_codex.rs`, replace the `objects` CREATE TABLE (lines 3-8) with:

```rust
CREATE TABLE IF NOT EXISTS objects (
    hash TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    data BLOB,
    codec TEXT NOT NULL DEFAULT 'none',
    dict_id INTEGER,
    uncompressed_len INTEGER NOT NULL DEFAULT 0,
    storage TEXT NOT NULL DEFAULT 'inline',
    file_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

(Note: `data` is now nullable — it is `NULL` when `storage='file'`. `codec='none'` means raw bytes, preserving every existing `store()` caller.)

- [ ] **Step 2: Create the archive domain fragment**

Create `crates/vox-db/src/schema/domains/context_archive.rs`:

```rust
//! Arca SQL: context-archive dedup/compression support (design 2026-06-20 §4).
pub const SCHEMA_CONTEXT_ARCHIVE: &str = r#"
CREATE TABLE IF NOT EXISTS chunk_members (
    item_hash  TEXT NOT NULL,
    ordinal    INTEGER NOT NULL,
    chunk_hash TEXT NOT NULL REFERENCES objects(hash),
    PRIMARY KEY (item_hash, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_chunk_members_chunk ON chunk_members(chunk_hash);

CREATE TABLE IF NOT EXISTS cas_refcount (
    hash TEXT PRIMARY KEY REFERENCES objects(hash),
    refs INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_cas_refcount_refs ON cas_refcount(refs);

CREATE TABLE IF NOT EXISTS zstd_dictionaries (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    version      INTEGER NOT NULL,
    bytes        BLOB    NOT NULL,
    sample_count INTEGER NOT NULL DEFAULT 0,
    trained_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    notes        TEXT
);
"#;
```

- [ ] **Step 3: Register the module**

In `crates/vox-db/src/schema/domains/mod.rs`, add in alphabetical position (after `pub mod conversations;`):

```rust
pub mod context_archive;
```

- [ ] **Step 4: Register the fragment + bump the version**

In `crates/vox-db/src/schema/manifest.rs`, change lines 12-15:

```rust
// 81: feat(context): archive dedup/compression — objects codec columns + chunk_members + cas_refcount + zstd_dictionaries (design 2026-06-20)
pub const BASELINE_VERSION: i64 = 81;
```

Then append to `SCHEMA_FRAGMENTS` (after the `context_windows` entry):

```rust
    SchemaFragment {
        name: "context_archive",
        sql: domains::context_archive::SCHEMA_CONTEXT_ARCHIVE,
    },
```

- [ ] **Step 5: Run the baseline policy test to get the new digest**

Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema -- --nocapture`
Expected: FAIL with a message containing `set repository_baseline_digest_hex to 0x...`. Copy that hex.

- [ ] **Step 6: Update the baseline policy YAML**

In `contracts/db/baseline-version-policy.yaml`, set:

```yaml
  repository_baseline_integer: 81
  repository_baseline_digest_hex: "0x<paste-from-step-5>"
```

- [ ] **Step 7: Re-run the policy test**

Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-db/src/schema/domains/cas_codex.rs crates/vox-db/src/schema/domains/context_archive.rs crates/vox-db/src/schema/domains/mod.rs crates/vox-db/src/schema/manifest.rs contracts/db/baseline-version-policy.yaml
git commit -m "feat(vox-db): archive schema — objects codec cols + chunk_members/cas_refcount/zstd_dictionaries (baseline 81)"
```

---

### Task 3: FastCDC chunking module (pure)

**Files:**
- Create: `crates/vox-db/src/archive/mod.rs`
- Create: `crates/vox-db/src/archive/chunking.rs`
- Modify: `crates/vox-db/src/lib.rs` (add `pub mod archive;` near the other `pub mod` lines, e.g. adjacent to `pub mod context_window_store;`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-db/src/archive/chunking.rs`:

```rust
//! Hybrid chunking: small items pass through whole; large items split via FastCDC (design §3).

/// Items at or above this byte length are content-defined-chunked; smaller items are whole.
pub const LARGE_ITEM_THRESHOLD: usize = 4 * 1024;

const MIN_CHUNK: u32 = 2 * 1024;
const AVG_CHUNK: u32 = 8 * 1024;
const MAX_CHUNK: u32 = 16 * 1024;

/// Split `content` into chunks. Returns a single-element vec (the whole content) when
/// `content.len() < LARGE_ITEM_THRESHOLD`; otherwise FastCDC content-defined chunks whose
/// concatenation equals `content` exactly.
pub fn chunk_content(content: &[u8]) -> Vec<Vec<u8>> {
    if content.len() < LARGE_ITEM_THRESHOLD {
        return vec![content.to_vec()];
    }
    fastcdc::v2020::FastCDC::new(content, MIN_CHUNK, AVG_CHUNK, MAX_CHUNK)
        .map(|c| content[c.offset..c.offset + c.length].to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_item_is_one_chunk() {
        let data = vec![7u8; 100];
        let chunks = chunk_content(&data);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], data);
    }

    #[test]
    fn large_item_splits_and_reassembles_exactly() {
        // 200 KB of varied content so CDC finds multiple boundaries.
        let data: Vec<u8> = (0..200_000).map(|i| (i * 2654435761usize) as u8).collect();
        let chunks = chunk_content(&data);
        assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
        let rejoined: Vec<u8> = chunks.concat();
        assert_eq!(rejoined, data, "concatenated chunks must equal original");
    }
}
```

Create `crates/vox-db/src/archive/mod.rs`:

```rust
//! Archive engine: chunking, compression, dedup pipeline, and codec-aware reads (design §3-§5).
pub mod chunking;
pub mod compression;
```

In `crates/vox-db/src/lib.rs`, add next to `pub mod context_window_store;`:

```rust
pub mod archive;
```

(`compression` is created in Task 4; to compile Task 3 alone, temporarily comment the `pub mod compression;` line, or implement Task 4 before running. Prefer implementing Task 4 next and running both together.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-db chunking::tests -- --nocapture`
Expected: FAIL to compile until `fastcdc` API is correct, then the two tests run.

- [ ] **Step 3: (Implementation already shown in Step 1.)** Adjust the `fastcdc` call if the crate's `Chunk` field names differ (v2020 exposes `offset` + `length`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-db chunking::tests`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/archive/mod.rs crates/vox-db/src/archive/chunking.rs crates/vox-db/src/lib.rs
git commit -m "feat(vox-db): hybrid FastCDC chunking module (4KB threshold, 8KB target)"
```

---

### Task 4: Compression module (pure, dictionary-aware)

**Files:**
- Create: `crates/vox-db/src/archive/compression.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-db/src/archive/compression.rs`:

```rust
//! zstd compression with optional trained dictionary (design §4.1, §5.5).

use crate::store::StoreError;

const ZSTD_LEVEL: i32 = 19;

/// Compress `data` with zstd. When `dict` is `Some`, uses it as a trained dictionary.
pub fn compress(data: &[u8], dict: Option<&[u8]>) -> Result<Vec<u8>, StoreError> {
    let out = match dict {
        Some(d) => {
            let mut c = zstd::bulk::Compressor::with_dictionary(ZSTD_LEVEL, d)
                .map_err(|e| StoreError::Db(format!("zstd compressor: {e}")))?;
            c.compress(data).map_err(|e| StoreError::Db(format!("zstd compress: {e}")))?
        }
        None => zstd::bulk::compress(data, ZSTD_LEVEL)
            .map_err(|e| StoreError::Db(format!("zstd compress: {e}")))?,
    };
    Ok(out)
}

/// Decompress `data`. `capacity` MUST be the original uncompressed length (stored as
/// `objects.uncompressed_len`). `dict` must match what was used to compress.
pub fn decompress(data: &[u8], capacity: usize, dict: Option<&[u8]>) -> Result<Vec<u8>, StoreError> {
    let out = match dict {
        Some(d) => {
            let mut dec = zstd::bulk::Decompressor::with_dictionary(d)
                .map_err(|e| StoreError::Db(format!("zstd decompressor: {e}")))?;
            dec.decompress(data, capacity)
                .map_err(|e| StoreError::Db(format!("zstd decompress: {e}")))?
        }
        None => zstd::bulk::decompress(data, capacity)
            .map_err(|e| StoreError::Db(format!("zstd decompress: {e}")))?,
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_no_dict() {
        let data = b"the quick brown fox jumps over the lazy dog".repeat(50);
        let comp = compress(&data, None).unwrap();
        assert!(comp.len() < data.len(), "should shrink repetitive data");
        let back = decompress(&comp, data.len(), None).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn round_trip_with_dict() {
        let dict = b"context window archive session message tool output ".repeat(20);
        let data = b"context window archive session foo bar".to_vec();
        let comp = compress(&data, Some(&dict)).unwrap();
        let back = decompress(&comp, data.len(), Some(&dict)).unwrap();
        assert_eq!(back, data);
    }
}
```

- [ ] **Step 2: Run test to verify it fails, then passes**

Run: `cargo test -p vox-db compression::tests`
Expected: compiles after fixing any `zstd` API drift; both tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/archive/compression.rs
git commit -m "feat(vox-db): zstd compress/decompress module with dictionary support"
```

---

### Task 5: Codec-aware `get()` + `store_compressed()`

**Files:**
- Modify: `crates/vox-db/src/store/ops_cas.rs:16-51`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-db/src/store/ops_cas.rs` (inside a `#[cfg(test)] mod` — create one if absent, using the crate's existing test-db helper `vox_test_harness`):

```rust
#[cfg(test)]
mod archive_cas_tests {
    use crate::archive::compression;

    #[tokio::test]
    async fn compressed_object_reads_back_transparently() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        let payload = b"archive payload ".repeat(64);
        let comp = compression::compress(&payload, None).unwrap();
        let hash = db
            .store_compressed("ctxwin-item", &payload, &comp, "zstd", None)
            .await
            .unwrap();
        // get() must return the ORIGINAL bytes, transparently decompressing.
        let got = db.get(&hash).await.unwrap();
        assert_eq!(got, payload);
    }
}
```

If the crate lacks `test_support::memory_db`, use the same helper that `context_window_store.rs` tests use (mirror that file's test setup exactly).

- [ ] **Step 2: Implement `store_compressed` and make `get` codec-aware**

Add to the `impl crate::VoxDb` block in `ops_cas.rs`:

```rust
    /// Write a compressed (or relocated) object. `hash` is the content hash of the ORIGINAL
    /// (uncompressed) bytes; `stored` is the bytes actually placed in `data` (e.g. zstd output).
    /// `codec` is `"zstd"` (or `"none"`). `dict_id` records which dictionary version compressed it.
    pub async fn store_compressed(
        &self,
        kind: &str,
        original: &[u8],
        stored: &[u8],
        codec: &str,
        dict_id: Option<i64>,
    ) -> Result<String, StoreError> {
        let hash = crate::hash::content_hash(original);
        let uncompressed_len = original.len() as i64;
        let (kind, codec) = (kind.to_string(), codec.to_string());
        let (hash_ins, stored) = (hash.clone(), stored.to_vec());
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO objects (hash, kind, data, codec, dict_id, uncompressed_len, storage)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'inline')",
                    params![
                        hash_ins.as_str(),
                        kind.as_str(),
                        stored.as_slice(),
                        codec.as_str(),
                        dict_id,
                        uncompressed_len
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await?;
        Ok(hash)
    }
```

Replace the body of `get()` so it decompresses when `codec='zstd'`:

```rust
    /// Read the object for `hash`, transparently decompressing when stored compressed.
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
        let data: Vec<u8> = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
        let codec: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
        if codec == "zstd" {
            let dict_id: Option<i64> = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let ulen: i64 = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            let dict = match dict_id {
                Some(id) => Some(self.dictionary_bytes(id).await?),
                None => None,
            };
            return crate::archive::compression::decompress(&data, ulen as usize, dict.as_deref());
        }
        Ok(data)
    }
```

(`dictionary_bytes` is implemented in Task 8. To compile Task 5 alone, stub it to `unimplemented!()` or implement Task 8 first; prefer ordering Task 8 before running this test.)

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vox-db archive_cas_tests`
Expected: PASS (after Task 8's `dictionary_bytes` exists).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db/src/store/ops_cas.rs
git commit -m "feat(vox-db): codec-aware get() + store_compressed() CAS ops"
```

---

### Task 6: `cas_refcount` accessors

**Files:**
- Create: `crates/vox-db/src/archive/refcount.rs`
- Modify: `crates/vox-db/src/archive/mod.rs` (add `pub mod refcount;`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-db/src/archive/refcount.rs`:

```rust
//! Reference counting over CAS objects for safe GC + the mining frequency signal (design §4.3).

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

/// Increment (creating at 1 if absent) the refcount for `hash`.
pub async fn incr(db: &VoxDb, hash: &str) -> Result<(), StoreError> {
    let hash = hash.to_string();
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO cas_refcount (hash, refs) VALUES (?1, 1)
                 ON CONFLICT(hash) DO UPDATE SET refs = refs + 1",
                params![hash.as_str()],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

/// Decrement the refcount for `hash` (floor 0).
pub async fn decr(db: &VoxDb, hash: &str) -> Result<(), StoreError> {
    let hash = hash.to_string();
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "UPDATE cas_refcount SET refs = MAX(0, refs - 1) WHERE hash = ?1",
                params![hash.as_str()],
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}

/// Current refcount for `hash` (0 if absent).
pub async fn refs_of(db: &VoxDb, hash: &str) -> Result<i64, StoreError> {
    let mut rows = db
        .conn
        .query("SELECT refs FROM cas_refcount WHERE hash = ?1", params![hash])
        .await?;
    match rows.next().await? {
        Some(r) => Ok(r.get::<i64>(0).map_err(|e| StoreError::Db(e.to_string()))?),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn incr_decr_tracks_refs() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        db.store("k", b"abc").await.unwrap();
        let h = crate::hash::content_hash(b"abc");
        incr(&db, &h).await.unwrap();
        incr(&db, &h).await.unwrap();
        assert_eq!(refs_of(&db, &h).await.unwrap(), 2);
        decr(&db, &h).await.unwrap();
        assert_eq!(refs_of(&db, &h).await.unwrap(), 1);
    }
}
```

Add to `crates/vox-db/src/archive/mod.rs`:

```rust
pub mod refcount;
```

- [ ] **Step 2: Run test to verify it fails, then passes**

Run: `cargo test -p vox-db archive::refcount::tests`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/archive/refcount.rs crates/vox-db/src/archive/mod.rs
git commit -m "feat(vox-db): cas_refcount incr/decr/refs_of accessors"
```

---

### Task 7: `chunk_members` accessors

**Files:**
- Create: `crates/vox-db/src/archive/members.rs`
- Modify: `crates/vox-db/src/archive/mod.rs` (add `pub mod members;`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-db/src/archive/members.rs`:

```rust
//! Reassembly map from a large item's hash to its ordered chunk hashes (design §4.2).

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

/// Record the ordered chunk hashes for `item_hash`. No-op for whole-message items
/// (call only when an item was actually split into >1 chunk).
pub async fn set_members(db: &VoxDb, item_hash: &str, chunk_hashes: &[String]) -> Result<(), StoreError> {
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
        out.push(r.get::<String>(0).map_err(|e| StoreError::Db(e.to_string()))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn members_round_trip_in_order() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
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
        set_members(&db, &item, &[ha.clone(), hb.clone(), hc.clone()]).await.unwrap();
        assert_eq!(members_of(&db, &item).await.unwrap(), vec![ha, hb, hc]);
    }
}
```

Add to `crates/vox-db/src/archive/mod.rs`:

```rust
pub mod members;
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vox-db archive::members::tests`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/archive/members.rs crates/vox-db/src/archive/mod.rs
git commit -m "feat(vox-db): chunk_members set/members_of accessors"
```

---

### Task 8: `zstd_dictionaries` accessors

**Files:**
- Create: `crates/vox-db/src/archive/dictionary.rs`
- Modify: `crates/vox-db/src/archive/mod.rs` (add `pub mod dictionary;`)
- Modify: `crates/vox-db/src/store/ops_cas.rs` (add `dictionary_bytes` used by `get()`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-db/src/archive/dictionary.rs`:

```rust
//! Versioned zstd dictionaries trained from the corpus (design §4.4, §5.5).

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

/// Insert a new dictionary version; returns its `id`. `version` is the prior max + 1.
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
    let row = rows.next().await?.ok_or_else(|| StoreError::Db("no dict after insert".into()))?;
    row.get::<i64>(0).map_err(|e| StoreError::Db(e.to_string()))
}

/// The newest dictionary as `(id, bytes)`, or `None` if none trained yet.
pub async fn latest_dictionary(db: &VoxDb) -> Result<Option<(i64, Vec<u8>)>, StoreError> {
    let mut rows = db
        .conn
        .query("SELECT id, bytes FROM zstd_dictionaries ORDER BY version DESC LIMIT 1", ())
        .await?;
    match rows.next().await? {
        Some(r) => Ok(Some((
            r.get::<i64>(0).map_err(|e| StoreError::Db(e.to_string()))?,
            r.get::<Vec<u8>>(1).map_err(|e| StoreError::Db(e.to_string()))?,
        ))),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn insert_then_latest() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        assert!(latest_dictionary(&db).await.unwrap().is_none());
        let id = insert_dictionary(&db, b"dict-bytes-v1", 10).await.unwrap();
        let (lid, bytes) = latest_dictionary(&db).await.unwrap().unwrap();
        assert_eq!(lid, id);
        assert_eq!(bytes, b"dict-bytes-v1");
    }
}
```

Add to `crates/vox-db/src/archive/mod.rs`:

```rust
pub mod dictionary;
```

- [ ] **Step 2: Add `dictionary_bytes` for codec-aware `get()`**

Add to the `impl crate::VoxDb` block in `ops_cas.rs`:

```rust
    /// Fetch a dictionary's raw bytes by id (used to decompress objects compressed under it).
    pub async fn dictionary_bytes(&self, dict_id: i64) -> Result<Vec<u8>, StoreError> {
        let mut rows = self
            .conn
            .query("SELECT bytes FROM zstd_dictionaries WHERE id = ?1", params![dict_id])
            .await?;
        let row = rows
            .next()
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("dictionary {dict_id}")))?;
        row.get::<Vec<u8>>(0).map_err(|e| StoreError::Db(e.to_string()))
    }
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p vox-db archive::dictionary::tests`
Expected: PASS. Then re-run Task 5's test: `cargo test -p vox-db archive_cas_tests` — now PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db/src/archive/dictionary.rs crates/vox-db/src/archive/mod.rs crates/vox-db/src/store/ops_cas.rs
git commit -m "feat(vox-db): zstd_dictionaries accessors + dictionary_bytes for get()"
```

---

### Task 9: Archive pipeline — `archive_window()`

**Files:**
- Create: `crates/vox-db/src/archive/pipeline.rs`
- Modify: `crates/vox-db/src/archive/mod.rs` (add `pub mod pipeline;`)

- [ ] **Step 1: Write the failing test (dedup across two windows)**

Create `crates/vox-db/src/archive/pipeline.rs`:

```rust
//! Archive pipeline: enumerate items → hybrid chunk → dedup+compress → refcount → mark cold (§5.1).

use crate::VoxDb;
use crate::store::StoreError;
use crate::archive::{chunking, compression, dictionary, members, refcount};
use turso::params;

/// Archive a window: dedup + compress all its items into the shared CAS, refcount every
/// referenced object, and mark the window `tier='cold'`. Idempotent per object via CAS.
pub async fn archive_window(db: &VoxDb, window_id: &str, now: i64) -> Result<(), StoreError> {
    // Resolve the dictionary once (None until the first training run).
    let dict = dictionary::latest_dictionary(db).await?;
    let (dict_id, dict_bytes) = match &dict {
        Some((id, b)) => (Some(*id), Some(b.as_slice())),
        None => (None, None),
    };

    // Enumerate item content hashes in order.
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

    for item_hash in item_hashes {
        // The item's raw bytes are already in CAS (added via context_window_store::add_item).
        let content = db.get(&item_hash).await?;
        let chunks = chunking::chunk_content(&content);

        if chunks.len() == 1 {
            // Whole-message item: compress in place under a NEW hash equal to its own content hash.
            let comp = compression::compress(&content, dict_bytes)?;
            let h = db.store_compressed("ctxwin-item", &content, &comp, "zstd", dict_id).await?;
            refcount::incr(db, &h).await?;
        } else {
            let mut chunk_hashes = Vec::with_capacity(chunks.len());
            for chunk in &chunks {
                let comp = compression::compress(chunk, dict_bytes)?;
                let h = db.store_compressed("ctxwin-chunk", chunk, &comp, "zstd", dict_id).await?;
                refcount::incr(db, &h).await?;
                chunk_hashes.push(h);
            }
            members::set_members(db, &item_hash, &chunk_hashes).await?;
        }
    }

    // Mark the window cold.
    let window_id = window_id.to_string();
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "UPDATE context_windows SET tier = 'cold', updated_at = ?2 WHERE id = ?1",
                params![window_id.as_str(), now],
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
    use crate::context_window_store as cws;

    #[tokio::test]
    async fn identical_content_across_windows_dedups() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        let payload = b"shared system prompt ".repeat(8); // < 4KB, whole-message path

        cws::create_window(&db, "w1", "repo", "chat", "w1", 1).await.unwrap();
        cws::add_item(&db, "i1", "w1", 0, "user", "msg", &payload, 1).await.unwrap();
        cws::create_window(&db, "w2", "repo", "chat", "w2", 1).await.unwrap();
        cws::add_item(&db, "i2", "w2", 0, "user", "msg", &payload, 1).await.unwrap();

        archive_window(&db, "w1", 10).await.unwrap();
        archive_window(&db, "w2", 11).await.unwrap();

        // One shared object, refcount == 2.
        let h = crate::hash::content_hash(&payload);
        assert_eq!(refcount::refs_of(&db, &h).await.unwrap(), 2);

        // Reads back losslessly.
        assert_eq!(db.get(&h).await.unwrap(), payload);
    }
}
```

Add to `crates/vox-db/src/archive/mod.rs`:

```rust
pub mod pipeline;
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vox-db archive::pipeline::tests`
Expected: PASS. (`content_hash(&payload)` matches the whole-message compressed object because `store_compressed` hashes the ORIGINAL bytes.)

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/archive/pipeline.rs crates/vox-db/src/archive/mod.rs
git commit -m "feat(vox-db): archive_window pipeline — hybrid dedup + compress + refcount"
```

---

### Task 10: Lossless read/reassembly — `read_item()`

**Files:**
- Modify: `crates/vox-db/src/archive/pipeline.rs` (add `read_item` + test)

- [ ] **Step 1: Write the failing test (large item, multi-chunk reassembly)**

Add to `pipeline.rs`:

```rust
/// Read an archived item's full content: reassemble from chunks (if any) or the single object.
pub async fn read_item(db: &VoxDb, item_hash: &str) -> Result<Vec<u8>, StoreError> {
    let chunk_hashes = members::members_of(db, item_hash).await?;
    if chunk_hashes.is_empty() {
        return db.get(item_hash).await; // whole-message object, get() decompresses
    }
    let mut out = Vec::new();
    for h in chunk_hashes {
        out.extend_from_slice(&db.get(&h).await?); // each chunk decompressed transparently
    }
    Ok(out)
}
```

Add this test to the `tests` module:

```rust
    #[tokio::test]
    async fn large_item_archives_and_reads_back_byte_identical() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        let big: Vec<u8> = (0..120_000).map(|i| (i * 2654435761usize) as u8).collect();

        cws::create_window(&db, "w", "repo", "chat", "w", 1).await.unwrap();
        cws::add_item(&db, "i", "w", 0, "user", "paste", &big, 1).await.unwrap();
        archive_window(&db, "w", 10).await.unwrap();

        let item_hash = crate::hash::content_hash(&big);
        let got = read_item(&db, &item_hash).await.unwrap();
        assert_eq!(got, big, "reassembled archived item must be byte-identical");
    }
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vox-db archive::pipeline::tests::large_item_archives_and_reads_back_byte_identical`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/archive/pipeline.rs
git commit -m "feat(vox-db): read_item reassembles archived items losslessly"
```

---

### Task 11: GC sweep — `sweep_unreferenced()`

**Files:**
- Create: `crates/vox-db/src/archive/gc.rs`
- Modify: `crates/vox-db/src/archive/mod.rs` (add `pub mod gc;`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-db/src/archive/gc.rs`:

```rust
//! Idle GC: delete CAS objects whose refcount has fallen to zero (design §5.4).

use crate::VoxDb;
use crate::store::StoreError;
use turso::params;

/// Delete every object at `cas_refcount.refs = 0` (and its refcount row + chunk_members rows).
/// Returns the number of objects deleted. External-file cleanup is handled by the spill module.
pub async fn sweep_unreferenced(db: &VoxDb) -> Result<i64, StoreError> {
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "DELETE FROM chunk_members WHERE chunk_hash IN (SELECT hash FROM cas_refcount WHERE refs = 0)",
                (),
            )
            .await?;
            conn.execute(
                "DELETE FROM objects WHERE hash IN (SELECT hash FROM cas_refcount WHERE refs = 0)",
                (),
            )
            .await?;
            conn.execute("DELETE FROM cas_refcount WHERE refs = 0", ()).await?;
            Ok::<(), StoreError>(())
        })
        .await?;
    let mut rows = db.conn.query("SELECT changes()", ()).await?;
    let row = rows.next().await?.ok_or_else(|| StoreError::Db("no changes()".into()))?;
    row.get::<i64>(0).map_err(|e| StoreError::Db(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::refcount;

    #[tokio::test]
    async fn shared_object_survives_until_refs_zero() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        db.store("k", b"shared").await.unwrap();
        let h = crate::hash::content_hash(b"shared");
        refcount::incr(&db, &h).await.unwrap();
        refcount::incr(&db, &h).await.unwrap(); // two windows reference it

        refcount::decr(&db, &h).await.unwrap(); // one window deleted
        sweep_unreferenced(&db).await.unwrap();
        assert_eq!(db.get(&h).await.unwrap(), b"shared", "must survive at refs=1");

        refcount::decr(&db, &h).await.unwrap(); // last window deleted
        sweep_unreferenced(&db).await.unwrap();
        assert!(db.get(&h).await.is_err(), "must be GC'd at refs=0");
    }
}
```

Add to `crates/vox-db/src/archive/mod.rs`:

```rust
pub mod gc;
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vox-db archive::gc::tests`
Expected: PASS. (Note: `changes()` reflects the last statement; the assertion only checks survival/deletion via `get`, so the exact return value is not asserted here.)

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/archive/gc.rs crates/vox-db/src/archive/mod.rs
git commit -m "feat(vox-db): GC sweep deletes refs=0 objects + cascades chunk_members"
```

---

### Task 12: Dictionary training — `train_from_corpus()`

**Files:**
- Modify: `crates/vox-db/src/archive/dictionary.rs` (add `train_from_corpus` + test)

- [ ] **Step 1: Write the failing test**

Add to `dictionary.rs`:

```rust
use crate::archive::refcount as _refcount; // for doc-link only

/// Train a new dictionary from the highest-frequency objects and persist it as a new version.
/// Samples up to `max_samples` objects ordered by refcount desc. Returns the new dict id, or
/// `Ok(None)` if there is not enough corpus to train.
pub async fn train_from_corpus(db: &VoxDb, max_samples: usize) -> Result<Option<i64>, StoreError> {
    // Gather the most-referenced object hashes.
    let mut rows = db
        .conn
        .query(
            "SELECT hash FROM cas_refcount ORDER BY refs DESC LIMIT ?1",
            params![max_samples as i64],
        )
        .await?;
    let mut samples: Vec<Vec<u8>> = Vec::new();
    let mut hashes = Vec::new();
    while let Some(r) = rows.next().await? {
        hashes.push(r.get::<String>(0).map_err(|e| StoreError::Db(e.to_string()))?);
    }
    for h in &hashes {
        samples.push(db.get(h).await?); // decompressed content
    }
    if samples.len() < 8 {
        return Ok(None); // zstd needs a reasonable number of samples
    }
    let dict = zstd::dict::from_samples(&samples, 112 * 1024)
        .map_err(|e| StoreError::Db(format!("zstd train: {e}")))?;
    let id = insert_dictionary(db, &dict, samples.len() as i64).await?;
    Ok(Some(id))
}
```

Add this test:

```rust
    #[tokio::test]
    async fn trains_when_enough_samples() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        for i in 0..16 {
            let body = format!("context window archive sample number {i} ").repeat(20);
            db.store("s", body.as_bytes()).await.unwrap();
            let h = crate::hash::content_hash(body.as_bytes());
            super::refcount::incr(&db, &h).await.unwrap();
        }
        let id = train_from_corpus(&db, 64).await.unwrap();
        assert!(id.is_some(), "should train a dictionary from 16 samples");
        assert!(latest_dictionary(&db).await.unwrap().is_some());
    }
```

(Remove the unused `use ... refcount as _refcount;` line if clippy objects; it is only a reminder that frequency comes from refcount.)

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p vox-db archive::dictionary::tests::trains_when_enough_samples`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-db/src/archive/dictionary.rs
git commit -m "feat(vox-db): train zstd dictionary from highest-frequency corpus objects"
```

---

### Task 13: Decompression LRU cache

**Files:**
- Create: `crates/vox-db/src/archive/cache.rs`
- Modify: `crates/vox-db/src/archive/mod.rs` (add `pub mod cache;`)
- Modify: workspace `Cargo.toml` + `crates/vox-db/Cargo.toml` (add `lru`)

- [ ] **Step 1: Add the `lru` dependency**

Workspace `[workspace.dependencies]`:

```toml
lru = "0.12"
```

`crates/vox-db/Cargo.toml` `[dependencies]`:

```toml
lru = { workspace = true }
```

- [ ] **Step 2: Write the failing test**

Create `crates/vox-db/src/archive/cache.rs`:

```rust
//! In-process LRU of decompressed chunk bytes; dedup means one entry serves many windows (§5.2).

use std::num::NonZeroUsize;
use std::sync::Mutex;
use lru::LruCache;

/// A byte-budgeted LRU keyed by content hash. Thread-safe via an internal mutex.
pub struct DecompressionCache {
    inner: Mutex<LruCache<String, Vec<u8>>>,
}

impl DecompressionCache {
    /// Create a cache holding up to `max_entries` decompressed objects.
    pub fn new(max_entries: usize) -> Self {
        let cap = NonZeroUsize::new(max_entries.max(1)).unwrap();
        Self { inner: Mutex::new(LruCache::new(cap)) }
    }

    /// Return a cached copy if present.
    pub fn get(&self, hash: &str) -> Option<Vec<u8>> {
        self.inner.lock().unwrap().get(hash).cloned()
    }

    /// Insert/replace an entry.
    pub fn put(&self, hash: &str, bytes: Vec<u8>) {
        self.inner.lock().unwrap().put(hash.to_string(), bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let c = DecompressionCache::new(2);
        c.put("a", vec![1]);
        c.put("b", vec![2]);
        let _ = c.get("a"); // touch a so b is LRU
        c.put("c", vec![3]); // evicts b
        assert!(c.get("b").is_none());
        assert_eq!(c.get("a"), Some(vec![1]));
        assert_eq!(c.get("c"), Some(vec![3]));
    }
}
```

Add to `crates/vox-db/src/archive/mod.rs`:

```rust
pub mod cache;
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vox-db archive::cache::tests`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db/src/archive/cache.rs crates/vox-db/src/archive/mod.rs Cargo.toml crates/vox-db/Cargo.toml Cargo.lock
git commit -m "feat(vox-db): byte-budgeted LRU decompression cache"
```

---

### Task 14: Full crate gate + clippy + ledger entry

**Files:**
- Modify: `docs/superpowers/antigravity-handoff-ledger.md` (append entry)

- [ ] **Step 1: Run the whole crate test suite**

Run: `cargo test -p vox-db`
Expected: PASS (all archive tests + existing tests, including `baseline_policy_matches_compiled_schema`).

- [ ] **Step 2: Clippy the crate**

Run: `cargo clippy -p vox-db --lib -- -D warnings`
Expected: no warnings. (If the build-broker shim reports `recursion detected`, run the real cargo directly per repo notes; the lint result is what matters.)

- [ ] **Step 3: Append the ledger entry**

Add a new `AGH-00XX` entry (use the next number after the latest in the file) summarizing: archive/dedup/compression engine landed in vox-db on the context-window-spine base; baseline 80→81; hybrid FastCDC chunking; zstd + trained dictionary; codec-aware `get()`; `cas_refcount` + GC; LRU cache; all `cargo test -p vox-db` green. Note Plan B (GUI/search/wiring) and Phase 2 (embeddings, hygiene gate, dead-table sweep) as follow-ons.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/antigravity-handoff-ledger.md
git commit -m "docs(ledger): record archive/dedup/compression engine (AGH-00XX)"
```

---

## Self-Review

**Spec coverage:**
- §2 tier model (Hot/Cold/Frozen) — Cold path is Tasks 9-10; `tier='cold'` set in Task 9. **Frozen spill is intentionally deferred** (relocation of compressed bytes to the external file store) — it is a small follow-up; the `objects.storage`/`file_path` columns exist (Task 2) so the schema is ready. Noted as a gap to add in Plan B or a Frozen-spill micro-plan.
- §3 hybrid dedup — Task 3 (chunking) + Task 9 (pipeline applies threshold).
- §4 data model — Task 2 (all tables/columns); §4.5 reuse — embeddings/search/processing_runs are Plan B / Phase 2.
- §5.1 archive pipeline — Task 9; §5.2 read + cache — Tasks 10, 13; §5.4 GC — Task 11; §5.5 dictionary — Task 12. **§5.1 background via `processing_runs` is Plan B** (this plan provides the synchronous `archive_window` the worker will call).
- §6 hygiene — §6.1 self-audit is realized (only 3 new tables). §6.2/§6.3/§6.4 (prune policy, named candidates, standing gate) are **Phase 2**, correctly out of this engine plan.
- §10 tests — covered per task (dedup, round-trip, GC safety, compression). Frozen-spill + search tests are deferred with their features.

**Placeholder scan:** No "TBD"/"handle errors"/"similar to" — every code step is complete. The one deferred item (Frozen spill) is explicitly flagged, not hidden.

**Type consistency:** `store_compressed(kind, original, stored, codec, dict_id)` is defined in Task 5 and called identically in Task 9. `refcount::{incr,decr,refs_of}`, `members::{set_members,members_of}`, `dictionary::{insert_dictionary,latest_dictionary,train_from_corpus}`, `compression::{compress,decompress}`, `chunking::chunk_content`, `dictionary_bytes` — all names match across defining and calling tasks. `crate::test_support::memory_db()` is assumed to be the crate's existing in-memory test helper; if its path differs, mirror the exact helper used in `context_window_store.rs`'s tests (Task 5 Step 1 notes this).

**Known cross-task ordering:** Task 5's `get()` references `dictionary_bytes` (Task 8) and `compression::decompress` (Task 4); implement Tasks 3-4 and 8 before running Task 5's test (noted inline).
