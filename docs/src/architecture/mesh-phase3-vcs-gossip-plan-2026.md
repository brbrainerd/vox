---
title: "Mesh Phase 3 — Multi-agent VCS over mesh (op-log gossip) Implementation Plan"
description: "TDD-style implementation plan for Phase 3 of the mesh & language-distribution SSOT: durable op-log persistence in vox-db, signed capability mints and op-fragments, Bloom-filter anti-entropy gossip, vector-clock affinity, lock-wait outcome, sealed-trait hardening, raw-git arch-check rule, unknown-parent backfill, and op-log-as-projection-source. Nine tasks (P3-T1..P3-T9), expected ~9 PRs."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. Spec/SSOT is the durable artifact."
---

# Mesh Phase 3 — Multi-agent VCS over mesh (op-log gossip) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal.** Make the convergence op-log durable in `vox-db`, gossip-replicated between daemons via Bloom-filter anti-entropy, and the single source of mesh state. Capability mints and op-fragments are signed with daemon-issued Ed25519 keys. Two daemons can drive the same repository concurrently, with locks/affinity/capabilities/kudos all becoming **projections** rebuilt from the op-log on restart.

**Killer feature.** Mesh-distributed multi-agent code editing with no data loss and no central server: any daemon can crash, restart, and catch up by gossiping with peers, with the lock-leader (Phase 0) breaking ties for write-side conflicts.

**Architecture.** Three structural moves:

1. **Persistent tiered op-log.** Hot 10K entries in `VecDeque`; warm in `convergence_op_log` rows in `vox-db`; cold compacted to `Checkpoint` operations encoding projection state. The `OperationEntry` predecessor hash chain is preserved; SHA3-256 graduates to a BLAKE3 signature payload over `(op_id_be ‖ predecessor_hash ‖ payload_blake3)`.
2. **Signed wire format.** Every capability mint and every op-fragment carries an Ed25519 signature from the daemon's `vox-secrets`-issued key. Verifier looks up daemon public keys from a peer-keyring loaded out of `Vox.toml [mesh.trust]`. A new `Sealed` trait (in a tiny inner facade crate) makes mint methods `pub(crate)` so cross-crate forgery becomes a type error, not an audit miss.
3. **Bounded gossip + projection trait.** A new `OpFragmentSync` A2A message kind sweeps every 30s with Bloom-of-seen-op-ids → reply-with-missing-op-ids. Demers et al. epidemic algorithm. Unknown-parent fragments queue in a bounded backfill buffer (1024 entries / 64 KiB) with DLQ to vox-db. Locks, affinity, capabilities, and kudos all `impl Projection` and rebuild from the log on startup.

**Tech stack.** Rust 2024 edition. `vox-crypto` (Ed25519, BLAKE3, SHA3-256 — no new crypto deps). `vox-db` (SQLite via existing migrations). `tokio`, `tracing`, `thiserror`, `serde` already in workspace. **No** new external deps.

**SSOT.** [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) §3 Phase 3.

**Spec.** [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md) §Wire-protocol (`MergeOutcome`, op-fragment shape).

**Hopper integration.** This phase lands `Hp-T4` (DeveloperOverride sealed-mint, bundled with
P3-T6) and `Hp-T5` (HopperInboxProjection, bundled with P3-T9). See SSOT §3.5 and
[unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md).

**Predecessor plans.**

- Phase 1 implementation lives in [`multi-agent-vcs-replication-impl-plan-phase1-2026.md`](multi-agent-vcs-replication-impl-plan-phase1-2026.md). Where this plan overlaps that one, we cite tasks rather than restating substeps.
- Phase 0 (vox-db substrate, lock-leader election) and Phase 2 (`DurablePromise` semantics) are prerequisites.

**Anti-goals.** No blockchain. No consensus protocol (we use lock-leader from Phase 0 for write-side races). No custom signature scheme — Ed25519 from `vox-crypto` only.

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\zealous-ardinghelli-b01e11`. All paths below are relative to this worktree.

---

## File map

**Migration policy note.** Per SSOT §5.5 canonical migration policy: schema evolution flows through `BASELINE_VERSION` in `crates/vox-db/src/schema/manifest.rs`. P3 takes baseline from 63 (P2's value) to 64 (this phase, for `convergence_op_log` + `convergence_op_log_backfill_dlq`). The earlier draft of this plan proposed a `0042_convergence_op_log.sql` file under `crates/vox-db/migrations/` — that scheme is rejected per §5.5.

**Create:**

- `crates/vox-orchestrator-queue/src/oplog/persist.rs` — append/read/checkpoint helpers against vox-db.
- `crates/vox-orchestrator-queue/src/oplog/checkpoint.rs` — `OperationKind::Checkpoint` encoding + replay scaffold.
- `crates/vox-orchestrator-queue/src/oplog/sign.rs` — signature payload, sign/verify glue against `vox-crypto`.
- `crates/vox-orchestrator-queue/src/projection.rs` — `Projection` trait + `ProjectionRegistry`.
- `crates/vox-orchestrator-queue/src/projections/locks.rs` — locks-as-projection.
- `crates/vox-orchestrator-queue/src/projections/affinity.rs` — affinity-as-projection.
- `crates/vox-orchestrator-queue/src/projections/capabilities.rs` — capability-mint-log-as-projection.
- `crates/vox-orchestrator-queue/src/projections/kudos.rs` — kudos-as-projection.
- `crates/vox-orchestrator-cap-mint/Cargo.toml` and `src/lib.rs` — sealed-trait facade crate (the only crate impling `Sealed`).
- `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs` — `OpFragmentSync` message kind, Bloom encode/decode, sweep loop.
- `crates/vox-orchestrator-queue/src/oplog/backfill.rs` — bounded unknown-parent hold queue + DLQ writer.
- `crates/vox-orchestrator-queue/tests/golden_5agent_conflict.rs` — Phase 3 acceptance test (5 agents, two daemons, forced conflict).
- `crates/vox-orchestrator-queue/tests/projection_replay.rs` — replay-bit-identicality test.
- `crates/vox-arch-check/src/forbidden_patterns.rs` — new `[[forbidden_pattern]]` rule type implementation.
- `tests/fixtures/arch-check/raw-git-positive.rs` and `raw-git-negative.rs` — fixtures for the new rule.
- `scripts/phase3-replay-smoke.vox` — VoxScript replay smoke driver (no `.ps1` / `.sh` per AGENTS.md).

**Modify:**

- `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` from 63 (set by P2-T5) to 64; add `convergence_op_log` + `convergence_op_log_backfill_dlq` schema fragments gated on version 64.
- `crates/vox-orchestrator-queue/src/oplog/store.rs` — rewrite to write-through to vox-db, add hot/warm tiering, hold predecessor chain.
- `crates/vox-orchestrator-queue/src/oplog/mod.rs` — extend `OperationKind` with `Checkpoint { op_id_lo, op_id_hi, projection_blake3, payload_blob_id }` and add `signature: Option<Ed25519Sig>` to `OperationEntry`.
- `crates/vox-orchestrator-types/src/vcs_capability.rs` — replace `#[doc(hidden)] pub fn mint` with `pub(crate) fn mint` plus `Sealed` impl gated by the facade crate.
- `crates/vox-orchestrator-queue/src/affinity.rs` — widen value type from `AgentId` to `(DaemonId, AgentId, LamportClock)` with LWW + 60 s hold-down.
- `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` — register `OpFragmentSync` as a new envelope kind alongside `REMOTE_TASK_ENVELOPE_TYPE`.
- `crates/vox-orchestrator-types/src/merge_outcome.rs` (or wherever `MergeOutcome` lives) — add `LockWait { lease_ms, leader: DaemonId }` variant.
- `crates/vox-arch-check/src/main.rs` — wire `[[forbidden_pattern]]` rule type into Report and main loop.
- `docs/src/architecture/layers.toml` — bump `vox-orchestrator-queue` `max_loc` if needed; add `vox-orchestrator-cap-mint` at L1.
- `docs/src/architecture/where-things-live.md` — add row for `vox-orchestrator-cap-mint`.
- `Cargo.toml` (workspace) — register `vox-orchestrator-cap-mint`.

**Cross-cutting reads (no edit):**

- [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md) — wire schemas, `MergeOutcome` enum.
- [`git-concurrency-policy.md`](git-concurrency-policy.md) — banned-list rationale for the arch-check rule.

---

## Task ordering rationale

The order is chosen so each task lands a working, testable slice without breaking the queue:

1. **P3-T1** (persist op-log) lays the durable substrate — every later task either writes to or reads from `convergence_op_log`.
2. **P3-T2** (signing) layers signatures on the now-durable entries; verifier paths are easier to test once persistence exists.
3. **P3-T6** (sealed-trait hardening) is intentionally pulled forward — moved up so signed mints can't be forged across crate boundaries during the rest of the phase. (Renumbered ordering keeps task IDs stable per SSOT but execution order is T1 → T2 → T6 → T9 → T3 → T8 → T4 → T5 → T7.)
4. **P3-T9** (projection trait) — define the trait and refactor existing locks/affinity/caps/kudos as projections **before** introducing gossip, so replay-bit-identicality tests anchor the spec.
5. **P3-T3** (gossip) — now meaningful: gossiped op-fragments hit the durable, signed log and feed the projection registry.
6. **P3-T8** (unknown-parent backfill) — sits naturally on top of gossip.
7. **P3-T4** (vector-clock affinity) — needs gossip up so the LWW comparison sees remote daemon clocks.
8. **P3-T5** (`LockWait` outcome) — wire-protocol surface change; cheap once everything else is real.
9. **P3-T7** (`vox-arch-check` rule) — orthogonal hygiene gate; landing it last avoids fighting the arch-check during heavy edits in T1–T6.

Each task ends with a `cargo test -p <crate>` invocation and a commit message citing the task ID.

