---
title: "Archive → Dedup → Compress → Mine (Context-Window Layer D deep-dive)"
category: "Architecture SSOTs"
status: draft
date: 2026-06-20
supersedes_section: "Layer D of docs/superpowers/specs/2026-06-20-context-window-management-design.md"
---

# Archive → Dedup → Compress → Mine

A concrete, buildable design for **Layer D** of the context-window spine: persistent
deduplication and compression of chat sessions, triggered manually on archive, with the
deduped corpus mined so the system keeps learning and searching despite compression.

This spec **refines and stays consistent with** the ContextWindow / CAS model in
`2026-06-20-context-window-management-design.md`. It does not introduce a parallel store.

## 1. Goals & non-goals

**Goals**
- Manual, per-session **archive** in the chat tab is the *sole* trigger for compression.
  CPU is paid once, deliberately (compression *and* future decompression are not free).
- **Deduplicate within a single archive and across the entire history**, for storage *and* search.
- Keep archived content **searchable and mineable** (the system must still learn from it).
- Scale: huge cold corpus must not bloat the live database or slow hot-path access.
- **Improve database hygiene** as part of the 80→81 baseline bump (see §6).

**Non-goals (v1)**
- Passive/automatic compression of live sessions (only explicit archive compresses).
- Lossy summarization on archive (archival is **lossless**; summarization is a separate concern).
- A full dead-table sweep of the existing schema (a *policy* + named candidates only — §6).

## 2. Tier model

A ContextWindow occupies one of three effective states. (This collapses the spine's
Hot/Warm/Cold/Frozen into what archival actually needs.)

| State | Bytes | Location | Transition trigger |
|-------|-------|----------|--------------------|
| **Hot** | raw, uncompressed | live tables (`conversation_messages`, window items) | default — live session |
| **Cold** | deduped + zstd-compressed | VoxDB CAS `objects` BLOBs | **manual archive** (chat tab) |
| **Frozen** | same compressed chunks, relocated | external git-style CAS file store | automatic: age ≥ 90d **or** live-db cold-bytes > 2 GB |

- **Archive (Hot→Cold)** is explicit and user-initiated. It is the only compression pass.
- **Unarchive (Cold→Hot)** rehydrates a live window by decompressing + reassembling chunks.
  The Cold copy is **retained** (re-archive is then near-free; chunks already stored).
- **Frozen** is *relocation only* of already-compressed bytes from VoxDB BLOBs to the external
  file store — not a second compression pass. It keeps the live `.db` bounded and hands off
  cleanly to the D/C/X storage-tiering mover.

**Why dedup-across-all-history is automatic:** every archive writes chunks into the *one shared*
content-addressed CAS keyed by hash. A chunk already stored by any prior archive (same session
or six months ago) is deduped on `INSERT OR IGNORE`. Within-archive repetition collapses by the
same mechanism.

## 3. Dedup granularity (hybrid)

- **Small items (< 4 KB): whole-message CAS.** The item *is* one object; its hash is the content hash.
- **Large items (≥ 4 KB): FastCDC content-defined chunking.** Rolling-hash boundaries, target
  ~8 KB (range 2–16 KB). Each chunk is CAS-stored; the item records its ordered chunk hashes.
  This catches partial overlap anywhere — a file pasted into many sessions dedups even when
  surrounded by different prose.

Rationale: small turns gain nothing from chunking overhead; the real cross-history savings live
in large pastes, tool outputs, and file dumps, which FastCDC captures.

## 4. Data model

All additions extend the existing CAS (`objects`) and the `context_windows` /
`context_window_items` spine. **Net-new tables are deliberately minimized** (see §6 hygiene):
after self-audit, only three new tables plus column additions.

### 4.1 Extend `objects` (CAS)

```sql
-- expressed as ALTERs for readability; implemented by editing the cas_codex baseline fragment
ALTER TABLE objects ADD codec            TEXT    NOT NULL DEFAULT 'none';  -- none | zstd
ALTER TABLE objects ADD dict_id          INTEGER;                          -- FK zstd_dictionaries.id, nullable
ALTER TABLE objects ADD uncompressed_len INTEGER NOT NULL DEFAULT 0;
ALTER TABLE objects ADD storage          TEXT    NOT NULL DEFAULT 'inline';-- inline | file
ALTER TABLE objects ADD file_path        TEXT;                             -- set when storage='file'
```

`data BLOB` holds compressed bytes when `codec='zstd'`; `NULL` when `storage='file'`
(bytes live in the external file store at `file_path`). `dict_id` is immutable per object —
an object compressed under dict v3 keeps `dict_id=3` forever.

### 4.2 New: `chunk_members` (reassembly)

```sql
CREATE TABLE IF NOT EXISTS chunk_members (
    item_hash  TEXT NOT NULL,
    ordinal    INTEGER NOT NULL,
    chunk_hash TEXT NOT NULL REFERENCES objects(hash),
    PRIMARY KEY (item_hash, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_chunk_members_chunk ON chunk_members(chunk_hash);
```

