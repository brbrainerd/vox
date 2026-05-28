---
title: "Populi Mesh — A2A Durability Spec (S1, 2026-05-01)"
description: "SUPERSEDED design spec for the SQLite/rusqlite mesh store. The shipped implementation uses VoxDb instead. Retained for historical context only."
category: "Architecture SSOTs"
status: "deprecated"
training_eligible: false
---

# Populi Mesh — A2A Durability (S1 child spec)

> **⚠️ SUPERSEDED.** This document describes the original SQLite/rusqlite `mesh.db` design that was considered during S1 planning. The shipped implementation uses **VoxDb** as the durable backing store for the A2A inbox, exec leases, and dispatch results. See [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md) and the `MeshStore` trait in `crates/vox-populi/src/transport/store/` for the canonical contract. The `store migrate/check/repair` commands described below were never implemented.
>
> This page is retained for historical context. Do not use it as implementation guidance.

**Parent.** [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md), Slice S1, Workstream W6 partial (also feeds W1 lease persistence in S2).

**Goal.** Make A2A inbox / exec-lease / dispatch-results storage durable enough that a `kill -9` during a write doesn't corrupt the inbox, two `vox populi serve` processes against the same store directory don't silently clobber each other, and the schema can evolve without manual file surgery.

**Non-goals.**
- Picking a long-term *ideal* backend — sled, fjall, redb, sqlite all merit consideration. This spec recommends **sqlite via rusqlite** for pragmatic reasons (§2.2). A future ADR can change it.
- Distributed consistency — that's W1 leases (S2).
- Retention / TTL — backlog `MESH-153`, not S1.

---

## Part 1 — Current state

[`vox-populi/src/transport/store.rs`](../../../crates/vox-populi/src/transport/store/mod.rs) maintains three independent JSON files:

- `a2a-store.json` — `Vec<A2AStoredMessage>`
- `exec-lease-store.json` — `Vec<RemoteExecLeaseRow>`
- `dispatch-store.json` — `HashMap<String, DispatchResponse>`

Path resolution (Clavis secret > sibling-of-registry > local-registry-default). Persistence is **load entire file → mutate in memory → write tmp → rename**. Atomicity per file but no `fsync` on the directory; no concurrency control beyond the in-process locks held by the caller; no schema version on disk.

**What's wrong.**
1. **No fsync.** `std::fs::write` followed by `std::fs::rename` is atomic at the rename step but neither file nor directory is fsynced. A power loss between rename and journal flush loses the write.
2. **No multi-process safety.** Two `vox populi serve` processes pointing at the same directory both load, both mutate, both rewrite — last writer wins, no warning.
3. **Quadratic writes.** Every `persist_*` call rewrites the entire file. With a few thousand messages, this is single-digit-ms; with a few hundred thousand, it dominates request latency.
4. **No schema version.** Adding a field to `A2AStoredMessage` works because of `serde(default)`, but removing or renaming requires hand-editing every operator's file.
5. **No corruption detection.** A truncated JSON file fails `serde_json::from_str` and the server refuses to start. There's no "I noticed this is corrupt, here's the last good snapshot" recovery.

---

## Part 2 — Design

### 2.1 The `MeshStore` trait

```rust
// vox:skip
pub trait MeshStore: Send + Sync {
    fn put_a2a(&self, msg: A2AStoredMessage) -> Result<(), MeshStoreError>;
    fn list_a2a(&self, page: A2APage) -> Result<Vec<A2AStoredMessage>, MeshStoreError>;
    fn ack_a2a(&self, message_id: &str, ack: A2AAck) -> Result<(), MeshStoreError>;

    fn put_exec_lease(&self, row: RemoteExecLeaseRow) -> Result<(), MeshStoreError>;
    fn list_exec_leases(&self) -> Result<Vec<RemoteExecLeaseRow>, MeshStoreError>;
    fn revoke_exec_lease(&self, lease_id: &str) -> Result<(), MeshStoreError>;

    fn put_dispatch_result(&self, key: &str, value: DispatchResponse)
        -> Result<(), MeshStoreError>;
    fn get_dispatch_result(&self, key: &str)
        -> Result<Option<DispatchResponse>, MeshStoreError>;

    /// Atomic across the three concerns where the caller needs it
    /// (e.g., put A2A row + put dispatch result).
    fn transaction<R>(
        &self,
        f: impl FnOnce(&dyn MeshStoreTxn) -> Result<R, MeshStoreError>,
    ) -> Result<R, MeshStoreError>;

    fn schema_version(&self) -> u32;
    fn integrity_check(&self) -> Result<IntegrityReport, MeshStoreError>;
}

pub trait MeshStoreTxn { /* same operations, run inside a transaction */ }

#[derive(Debug, thiserror::Error)]
pub enum MeshStoreError {
    #[error("io: {0}")]
    Io(String),
    #[error("schema mismatch: store is v{stored}, code expects v{expected}")]
    SchemaMismatch { stored: u32, expected: u32 },
    #[error("locked by another process")]
    LockContention,
    #[error("corrupt: {0}")]
    Corrupt(String),
    #[error("other: {0}")]
    Other(String),
}
```