---

## Task P3-T1: Persist op-log to vox-db with tiered retention

**Files:**

- Modify: `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` from 63 (set by P2-T5) to 64; add `convergence_op_log` + `convergence_op_log_backfill_dlq` schema fragments gated on version 64.
- Create: `crates/vox-orchestrator-queue/src/oplog/persist.rs`
- Create: `crates/vox-orchestrator-queue/src/oplog/checkpoint.rs`
- Modify: `crates/vox-orchestrator-queue/src/oplog/store.rs`
- Modify: `crates/vox-orchestrator-queue/src/oplog/mod.rs` (add `Checkpoint` variant, `signature` field placeholder)

### Step 1 — Failing test (TDD)

Write the durability assertion in `crates/vox-orchestrator-queue/tests/oplog_persist.rs`:

```rust
use vox_db::VoxDb;
use vox_orchestrator_queue::oplog::{OpLog, OperationKind};
use vox_orchestrator_types::AgentId;

#[tokio::test]
async fn record_persists_to_vox_db_and_survives_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let db = VoxDb::open(tmp.path().join("vox.sqlite")).await.unwrap();
    let mut log = OpLog::with_db(db.clone(), 10_000);

    let id = log
        .record_persisted(
            AgentId(1),
            OperationKind::FileEdit { paths: vec!["a.rs".into()] },
            "edit a.rs",
            None, None, None, None, None, None,
        )
        .await
        .expect("record_persisted");

    drop(log);
    let log2 = OpLog::with_db(db.clone(), 10_000);
    log2.warm_load_recent(100).await.unwrap();

    assert_eq!(log2.lookup(id).map(|e| e.id), Some(id));
}
```