Maps a large (FastCDC-split) item to its ordered chunk hashes. Small whole-message items have
**no rows** here (the item is the object). This is the only read-time indirection, and only for
large items.

### 4.3 New: `cas_refcount` (GC) — also mandated by the spine audit

```sql
CREATE TABLE IF NOT EXISTS cas_refcount (
    hash TEXT PRIMARY KEY REFERENCES objects(hash),
    refs INTEGER NOT NULL DEFAULT 0
);
```

Authoritative reference count for **all** CAS objects (messages and chunks). Dedup means many
windows share one object — deletion is only safe at `refs = 0`. **This table also serves the
mining "frequency" signal**: "what your work repeats most" is `SELECT hash FROM cas_refcount
ORDER BY refs DESC`. (This is why no separate `chunk_stats` table exists — see §6.)

### 4.4 New: `zstd_dictionaries`

```sql
CREATE TABLE IF NOT EXISTS zstd_dictionaries (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    version      INTEGER NOT NULL,
    bytes        BLOB    NOT NULL,
    sample_count INTEGER NOT NULL DEFAULT 0,
    trained_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    notes        TEXT
);
```

Versioned dictionaries trained from the corpus. Refcounted implicitly via `objects.dict_id`;
never deleted while any object references them.

### 4.5 Reused (no new tables)

- **Embeddings:** the existing `embeddings` table (`knowledge.rs`) — populated by the Phase-2
  mining job, keyed by chunk/item hash.
- **Keyword/semantic search index:** the existing `search_documents` / `search_document_chunks`
  / `search_indexing_jobs` pipeline, surfaced via a **new `ContextArchive` `SearchCorpus`
  variant** (no new FTS table — see §6).
- **Background work:** the existing `processing_runs` / `processing_run_steps` tables drive the
  archive job (no new queue table).

## 5. Pipelines

### 5.1 Archive (Hot → Cold), background

Archiving enqueues a `processing_runs` row (kind `context_archive`); a worker runs the steps,
the window shows `archiving… → archived`, and the job is observable + retryable. Steps:

1. **Enumerate** the window's items (`context_window_items` → `content_hash` → `objects`).
2. **Chunk** each item: `< 4 KB` keep whole-message hash; `≥ 4 KB` FastCDC → ordered chunk
   hashes; write `chunk_members` rows.
3. **Dedup**: `INSERT OR IGNORE` each message/chunk object; for every referenced hash, upsert
   `cas_refcount` (`refs = refs + 1`). Objects already present are already compressed — skip.
4. **Compress** each *new* object: zstd (level ~19) under the current dictionary; set
   `codec='zstd'`, `dict_id`, `uncompressed_len`.
5. **Index for search**: enqueue/refresh `search_document_chunks` rows (decompressed text) under
   the `ContextArchive` corpus; (Phase 2) enqueue embedding jobs.
6. **Mark** the window `tier='cold'`, set `archived_at`, emit `vox://context-archived`.

### 5.2 Read (Cold/Frozen)

Resolve item → if `chunk_members` rows exist, fetch each chunk in `ordinal` order, else fetch the
single object → if `storage='file'` read the external file → zstd-decompress with `dict_id` →
reassemble. Never `SELECT *` a window — page items lazily (glogg-scale virtualization, per spine).

**Decompression cache:** an in-process LRU of decompressed chunks (~256 MB cap), keyed by hash.
Because dedup is global, one hot chunk serves many windows — overlapping reads hit the warm cache.

### 5.3 Unarchive (Cold → Hot)

Rehydrate items into live tables; flip `tier='hot'`. Cold chunks remain stored and refcounted,
so re-archiving is near-free.

### 5.4 GC sweep (idle)

Scheduled idle pass: delete objects at `cas_refcount.refs = 0` together with their
`search_document_chunks`, embeddings, and external file (if any). Refcount is **decremented on
window hard-delete/trim**, never inline with a user delete (so deletes stay fast and the sweep is
batchable). Dictionaries that become unreferenced (`no objects.dict_id = id`) are eligible too.

### 5.5 Dictionary training (idle)

Every 50 archives **or** weekly idle: sample top-frequency objects (`cas_refcount ORDER BY refs
DESC`) → train a new zstd dictionary version → future archives use it. Existing objects keep
their `dict_id`; old dictionaries are retained while referenced.

## 6. Database hygiene (80 → 81)

The archive feature already bumps `BASELINE_VERSION` 80 → 81 (and updates the
`contracts/db/baseline-version-policy.yaml` digest + integer). We use that bump as the occasion
for **conservative, evidence-based** hygiene — not a blind sweep.

### 6.1 Self-audit (applied in this spec)

Before adding tables we audited our own additions against existing infrastructure and removed
needless complexity:

- **Removed `chunk_stats`** — its `ref_count` duplicated `cas_refcount.refs` and its timestamps
  duplicated `objects.created_at`. The dedup refcount *is* the frequency signal. (−1 table, −1 write path.)
- **Removed `archive_fts`** — reuse `search_document_chunks` + a `ContextArchive` `SearchCorpus`
  variant instead of a parallel FTS table. (−1 table.)