### 2.2 sqlite implementation

`vox-populi/src/transport/store/sqlite.rs`:

- One database file per Populi instance, default `local_registry_path().with_file_name("mesh.db")`.
- WAL mode (`PRAGMA journal_mode=WAL;`) for concurrent reader safety.
- `PRAGMA synchronous=FULL` for durability (this is a power-user setting; the speed cost is acceptable at S1 scale).
- Single writer enforced via `BEGIN IMMEDIATE` on writes; readers don't block.
- File-lock advisory check on open: if another process holds the lock, return `LockContention` rather than racing.

**Schema (v1).**

```sql
CREATE TABLE schema_version (version INTEGER NOT NULL);

CREATE TABLE a2a_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id TEXT NOT NULL UNIQUE,
    sender_agent_id TEXT NOT NULL,
    receiver_agent_id TEXT NOT NULL,
    message_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    idempotency_key TEXT,
    idempotency_dedupe_key TEXT,
    privacy_class TEXT,
    payload_blake3_hex TEXT,
    worker_ed25519_sig_b64 TEXT,
    jwe_payload TEXT,
    priority INTEGER NOT NULL,
    task_kind TEXT,
    model_id TEXT,
    created_at INTEGER NOT NULL,        -- unix millis
    acked_at INTEGER,
    schema_version INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_a2a_receiver ON a2a_messages(receiver_agent_id, acked_at);
CREATE INDEX idx_a2a_idempotency ON a2a_messages(idempotency_key) WHERE idempotency_key IS NOT NULL;

CREATE TABLE exec_leases (
    lease_id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    holder_node_id TEXT NOT NULL,
    granted_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    state TEXT NOT NULL,                -- 'granted'|'renewed'|'expired'|'revoked'|'completed'
    metadata_json TEXT,
    schema_version INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_lease_task ON exec_leases(task_id);
CREATE INDEX idx_lease_state ON exec_leases(state, expires_at);

CREATE TABLE dispatch_results (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1
);
```

The `state` column on `exec_leases` is intentionally pre-introduced even though full lease lifecycle is S2: this lets S1 ship a schema that S2 doesn't have to migrate.

### 2.3 In-memory implementation (test-only)

`vox-populi/src/transport/store/in_memory.rs`, `pub(crate)` under `#[cfg(test)]`. Identical trait surface; no persistence; behaves identically except for `LockContention` (never returned). All existing tests that touch the JSON store get rewritten against this.

### 2.4 Migration tool

`vox populi store migrate` — reads the existing three JSON files, validates them, writes a new `mesh.db` next to them, renames the JSON files to `*.json.migrated`. Idempotent: re-running with a populated `mesh.db` is a no-op with a warning.

If the operator runs `vox populi serve` and detects only legacy JSON files in the store directory, the server refuses to start with a clear message: `"Found legacy JSON store; run 'vox populi store migrate' to migrate to the durable backend."`

### 2.5 Integrity check

`vox populi store check` — opens the database read-only, runs `PRAGMA integrity_check;`, validates schema version, validates that every A2A message with a non-null `idempotency_dedupe_key` has at most one row with that key. Returns a structured report.

`vox populi store repair` — for now, restores the most recent SQLite WAL checkpoint and reports orphaned rows; it does *not* perform destructive recovery automatically. Aggressive repair is a follow-on backlog item (`MESH-152`).

### 2.6 Serialization compatibility

The on-disk row format is the existing Rust `A2AStoredMessage`/`RemoteExecLeaseRow`/`DispatchResponse` serialized to TEXT columns where they're complex (e.g., `metadata_json`). The dedicated columns (sender, receiver, etc.) are denormalized for indexing but the serialized form remains the source of truth — schema-additive changes to the structs work as long as `serde(default)` covers them.

### 2.7 Performance characteristics

Single-row insert in WAL mode is dominated by `synchronous=FULL` fsync; ~1–5 ms on consumer SSDs, which is fine for the dispatch path. Reads are well under 1 ms for indexed lookups. The current JSON-file pattern hits ~10–50 ms once the file grows past a megabyte, so this is faster on average even with synchronous=FULL.

If a contributor benchmarks and shows that dispatch-result writes dominate latency, the `dispatch_results` table can be moved to `synchronous=NORMAL` independently — backlog item.