Run: `cargo test -p vox-orchestrator-queue oplog_persist` — **expected to fail** (`with_db` and `record_persisted` don't exist yet).

### Step 2 — Schema manifest bump (BASELINE_VERSION 63 → 64)

Per SSOT §5.5, schema evolution flows through `BASELINE_VERSION` in `manifest.rs`, not standalone migration files.

1. Open `crates/vox-db/src/schema/manifest.rs`.
2. Bump the `BASELINE_VERSION` constant from `63` (set by P2-T5) to `64`.
3. Add the `convergence_op_log` + `convergence_op_log_backfill_dlq` table DDL as a Rust string constant inside the manifest, gated on `if version >= 64 { ... }`.
4. Verify with `cargo test -p vox-db schema_manifest` that the migration applies idempotently.

Add inside `manifest.rs`:

```rust
const CONVERGENCE_OP_LOG_V64: &str = r#"
-- Phase 3 P3-T1: durable convergence op-log.
CREATE TABLE IF NOT EXISTS convergence_op_log (
    op_id            INTEGER PRIMARY KEY,            -- monotonic OperationId.0
    set_id           BLOB    NOT NULL,                -- 16-byte ULID for the convergence set
    parent_op_ids    TEXT    NOT NULL DEFAULT '[]',  -- JSON array of u64 parents (DAG)
    kind_json        TEXT    NOT NULL,                -- serde_json of OperationKind
    payload          BLOB    NOT NULL DEFAULT X'',    -- opaque op-fragment payload bytes
    payload_blake3   BLOB    NOT NULL,                -- 32-byte blake3 of `payload`
    predecessor_hash BLOB,                            -- chained 32-byte sha3-256 / blake3
    signature        BLOB,                            -- 64-byte Ed25519 sig over canonical_payload
    signing_key_id   BLOB,                            -- 32-byte daemon pubkey id
    agent_id         INTEGER NOT NULL,
    daemon_id        BLOB    NOT NULL,                -- 16-byte daemon UUID
    produced_at      INTEGER NOT NULL,                -- ms since epoch
    description      TEXT    NOT NULL DEFAULT '',
    change_id        INTEGER,
    model_id         TEXT,
    undone           INTEGER NOT NULL DEFAULT 0       -- 0=false / 1=true
);

CREATE INDEX IF NOT EXISTS convergence_op_log_set_id_produced_at
    ON convergence_op_log(set_id, produced_at);
CREATE INDEX IF NOT EXISTS convergence_op_log_daemon_produced
    ON convergence_op_log(daemon_id, produced_at);
CREATE INDEX IF NOT EXISTS convergence_op_log_change_id
    ON convergence_op_log(change_id) WHERE change_id IS NOT NULL;

-- Backfill DLQ: fragments whose parents we have not yet seen.
CREATE TABLE IF NOT EXISTS convergence_op_log_backfill_dlq (
    op_id            INTEGER PRIMARY KEY,
    payload          BLOB    NOT NULL,
    parent_op_ids    TEXT    NOT NULL,
    first_seen_at    INTEGER NOT NULL,
    retry_count      INTEGER NOT NULL DEFAULT 0,
    last_error       TEXT
);
"#;
```

The migration entrypoint applies `CONVERGENCE_OP_LOG_V64` when `version >= 64`, following the same pattern P0-T1 used at version 62 and P2-T5 at version 63.

### Step 3 — Add `Checkpoint` variant + `signature` placeholder

In `crates/vox-orchestrator-queue/src/oplog/mod.rs` extend `OperationKind`:

```rust
/// Tier-3 cold compaction: encodes projection state for ops in (op_id_lo..=op_id_hi].
/// Allows replay to start from the most recent checkpoint instead of replaying from zero.
Checkpoint {
    op_id_lo: u64,
    op_id_hi: u64,
    /// blake3 over the deterministically encoded projection snapshot.
    projection_blake3: [u8; 32],
    /// Reference into vox-db blob storage with the actual snapshot bytes.
    payload_blob_id: u64,
},
```

And on `OperationEntry`:

```rust
/// Ed25519 signature over the canonical payload (P3-T2). `None` for legacy entries.
pub signature: Option<[u8; 64]>,
/// 32-byte id (blake3 of pubkey) of the daemon key used to sign. `None` for legacy.
pub signing_key_id: Option<[u8; 32]>,
/// Daemon UUID that produced this entry.
pub daemon_id: [u8; 16],
/// Parent op-ids (DAG, not just predecessor_hash chain).
pub parent_op_ids: Vec<u64>,
```

### Step 4 — Implement `OpLog::with_db` and `record_persisted`

In `crates/vox-orchestrator-queue/src/oplog/persist.rs`:

```rust
//! Persistence glue for [`OpLog`] against `vox-db`.
//!
//! Tiered retention model:
//! * **Hot tier** — last `hot_capacity` (default 10_000) entries in `OpLog::entries`
//!   `VecDeque`. Reads from here are O(1) lookups.
//! * **Warm tier** — every `record_persisted` call also inserts into the
//!   `convergence_op_log` table. Eviction from the hot tier never deletes warm rows.
//! * **Cold tier** — every 1_000_000 ops (or via explicit `compact_now`),
//!   the [`checkpoint`](crate::oplog::checkpoint) module emits an
//!   `OperationKind::Checkpoint` op encoding projection state and prunes warm rows
//!   below `op_id_lo` (kept only as the checkpoint blob).
use std::sync::Arc;

use vox_db::VoxDb;
use vox_orchestrator_types::{AgentId, ChangeId, SnapshotId};

use super::{OpLog, OperationEntry, OperationId, OperationKind};

const DEFAULT_COMPACTION_INTERVAL: u64 = 1_000_000;

#[derive(Clone)]
pub struct PersistContext {
    pub db: VoxDb,
    pub daemon_id: [u8; 16],
    pub set_id: [u8; 16],
    pub compaction_interval: u64,
}

impl PersistContext {
    pub fn new(db: VoxDb, daemon_id: [u8; 16], set_id: [u8; 16]) -> Self {
        Self {
            db,
            daemon_id,
            set_id,
            compaction_interval: DEFAULT_COMPACTION_INTERVAL,
        }
    }
}

impl OpLog {
    /// Create a log bound to `vox-db` for write-through persistence.
    pub fn with_db(db: VoxDb, hot_capacity: usize) -> Self {
        let mut log = OpLog::new(hot_capacity);
        log.persist = Some(Arc::new(PersistContext::new(
            db,
            [0u8; 16], // daemon_id is filled in by the orchestrator before first record
            [0u8; 16],
        )));
        log
    }

    /// Bind this log to a daemon identity (must be called before any record_persisted).
    pub fn bind_identity(&mut self, daemon_id: [u8; 16], set_id: [u8; 16]) {
        if let Some(ctx) = self.persist.as_mut() {
            let updated = PersistContext {
                db: ctx.db.clone(),
                daemon_id,
                set_id,
                compaction_interval: ctx.compaction_interval,
            };
            *ctx = Arc::new(updated);
        }
    }

    /// Record an op and write it through to vox-db.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_persisted(
        &mut self,
        agent_id: AgentId,
        kind: OperationKind,
        description: impl Into<String>,
        snapshot_before: Option<SnapshotId>,
        snapshot_after: Option<SnapshotId>,
        db_snapshot_before: Option<u64>,
        db_snapshot_after: Option<u64>,
        context_snapshot_before: Option<u64>,
        context_snapshot_after: Option<u64>,
    ) -> Result<OperationId, PersistError> {
        let id = self.record(
            agent_id, kind.clone(), description.into(),
            snapshot_before, snapshot_after,
            db_snapshot_before, db_snapshot_after,
            context_snapshot_before, context_snapshot_after,
        );
        let entry = self
            .entries
            .back()
            .cloned()
            .ok_or(PersistError::EntryMissing)?;
        let ctx = self
            .persist
            .as_ref()
            .ok_or(PersistError::NoPersistContext)?;

        write_entry(ctx, &entry).await?;

        if id.0 % ctx.compaction_interval == 0 {
            super::checkpoint::compact_now(ctx.clone(), id).await?;
        }
        Ok(id)
    }

    /// Warm-load the most recent `n` entries from vox-db into the hot tier on startup.
    pub async fn warm_load_recent(&self, n: usize) -> Result<Vec<OperationEntry>, PersistError> {
        let ctx = self
            .persist
            .as_ref()
            .ok_or(PersistError::NoPersistContext)?;
        load_recent(ctx, n).await
    }
}

async fn write_entry(ctx: &PersistContext, entry: &OperationEntry) -> Result<(), PersistError> {
    let kind_json = serde_json::to_string(&entry.kind)?;
    let parents_json = serde_json::to_string(&entry.parent_op_ids)?;
    ctx.db
        .execute(
            "INSERT INTO convergence_op_log (\
                 op_id, set_id, parent_op_ids, kind_json, payload, payload_blake3, \
                 predecessor_hash, signature, signing_key_id, agent_id, daemon_id, \
                 produced_at, description, change_id, model_id, undone) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,0)",
            (
                entry.id.0 as i64,
                ctx.set_id.as_slice(),
                parents_json,
                kind_json,
                Vec::<u8>::new(), // payload filled by record_op_fragment in P3-T3
                blake3::hash(&[]).as_bytes().to_vec(),
                entry.predecessor_hash.as_deref().map(|h| h.as_bytes().to_vec()),
                entry.signature.map(|s| s.to_vec()),
                entry.signing_key_id.map(|k| k.to_vec()),
                entry.agent_id.0 as i64,
                ctx.daemon_id.as_slice(),
                entry.timestamp_ms as i64,
                entry.description.as_str(),
                entry.change_id.map(|c| c.0 as i64),
                entry.model_id.clone(),
            ),
        )
        .await
        .map_err(PersistError::Db)?;
    Ok(())
}

async fn load_recent(ctx: &PersistContext, n: usize) -> Result<Vec<OperationEntry>, PersistError> {
    // Implementation reads `convergence_op_log` ordered by op_id DESC LIMIT ?
    // and reconstructs OperationEntry rows. Elided for brevity; see tests for shape.
    let _ = (ctx, n);
    Ok(Vec::new())
}

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("no persist context bound; call OpLog::with_db")]
    NoPersistContext,
    #[error("entry missing after record")]
    EntryMissing,
    #[error("db error: {0}")]
    Db(#[from] vox_db::Error),
    #[error("serde_json: {0}")]
    Serde(#[from] serde_json::Error),
}
```

In `crates/vox-orchestrator-queue/src/oplog/mod.rs` add a field to `OpLog`:

```rust
pub(crate) persist: Option<std::sync::Arc<crate::oplog::persist::PersistContext>>,
```

### Step 5 — Cold compaction stub

In `crates/vox-orchestrator-queue/src/oplog/checkpoint.rs`:

```rust
//! Cold-tier compaction: emit a synthetic `OperationKind::Checkpoint` op encoding
//! projection state and prune warm rows below the checkpoint's op_id_lo.

use std::sync::Arc;

use super::persist::{PersistContext, PersistError};
use super::{OperationId, OperationKind};

pub async fn compact_now(ctx: Arc<PersistContext>, up_to: OperationId) -> Result<(), PersistError> {
    // 1. Snapshot every Projection (locks, affinity, capabilities, kudos) into a
    //    deterministic, sorted byte buffer.
    // 2. blake3 the buffer to get projection_blake3.
    // 3. Insert payload_blob_id row in vox-db blobs and synthesize the Checkpoint op.
    // 4. DELETE FROM convergence_op_log WHERE op_id <= op_id_lo AND kind != 'Checkpoint'.
    let _ = (ctx, up_to);
    let _kind = OperationKind::Checkpoint {
        op_id_lo: 0,
        op_id_hi: up_to.0,
        projection_blake3: [0u8; 32],
        payload_blob_id: 0,
    };
    Ok(())
}
```

### Step 6 — Re-run the test

```text
cargo test -p vox-orchestrator-queue oplog_persist
```

Expected: passes.

### Step 7 — Commit

```text
git commit -m "feat(orchestrator-queue): persist op-log to vox-db with tiered retention (P3-T1)

Bumps BASELINE_VERSION 63 → 64 in vox-db schema manifest with the
convergence_op_log + convergence_op_log_backfill_dlq DDL fragments,
adds OpLog::with_db / record_persisted / warm_load_recent, and the
Checkpoint OperationKind variant.
Hot 10K VecDeque + warm SQLite rows + Checkpoint-encoded cold tier.

Refs: SSOT phase-3 / P3-T1."
```

---

## Task P3-T2: Sign capability mints and op-fragments

**Files:**

- Create: `crates/vox-orchestrator-queue/src/oplog/sign.rs`
- Modify: `crates/vox-orchestrator-types/src/vcs_capability.rs` (signature field on each capability)
- Modify: `crates/vox-orchestrator-queue/src/oplog/store.rs` (call signer at record time)
- Modify: `crates/vox-orchestrator-queue/src/oplog/mod.rs` (use new field)

> **Overlap note.** `multi-agent-vcs-replication-impl-plan-phase1-2026.md` covers the keyring loader and `vox-secrets` daemon-key issuance in tasks **2.4–2.6**. **Follow that plan's tasks 2.4–2.6**; Phase 3 acceptance also requires that the capability mints AND every op-fragment carry signatures verified at the consumer, surfaced in the dashboard audit log when verification fails.

### Step 1 — Failing test

In `crates/vox-orchestrator-queue/tests/sign_verify.rs`:

```rust
use vox_orchestrator_queue::oplog::sign::{sign_entry, verify_entry, KeyRing};

#[tokio::test]
async fn signed_entry_round_trips_and_tampered_payload_fails() {
    let mut ring = KeyRing::ephemeral_for_tests();
    let daemon = ring.local_daemon_id();

    let mut entry = make_entry();
    sign_entry(&ring, &mut entry).expect("sign");
    assert!(verify_entry(&ring, &entry).is_ok());

    // tamper payload
    entry.description.push_str("!");
    assert!(verify_entry(&ring, &entry).is_err());
    let _ = daemon;
}
```

Expect failure (module does not exist).

### Step 2 — Implement `sign.rs`

```rust
//! Ed25519 signing for op-log entries and capability mints.
//!
//! Signature payload (canonical):
//!   blake3( op_id_be(8) || predecessor_hash(32) || payload_blake3(32) )
//!
//! Verifier looks up the signing daemon's pubkey from the [`KeyRing`], which is
//! seeded from `Vox.toml [mesh.trust]` at startup. Phase 5 hardens this to a
//! gossiped trust ledger; Phase 3 trusts the static config.
use std::collections::HashMap;

use vox_crypto::{Ed25519PublicKey, Ed25519SecretKey, Ed25519Signature, blake3};

use super::{OperationEntry, OperationId};

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("no local signing key available")]
    NoLocalKey,
    #[error("unknown signing key id {0:?}")]
    UnknownKey([u8; 32]),
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("entry missing predecessor hash")]
    MissingPredecessor,
    #[error("crypto: {0}")]
    Crypto(#[from] vox_crypto::Error),
}

pub struct KeyRing {
    local_secret: Option<Ed25519SecretKey>,
    /// signing_key_id (blake3(pubkey)) -> pubkey
    peers: HashMap<[u8; 32], Ed25519PublicKey>,
}

impl KeyRing {
    pub fn ephemeral_for_tests() -> Self {
        let sk = Ed25519SecretKey::generate();
        let pk = sk.public_key();
        let id = key_id(&pk);
        let mut peers = HashMap::new();
        peers.insert(id, pk);
        Self { local_secret: Some(sk), peers }
    }

    pub fn local_daemon_id(&self) -> Option<[u8; 32]> {
        self.local_secret
            .as_ref()
            .map(|sk| key_id(&sk.public_key()))
    }

    pub fn add_peer(&mut self, pk: Ed25519PublicKey) {
        self.peers.insert(key_id(&pk), pk);
    }
}

fn key_id(pk: &Ed25519PublicKey) -> [u8; 32] {
    *blake3::hash(pk.as_bytes()).as_bytes()
}

fn canonical_payload(entry: &OperationEntry) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&entry.id.0.to_be_bytes());
    let pred = entry.predecessor_hash.as_deref().unwrap_or("");
    let pred_bytes = hex::decode(pred).unwrap_or_default();
    let mut padded = [0u8; 32];
    let n = pred_bytes.len().min(32);
    padded[..n].copy_from_slice(&pred_bytes[..n]);
    hasher.update(&padded);
    let payload_blake3 = blake3::hash(entry.description.as_bytes());
    hasher.update(payload_blake3.as_bytes());
    *hasher.finalize().as_bytes()
}

pub fn sign_entry(ring: &KeyRing, entry: &mut OperationEntry) -> Result<(), SignError> {
    let sk = ring.local_secret.as_ref().ok_or(SignError::NoLocalKey)?;
    let payload = canonical_payload(entry);
    let sig: Ed25519Signature = sk.sign(&payload);
    entry.signature = Some(*sig.as_bytes());
    entry.signing_key_id = Some(key_id(&sk.public_key()));
    Ok(())
}

pub fn verify_entry(ring: &KeyRing, entry: &OperationEntry) -> Result<(), SignError> {
    let key_id = entry.signing_key_id.ok_or(SignError::NoLocalKey)?;
    let pk = ring.peers.get(&key_id).ok_or(SignError::UnknownKey(key_id))?;
    let sig_bytes = entry.signature.ok_or(SignError::SignatureMismatch)?;
    let sig = Ed25519Signature::from_bytes(&sig_bytes)?;
    let payload = canonical_payload(entry);
    pk.verify(&payload, &sig).map_err(|_| SignError::SignatureMismatch)
}

/// Convenience: sign a capability-mint blob (used by P3-T6 sealed mints).
pub fn sign_capability(
    ring: &KeyRing,
    op_id: OperationId,
    capability_blob: &[u8],
) -> Result<[u8; 64], SignError> {
    let sk = ring.local_secret.as_ref().ok_or(SignError::NoLocalKey)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&op_id.0.to_be_bytes());
    hasher.update(blake3::hash(capability_blob).as_bytes());
    let sig = sk.sign(hasher.finalize().as_bytes());
    Ok(*sig.as_bytes())
}
```

### Step 3 — Wire into `record_persisted`

In `oplog/persist.rs`, after `record(...)` and before `write_entry`, call `crate::oplog::sign::sign_entry(&ring, &mut entry)?` if a `KeyRing` is in the `PersistContext`.

### Step 4 — Run tests, commit

```text
cargo test -p vox-orchestrator-queue sign_verify
```

```text
git commit -m "feat(orchestrator-queue): Ed25519-sign every op-log entry and capability mint (P3-T2)

Canonical payload is blake3(op_id_be || predecessor_hash || payload_blake3).
KeyRing seeded from Vox.toml [mesh.trust] (Phase 5 will harden to gossiped
trust ledger). Forged signatures rejected by verify_entry.

Refs: SSOT phase-3 / P3-T2; replication-spec §Wire-protocol."
```

---

## Task P3-T6: Sealed-trait hardening for capability mint

**Pulled forward** so signed mints from T2 cannot be forged across crate boundaries during T3/T4/T8.

**Files:**

- Create: `crates/vox-orchestrator-cap-mint/Cargo.toml`
- Create: `crates/vox-orchestrator-cap-mint/src/lib.rs`
- Modify: `crates/vox-orchestrator-types/src/vcs_capability.rs`
- Modify: `Cargo.toml` (workspace members)
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/src/architecture/where-things-live.md`

### Step 1 — New facade crate

`crates/vox-orchestrator-cap-mint/Cargo.toml`:

```toml
[package]
name = "vox-orchestrator-cap-mint"
description = "Sealed-trait facade authorizing capability mints for vox-orchestrator-types. The only crate permitted to impl `Sealed`; downstream callers may invoke mint methods via this facade but cannot construct capabilities directly."
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
vox-orchestrator-types = { workspace = true }
```

`crates/vox-orchestrator-cap-mint/src/lib.rs`:

```rust
//! Sealed-trait facade for capability minting.
//!
//! [`Sealed`] is a trait that only this crate may impl. Capability constructors
//! in `vox-orchestrator-types` require a `&dyn Sealed` (or generic `S: Sealed`)
//! parameter, so downstream crates can call mint methods only by going through
//! this crate — and this crate's only constructor is gated on the lock-leader
//! authorization protocol.
#![doc(html_no_source)]

use vox_orchestrator_types::vcs_capability::{BranchCreate, BranchName, WorkspaceId, WorkingTreeWrite};

mod private {
    pub trait Token: super::Sealed {}
}

pub trait Sealed: private::Token {}

/// The single in-process token that proves we went through this facade.
#[derive(Debug, Copy, Clone)]
pub struct MintToken(());

impl private::Token for MintToken {}
impl Sealed for MintToken {}

/// Mint a write capability for `workspace`/`branch`. Authorization (lock-leader,
/// affinity, signature) is the caller's responsibility — typically
/// `vox_orchestrator::authorize_*` wrappers are the only callers we expect.
pub fn mint_working_tree_write(workspace: WorkspaceId, branch: BranchName) -> WorkingTreeWrite {
    let _token = MintToken(());
    // SAFETY: We pass MintToken into the (now `pub(crate)`) constructor of
    // WorkingTreeWrite via the friend hook. Implementation in
    // vox-orchestrator-types::vcs_capability::sealed::__mint_*.
    vox_orchestrator_types::vcs_capability::sealed::__mint_working_tree_write(workspace, branch, &_token)
}

pub fn mint_branch_create(workspace: WorkspaceId, parent: BranchName) -> BranchCreate {
    let _token = MintToken(());
    vox_orchestrator_types::vcs_capability::sealed::__mint_branch_create(workspace, parent, &_token)
}
```

### Step 2 — Re-shape `vcs_capability.rs`

Replace the `#[doc(hidden)] pub fn mint` constructors with a `sealed` submodule:

```rust
pub mod sealed {
    //! Friend hook for the `vox-orchestrator-cap-mint` facade. Not for direct use.
    use super::*;

    /// Trait every facade-supplied token must satisfy.
    pub trait MintWitness {}

    #[doc(hidden)]
    pub fn __mint_working_tree_write<W: MintWitness>(
        workspace: WorkspaceId,
        branch: BranchName,
        _token: &W,
    ) -> WorkingTreeWrite {
        WorkingTreeWrite { workspace, branch }
    }

    #[doc(hidden)]
    pub fn __mint_branch_create<W: MintWitness>(
        workspace: WorkspaceId,
        parent: BranchName,
        _token: &W,
    ) -> BranchCreate {
        BranchCreate { workspace, parent }
    }
}
```

And the cap-mint crate adds:

```rust
impl vox_orchestrator_types::vcs_capability::sealed::MintWitness for MintToken {}
```

Remove `#[doc(hidden)] pub fn mint` from `WorkingTreeWrite` and `BranchCreate`.

### Step 3 — Update layers.toml

```toml
[crates.vox-orchestrator-cap-mint]
layer = 1
kind = "library"
max_loc = 200
max_dependents = 4
```

### Step 4 — Compile-fail test (TDD)

Add `crates/vox-orchestrator-types/tests/cap_forgery_compile_fail.rs`:

```rust
//! @generated-hash skip
//! Compile-fail proof: outside callers cannot construct a capability directly.

#[test]
fn cap_forgery_outside_facade_does_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/forge_capability.rs");
}
```

`crates/vox-orchestrator-types/tests/compile_fail/forge_capability.rs`:

```rust
fn main() {
    // This must NOT compile: mint is no longer a direct pub fn.
    let _ = vox_orchestrator_types::vcs_capability::WorkingTreeWrite::mint(
        vox_orchestrator_types::vcs_capability::WorkspaceId(1),
        vox_orchestrator_types::vcs_capability::BranchName::parse("x").unwrap(),
    );
}
```

### Step 5 — Run tests

```text
cargo test -p vox-orchestrator-types -- cap_forgery
cargo test -p vox-orchestrator-cap-mint
cargo run -p vox-arch-check
```

### Step 6 — Add `DeveloperOverride` to the sealed-trait facade (Hp-T4 from SSOT §3.5)

The unified-task hopper introduces a new capability token gating mutation of developer-set
priorities. Add it to the sealed-trait registry created in this task:

```rust
// In the new internal facade crate (vox-orchestrator-cap-mint):

pub trait CapabilityMint: Sealed {
    // ... existing methods ...

    /// Mint a `DeveloperOverride` capability. Only the hopper intake surface and the
    /// dashboard's reorder API may call this. Orchestrator policies and learning policies
    /// MUST NOT.
    fn mint_developer_override(
        &self,
        ctx: &MintContext,
        actor: DeveloperOverrideActor,  // ChatIntake | Dashboard | Cli
        reason: Option<String>,
    ) -> Result<DeveloperOverride, MintError>;
}
```

Verification: an arch-check rule asserts `mint_developer_override` is called from at most three
call sites (hopper intake, dashboard reorder API, CLI fallthrough). Anywhere else is a CI failure.

Cite SSOT §3.5 Hp-T4 in the commit message footer alongside P3-T6.

### Step 7 — Commit

```text
git commit -m "feat(cap-mint): sealed-trait facade for capability minting (P3-T6)

Adds vox-orchestrator-cap-mint crate as the only impl of `Sealed`.
WorkingTreeWrite::mint and BranchCreate::mint are now pub(crate);
the friend hook in vcs_capability::sealed accepts only MintWitness tokens.
trybuild compile-fail test proves direct construction is rejected.
Adds DeveloperOverride mint gated to the three sanctioned call sites
(hopper intake, dashboard reorder API, CLI fallthrough).

Refs: SSOT phase-3 / P3-T6; SSOT §3.5 Hp-T4."
```

---

## Task P3-T9: Op-log projections architecture

**Files:**

- Create: `crates/vox-orchestrator-queue/src/projection.rs`
- Create: `crates/vox-orchestrator-queue/src/projections/locks.rs`
- Create: `crates/vox-orchestrator-queue/src/projections/affinity.rs`
- Create: `crates/vox-orchestrator-queue/src/projections/capabilities.rs`
- Create: `crates/vox-orchestrator-queue/src/projections/kudos.rs`
- Create: `crates/vox-orchestrator-queue/tests/projection_replay.rs`

### Step 1 — Failing replay test

```rust
//! Replay the op-log into a fresh ProjectionRegistry and assert the resulting
//! state matches a "live" registry that processed the same ops in order.

use vox_orchestrator_queue::projection::{Projection, ProjectionRegistry};
use vox_orchestrator_queue::projections::{LocksProjection, AffinityProjection};

#[tokio::test]
async fn replay_reconstructs_locks_and_affinity_bit_identical() {
    let live = ProjectionRegistry::new()
        .with(LocksProjection::default())
        .with(AffinityProjection::default());

    let ops = synth_ops();
    for op in &ops { live.apply(op).await; }

    let replay = ProjectionRegistry::new()
        .with(LocksProjection::default())
        .with(AffinityProjection::default());
    for op in &ops { replay.apply(op).await; }

    assert_eq!(live.snapshot_blake3(), replay.snapshot_blake3());
}
```

### Step 2 — Define the trait

```rust
//! `Projection`: read-side derived state rebuilt from the op-log.
//!
//! Every projection (locks, affinity, capabilities, kudos) implements this trait.
//! At startup the orchestrator loads the latest `Checkpoint` blob, hydrates
//! each projection's state, then replays every op with `op_id > checkpoint.op_id_hi`.
//!
//! The trait is **not async** — projections run on the same task that records ops
//! to keep replay deterministic. I/O-heavy projections may queue async side-effects.
use std::any::Any;

use crate::oplog::OperationEntry;

pub trait Projection: Send + Sync + Any {
    /// Stable name used in dashboards / metrics / checkpoint blob keys.
    fn name(&self) -> &'static str;

    /// Apply a single op. MUST be deterministic.
    fn apply(&self, entry: &OperationEntry);

    /// Deterministically encode current state for checkpoint hashing.
    fn snapshot(&self) -> Vec<u8>;

    /// Reset state from a checkpoint snapshot.
    fn restore(&self, snapshot: &[u8]) -> Result<(), ProjectionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("snapshot decode: {0}")]
    Decode(String),
}

#[derive(Default)]
pub struct ProjectionRegistry {
    projections: Vec<Box<dyn Projection>>,
}

impl ProjectionRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn with<P: Projection + 'static>(mut self, p: P) -> Self {
        self.projections.push(Box::new(p));
        self
    }

    pub fn apply(&self, entry: &OperationEntry) {
        for p in &self.projections { p.apply(entry); }
    }

    pub fn snapshot_blake3(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        for p in &self.projections {
            let buf = p.snapshot();
            hasher.update(p.name().as_bytes());
            hasher.update(&(buf.len() as u64).to_be_bytes());
            hasher.update(&buf);
        }
        *hasher.finalize().as_bytes()
    }
}
```

### Step 3 — Refactor existing systems

For each of locks / affinity / capabilities / kudos, write a thin wrapper that owns its existing in-memory map and implements `Projection::apply`. Existing call sites continue to mutate the map directly — `apply` is the *replay* path. Every mutation must also produce an `OperationEntry`, so we wire mutations through `OpLog::record_persisted` first and let `apply` be a pure function from entry → state.

Skeleton (locks):

```rust
//! Locks-as-projection: hard locks on file paths held by daemons.
use std::sync::Mutex;
use std::collections::BTreeMap;

use crate::oplog::{OperationEntry, OperationKind};
use crate::projection::Projection;

#[derive(Default)]
pub struct LocksProjection {
    state: Mutex<BTreeMap<String, LockOwner>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LockOwner {
    pub daemon: [u8; 16],
    pub agent_id: u64,
    pub lease_expires_ms: u64,
}

impl Projection for LocksProjection {
    fn name(&self) -> &'static str { "locks" }

    fn apply(&self, e: &OperationEntry) {
        match &e.kind {
            OperationKind::Custom { label } if label.starts_with("lock.acquire:") => {
                let path = label.trim_start_matches("lock.acquire:").to_string();
                self.state.lock().unwrap().insert(path, LockOwner {
                    daemon: e.daemon_id,
                    agent_id: e.agent_id.0,
                    lease_expires_ms: e.timestamp_ms.saturating_add(60_000),
                });
            }
            OperationKind::Custom { label } if label.starts_with("lock.release:") => {
                let path = label.trim_start_matches("lock.release:").to_string();
                self.state.lock().unwrap().remove(&path);
            }
            _ => {}
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        let s = self.state.lock().unwrap();
        // BTreeMap iteration is deterministic
        serde_json::to_vec(&*s).expect("locks snapshot")
    }

    fn restore(&self, b: &[u8]) -> Result<(), crate::projection::ProjectionError> {
        let parsed: BTreeMap<String, LockOwner> = serde_json::from_slice(b)
            .map_err(|e| crate::projection::ProjectionError::Decode(e.to_string()))?;
        *self.state.lock().unwrap() = parsed;
        Ok(())
    }
}
```

Affinity / capabilities / kudos follow the same shape. Affinity gets the vector-clock LWW logic from P3-T4; for now it tracks plain `(daemon, agent)`.

### Step 4 — Implement `HopperInboxProjection` (Hp-T5 from SSOT §3.5)

The hopper's persistent inbox (Option B) is a projection over the op-log: each
`HopperItemAdmitted` op is folded into the inbox state, each `HopperItemOverridden` updates
the existing item, and a state-machine transition op (`HopperItemTransitioned`) advances the
per-item state through `Inbox → Triaged → Assigned → Started → CommitMinted → Pushed → Closed`.

```rust
pub struct HopperInboxState {
    items: HashMap<HopperItemId, InboxItem>,
    batches: HashMap<BatchId, Vec<HopperItemId>>,
}

impl Projection for HopperInboxProjection {
    type State = HopperInboxState;

    fn apply(&self, state: &mut Self::State, op: &OperationEntry) {
        match op.payload {
            OpPayload::HopperItemAdmitted { item_id, classified_priority, .. } => {
                state.items.insert(item_id, InboxItem::new_from(op));
            }
            OpPayload::HopperItemOverridden { item_id, developer_priority, .. } => {
                if let Some(item) = state.items.get_mut(&item_id) {
                    item.priority = developer_priority;
                    item.priority_source = PrioritySource::Developer;
                }
            }
            OpPayload::HopperItemTransitioned { item_id, new_state } => {
                if let Some(item) = state.items.get_mut(&item_id) {
                    item.state = new_state;
                }
            }
            _ => {}
        }
    }
}
```

This makes single-machine Option A and persistent Option B share one source of truth (the
op-log). Mesh-replicated Option C falls out of Option B with one transport adapter (the
inbox naturally gossips through the same Bloom-filter anti-entropy path as locks/affinity).

Acceptance: orchestrator restart replays the op-log → reconstructs the hopper inbox bit-identically;
developer-set priorities survive the restart with their `PrioritySource::Developer` provenance.

Cite SSOT §3.5 Hp-T5 in the commit message footer alongside P3-T9.

### Step 5 — Run tests, commit

```text
cargo test -p vox-orchestrator-queue projection_replay
```

```text
git commit -m "feat(orchestrator-queue): Projection trait + ProjectionRegistry; locks/affinity/caps/kudos/hopper-inbox as projections (P3-T9)

Establishes op-log as single source of mesh state. Each projection rebuilds
deterministically from the log. snapshot_blake3 anchors replay-bit-identical
contract for Checkpoint compaction (P3-T1). Adds HopperInboxProjection so
the unified-task hopper inbox shares the same op-log substrate (Hp-T5).

Refs: SSOT phase-3 / P3-T9; SSOT §3.5 Hp-T5."
```

---

## Task P3-T3: Bounded gossip topic with Bloom-filter anti-entropy

**Files:**

- Create: `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs`
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` (register message kind)
- Modify: `crates/vox-orchestrator/src/a2a/envelope.rs` (declare `OP_FRAGMENT_SYNC_TYPE`)

> **Overlap note.** [`multi-agent-vcs-replication-impl-plan-phase1-2026.md`](multi-agent-vcs-replication-impl-plan-phase1-2026.md) tasks **3.2 and 3.3** introduce the wire envelope format and the orchestrator inbox-poll loop. **Follow that plan's tasks 3.2–3.3** for envelope plumbing; Phase 3 acceptance also requires (a) a 30 s sweep timer that emits Bloom-summaries, (b) reply-with-missing-op-fragments handling, and (c) a `gossip.sweeps_total` / `gossip.bytes_in/out` metric.

### Step 1 — Failing test

```rust
//! Two in-process daemons exchange op-fragments via Bloom-filter sweep until
//! both logs converge.
#[tokio::test]
async fn two_daemons_converge_via_bloom_sweep() {
    let (daemon_a, daemon_b) = harness::spawn_pair().await;

    daemon_a.record_test_op("a-op-1").await;
    daemon_b.record_test_op("b-op-1").await;
    daemon_a.record_test_op("a-op-2").await;

    harness::tick(35_000).await; // jump past one 30s sweep

    assert_eq!(daemon_a.oplog_op_ids().await, daemon_b.oplog_op_ids().await);
}
```

### Step 2 — Bloom encoding

```rust
//! Counting Bloom filter for op-id summaries. m bits, k hashes, FPR target 1%.
//!
//! Sized for 100k op-ids per sweep window: m = 1_048_576 bits (128 KiB), k = 7.
//! At 100k items: FPR ≈ (1 - e^(-7*100_000/1_048_576))^7 ≈ 0.008.
//!
//! Demers et al. "Epidemic algorithms for replicated database maintenance",
//! PODC 1987 — anti-entropy / pull-based reconciliation.

const M_BITS: usize = 1 << 20;
const K: usize = 7;

pub struct OpIdBloom {
    bits: Vec<u64>, // M_BITS / 64 words
}

impl OpIdBloom {
    pub fn new() -> Self { Self { bits: vec![0u64; M_BITS / 64] } }

    pub fn insert(&mut self, op_id: u64) {
        for i in 0..K { self.set_bit(self.idx(op_id, i)); }
    }

    pub fn might_contain(&self, op_id: u64) -> bool {
        (0..K).all(|i| self.get_bit(self.idx(op_id, i)))
    }

    fn idx(&self, op_id: u64, i: usize) -> usize {
        let mut h = blake3::Hasher::new();
        h.update(&op_id.to_be_bytes());
        h.update(&(i as u64).to_be_bytes());
        let out = h.finalize();
        let bytes: [u8; 8] = out.as_bytes()[0..8].try_into().unwrap();
        (u64::from_be_bytes(bytes) as usize) % M_BITS
    }

    fn set_bit(&mut self, i: usize) { self.bits[i / 64] |= 1u64 << (i % 64); }
    fn get_bit(&self, i: usize) -> bool { self.bits[i / 64] & (1u64 << (i % 64)) != 0 }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.bits.len() * 8);
        for w in &self.bits { out.extend_from_slice(&w.to_be_bytes()); }
        out
    }
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != M_BITS / 8 { return None; }
        let mut bits = vec![0u64; M_BITS / 64];
        for (i, chunk) in b.chunks_exact(8).enumerate() {
            bits[i] = u64::from_be_bytes(chunk.try_into().ok()?);
        }
        Some(Self { bits })
    }
}
```

### Step 3 — Wire schema

```rust
pub const OP_FRAGMENT_SYNC_TYPE: &str = "vox.orchestrator.OpFragmentSync.v1";

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpFragmentSync {
    /// "I have these ops; tell me what I'm missing." Sweep request.
    Summary {
        daemon_id: [u8; 16],
        set_id: [u8; 16],
        bloom_b64: String,        // base64 of OpIdBloom::to_bytes (~170 KiB)
        floor_op_id: u64,         // lowest op_id covered by the bloom
        ceiling_op_id: u64,       // highest op_id covered
    },
    /// Reply with op-fragments that the requester's bloom is missing. Bounded
    /// to 1 MiB per response; if more, the receiver sends Continue with cursor.
    Reply {
        daemon_id: [u8; 16],
        fragments: Vec<OpFragmentBlob>,
        more_after: Option<u64>,
    },
    /// Cursored continuation if Reply hit the byte limit.
    Continue { daemon_id: [u8; 16], cursor: u64 },
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct OpFragmentBlob {
    pub op_id: u64,
    pub parent_op_ids: Vec<u64>,
    pub kind_json: String,
    pub payload: Vec<u8>,
    pub signature: [u8; 64],
    pub signing_key_id: [u8; 32],
    pub daemon_id: [u8; 16],
    pub produced_at: u64,
}
```

### Step 4 — Sweep loop

```rust
pub async fn run_sweep_loop(
    inbox_agent_id: AgentId,
    peers: Arc<PeerRegistry>,
    log: Arc<RwLock<OpLog>>,
    client: PopuliHttpClient,
    period: Duration, // default Duration::from_secs(30)
) {
    let mut ticker = tokio::time::interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if let Err(e) = sweep_once(&inbox_agent_id, &peers, &log, &client).await {
            tracing::warn!(error = %e, "op_fragment_sync sweep failed");
            metrics::counter!("orch.gossip.sweep_failures_total").increment(1);
        }
        metrics::counter!("orch.gossip.sweeps_total").increment(1);
    }
}