- **Reused `embeddings`, `processing_runs`, `search_*`** rather than new equivalents.

Net new: **3 tables** (`chunk_members`, `cas_refcount`, `zstd_dictionaries`) + `objects` columns.

### 6.2 Prune policy (safe by construction)

Dropping a table from the monolithic, content-addressed baseline is high-blast-radius: existing
databases retain the table (no migration drops it), and the baseline digest changes. Therefore:

- The 80→81 bump is **additive + opportunistic-prune only for tables proven dead with high
  confidence** (no typed accessor, no raw-SQL reference in *any* crate, no contract/test reference).
- Verification for a drop candidate = repo-wide search for the table name across `crates/**`,
  `contracts/**`, and `*.sql`, plus a check that no `SearchCorpus`/projection names it. A candidate
  that passes is removed from the baseline fragment **and** given an idempotent `DROP TABLE IF
  EXISTS` in an additive cleanup fragment (so existing DBs converge), behind a test.

### 6.3 Named candidates to investigate (NOT auto-dropped here)

High-confidence *smells* surfaced during the schema scan, to be verified under §6.2 before any drop:

- **`news_publish_approvals` vs `news_publish_approvals_v2`** — a v1/v2 pair; one is likely
  superseded.
- **`a2a_messages` (coordination.sql) vs `mesh_a2a_messages` (vox_mesh.rs)** — two A2A message
  tables; possible split-brain/overlap.
- Tables defined in `SCHEMA_FRAGMENTS` with no typed accessor in `vox-db` and no reference
  elsewhere (candidate list produced by the §6.4 gate).

### 6.4 Ongoing gate (optional follow-up, not Phase 1)

A `vox ci db-hygiene` check (or unit test) that flags any table in `SCHEMA_FRAGMENTS` never
referenced by a typed accessor or raw-SQL site, turning hygiene into a standing gate rather than a
one-time cleanup. Proposed as a follow-up so it does not block the archive feature.

## 7. Phasing

- **Phase 1** — tier model, hybrid dedup (message-CAS + FastCDC), zstd + trained dictionary,
  VoxDB-blob storage + Frozen spill, decompression cache, `cas_refcount` + GC sweep, keyword
  search via `ContextArchive` `SearchCorpus`, background archive via `processing_runs`,
  chat-tab archive/unarchive controls, the §6.1 self-audit additions, and the 80→81 bump.
- **Phase 2** — embeddings over deduped chunks (revive the dormant `embeddings` path) for
  semantic recall + the Graphify join; the §6.4 hygiene gate; a full §6.2 dead-table sweep.

Embeddings are Phase 2 because that path is currently dormant in the codebase; isolating it keeps
dedup/compression/search shippable without being blocked on reviving it.

## 8. Scope of the v1 trigger

The chat-tab archive control operates on **GUI chat sessions** (`conversations`). The underlying
pipeline operates on any `ContextWindow`, so agent-context and tab windows adopt archival later
with no redesign.

## 9. Repo-specific build notes

- **Baseline bump:** edit the `cas_codex` baseline fragment + add the new fragments; set
  `BASELINE_VERSION = 81`; regenerate and update `contracts/db/baseline-version-policy.yaml`
  (`repository_baseline_integer` + `repository_baseline_digest_hex`). The
  `baseline_policy_matches_compiled_schema` test enforces this.
- **Crates:** `zstd` (with dictionary API) and a FastCDC implementation (`fastcdc`) — both
  pure-Rust, cross-platform; verify license + `cargo deny` before adding.
- **`SearchCorpus::ContextArchive`** touches the ~3 known match sites
  (`orchestrator/.../goal.rs`, `vox-gui/src/commands/search.rs`, `vox-search/src/execution.rs`).
- **Store accessors** follow the `context_window_store.rs` / `history_store.rs` breaker pattern
  (`db.breaker.clone()` + `db.conn.clone()` + `breaker.call(...)`).
- **Reactivity:** `vox://context-archived` (+ existing `vox://context-*`) for GUI refresh.

## 10. Test strategy

- **Dedup correctness:** identical content across two archives stores one object; `cas_refcount`
  reaches 2; total stored bytes ≈ one copy. FastCDC: a large blob pasted into two windows with
  different surrounding text shares interior chunks.
- **Round-trip:** archive → unarchive yields byte-identical items (lossless), including
  multi-chunk reassembly order.
- **Compression:** `uncompressed_len` exact; decompress under the recorded `dict_id` succeeds
  across dictionary versions.
- **GC safety:** an object referenced by two windows survives one window's deletion; drops only at
  `refs = 0`; sweep also removes search/embedding/file rows.
- **Frozen spill:** a cold chunk past threshold relocates to the file store (`storage='file'`,
  `data IS NULL`) and still reads back identically.
- **Search:** archived text is findable via the `ContextArchive` corpus without decompressing the
  window.
- **Baseline:** `baseline_policy_matches_compiled_schema` green at version 81.

## 11. Open questions

None blocking. Dictionary-training sample size and the exact Frozen thresholds are config-driven
and tunable post-landing.