### 2.8 Observability

Each store operation emits a span:
- `vox.mesh.store.op` (`put_a2a`, `list_a2a`, `ack_a2a`, …)
- `vox.mesh.store.duration_ms`
- `vox.mesh.store.row_count` (for list ops)
- `vox.mesh.store.error` (only on failure)

A periodic gauge exports `vox.mesh.store.size_bytes` and `vox.mesh.store.row_count` per table.

---

## Part 3 — Test plan

### 3.1 Unit tests (new `crates/vox-populi/src/transport/store/tests.rs`)

- `in_memory_a2a_round_trip` — put → list → ack → list excludes acked.
- `in_memory_idempotency` — duplicate `idempotency_key` returns same `message_id`.
- `in_memory_pagination` — `A2APage { since, limit }` returns rows in stable order.
- `in_memory_lease_state_transitions` — granted → renewed → expired/revoked/completed (legal transitions only at the trait surface; semantics tested by S2).
- `in_memory_dispatch_get_or_none` — get on missing key returns `None`.
- `in_memory_transaction_rollback` — error inside `transaction` rolls back all changes.

### 3.2 SQLite-specific integration tests (`tests/store_sqlite.rs`)

- `fresh_db_starts_at_v1` — schema version 1 after `connect_or_init`.
- `concurrent_writers_serialize` — two threads writing 1000 messages each; final count is 2000, no duplicates.
- `lock_contention_returns_error` — open from process A, attempt open from process B (using a child process), assert `LockContention`.
- `crash_during_write_recovers` — simulate kill mid-transaction (use a second process + `Drop` impl that aborts the transaction); verify on next open the store is consistent.
- `wal_checkpoint_runs_periodically` — confirm the WAL doesn't grow unbounded under sustained writes.
- `integrity_check_clean_db_passes` — after a normal session, `vox populi store check` passes.
- `integrity_check_detects_dedupe_violation` — manually insert two rows with the same `idempotency_dedupe_key`; check fails with structured diagnostic.

### 3.3 Migration tests (`tests/store_migration.rs`)

- `migrate_empty_directory_creates_v1_db` — no-op happy path.
- `migrate_three_legacy_files_succeeds` — fixture directory with all three legacy JSON files, post-migration the SQLite store has the same data and the JSON files are renamed.
- `migrate_idempotent` — running migrate twice is safe.
- `serve_refuses_legacy_only` — start `vox populi serve` against a directory with only legacy JSON files; confirm structured refusal.

### 3.4 Contract tests (rewrite of existing `dispatch_persistence.rs`)

The existing [`crates/vox-populi/tests/dispatch_persistence.rs`](../../../crates/vox-populi/tests/dispatch_persistence.rs) gets a sibling `dispatch_persistence_sqlite.rs` that runs the same scenarios against the SQLite backend, parameterized on `Box<dyn MeshStore>`.

---

## Part 4 — Acceptance criteria

1. `MeshStore` trait shipped with both `sqlite` and `in_memory` implementations.
2. All existing transport tests pass against both backends.
3. `vox populi store migrate` migrates a populated legacy directory to SQLite with byte-equivalent A2A data.
4. `vox populi store check` reports clean for a healthy DB and structurally for any inconsistency.
5. Two `vox populi serve` processes against the same store directory: one wins, the other gets a clear error.
6. A `kill -9` during a write loses *that* write but leaves the store consistent (no orphan rows, no corruption).
7. `vox.mesh.store.*` spans appear on every operation.
8. Backlog items closed: `MESH-005`, `MESH-012`, `MESH-013`, `MESH-151`, `MESH-152` (partial — `check` only, repair deferred), `MESH-154`–`MESH-156`, `MESH-158`.

---

## Part 5 — Out-of-scope items

- **Lease lifecycle semantics** — S2 (`populi-mesh-leases-spec`).
- **Retention / TTL** — backlog `MESH-153`.
- **Aggressive repair** — backlog `MESH-152` (extension).
- **Backend choice ADR** — separate ADR if/when sqlite proves insufficient.
- **Cross-node replication** — explicitly out of scope at any slice.

---

## Part 6 — Rough cost

- New trait + sqlite backend: ~600 LOC, depends on `rusqlite` (already a workspace dependency? if not, single new dep).
- In-memory backend: ~250 LOC.
- Migration tool: ~200 LOC.
- Tests: ~800 LOC across unit / integration / migration.
- CLI plumbing for `store migrate` / `store check`: ~150 LOC.

Total: ~2000 LOC, plus possibly one new dep.

---

## Revision history

- **2026-05-01.** Initial S1 child spec.