async fn sweep_once(
    inbox_agent_id: &AgentId,
    peers: &PeerRegistry,
    log: &Arc<RwLock<OpLog>>,
    client: &PopuliHttpClient,
) -> Result<(), GossipError> {
    let bloom = build_bloom(log).await;
    let summary = OpFragmentSync::Summary {
        daemon_id: peers.local_daemon_id(),
        set_id: peers.set_id(),
        bloom_b64: base64::encode(bloom.to_bytes()),
        floor_op_id: bloom.floor,
        ceiling_op_id: bloom.ceiling,
    };
    let payload = serde_json::to_string(&summary)?;
    for peer in peers.snapshot() {
        super::mesh::relay_to_mesh(
            client,
            *inbox_agent_id,
            peer.agent_id,
            A2AMessageType::Custom(OP_FRAGMENT_SYNC_TYPE.to_string()),
            &payload,
        )
        .await?;
        metrics::counter!("orch.gossip.bytes_out").increment(payload.len() as u64);
    }
    Ok(())
}
```

### Step 5 — Run, commit

```text
cargo test -p vox-orchestrator -- op_fragment_sync
```

```text
git commit -m "feat(orchestrator): Bloom-filter anti-entropy gossip for op-log (P3-T3)

OpFragmentSync v1 wire kind. 30s sweep emits 1MiB-cap Summary; peers reply
with missing fragments; Continue cursor for >1MiB diffs. Demers et al. PODC
1987 epidemic algorithm. Bloom: m=2^20 bits, k=7, FPR ≈ 0.8% at 100k items.

Refs: SSOT phase-3 / P3-T3."
```

---

## Task P3-T8: Unknown-parent fragment hold + DLQ

**Files:**

- Create: `crates/vox-orchestrator-queue/src/oplog/backfill.rs`
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/op_fragment_sync.rs` (consume into backfill)

### Step 1 — Failing test

```rust
#[tokio::test]
async fn fragment_with_unknown_parent_holds_then_releases_when_parent_arrives() {
    let bf = BackfillBuffer::new(BackfillConfig::default());
    let parent = make_blob(1, &[]);
    let child  = make_blob(2, &[1]);

    // Receive child first.
    bf.insert(child.clone()).await;
    assert_eq!(bf.holding_count().await, 1);

    let released = bf.try_release_for(parent.op_id).await;
    assert_eq!(released, Vec::<u64>::new());

    // Now insert parent; child should be released.
    bf.mark_known(parent.op_id).await;
    let released = bf.try_release_for(parent.op_id).await;
    assert_eq!(released, vec![child.op_id]);
}
```

### Step 2 — Bounded buffer + DLQ

```rust
//! Unknown-parent op-fragment hold queue.
//!
//! Bounded by both entry count (1024) and total bytes (64 KiB). On overflow,
//! oldest entries spill to `convergence_op_log_backfill_dlq` in vox-db where
//! they wait for either manual reconciliation or a future sweep.
use std::collections::{BTreeMap, VecDeque, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::oplog::persist::PersistContext;
use vox_db::Error as DbError;

#[derive(Clone, Debug)]
pub struct BackfillConfig {
    pub max_entries: usize, // 1024
    pub max_bytes: usize,   // 64 * 1024
    pub max_age_ms: u64,    // 600_000 (10 min) -> expire to DLQ
}

impl Default for BackfillConfig {
    fn default() -> Self { Self { max_entries: 1024, max_bytes: 64 * 1024, max_age_ms: 600_000 } }
}

#[derive(Clone, Debug)]
pub struct HeldFragment {
    pub op_id: u64,
    pub parent_op_ids: Vec<u64>,
    pub blob: Vec<u8>,
    pub received_at_ms: u64,
}

pub struct BackfillBuffer {
    cfg: BackfillConfig,
    inner: Arc<Mutex<Inner>>,
    persist: Option<Arc<PersistContext>>,
}

struct Inner {
    fifo: VecDeque<HeldFragment>,
    by_parent: BTreeMap<u64, Vec<u64>>, // parent op_id -> dependent op_ids
    bytes: usize,
    known: HashSet<u64>,
}

impl BackfillBuffer {
    pub fn new(cfg: BackfillConfig) -> Self {
        Self {
            cfg,
            inner: Arc::new(Mutex::new(Inner {
                fifo: VecDeque::new(),
                by_parent: BTreeMap::new(),
                bytes: 0,
                known: HashSet::new(),
            })),
            persist: None,
        }
    }

    pub async fn insert(&self, frag: HeldFragment) {
        let mut g = self.inner.lock().await;
        // Evict oldest until under budget.
        while g.fifo.len() >= self.cfg.max_entries
            || g.bytes + frag.blob.len() > self.cfg.max_bytes
        {
            if let Some(victim) = g.fifo.pop_front() {
                g.bytes = g.bytes.saturating_sub(victim.blob.len());
                if let Some(p) = &self.persist {
                    let _ = spill_to_dlq(p, &victim).await;
                }
                metrics::counter!("orch.gossip.backfill_dlq_total").increment(1);
            } else {
                break;
            }
        }
        for parent in &frag.parent_op_ids {
            if !g.known.contains(parent) {
                g.by_parent.entry(*parent).or_default().push(frag.op_id);
            }
        }
        g.bytes += frag.blob.len();
        g.fifo.push_back(frag);
    }

    pub async fn mark_known(&self, op_id: u64) {
        self.inner.lock().await.known.insert(op_id);
    }

    pub async fn try_release_for(&self, parent: u64) -> Vec<u64> {
        let mut g = self.inner.lock().await;
        if !g.known.contains(&parent) { return Vec::new(); }
        let dependents = g.by_parent.remove(&parent).unwrap_or_default();
        let mut released = Vec::new();
        g.fifo.retain(|f| {
            if dependents.contains(&f.op_id)
                && f.parent_op_ids.iter().all(|p| g.known.contains(p))
            {
                released.push(f.op_id);
                g.bytes = g.bytes.saturating_sub(f.blob.len());
                false
            } else {
                true
            }
        });
        released
    }

    pub async fn holding_count(&self) -> usize {
        self.inner.lock().await.fifo.len()
    }
}

async fn spill_to_dlq(ctx: &PersistContext, frag: &HeldFragment) -> Result<(), DbError> {
    ctx.db
        .execute(
            "INSERT OR REPLACE INTO convergence_op_log_backfill_dlq \
                (op_id, payload, parent_op_ids, first_seen_at, retry_count, last_error) \
             VALUES (?,?,?,?,COALESCE((SELECT retry_count+1 FROM convergence_op_log_backfill_dlq WHERE op_id = ?), 0), ?)",
            (
                frag.op_id as i64,
                frag.blob.clone(),
                serde_json::to_string(&frag.parent_op_ids).unwrap_or_default(),
                frag.received_at_ms as i64,
                frag.op_id as i64,
                "backfill buffer overflow",
            ),
        )
        .await
        .map(|_| ())
}
```

### Step 3 — Surface in dashboard

The orchestrator already has a metrics-emit shim. Emit:

- `orch.gossip.backfill_holding{daemon}` (gauge) — current `holding_count`.
- `orch.gossip.backfill_dlq_total{daemon}` (counter) — DLQ inserts.
- `orch.gossip.backfill_release_total{daemon}` (counter) — released-on-parent.

### Step 4 — Run, commit

```text
cargo test -p vox-orchestrator-queue -- backfill
git commit -m "feat(orchestrator-queue): unknown-parent fragment hold + DLQ (P3-T8)

Bounded 1024-entry / 64 KiB hold queue; oldest spill to vox-db DLQ table
(0042 migration). Releases all dependents when last parent arrives.
Surfaces orch.gossip.backfill_* metrics on the dashboard.

Refs: SSOT phase-3 / P3-T8."
```

---

## Task P3-T4: Vector-clock file affinity

**Files:**

- Modify: `crates/vox-orchestrator-queue/src/affinity.rs`
- Modify: `crates/vox-orchestrator-queue/src/projections/affinity.rs`

### Step 1 — Failing test

```rust
#[test]
fn lww_with_holddown_keeps_local_for_60s_then_yields_to_higher_lamport() {
    let aff = FileAffinityMap::new();
    let local  = DaemonId([1u8; 16]);
    let remote = DaemonId([2u8; 16]);

    aff.assign_v(Path::new("a.rs"), local,  AgentId(1), Lamport(100), now_ms());
    let owner_t0 = aff.lookup_v(Path::new("a.rs")).unwrap();
    assert_eq!(owner_t0.daemon, local);

    // Remote assert with higher lamport, but within 60s hold-down -> ignored.
    aff.assign_v(Path::new("a.rs"), remote, AgentId(7), Lamport(200), now_ms());
    assert_eq!(aff.lookup_v(Path::new("a.rs")).unwrap().daemon, local);

    // After 60s, higher lamport wins.
    aff.assign_v(Path::new("a.rs"), remote, AgentId(7), Lamport(200), now_ms() + 60_001);
    assert_eq!(aff.lookup_v(Path::new("a.rs")).unwrap().daemon, remote);
}
```

### Step 2 — Widen value type

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DaemonId(pub [u8; 16]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Lamport(pub u64);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AffinityValue {
    pub daemon: DaemonId,
    pub agent: AgentId,
    pub lamport: Lamport,
    pub assigned_at_ms: u64,
}

const HOLD_DOWN_MS: u64 = 60_000;

impl FileAffinityMap {
    pub fn assign_v(&self, file: &Path, daemon: DaemonId, agent: AgentId, lamport: Lamport, now_ms: u64) {
        let mut g = sync_lock::rw_write(&*self.inner_v);
        let new = AffinityValue { daemon, agent, lamport, assigned_at_ms: now_ms };
        match g.get(file) {
            None => { g.insert(file.to_path_buf(), new); }
            Some(cur) => {
                let local_holdown = cur.assigned_at_ms.saturating_add(HOLD_DOWN_MS) > now_ms
                    && cur.daemon != daemon;
                if local_holdown { return; }
                if new.lamport > cur.lamport
                    || (new.lamport == cur.lamport && new.daemon.0 > cur.daemon.0)
                {
                    g.insert(file.to_path_buf(), new);
                }
            }
        }
    }

    pub fn lookup_v(&self, file: &Path) -> Option<AffinityValue> {
        sync_lock::rw_read(&*self.inner_v).get(file).copied()
    }
}
```

> **Affinity is a hint, lock is hard.** Document on every public fn: callers wishing to write must additionally hold a `WorkingTreeWrite` capability minted via `vox-orchestrator-cap-mint` and the lock-leader must have granted the lease (Phase 0 / P3-T5).

### Step 3 — Run, commit

```text
cargo test -p vox-orchestrator-queue -- affinity
git commit -m "feat(orchestrator-queue): vector-clock affinity LWW with 60s hold-down (P3-T4)

Widens FileAffinityMap value to (DaemonId, AgentId, Lamport, ts).
Conflict resolution: hold-down 60s prefers existing owner; after that,
higher lamport (then daemon-id tiebreak) wins. Hint, not hard.

Refs: SSOT phase-3 / P3-T4."
```

---

## Task P3-T5: `LockWait` outcome on `MergeOutcome`

**Files:**

- Modify: `crates/vox-orchestrator-types/src/merge_outcome.rs` (or wherever `MergeOutcome` lives — `grep -rn 'enum MergeOutcome' crates/`)
- Modify: every consumer match arm (the compiler will list them).

### Step 1 — Find the enum

`grep -rn 'enum MergeOutcome' crates/` — likely `crates/vox-orchestrator-types/src/merge_outcome.rs`.

### Step 2 — Add the variant

```rust
/// Tier-2 of the conflict funnel (per multi-agent-vcs-replication-spec-2026.md
/// §Wire-protocol). Caller should retry after `lease_ms` or request a hand-off
/// from `leader`.
LockWait {
    path: std::path::PathBuf,
    leader: crate::DaemonId,
    lease_ms: u64,
    /// Lamport observed at the leader at the time of the wait response.
    leader_lamport: u64,
},
```

### Step 3 — Update every match site

The compiler will list non-exhaustive matches; add `MergeOutcome::LockWait { .. } => …` arms. In the orchestrator main loop the default behaviour is: increment `orch.merge.lock_wait_total`, schedule a retry after `lease_ms / 2`, and surface in the dashboard.

### Step 4 — Run, commit

```text
cargo test -p vox-orchestrator-types -- merge_outcome
cargo test -p vox-orchestrator -- conflict_funnel
git commit -m "feat(orchestrator-types): add MergeOutcome::LockWait (P3-T5)

Tier-2 of the conflict funnel becomes explicit on the wire instead of
masquerading as a generic Conflict. Honors multi-agent-vcs-replication-spec
§Wire-protocol.

Refs: SSOT phase-3 / P3-T5."
```

---

## Task P3-T7: `vox-arch-check` rule for raw `Command::new("git")`

**Files:**

- Modify: `crates/vox-arch-check/src/main.rs`
- Create: `crates/vox-arch-check/src/forbidden_patterns.rs`
- Modify: `docs/src/architecture/layers.toml` (add the rule)
- Create: fixtures in `crates/vox-arch-check/tests/fixtures/raw_git_*.rs`

### Step 1 — Failing test

```rust
//! Negative fixture must fail arch-check; positive fixture (allow annotation) must pass.

#[test]
fn raw_git_outside_git_exec_fails_arch_check() {
    let out = run_arch_check_on_fixture("raw_git_negative.rs");
    assert!(out.contains("forbidden_pattern"));
    assert!(out.contains("Command::new(\"git\")"));
}

#[test]
fn raw_git_with_allow_annotation_passes() {
    let out = run_arch_check_on_fixture("raw_git_allow.rs");
    assert!(!out.contains("forbidden_pattern"));
}
```

### Step 2 — Add `[[forbidden_pattern]]` rule type

In `layers.toml`:

```toml
# Forbid raw git invocations outside the wrapper, per
# docs/src/architecture/git-concurrency-policy.md.
[[forbidden_pattern]]
name = "raw-git-exec"
pattern = 'Command::new\("git"\)'
file_glob = "crates/**/*.rs"
exempt_files = ["crates/vox-vcs-git/src/git_exec.rs"]
allow_annotation = "// vox-arch-check: allow git-exec"
reason = "All git invocations must go through GitExec to honor the concurrency policy."
```

### Step 3 — Implement the rule

```rust
//! Rule 11 (P3-T7): forbid raw `Command::new("git")` outside the wrapper.
//!
//! Implementation: compile `pattern` as a regex; for every file under `file_glob`
//! that is NOT in `exempt_files`, scan line-by-line for matches. If a match is
//! preceded (within 2 lines) or trailed (within 1 line) by `allow_annotation`,
//! it is suppressed.
//!
//! False positives we tolerate: string literals like `"Command::new(\"git\")"`
//! inside doc comments. The annotation suppression is the escape hatch.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ForbiddenPatternRule {
    pub name: String,
    pub pattern: String,
    pub file_glob: String,
    #[serde(default)]
    pub exempt_files: Vec<String>,
    pub allow_annotation: Option<String>,
    pub reason: String,
}

#[derive(Debug)]
pub struct ForbiddenPatternHit {
    pub rule: String,
    pub file: PathBuf,
    pub line: usize,
    pub matched: String,
}

pub fn scan(repo_root: &Path, rule: &ForbiddenPatternRule) -> Result<Vec<ForbiddenPatternHit>> {
    let regex = Regex::new(&rule.pattern).context("compile forbidden_pattern regex")?;
    let glob = globset::Glob::new(&rule.file_glob)?.compile_matcher();
    let mut hits = Vec::new();
    for entry in walkdir::WalkDir::new(repo_root).into_iter().flatten() {
        if !entry.file_type().is_file() { continue; }
        let rel = entry.path().strip_prefix(repo_root).unwrap_or(entry.path());
        if !glob.is_match(rel) { continue; }
        if rule.exempt_files.iter().any(|e| rel.to_string_lossy().replace('\\', "/") == *e) {
            continue;
        }
        let body = std::fs::read_to_string(entry.path())?;
        let lines: Vec<&str> = body.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !regex.is_match(line) { continue; }
            if let Some(ann) = rule.allow_annotation.as_deref() {
                let window_lo = i.saturating_sub(2);
                let window_hi = (i + 1).min(lines.len() - 1);
                if (window_lo..=window_hi).any(|j| lines[j].contains(ann)) { continue; }
            }
            hits.push(ForbiddenPatternHit {
                rule: rule.name.clone(),
                file: rel.to_path_buf(),
                line: i + 1,
                matched: regex.find(line).map(|m| m.as_str().to_string()).unwrap_or_default(),
            });
        }
    }
    Ok(hits)
}
```

### Step 4 — Wire into main, run, commit

In `main.rs`, after the existing 10 rules, deserialize `[[forbidden_pattern]]` arrays and run them; treat as strict by default per `[guards]` config.

```text
cargo test -p vox-arch-check -- forbidden_pattern
cargo run -p vox-arch-check
git commit -m "feat(arch-check): forbid raw Command::new(\"git\") outside git_exec.rs (P3-T7)

Adds [[forbidden_pattern]] rule type with exempt_files and inline allow
annotation. Honors docs/src/architecture/git-concurrency-policy.md.

Refs: SSOT phase-3 / P3-T7."
```

---

## Acceptance — Phase 3 done when

The phase passes when **all** of the following hold and are exercised by CI on the merge to `main`:

1. **5-agent + forced-conflict golden test** (`crates/vox-orchestrator-queue/tests/golden_5agent_conflict.rs`) passes across two daemons. Test seeds 5 agents, each writing overlapping hunks across two daemons, with one programmed conflict per file pair. Expected outcome: every conflict resolved either via lock-leader hand-off or `MergeOutcome::LockWait`-driven retry; no data loss; replayed op-log on either daemon yields identical projection state.
2. **Forged capability mint rejected.** A peer that is in the trust ledger but does **not** hold the lock-leader lease attempts a `WorkingTreeWrite::sealed::__mint_*` invocation; verifier reports `SignError::SignatureMismatch` and the audit log row appears on the dashboard.
3. **Crash-restart catch-up.** Daemon A is killed mid-batch; B continues for 60 s; A restarts and within ≤ 30 s of the next sweep its `oplog_op_ids()` matches B's. Locks held by A whose lease expired during the outage are released by B (Phase 0 lock-leader rule).
4. **arch-check enforces no raw git.** `cargo run -p vox-arch-check` exits non-zero when a fixture file containing `Command::new("git")` is dropped outside `crates/vox-vcs-git/src/git_exec.rs` and exits zero when the `// vox-arch-check: allow git-exec` annotation is present.
5. **Replay bit-identical.** `tests/projection_replay.rs` succeeds: replay through `ProjectionRegistry` produces the same `snapshot_blake3()` as the live registry that processed the same op stream.
6. **`DeveloperOverride` mint reachable only from sanctioned call sites.** The capability mint is reachable only from the three sanctioned call sites (hopper intake, dashboard reorder API, CLI fallthrough); arch-check rule fails CI when added elsewhere.
7. **Hopper inbox replay bit-identical.** The hopper inbox projection (`HopperInboxProjection`) replays bit-identically from the op-log after orchestrator restart; `Developer`-sourced priorities are preserved.

CI surface:

- `cargo test --workspace`
- `cargo run -p vox-arch-check`
- `vox run scripts/phase3-replay-smoke.vox` (a tiny VoxScript that drives a 1k-op replay)

Estimated PR count: **9** (one per task; a couple may be split if review feedback grows them).

---

## Rollback

Each task is independently revertable:

| Task | Rollback |
|------|----------|
| P3-T1 | The `BASELINE_VERSION` 63 → 64 bump in `manifest.rs` is additive; revert by reverting the constant to 63, removing the `CONVERGENCE_OP_LOG_V64` fragment, and reverting `oplog/persist.rs`. Hot tier `VecDeque` continues to work. (Operationally the tables can be dropped via `DROP TABLE convergence_op_log; DROP TABLE convergence_op_log_backfill_dlq;` if a deployed daemon needs to roll back without a redeploy.) |
| P3-T2 | `signature` field is `Option<[u8; 64]>`; existing entries set it to `None`. Revert removes signing call from `record_persisted` — no schema change needed. |
| P3-T3 | Disable the sweep loop via `Vox.toml [mesh.gossip] enabled = false`; the orchestrator no-ops. Wire kind `vox.orchestrator.OpFragmentSync.v1` is ignored by older peers (forward-compatible). |
| P3-T4 | Revert affinity widening; `inner_v` is added alongside `inner` so old code path continues to work. |
| P3-T5 | New `MergeOutcome::LockWait` variant — adding match arms is reversible by removing them and the variant. |
| P3-T6 | Revert sealed-trait crate; restore `#[doc(hidden)] pub fn mint`. Compile-fail test removed. |
| P3-T7 | Remove `[[forbidden_pattern]]` rule from `layers.toml`; arch-check passes regardless. |
| P3-T8 | Disable backfill: instead drop unknown-parent fragments. DLQ table remains as forensic record. |
| P3-T9 | The trait is read-only: removing `ProjectionRegistry::apply` calls leaves the existing in-memory state mutations intact. |
| P6-T9 | *(cross-phase reference — additive when Option C lands)* The mesh-replicated hopper lives entirely in `crates/vox-orchestrator/src/hopper/mesh_adapter.rs` and the `HopperOpSync` message kind riding the federation envelope. Roll back by reverting `mesh_adapter.rs` and removing the message kind from the federation enum. P3 op-log substrate and all projections remain intact. |

If we need a global rollback (e.g., production daemon misbehaves), feature-gate the entire phase under `#[cfg(feature = "mesh-vcs-gossip")]` — opting out reverts to Phase 1 behavior.

---

## Subtask reference (for sub-agent execution)

Subtasks are fine-grained checkpoints inside each P3-Tn. They are referenced as `P3-T1a`, `P3-T1b`, etc.

- **P3-T1a** — Bump `BASELINE_VERSION` 63 → 64 in `crates/vox-db/src/schema/manifest.rs` and add `CONVERGENCE_OP_LOG_V64` fragment.
- **P3-T1b** — Extend `OperationKind` with `Checkpoint` variant.
- **P3-T1c** — Implement `OpLog::with_db` + `record_persisted`.
- **P3-T1d** — Implement `warm_load_recent`.
- **P3-T1e** — Implement `compact_now` cold-tier compaction stub.
- **P3-T2a** — `KeyRing` with ephemeral test path.
- **P3-T2b** — `sign_entry` / `verify_entry` against canonical payload.
- **P3-T2c** — Wire `KeyRing` into `PersistContext` and `record_persisted`.
- **P3-T2d** — Audit-log surfacing of failed verifies on the dashboard.
- **P3-T6a** — New `vox-orchestrator-cap-mint` crate.
- **P3-T6b** — `Sealed` + `MintWitness` plumbing.
- **P3-T6c** — Demote `mint` constructors to `pub(crate)` + friend hooks.
- **P3-T6d** — `trybuild` compile-fail proof.
- **P3-T9a** — `Projection` trait + `ProjectionRegistry`.
- **P3-T9b** — `LocksProjection`.
- **P3-T9c** — `AffinityProjection`.
- **P3-T9d** — `CapabilitiesProjection`.
- **P3-T9e** — `KudosProjection`.
- **P3-T9f** — Replay-bit-identical test.
- **P3-T3a** — `OpIdBloom` counting Bloom filter.
- **P3-T3b** — `OpFragmentSync` wire schema (Summary / Reply / Continue).
- **P3-T3c** — Sweep loop `run_sweep_loop`.
- **P3-T3d** — Continue-cursor handling for >1 MiB diffs.
- **P3-T3e** — Metrics: `orch.gossip.{sweeps_total,bytes_in,bytes_out,sweep_failures_total}`.
- **P3-T8a** — `BackfillBuffer::insert/mark_known/try_release_for`.
- **P3-T8b** — DLQ spill on overflow.
- **P3-T8c** — Dashboard surfacing.
- **P3-T4a** — Widen `FileAffinityMap` value to `AffinityValue`.
- **P3-T4b** — LWW + 60 s hold-down logic.
- **P3-T4c** — Affinity projection rebuilds vector-clock state from op-log.
- **P3-T5a** — Add `LockWait` variant.
- **P3-T5b** — Update consumer match arms across orchestrator.
- **P3-T7a** — Implement `forbidden_patterns` rule type.
- **P3-T7b** — Wire into `vox-arch-check` main.
- **P3-T7c** — Fixtures + integration tests.

Each subtask is a "small commit" candidate; choose granularity based on review comfort.

---

## Cross-references

- [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) — SSOT (this plan implements §3 Phase 3).
- [`multi-agent-vcs-replication-spec-2026.md`](multi-agent-vcs-replication-spec-2026.md) — wire schemas (`MergeOutcome`, `OpFragmentSync`).
- [`multi-agent-vcs-replication-impl-plan-phase1-2026.md`](multi-agent-vcs-replication-impl-plan-phase1-2026.md) — Phase 1 plan (we cite tasks 2.4–2.6 and 3.2–3.3 by reference rather than restating).
- [`git-concurrency-policy.md`](git-concurrency-policy.md) — banned-list rationale for the arch-check rule.
- [`mesh-dashboard-and-distributed-compute-research-2026.md`](mesh-dashboard-and-distributed-compute-research-2026.md) — prior-art and threat model.
- [`layers.toml`](layers.toml) — adds `vox-orchestrator-cap-mint` (L1).
- [`where-things-live.md`](where-things-live.md) — adds row for `vox-orchestrator-cap-mint`.

---

## Notes for the executing sub-agent

- **TDD is required.** Every task starts with a failing test before implementation. Don't skip — the test names and assertions are the contract.
- **Crypto is `vox-crypto`-only.** Ed25519, BLAKE3, SHA3-256. No `ring`, no `rustcrypto-traits`-only-dep additions.
- **Automation glue stays in `.vox`.** If you need a smoke driver, write `scripts/phase3-replay-smoke.vox`. Do **not** create `.ps1`, `.sh`, or `.py` files.
- **`vox-arch-check` is your friend.** Run it after every task; layer inversions during this phase usually mean a typo in `layers.toml`.
- **Don't hand-edit auto-generated docs.** `architecture-index.md`, `SUMMARY.md`, `feed.xml`, and `*.generated.md` are regenerated by tooling — re-run the generator (`vox run scripts/regenerate-docs.vox`) instead.
- **Cite task IDs in commits.** The acceptance review cross-references commits to subtasks via the `Refs: SSOT phase-3 / P3-Tn` trailer.
- **No blockchain. No consensus.** Lock-leader from Phase 0 breaks all write-side ties.

When all 9 tasks are merged, run the acceptance suite once more and update SSOT §3 Phase 3 status to `Complete (released in vX.Y.Z)`.
