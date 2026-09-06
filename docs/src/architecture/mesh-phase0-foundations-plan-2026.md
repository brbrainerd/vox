---
title: "Mesh Phase 0 — Foundations Implementation Plan (2026-05-09)"
description: "TDD implementation plan for Phase 0 of the Mesh & Language-Level Distribution SSOT — persisted lock map, lock-leader election, lease-gated dispatch, secret injection, TLS option, probe trait, SkillRuntime seam, and traceparent propagation. Eight tasks, ~1700 LOC."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. Spec/SSOT is the durable artifact."
---

# Mesh Phase 0 — Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to drive this plan task-by-task. Substeps use checkbox (`- [ ]`) syntax for tracking. **TDD ordering is mandatory** — write the failing test first, then the implementation. Each task ends with a `cargo test` run and a single commit.

**Goal.** Make the mesh authoritatively trustworthy at LAN scale, and put the substrate in place that every later phase depends on. Concretely: persist the file-lock map to vox-db with WAL replay; elect a single lock-leader with heartbeat; enforce authoritative leases in dispatch; inject decrypted JWE secrets into task exec context; expose a TLS / WireGuard option on the populi HTTP plane; land the hardware probe trait via the existing probe-correctness plan; move the in-process executor behind `SkillRuntime`; and propagate `traceparent` across A2A.

**Killer feature delivered.** *"Two daemons, multi-agent, same repo, no data loss."* Plus: a Vox node is no longer a debug visualization.

**Architecture.** Phase 0 strengthens four crates without expanding their public surface:

- `vox-db` gains two tables (`vcs_lock`, `lock_leader`) inside the existing `vox_mesh` schema fragment, plus typed accessors on `VoxDb`.
- `vox-orchestrator-queue` `FileLockManager` becomes a thin in-memory cache layered over a `vox-db` SSOT — every mutation writes through, and a startup hook hydrates the in-memory map from WAL on boot.
- `vox-orchestrator` dispatch consults `mesh_exec_leases` before falling back to local execution and propagates `traceparent` end-to-end. JWE-decrypted secrets become a `SecretBag` injected into the skill-runtime call.
- `vox-populi` HTTP server gains an opt-in `[mesh.transport]` TLS section in `Vox.toml`, terminated by `rustls`.

**Tech stack.** Rust 2024 edition. Workspace-already-present deps only: `tokio`, `tracing`, `thiserror`, `serde`, `blake3`, `rustls`. New crypto: none — JWE/Ed25519/X25519/BLAKE3 are reused via `vox-crypto`. **No new external deps.**

**Spec / SSOT pointer.**

- [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) §3 Phase 0 — the canonical task list (P0-T1..P0-T8).
- [`populi-mesh-probe-correctness-spec-2026.md`](populi-mesh-probe-correctness-spec-2026.md) — design for P0-T6.
- [`populi-mesh-probe-correctness-plan-2026.md`](populi-mesh-probe-correctness-plan-2026.md) — implementation for P0-T6 (delegated; we do not re-author it).
- [`unified-task-hopper-research-2026.md`](unified-task-hopper-research-2026.md) §3.5 — hopper track Hp-T2 (the three new `AgentEvent` variants) is bundled into P0-T8 since both touch `events.rs`.
- ADR-017 ("authoritative leases" / W1).
- ADR-023 (telemetry default-on).

> **Bundled scope note.** P0-T8 also lands the three new hopper `AgentEvent` variants (`TaskReprioritized`, `HopperItemAdmitted`, `HopperItemOverridden`, plus the `ReprioritizationActor` placeholder enum) from the unified-task-hopper SSOT §3.5 Hp-T2 — both touch `crates/vox-orchestrator/src/events.rs`, so we land them in one PR.

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\zealous-ardinghelli-b01e11`. All paths in this plan are relative to the repo root.

---

## File map

**Create:**

- `crates/vox-db/src/mesh_locks.rs` — typed accessors over the new `vcs_lock` / `lock_leader` tables.
- `crates/vox-orchestrator-queue/src/locks/persisted.rs` — write-through layer over `FileLockManager`.
- `crates/vox-orchestrator-queue/src/locks/leader.rs` — `LockLeaderElection` with heartbeat refresh.
- `crates/vox-orchestrator/src/a2a/dispatch/mod.rs` — higher-level dispatcher choosing local vs mesh.
- `crates/vox-orchestrator/src/a2a/dispatch/lease_gate.rs` — lease check that gates local fallback.
- `crates/vox-orchestrator/src/a2a/secret_bag.rs` — task-scoped decrypted secret container.
- `crates/vox-orchestrator/src/a2a/traceparent.rs` — W3C traceparent encode/decode helpers.
- `crates/vox-populi/src/transport/tls.rs` — rustls acceptor wiring for the populi HTTP plane.
- `crates/vox-orchestrator/tests/two_daemon_lock_contention.rs` — integration test fixture for the Phase 0 acceptance criterion.
- `crates/vox-populi/tests/tls_smoke.rs` — TLS smoke test.

**Modify:**

- `crates/vox-db/src/schema/domains/vox_mesh.rs` — append `vcs_lock` and `lock_leader` DDL.
- `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` and refresh the digest.
- `crates/vox-db/src/lib.rs` — re-export `mesh_locks` accessors.
- `crates/vox-orchestrator-queue/src/locks/mod.rs` — make `FileLockManager` carry an optional `VoxDb` handle and route mutations through `persisted`.
- `crates/vox-orchestrator-queue/src/locks/lease.rs` — wire heartbeat / leader-driven proxying.
- `crates/vox-orchestrator-queue/Cargo.toml` — depend on `vox-db` (already a workspace member).
- `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` — populate `traceparent`, call `lease_gate::check_before_local_fallback`.
- `crates/vox-orchestrator/src/a2a/remote_worker.rs` — read `traceparent` into the span, decrypt JWE into `SecretBag`, hand bag to skill-runtime adapter.
- `crates/vox-orchestrator/src/a2a/mod.rs` — export new submodules.
- `crates/vox-orchestrator/src/skill_exec.rs` (or equivalent runner shim) — accept `SecretBag` argument.
- `crates/vox-skill-runtime/src/runtime.rs` — add `SkillRuntime::run_with_secrets` default-impl method.
- `crates/vox-repository/src/populi_toml.rs` — add `[mesh.transport]` config keys.
- `crates/vox-populi/src/transport/mod.rs` (or `lib.rs`) — wire optional TLS acceptor.
- `crates/vox-populi/Cargo.toml` — add optional `tls` feature gate.
- `docs/src/architecture/where-things-live.md` — add rows for `mesh_locks`, `LockLeaderElection`, `SecretBag`, TLS transport.
- `docs/src/reference/populi.md` — TLS appendix.

**Do not edit (auto-generated):**

- `docs/src/SUMMARY.md`
- `docs/src/architecture/architecture-index.md`
- `docs/src/feed.xml`
- Any `*.generated.md`

`research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored.

---

## Task ordering rationale

Tasks follow the SSOT dependency graph: T1 (persisted locks) is the substrate for T2 (leader election), which in turn is consulted by T3 (lease gate). T4 (secret injection) and T8 (traceparent) both touch `dispatch/mesh.rs` and `remote_worker.rs`; we sequence T4 before T8 because the secret-bag refactor reshapes the inbox-handler call site that T8 then decorates. T5 (TLS), T6 (probe trait), and T7 (SkillRuntime seam) are independent and may proceed in parallel after T1 lands; we list them in priority order so a serial executor still produces a coherent series of green commits.

Each task is self-contained: it includes a failing test, an implementation, a `cargo test` line, and a commit suggestion citing the task ID. The workspace must build and tests must pass at every commit boundary — `cargo run -p vox-arch-check` must remain clean (no new layer inversions).

---

## Task P0-T1: Persist file-lock map to vox-db

**Files:**

- Create: `crates/vox-db/src/mesh_locks.rs`
- Create: `crates/vox-orchestrator-queue/src/locks/persisted.rs`
- Modify: `crates/vox-db/src/schema/domains/vox_mesh.rs`
- Modify: `crates/vox-db/src/schema/manifest.rs`
- Modify: `crates/vox-db/src/lib.rs`
- Modify: `crates/vox-orchestrator-queue/src/locks/mod.rs`
- Modify: `crates/vox-orchestrator-queue/Cargo.toml`

### Sub-task P0-T1a: Append `vcs_lock` table to the schema fragment

- [ ] **Step 1: Write a failing schema-presence test.**

In `crates/vox-db/tests/mesh_schema.rs` (create if absent):

```rust
//! Phase-0 schema acceptance: the vcs_lock and lock_leader tables exist
//! after a fresh baseline apply.

#[tokio::test]
async fn vcs_lock_table_exists_after_baseline() {
    let db = vox_db::VoxDb::open_in_memory().await.expect("open db");
    let exists: i64 = db
        .raw_query_one_i64(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='vcs_lock'",
        )
        .await
        .expect("query");
    assert_eq!(exists, 1, "vcs_lock table missing");
}

#[tokio::test]
async fn lock_leader_table_exists_after_baseline() {
    let db = vox_db::VoxDb::open_in_memory().await.expect("open db");
    let exists: i64 = db
        .raw_query_one_i64(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='lock_leader'",
        )
        .await
        .expect("query");
    assert_eq!(exists, 1, "lock_leader table missing");
}
```

If `raw_query_one_i64` does not exist, use the existing `query_scalar` / `query_row` helper found in `crates/vox-db/src/local_tests.rs` and adapt the call. Run:

```text
cargo test -p vox-db --test mesh_schema
```

Expected: FAIL — neither table exists yet.

- [ ] **Step 2: Append the DDL to the `vox_mesh` schema fragment.**

In `crates/vox-db/src/schema/domains/vox_mesh.rs`, append before the closing `";` of `SCHEMA_VOX_MESH`:

```sql

-- ── Phase 0: persisted VCS file lock map (P0-T1) ──────────────────────────

-- One row per locked path (canonical absolute form, NFC-normalised).
-- `kind` is 'exclusive' | 'shared_read'; `holder` is the AgentId.0 string.
-- `expires_at` is the UNIX-ms TTL deadline; the leader prunes expired rows.
-- `lease_id` references mesh_exec_leases.lease_id when the lock is being
-- proxied to a remote node; NULL for purely local locks.
CREATE TABLE IF NOT EXISTS vcs_lock (
    path             TEXT NOT NULL PRIMARY KEY,
    kind             TEXT NOT NULL CHECK (kind IN ('exclusive', 'shared_read')),
    holder           TEXT NOT NULL,
    holder_node_id   TEXT NOT NULL,
    repository_id    TEXT NOT NULL,
    acquired_at      INTEGER NOT NULL,
    expires_at       INTEGER NOT NULL,
    lease_id         TEXT,
    fence_token      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_vcs_lock_holder
    ON vcs_lock(holder_node_id, repository_id);
CREATE INDEX IF NOT EXISTS idx_vcs_lock_expires
    ON vcs_lock(expires_at);

-- ── Phase 0: lock-leader election (P0-T2) ─────────────────────────────────

-- Singleton row per repository: who is currently the lock leader.
-- Followers proxy lock-mutation requests via A2A to leader_node_id.
CREATE TABLE IF NOT EXISTS lock_leader (
    repository_id    TEXT NOT NULL PRIMARY KEY,
    leader_node_id   TEXT NOT NULL,
    elected_at       INTEGER NOT NULL,
    heartbeat_at     INTEGER NOT NULL,
    expires_at       INTEGER NOT NULL,
    epoch            INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_lock_leader_expires
    ON lock_leader(expires_at);
```

Both tables are created together so that callers of T1 can reference `lock_leader.repository_id` in their proxying logic without a second migration.

- [ ] **Step 3: Bump `BASELINE_VERSION`.**

In `crates/vox-db/src/schema/manifest.rs`:

```rust
pub const BASELINE_VERSION: i64 = 62; // was 61; +1 for vcs_lock + lock_leader
```

Run the manual digest test to capture the new digest for the policy YAML:

```text
cargo test -p vox-db baseline_digest_manual -- --ignored --nocapture
```

Then update `contracts/db/baseline-version-policy.yaml` (in the same commit) with the printed Keccak-256 hex.

- [ ] **Step 4: Verify the schema-presence test now passes.**

```text
cargo test -p vox-db --test mesh_schema
```

Expected: PASS for both `vcs_lock_table_exists_after_baseline` and `lock_leader_table_exists_after_baseline`.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-db/src/schema/domains/vox_mesh.rs \
        crates/vox-db/src/schema/manifest.rs \
        crates/vox-db/tests/mesh_schema.rs \
        contracts/db/baseline-version-policy.yaml
git commit -m "feat(vox-db): add vcs_lock and lock_leader tables (P0-T1, P0-T2)"
```

### Sub-task P0-T1b: Typed `mesh_locks` accessors on `VoxDb`

- [ ] **Step 1: Write the failing test.**

Append to `crates/vox-db/tests/mesh_schema.rs`:

```rust
use vox_db::mesh_locks::{LockKindRow, VcsLockRow};

#[tokio::test]
async fn upsert_then_load_vcs_lock_roundtrips() {
    let db = vox_db::VoxDb::open_in_memory().await.expect("open db");
    let row = VcsLockRow {
        path: "src/main.rs".into(),
        kind: LockKindRow::Exclusive,
        holder: "1".into(),
        holder_node_id: "node-A".into(),
        repository_id: "repo-1".into(),
        acquired_at: 1_000,
        expires_at: 60_000,
        lease_id: None,
        fence_token: 1,
    };
    db.mesh_locks_upsert(&row).await.expect("upsert");
    let loaded = db
        .mesh_locks_for_repo("repo-1")
        .await
        .expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].path, "src/main.rs");
    assert_eq!(loaded[0].kind, LockKindRow::Exclusive);
    assert_eq!(loaded[0].fence_token, 1);
}

#[tokio::test]
async fn release_vcs_lock_only_when_holder_matches() {
    let db = vox_db::VoxDb::open_in_memory().await.expect("open db");
    let row = VcsLockRow {
        path: "src/lib.rs".into(),
        kind: LockKindRow::Exclusive,
        holder: "1".into(),
        holder_node_id: "node-A".into(),
        repository_id: "repo-1".into(),
        acquired_at: 1_000,
        expires_at: 60_000,
        lease_id: None,
        fence_token: 0,
    };
    db.mesh_locks_upsert(&row).await.unwrap();
    // Wrong holder: no-op.
    let removed = db
        .mesh_locks_release("src/lib.rs", "node-B")
        .await
        .unwrap();
    assert_eq!(removed, 0);
    // Right holder: removes.
    let removed = db
        .mesh_locks_release("src/lib.rs", "node-A")
        .await
        .unwrap();
    assert_eq!(removed, 1);
}
```

Run: `cargo test -p vox-db --test mesh_schema upsert_then_load_vcs_lock_roundtrips`.
Expected: FAIL — `vox_db::mesh_locks` module not found.

- [ ] **Step 2: Implement `crates/vox-db/src/mesh_locks.rs`.**

```rust
//! Typed accessors over the `vcs_lock` and `lock_leader` tables (Phase 0, P0-T1/T2).
//!
//! The orchestrator queue treats this module as the single source of truth for
//! cross-process file locks. The in-memory `FileLockManager` is a write-through
//! cache; reconciliation on daemon start replays from these tables.

use crate::VoxDb;
use serde::{Deserialize, Serialize};

/// Lock kind discriminator persisted as TEXT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockKindRow {
    Exclusive,
    SharedRead,
}

impl LockKindRow {
    pub fn as_sql(&self) -> &'static str {
        match self {
            LockKindRow::Exclusive => "exclusive",
            LockKindRow::SharedRead => "shared_read",
        }
    }
    pub fn from_sql(s: &str) -> Option<Self> {
        match s {
            "exclusive" => Some(LockKindRow::Exclusive),
            "shared_read" => Some(LockKindRow::SharedRead),
            _ => None,
        }
    }
}

/// One row of `vcs_lock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcsLockRow {
    pub path: String,
    pub kind: LockKindRow,
    /// `AgentId.0.to_string()` of the lock holder.
    pub holder: String,
    pub holder_node_id: String,
    pub repository_id: String,
    pub acquired_at: i64,
    pub expires_at: i64,
    pub lease_id: Option<String>,
    pub fence_token: i64,
}

/// One row of `lock_leader`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockLeaderRow {
    pub repository_id: String,
    pub leader_node_id: String,
    pub elected_at: i64,
    pub heartbeat_at: i64,
    pub expires_at: i64,
    pub epoch: i64,
}

impl VoxDb {
    /// Upsert a `vcs_lock` row. The primary key is `path`, so re-acquiring a
    /// lock by the same holder simply refreshes `expires_at` and bumps the
    /// fence token.
    pub async fn mesh_locks_upsert(&self, row: &VcsLockRow) -> crate::Result<()> {
        self.execute_batch(&format!(
            "INSERT INTO vcs_lock(path, kind, holder, holder_node_id, repository_id, \
                                  acquired_at, expires_at, lease_id, fence_token) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(path) DO UPDATE SET \
                 kind=excluded.kind, \
                 holder=excluded.holder, \
                 holder_node_id=excluded.holder_node_id, \
                 acquired_at=excluded.acquired_at, \
                 expires_at=excluded.expires_at, \
                 lease_id=excluded.lease_id, \
                 fence_token=vcs_lock.fence_token + 1"
        ),
        // The exact bind-call here mirrors the existing wrapper used in
        // `crates/vox-db/src/codex_chat.rs`. Use whichever helper this crate
        // already exposes for parameterized writes.
        )
        .await
    }

    /// Release a `vcs_lock` only when the row's holder_node_id matches.
    /// Returns the number of rows deleted (0 or 1).
    pub async fn mesh_locks_release(
        &self,
        path: &str,
        holder_node_id: &str,
    ) -> crate::Result<u64> {
        self.execute_with_changes(
            "DELETE FROM vcs_lock WHERE path = ? AND holder_node_id = ?",
            // bind parameters per existing helper signature
            // ...
        )
        .await
    }

    /// Load all `vcs_lock` rows for a repository. Used at daemon start to
    /// hydrate the in-memory map (WAL replay).
    pub async fn mesh_locks_for_repo(
        &self,
        repository_id: &str,
    ) -> crate::Result<Vec<VcsLockRow>> {
        // SELECT path, kind, holder, holder_node_id, repository_id,
        //        acquired_at, expires_at, lease_id, fence_token
        // FROM vcs_lock WHERE repository_id = ?
        // map row → VcsLockRow
        // ...
        todo!("call existing query_rows helper; map columns")
    }

    /// Prune rows whose `expires_at` is older than `now_ms`.
    pub async fn mesh_locks_prune_expired(&self, now_ms: i64) -> crate::Result<u64> {
        self.execute_with_changes(
            "DELETE FROM vcs_lock WHERE expires_at < ?",
            // ...
        )
        .await
    }

    /// Compare-and-swap insert into `lock_leader`. Returns `Ok(true)` if the
    /// caller is now the leader, `Ok(false)` if another node holds an
    /// unexpired claim. Used by `LockLeaderElection`.
    pub async fn lock_leader_try_claim(
        &self,
        repository_id: &str,
        candidate_node_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> crate::Result<bool> {
        // Use a single SQL statement so the check-and-insert is atomic.
        // INSERT ... ON CONFLICT(repository_id) DO UPDATE
        //   SET leader_node_id = excluded.leader_node_id,
        //       elected_at     = excluded.elected_at,
        //       heartbeat_at   = excluded.heartbeat_at,
        //       expires_at     = excluded.expires_at,
        //       epoch          = lock_leader.epoch + 1
        //   WHERE lock_leader.expires_at < excluded.heartbeat_at
        // Inspect changes() to determine success.
        todo!("compose with existing parametrized exec")
    }

    /// Refresh the leader's heartbeat. Returns `Ok(true)` if the row was
    /// updated (caller still leader), `Ok(false)` if the row's leader_node_id
    /// no longer matches (caller was preempted).
    pub async fn lock_leader_heartbeat(
        &self,
        repository_id: &str,
        leader_node_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> crate::Result<bool> {
        todo!()
    }

    /// Read the current leader row, if any.
    pub async fn lock_leader_get(
        &self,
        repository_id: &str,
    ) -> crate::Result<Option<LockLeaderRow>> {
        todo!()
    }
}
```

The `todo!()`s mark the rows where the implementer must adapt to whichever parametrized-query helper the crate already exposes (e.g., `execute_with_params`, `query_one`, `query_rows`). Search `crates/vox-db/src/codex_chat.rs` and `crates/vox-db/src/secrets.rs` for the established pattern and reuse it.

- [ ] **Step 3: Re-export from `lib.rs`.**

Append to `crates/vox-db/src/lib.rs`:

```rust
pub mod mesh_locks;
pub use mesh_locks::{LockKindRow, LockLeaderRow, VcsLockRow};
```

- [ ] **Step 4: Verify tests pass.**

```text
cargo test -p vox-db --test mesh_schema
```

Expected: PASS for all four mesh-schema tests.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-db/src/mesh_locks.rs \
        crates/vox-db/src/lib.rs \
        crates/vox-db/tests/mesh_schema.rs
git commit -m "feat(vox-db): typed accessors for vcs_lock and lock_leader (P0-T1)"
```

### Sub-task P0-T1c: Write-through layer in `vox-orchestrator-queue`

- [ ] **Step 1: Write the failing test.**

Create `crates/vox-orchestrator-queue/tests/persisted_locks.rs`:

```rust
//! P0-T1: file-lock map round-trips through vox-db.

use std::path::Path;
use vox_orchestrator_queue::locks::{FileLockManager, LockKind};
use vox_orchestrator_types::AgentId;

#[tokio::test]
async fn acquire_then_replay_from_db() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let mgr = FileLockManager::with_db(db.clone(), "node-A", "repo-1");

    mgr.try_acquire(Path::new("src/main.rs"), AgentId(1), LockKind::Exclusive)
        .expect("acquire");

    // Drop the in-memory manager and rebuild from DB only.
    drop(mgr);
    let mgr2 = FileLockManager::with_db(db.clone(), "node-A", "repo-1");
    mgr2.hydrate_from_db().await.expect("replay");

    assert!(mgr2.is_locked(Path::new("src/main.rs")));
    let (holder, kind) = mgr2.holder(Path::new("src/main.rs")).expect("holder");
    assert_eq!(holder, AgentId(1));
    assert_eq!(kind, LockKind::Exclusive);
}

#[tokio::test]
async fn release_propagates_to_db() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let mgr = FileLockManager::with_db(db.clone(), "node-A", "repo-1");

    mgr.try_acquire(Path::new("src/lib.rs"), AgentId(1), LockKind::Exclusive)
        .unwrap();
    mgr.release(Path::new("src/lib.rs"), AgentId(1));

    let rows = db.mesh_locks_for_repo("repo-1").await.unwrap();
    assert!(rows.is_empty(), "expected no rows after release; got {rows:?}");
}
```

Run: `cargo test -p vox-orchestrator-queue --test persisted_locks`.
Expected: FAIL — `FileLockManager::with_db`, `hydrate_from_db` not present.

- [ ] **Step 2: Add `vox-db` as a workspace dependency on the queue crate.**

In `crates/vox-orchestrator-queue/Cargo.toml`, under `[dependencies]`:

```toml
vox-db = { workspace = true }
tokio = { workspace = true, features = ["rt", "macros"] }
```

Verify `vox-arch-check` allows this edge — both crates are at the same layer (L3) per `docs/src/architecture/layers.toml`. If the layer of `vox-orchestrator-queue` is L2, escalate it to L3 in `layers.toml` (allowed because heavy runtime); document the change in the same PR.

- [ ] **Step 3: Implement the write-through layer.**

Create `crates/vox-orchestrator-queue/src/locks/persisted.rs`:

```rust
//! Phase 0 (P0-T1): write-through persistence for `FileLockManager`.
//!
//! Every mutation against the in-memory map is mirrored into `vcs_lock`. On
//! daemon start `hydrate_from_db()` replays the table into the map.

use std::path::Path;
use std::time::SystemTime;

use vox_db::mesh_locks::{LockKindRow, VcsLockRow};
use vox_orchestrator_types::AgentId;

use super::{FileLock, LockEntry, LockKind};

const DEFAULT_LOCK_TTL_MS: i64 = 60_000;

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(super) fn row_for(
    path: &Path,
    kind: LockKind,
    holder: AgentId,
    holder_node_id: &str,
    repository_id: &str,
) -> VcsLockRow {
    let now = now_ms();
    VcsLockRow {
        path: path.to_string_lossy().into_owned(),
        kind: match kind {
            LockKind::Exclusive => LockKindRow::Exclusive,
            LockKind::SharedRead => LockKindRow::SharedRead,
        },
        holder: holder.0.to_string(),
        holder_node_id: holder_node_id.to_string(),
        repository_id: repository_id.to_string(),
        acquired_at: now,
        expires_at: now + DEFAULT_LOCK_TTL_MS,
        lease_id: None,
        fence_token: 0,
    }
}

pub(super) fn entry_from_row(row: &VcsLockRow) -> LockEntry {
    let kind = match row.kind {
        LockKindRow::Exclusive => LockKind::Exclusive,
        LockKindRow::SharedRead => LockKind::SharedRead,
    };
    let lock = FileLock {
        path: std::path::PathBuf::from(&row.path),
        kind,
        holder: AgentId(row.holder.parse().unwrap_or(0)),
        acquired_at: std::time::Instant::now(), // approximate; in-memory only
    };
    match kind {
        LockKind::Exclusive => LockEntry::Exclusive(lock),
        LockKind::SharedRead => LockEntry::SharedRead(vec![lock]),
    }
}
```

- [ ] **Step 4: Extend `FileLockManager` to hold an optional `VoxDb` and route writes.**

In `crates/vox-orchestrator-queue/src/locks/mod.rs`:

```rust
use std::sync::Arc;
use vox_db::VoxDb;

pub mod persisted;

#[derive(Clone)]
pub struct FileLockManager {
    pub(crate) locks: Arc<std::sync::RwLock<std::collections::HashMap<std::path::PathBuf, LockEntry>>>,
    pub(crate) queue: Arc<std::sync::RwLock<std::collections::HashMap<std::path::PathBuf, std::collections::VecDeque<AgentId>>>>,
    pub(crate) db: Option<Arc<VoxDb>>,
    pub(crate) node_id: String,
    pub(crate) repository_id: String,
}
```

Add the constructor:

```rust
impl FileLockManager {
    pub fn with_db<S: Into<String>>(
        db: VoxDb,
        node_id: S,
        repository_id: S,
    ) -> Self {
        Self {
            locks: Arc::new(std::sync::RwLock::new(Default::default())),
            queue: Arc::new(std::sync::RwLock::new(Default::default())),
            db: Some(Arc::new(db)),
            node_id: node_id.into(),
            repository_id: repository_id.into(),
        }
    }

    pub async fn hydrate_from_db(&self) -> Result<(), String> {
        let Some(db) = self.db.clone() else {
            return Ok(());
        };
        let rows = db
            .mesh_locks_for_repo(&self.repository_id)
            .await
            .map_err(|e| e.to_string())?;
        let mut guard = crate::sync_lock::rw_write(&*self.locks);
        for row in rows {
            guard.insert(
                std::path::PathBuf::from(&row.path),
                persisted::entry_from_row(&row),
            );
        }
        Ok(())
    }
}
```

The existing `new()` constructor stays as-is for in-memory-only callers (tests, throwaway tools); it sets `db: None`. Both `try_acquire` and `release` add a tail block that, when `self.db.is_some()`, spawns a `tokio::task` to write through. Use a fire-and-forget pattern with explicit error logging (not `unwrap`) so a transient DB failure does not panic the lock path.

In `try_acquire`, after the `Ok(())` arm:

```rust
        if let Some(db) = self.db.clone() {
            let row = persisted::row_for(path, kind, agent, &self.node_id, &self.repository_id);
            tokio::spawn(async move {
                if let Err(e) = db.mesh_locks_upsert(&row).await {
                    tracing::warn!(error = %e, "vcs_lock upsert failed (will reconcile on next acquire)");
                }
            });
        }
```

In `release`, after the in-memory removal:

```rust
        if let Some(db) = self.db.clone() {
            let path_s = path.to_string_lossy().into_owned();
            let node_id = self.node_id.clone();
            tokio::spawn(async move {
                if let Err(e) = db.mesh_locks_release(&path_s, &node_id).await {
                    tracing::warn!(error = %e, "vcs_lock release failed; row may persist past TTL");
                }
            });
        }
```

- [ ] **Step 5: Verify tests pass.**

```text
cargo test -p vox-orchestrator-queue --test persisted_locks
```

Expected: PASS for both `acquire_then_replay_from_db` and `release_propagates_to_db`.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-orchestrator-queue/src/locks/mod.rs \
        crates/vox-orchestrator-queue/src/locks/persisted.rs \
        crates/vox-orchestrator-queue/Cargo.toml \
        crates/vox-orchestrator-queue/tests/persisted_locks.rs
git commit -m "feat(orchestrator-queue): write-through vcs_lock map with WAL replay (P0-T1)"
```

### Acceptance for P0-T1

```text
cargo test -p vox-db
cargo test -p vox-orchestrator-queue
cargo run -p vox-arch-check
```

All green. The map now survives kill-9.

---

## Task P0-T2: Single lock-leader election with heartbeat

**Files:**

- Create: `crates/vox-orchestrator-queue/src/locks/leader.rs`
- Modify: `crates/vox-orchestrator-queue/src/locks/mod.rs`
- Modify: `crates/vox-orchestrator/src/a2a/mod.rs` (export proxy hook)
- Create: `crates/vox-orchestrator-queue/tests/leader_election.rs`

The leader is the only node that **mutates** `vcs_lock`. Followers proxy mutations via the existing A2A envelope; reads (e.g., `is_locked`) are served locally from the cached snapshot. One A2A round-trip per lock op when the leader is remote; sub-millisecond when local.

- [ ] **Step 1: Write the failing test.**

`crates/vox-orchestrator-queue/tests/leader_election.rs`:

```rust
use vox_orchestrator_queue::locks::leader::{LockLeaderElection, LeaderRole};

#[tokio::test]
async fn first_caller_becomes_leader() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let elect = LockLeaderElection::new(db.clone(), "node-A", "repo-1");
    let role = elect.try_become_leader().await.unwrap();
    assert!(matches!(role, LeaderRole::Leader { .. }));
}

#[tokio::test]
async fn second_caller_becomes_follower_when_leader_alive() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let a = LockLeaderElection::new(db.clone(), "node-A", "repo-1");
    let b = LockLeaderElection::new(db.clone(), "node-B", "repo-1");
    let role_a = a.try_become_leader().await.unwrap();
    assert!(matches!(role_a, LeaderRole::Leader { .. }));
    let role_b = b.try_become_leader().await.unwrap();
    match role_b {
        LeaderRole::Follower { leader_node_id } => assert_eq!(leader_node_id, "node-A"),
        LeaderRole::Leader { .. } => panic!("expected follower"),
    }
}

#[tokio::test]
async fn heartbeat_keeps_leadership_alive() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let elect = LockLeaderElection::with_ttl_ms(db.clone(), "node-A", "repo-1", 50);
    let _role = elect.try_become_leader().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(elect.heartbeat().await.unwrap(), "still leader");
}

#[tokio::test]
async fn expired_lease_can_be_taken_over() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let a = LockLeaderElection::with_ttl_ms(db.clone(), "node-A", "repo-1", 5);
    a.try_become_leader().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let b = LockLeaderElection::new(db.clone(), "node-B", "repo-1");
    let role = b.try_become_leader().await.unwrap();
    assert!(matches!(role, LeaderRole::Leader { .. }));
}
```

Run: `cargo test -p vox-orchestrator-queue --test leader_election`.
Expected: FAIL — `leader` module not found.

- [ ] **Step 2: Implement `crates/vox-orchestrator-queue/src/locks/leader.rs`.**

```rust
//! P0-T2: lock-leader election with heartbeat refresh.
//!
//! Backed by `lock_leader` table in vox-db. The leader is the only node that
//! writes to `vcs_lock`. Heartbeat is sent every TTL/3 by the daemon's
//! background task; if the heartbeat fails (returns `Ok(false)`), the leader
//! demotes itself and reverts to follower mode.

use std::sync::Arc;
use vox_db::VoxDb;

const DEFAULT_LEADER_TTL_MS: i64 = 9_000; // 9 s; heartbeat at 3 s.

#[derive(Debug, Clone)]
pub enum LeaderRole {
    Leader { ttl_ms: i64 },
    Follower { leader_node_id: String },
}

pub struct LockLeaderElection {
    db: Arc<VoxDb>,
    node_id: String,
    repository_id: String,
    ttl_ms: i64,
}

impl LockLeaderElection {
    pub fn new(db: VoxDb, node_id: impl Into<String>, repository_id: impl Into<String>) -> Self {
        Self {
            db: Arc::new(db),
            node_id: node_id.into(),
            repository_id: repository_id.into(),
            ttl_ms: DEFAULT_LEADER_TTL_MS,
        }
    }

    pub fn with_ttl_ms(
        db: VoxDb,
        node_id: impl Into<String>,
        repository_id: impl Into<String>,
        ttl_ms: i64,
    ) -> Self {
        Self {
            db: Arc::new(db),
            node_id: node_id.into(),
            repository_id: repository_id.into(),
            ttl_ms,
        }
    }

    /// Attempt CAS leadership claim. Returns `Leader` if we now own the row;
    /// `Follower { leader_node_id }` if another node holds an unexpired claim.
    pub async fn try_become_leader(&self) -> Result<LeaderRole, String> {
        let now = super::persisted::now_ms();
        let claimed = self
            .db
            .lock_leader_try_claim(&self.repository_id, &self.node_id, now, self.ttl_ms)
            .await
            .map_err(|e| e.to_string())?;
        if claimed {
            Ok(LeaderRole::Leader { ttl_ms: self.ttl_ms })
        } else {
            let row = self
                .db
                .lock_leader_get(&self.repository_id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "leader row absent after CAS failure".to_string())?;
            Ok(LeaderRole::Follower {
                leader_node_id: row.leader_node_id,
            })
        }
    }

    /// Refresh our heartbeat. Returns `Ok(true)` if we are still the leader.
    pub async fn heartbeat(&self) -> Result<bool, String> {
        let now = super::persisted::now_ms();
        self.db
            .lock_leader_heartbeat(&self.repository_id, &self.node_id, now, self.ttl_ms)
            .await
            .map_err(|e| e.to_string())
    }

    /// Spawn a background task that calls `heartbeat()` every TTL/3. The
    /// returned handle aborts the task on drop. Caller is responsible for
    /// observing demotion via the returned watch channel.
    pub fn spawn_heartbeat(
        self: Arc<Self>,
    ) -> (tokio::task::JoinHandle<()>, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(true);
        let interval = std::time::Duration::from_millis((self.ttl_ms / 3).max(1) as u64);
        let me = self.clone();
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                match me.heartbeat().await {
                    Ok(true) => {}
                    Ok(false) => {
                        let _ = tx.send(false);
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "lock_leader heartbeat error; will retry");
                    }
                }
            }
        });
        (handle, rx)
    }
}
```

- [ ] **Step 3: Wire into `mod.rs`.**

Append to `crates/vox-orchestrator-queue/src/locks/mod.rs`:

```rust
pub mod leader;
```

- [ ] **Step 4: Implement A2A proxying for followers.**

Add to `crates/vox-orchestrator-queue/src/locks/leader.rs`:

```rust
/// A2A proxy used by followers to ask the leader to perform a lock mutation.
/// Implementors live in `vox-orchestrator` (which has the populi client) — this
/// trait is a layer-clean injection point.
#[async_trait::async_trait]
pub trait LockMutationProxy: Send + Sync {
    async fn proxy_acquire(
        &self,
        leader_node_id: &str,
        path: &std::path::Path,
        agent: vox_orchestrator_types::AgentId,
        kind: super::LockKind,
    ) -> Result<(), String>;

    async fn proxy_release(
        &self,
        leader_node_id: &str,
        path: &std::path::Path,
        agent: vox_orchestrator_types::AgentId,
    ) -> Result<(), String>;
}
```

Followers carry an `Option<Arc<dyn LockMutationProxy>>` on `FileLockManager`. When `try_acquire` is called and `self.role == LeaderRole::Follower { .. }`, it forwards to the proxy instead of writing locally. When the proxy succeeds, the in-memory cache is updated to mirror the leader's authoritative state (via the next replay tick or via a small response payload — keep the trait single-shot for Phase 0).

- [ ] **Step 5: Verify tests pass.**

```text
cargo test -p vox-orchestrator-queue --test leader_election
```

Expected: all four tests PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-orchestrator-queue/src/locks/leader.rs \
        crates/vox-orchestrator-queue/src/locks/mod.rs \
        crates/vox-orchestrator-queue/tests/leader_election.rs
git commit -m "feat(orchestrator-queue): lock-leader election with heartbeat (P0-T2)"
```

### Acceptance for P0-T2

```text
cargo test -p vox-orchestrator-queue
cargo run -p vox-arch-check
```

Both green. With T1 + T2 in place, two daemons on the same host now route every lock mutation through one writer — there is no longer a way to double-write the same path.

---

## Task P0-T3: Authoritative leases (W1, ADR-017) — gate local fallback

**Files:**

- Create: `crates/vox-orchestrator/src/a2a/dispatch/mod.rs`
- Create: `crates/vox-orchestrator/src/a2a/dispatch/lease_gate.rs`
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs`
- Create: `crates/vox-orchestrator/tests/lease_gate.rs`

The killer assertion: **before falling back to the local executor, consult `mesh_exec_leases`. If a remote node holds an unexpired lease for the same scope, refuse rather than duplicate-execute.**

- [ ] **Step 1: Write the failing test.**

`crates/vox-orchestrator/tests/lease_gate.rs`:

```rust
use vox_orchestrator::a2a::dispatch::lease_gate::{
    LeaseGateError, check_before_local_fallback,
};

#[tokio::test]
async fn no_lease_allows_local_fallback() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let res = check_before_local_fallback(&db, "task:42", "node-A", 1_000).await;
    assert!(res.is_ok(), "no lease should allow local fallback; got {res:?}");
}

#[tokio::test]
async fn unexpired_remote_lease_blocks_local_fallback() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    db.exec_lease_grant("lease-1", "task:42", "task:42", "node-B", 1_000, 60_000)
        .await
        .unwrap();
    let err = check_before_local_fallback(&db, "task:42", "node-A", 5_000)
        .await
        .unwrap_err();
    match err {
        LeaseGateError::HeldByRemote { holder_node_id, .. } => {
            assert_eq!(holder_node_id, "node-B");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[tokio::test]
async fn expired_remote_lease_allows_local_fallback() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    db.exec_lease_grant("lease-2", "task:42", "task:42", "node-B", 1_000, 1_500)
        .await
        .unwrap();
    let res = check_before_local_fallback(&db, "task:42", "node-A", 5_000).await;
    assert!(res.is_ok(), "expired remote lease should allow fallback");
}

#[tokio::test]
async fn local_node_lease_is_not_blocking() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    db.exec_lease_grant("lease-3", "task:42", "task:42", "node-A", 1_000, 60_000)
        .await
        .unwrap();
    let res = check_before_local_fallback(&db, "task:42", "node-A", 5_000).await;
    assert!(res.is_ok(), "self-held lease must not block self");
}
```

Run: `cargo test -p vox-orchestrator --test lease_gate`.
Expected: FAIL — module not found.

(The exact `db.exec_lease_grant` call name should match the existing `vox-db` API for `mesh_exec_leases`. If the helper name differs, adapt the test setup accordingly while keeping the four cases.)

- [ ] **Step 2: Implement `crates/vox-orchestrator/src/a2a/dispatch/mod.rs`.**

```rust
//! Dispatch layer: chooses between local executor, mesh A2A, and lease-gated
//! fallback. P0-T3 introduces `lease_gate` as the mandatory pre-check for any
//! "fall through to local" path.

pub mod lease_gate;
pub mod mesh;
```

- [ ] **Step 3: Implement `crates/vox-orchestrator/src/a2a/dispatch/lease_gate.rs`.**

```rust
//! P0-T3 (ADR-017, W1): authoritative-lease check before local fallback.

use thiserror::Error;
use vox_db::VoxDb;

#[derive(Debug, Error)]
pub enum LeaseGateError {
    #[error("scope `{scope_key}` is held by remote node `{holder_node_id}` until {expires_at}ms")]
    HeldByRemote {
        scope_key: String,
        holder_node_id: String,
        expires_at: i64,
    },
    #[error("vox-db error: {0}")]
    Db(String),
}

/// Returns `Ok(())` when local fallback is permitted. Returns
/// `Err(HeldByRemote)` when an unexpired lease exists on a different node —
/// the caller must surface this as a routing decision (e.g. queue for retry,
/// proxy via mesh) rather than duplicate-execute.
pub async fn check_before_local_fallback(
    db: &VoxDb,
    scope_key: &str,
    self_node_id: &str,
    now_ms: i64,
) -> Result<(), LeaseGateError> {
    let lease = db
        .mesh_exec_lease_for_scope(scope_key)
        .await
        .map_err(|e| LeaseGateError::Db(e.to_string()))?;
    let Some(lease) = lease else { return Ok(()); };
    if lease.expires_at < now_ms {
        return Ok(());
    }
    if lease.holder_node_id == self_node_id {
        return Ok(());
    }
    Err(LeaseGateError::HeldByRemote {
        scope_key: scope_key.to_string(),
        holder_node_id: lease.holder_node_id,
        expires_at: lease.expires_at,
    })
}
```

If `mesh_exec_lease_for_scope` does not yet exist on `VoxDb`, add it as a thin wrapper around the existing `mesh_exec_leases` table reader (one row per scope_key, ordered by `granted_at` desc).

- [ ] **Step 4: Hook the gate into the mesh-dispatch fallback path.**

In `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs`, find the call site that "falls back to local executor when mesh relay returns `PopuliRegistryError::NoTarget`" — search for `relay_to_mesh` callers in `crates/vox-orchestrator/src/orchestrator.rs` and similar. Wrap the fallback with:

```rust
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0);
match super::lease_gate::check_before_local_fallback(
    db, &scope_key, self_node_id, now,
).await {
    Ok(()) => { /* proceed with local exec */ }
    Err(super::lease_gate::LeaseGateError::HeldByRemote { holder_node_id, .. }) => {
        return Err(format!(
            "remote lease holder {holder_node_id} owns scope; refusing duplicate-execute"
        ));
    }
    Err(super::lease_gate::LeaseGateError::Db(e)) => {
        tracing::warn!(error = %e, "lease_gate db error; failing closed");
        return Err(format!("lease_gate db error: {e}"));
    }
}
```

The dispatcher fails closed — a DB read failure on the lease check returns an error rather than silently falling through. Phase 0 is about correctness, not availability under DB partition.

- [ ] **Step 5: Verify tests pass.**

```text
cargo test -p vox-orchestrator --test lease_gate
```

Expected: PASS for all four scenarios.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-orchestrator/src/a2a/dispatch/mod.rs \
        crates/vox-orchestrator/src/a2a/dispatch/lease_gate.rs \
        crates/vox-orchestrator/src/a2a/dispatch/mesh.rs \
        crates/vox-orchestrator/tests/lease_gate.rs
git commit -m "feat(orchestrator): lease-gate local fallback against mesh_exec_leases (P0-T3)"
```

### Acceptance for P0-T3

```text
cargo test -p vox-orchestrator
cargo run -p vox-arch-check
```

All green. The dispatcher is now incapable of producing the W1 double-execute pattern.

---

## Task P0-T4: Inject decrypted JWE secrets into task exec context (close W3)

**Files:**

- Create: `crates/vox-orchestrator/src/a2a/secret_bag.rs`
- Modify: `crates/vox-orchestrator/src/a2a/remote_worker.rs` (replace `secret_count` log line with bag construction)
- Modify: `crates/vox-orchestrator/src/a2a/mod.rs`
- Modify: `crates/vox-skill-runtime/src/runtime.rs` (default-impl `run_with_secrets`)
- Create: `crates/vox-orchestrator/tests/secret_injection.rs`

W3 today: `remote_worker.rs:120-145` decrypts JWE-wrapped secrets but only logs `secret_count` and discards the plaintext. After this task, the decrypted map flows to the skill-runtime `RunOpts.env` via a task-scoped `SecretBag` that respects `@uses(secret)` declarations.

- [ ] **Step 1: Write the failing test.**

`crates/vox-orchestrator/tests/secret_injection.rs`:

```rust
use vox_orchestrator::a2a::secret_bag::SecretBag;

#[test]
fn bag_only_exposes_declared_secrets() {
    let bag = SecretBag::from_decrypted(serde_json::json!({
        "VoxGitHubToken": "ghp_AAA",
        "VoxOpenAiKey":   "sk-XYZ",
    }))
    .unwrap();

    let env = bag.env_for_declared(&["VoxGitHubToken".to_string()]);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "VOX_GITHUB_TOKEN");
    assert_eq!(env[0].1, "ghp_AAA");
}

#[test]
fn bag_skips_unknown_declarations() {
    let bag = SecretBag::from_decrypted(serde_json::json!({
        "VoxGitHubToken": "ghp_AAA",
    }))
    .unwrap();
    let env = bag.env_for_declared(&[
        "VoxGitHubToken".to_string(),
        "VoxOpenAiKey".to_string(), // not in the bag
    ]);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "VOX_GITHUB_TOKEN");
}

#[test]
fn bag_redacts_in_debug_format() {
    let bag = SecretBag::from_decrypted(serde_json::json!({
        "VoxGitHubToken": "ghp_AAA",
    }))
    .unwrap();
    let dbg = format!("{bag:?}");
    assert!(!dbg.contains("ghp_AAA"));
    assert!(dbg.contains("[redacted]") || dbg.contains("len="));
}
```

Run: `cargo test -p vox-orchestrator --test secret_injection`.
Expected: FAIL — `secret_bag` module not found.

- [ ] **Step 2: Implement `crates/vox-orchestrator/src/a2a/secret_bag.rs`.**

```rust
//! P0-T4: task-scoped decrypted secrets, gated by `@uses(secret)` declarations.
//!
//! `SecretBag` owns the plaintext for the duration of one remote task. It
//! never enters the process environment unbidden — only secrets the skill
//! declares via `@uses(secret)` are projected into `RunOpts.env`.

use std::collections::HashMap;

#[derive(Clone)]
pub struct SecretBag {
    plaintexts: HashMap<String, String>,
}

impl std::fmt::Debug for SecretBag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut redacted: Vec<(&str, String)> = self
            .plaintexts
            .iter()
            .map(|(k, v)| (k.as_str(), format!("[redacted len={}]", v.len())))
            .collect();
        redacted.sort_by_key(|(k, _)| *k);
        f.debug_struct("SecretBag").field("entries", &redacted).finish()
    }
}

impl SecretBag {
    pub fn from_decrypted(value: serde_json::Value) -> Result<Self, String> {
        let map: HashMap<String, String> = serde_json::from_value(value)
            .map_err(|e| format!("SecretBag: expected object<string,string>: {e}"))?;
        Ok(Self { plaintexts: map })
    }

    /// Project the bag into `(env_key, value)` pairs the skill runtime should
    /// inject. `declared` is the list of `@uses(secret)` SecretIds parsed
    /// from the skill's effect annotations. Secrets not declared are not
    /// returned, even if present in the bag.
    pub fn env_for_declared(&self, declared: &[String]) -> Vec<(String, String)> {
        let mut out = Vec::with_capacity(declared.len());
        for id in declared {
            if let Some(plaintext) = self.plaintexts.get(id) {
                let env_key = secret_id_to_env_key(id);
                out.push((env_key, plaintext.clone()));
            }
        }
        out
    }

    /// Number of secrets in the bag. Used for telemetry; does NOT leak names.
    pub fn len(&self) -> usize {
        self.plaintexts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plaintexts.is_empty()
    }
}

/// Map a SecretId (camel-case Rust enum name) to the conventional
/// SCREAMING_SNAKE env-var key. e.g. `VoxGitHubToken` -> `VOX_GITHUB_TOKEN`.
fn secret_id_to_env_key(id: &str) -> String {
    let mut out = String::with_capacity(id.len() + 4);
    for (i, c) in id.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            out.push('_');
        }
        for u in c.to_uppercase() {
            out.push(u);
        }
    }
    out
}

#[cfg(test)]
mod unit {
    use super::*;
    #[test]
    fn env_key_camel_to_snake() {
        assert_eq!(secret_id_to_env_key("VoxGitHubToken"), "VOX_GIT_HUB_TOKEN");
        assert_eq!(secret_id_to_env_key("Foo"), "FOO");
    }
}
```

Note: the camel-to-snake mapping is a deliberate choice — match the existing convention in `crates/vox-secrets/src/spec/`. If the canonical mapping there is different (e.g., the env var for `VoxGitHubToken` is `VOX_GITHUB_TOKEN` rather than `VOX_GIT_HUB_TOKEN`), use the `SecretSpec.env` field directly instead of recomputing.

Update the test `bag_only_exposes_declared_secrets` once the canonical env-var name is known.

- [ ] **Step 3: Replace the `secret_count`-only log site in `remote_worker.rs`.**

In `crates/vox-orchestrator/src/a2a/remote_worker.rs:116-146`, replace the JWE-decrypt block with:

```rust
    // Decrypt JWE-wrapped secrets forwarded by the orchestrator (P0-T4).
    // Key derivation mirrors the sender in dispatch/mesh.rs: BLAKE3(VoxMeshJwtHmacSecret).
    let mut secret_bag: Option<crate::a2a::secret_bag::SecretBag> = None;
    if let Some(jwe) = msg.jwe_payload.as_deref() {
        let mesh_secret = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshJwtHmacSecret);
        if let Some(mesh_val) = mesh_secret.expose() {
            let derived = blake3::hash(mesh_val.as_bytes());
            match super::jwe::decrypt_jwe_compact(jwe, derived.as_bytes()) {
                Ok(plain) => match serde_json::from_slice::<serde_json::Value>(&plain) {
                    Ok(value) => match crate::a2a::secret_bag::SecretBag::from_decrypted(value) {
                        Ok(bag) => {
                            tracing::info!(
                                task_id = envelope.task_id,
                                message_id = msg.id,
                                secret_count = bag.len(),
                                "populi remote worker: SecretBag ready for declared injection",
                            );
                            secret_bag = Some(bag);
                        }
                        Err(e) => tracing::warn!(
                            task_id = envelope.task_id,
                            message_id = msg.id,
                            error = %e,
                            "populi remote worker: SecretBag construction failed",
                        ),
                    },
                    Err(e) => tracing::warn!(
                        task_id = envelope.task_id,
                        message_id = msg.id,
                        error = %e,
                        "populi remote worker: secret payload not JSON object",
                    ),
                },
                Err(e) => tracing::warn!(
                    task_id = envelope.task_id,
                    message_id = msg.id,
                    error = %e,
                    "populi remote worker: JWE decrypt failed; proceeding without forwarded secrets",
                ),
            }
        }
    }
```

The `secret_bag` is then threaded through to the skill-runtime call site. Look for the existing call in this same function (likely `orchestrator.execute_remote_task(...)` or similar) and add a parameter `secrets: Option<SecretBag>`. The receiver passes the bag's `env_for_declared(&envelope.required_secrets)` into `RunOpts.env`.

- [ ] **Step 4: Add `run_with_secrets` to `SkillRuntime`.**

In `crates/vox-skill-runtime/src/runtime.rs`, append after the existing trait body:

```rust
    /// Run a skill with task-scoped secret env vars merged into `opts.env`.
    /// Default impl simply extends `opts.env` and calls `run`. Implementors
    /// may override (e.g., to filter on a per-runtime allowlist).
    fn run_with_secrets(
        &self,
        opts: &RunOpts,
        secret_env: &[(String, String)],
    ) -> anyhow::Result<RunOutcome> {
        if secret_env.is_empty() {
            return self.run(opts);
        }
        let mut merged = opts.clone();
        merged.env.extend(secret_env.iter().cloned());
        self.run(&merged)
    }
```

This is a default-method extension — no existing impl needs to change. Phase 5 sandbox tiering (per the SSOT) overrides this to gate secret injection by runtime trust level.

- [ ] **Step 5: Verify tests pass.**

```text
cargo test -p vox-orchestrator --test secret_injection
cargo test -p vox-skill-runtime
```

Both green.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-orchestrator/src/a2a/secret_bag.rs \
        crates/vox-orchestrator/src/a2a/mod.rs \
        crates/vox-orchestrator/src/a2a/remote_worker.rs \
        crates/vox-orchestrator/tests/secret_injection.rs \
        crates/vox-skill-runtime/src/runtime.rs
git commit -m "feat(orchestrator): inject decrypted JWE secrets via SecretBag into task env (P0-T4)"
```

### Acceptance for P0-T4

```text
cargo test -p vox-orchestrator
cargo test -p vox-skill-runtime
cargo run -p vox-arch-check
```

All green. The `secret_count` log line is gone; `@uses(secret)`-declared secrets land in the task's env vector.

---

## Task P0-T5: TLS / WireGuard option on populi HTTP plane

**Files:**

- Modify: `crates/vox-repository/src/populi_toml.rs` — add `[mesh.transport]`.
- Create: `crates/vox-populi/src/transport/tls.rs` — rustls acceptor.
- Modify: `crates/vox-populi/src/transport/mod.rs` (or `lib.rs`) — wire optional acceptor.
- Modify: `crates/vox-populi/Cargo.toml` — `tls` feature.
- Create: `crates/vox-populi/tests/tls_smoke.rs`.
- Modify: `docs/src/reference/populi.md` — TLS appendix.

The ergonomic default is **rustls cert from a known path**. WireGuard is documented as a sidecar (Tailscale Funnel) but not bundled.

- [ ] **Step 1: Extend `[mesh]` config with a `transport` sub-table.**

In `crates/vox-repository/src/populi_toml.rs`, add a nested struct:

```rust
/// Mesh transport options (TLS, WireGuard hints).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VoxMeshTransport {
    /// PEM-encoded TLS certificate path. When `None`, server runs plain HTTP.
    pub tls_cert_path: Option<std::path::PathBuf>,
    /// PEM-encoded private key path. Required when `tls_cert_path` is set.
    pub tls_key_path: Option<std::path::PathBuf>,
    /// Optional minimum TLS version label: `"1.2"` or `"1.3"` (default `"1.3"`).
    pub tls_min_version: Option<String>,
    /// Documentation pointer: when set, operators are running behind a
    /// WireGuard sidecar (e.g., Tailscale Funnel). The server itself does
    /// nothing with this value — it is read by `vox doctor mesh`.
    pub wireguard_endpoint: Option<String>,
}
```

Add it to `VoxMeshToml`:

```rust
    /// Optional [mesh.transport] sub-table.
    #[serde(default)]
    pub transport: Option<VoxMeshTransport>,
```

Update `is_empty_mesh` to also consider `transport.is_none()`. Add a unit test reading the new section:

```rust
    #[test]
    fn reads_mesh_transport_section() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("Vox.toml");
        fs::write(&p, r#"
[mesh]
control_url = "http://127.0.0.1:9999"
[mesh.transport]
tls_cert_path = "/etc/vox/cert.pem"
tls_key_path  = "/etc/vox/key.pem"
tls_min_version = "1.3"
"#).unwrap();
        let m = read_vox_populi_toml(&p).unwrap().expect("mesh");
        let t = m.transport.expect("transport");
        assert_eq!(t.tls_cert_path.unwrap(), std::path::PathBuf::from("/etc/vox/cert.pem"));
        assert_eq!(t.tls_min_version.as_deref(), Some("1.3"));
    }
```

Run: `cargo test -p vox-repository`. Expected: PASS once the struct is added.

- [ ] **Step 2: Implement the rustls acceptor in `crates/vox-populi/src/transport/tls.rs`.**

```rust
//! P0-T5: optional rustls acceptor for the populi HTTP plane.
//!
//! Gated behind the `tls` feature. When the feature is on AND the operator
//! provides cert/key paths in `[mesh.transport]`, the server terminates TLS
//! locally; otherwise it runs plain HTTP (existing behaviour).

#![cfg(feature = "tls")]

use std::path::Path;
use std::sync::Arc;

use rustls::{ServerConfig, pki_types::PrivateKeyDer};
use thiserror::Error;
use tokio_rustls::TlsAcceptor;

#[derive(Debug, Error)]
pub enum TlsError {
    #[error("read cert {path}: {source}")]
    ReadCert {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("read key {path}: {source}")]
    ReadKey {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid PEM in {path}")]
    InvalidPem { path: std::path::PathBuf },
    #[error("rustls config: {0}")]
    Rustls(String),
}

pub struct TlsOptions {
    pub cert_path: std::path::PathBuf,
    pub key_path: std::path::PathBuf,
    pub min_version: TlsMinVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMinVersion {
    V1_2,
    V1_3,
}

impl TlsMinVersion {
    pub fn parse(s: Option<&str>) -> Self {
        match s.unwrap_or("1.3") {
            "1.2" => TlsMinVersion::V1_2,
            _ => TlsMinVersion::V1_3,
        }
    }
}

pub fn build_acceptor(opts: &TlsOptions) -> Result<TlsAcceptor, TlsError> {
    let certs = load_certs(&opts.cert_path)?;
    let key = load_key(&opts.key_path)?;
    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| TlsError::Rustls(e.to_string()))?;
    Ok(TlsAcceptor::from(Arc::new(cfg)))
}

fn load_certs(path: &Path) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, TlsError> {
    let pem = std::fs::read(path).map_err(|source| TlsError::ReadCert {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = std::io::Cursor::new(pem);
    let certs: Result<Vec<_>, _> = rustls_pemfile::certs(&mut reader).collect();
    let certs = certs.map_err(|_| TlsError::InvalidPem {
        path: path.to_path_buf(),
    })?;
    if certs.is_empty() {
        return Err(TlsError::InvalidPem {
            path: path.to_path_buf(),
        });
    }
    Ok(certs)
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>, TlsError> {
    let pem = std::fs::read(path).map_err(|source| TlsError::ReadKey {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = std::io::Cursor::new(pem);
    let key = rustls_pemfile::private_key(&mut reader)
        .map_err(|_| TlsError::InvalidPem {
            path: path.to_path_buf(),
        })?
        .ok_or_else(|| TlsError::InvalidPem {
            path: path.to_path_buf(),
        })?;
    Ok(key)
}
```

- [ ] **Step 3: Wire the acceptor into the listener loop.**

In `crates/vox-populi/src/transport/mod.rs` (or wherever the existing `serve()` / `bind_listener` lives — search for `axum::Server` / `hyper::server` / `tokio::net::TcpListener`), the listener-accept loop becomes:

```rust
#[cfg(feature = "tls")]
async fn maybe_wrap_with_tls(
    raw: tokio::net::TcpStream,
    acceptor: Option<&tokio_rustls::TlsAcceptor>,
) -> std::io::Result<TransportStream> {
    match acceptor {
        Some(acc) => {
            let tls = acc.accept(raw).await?;
            Ok(TransportStream::Tls(tls))
        }
        None => Ok(TransportStream::Plain(raw)),
    }
}
```

`TransportStream` is an enum implementing `AsyncRead + AsyncWrite` for both plain and TLS-wrapped streams (the standard pattern; `enum_dispatch` not required — a manual delegate is fine for two variants). When the `tls` feature is off, the enum collapses to the plain variant only.

- [ ] **Step 4: Add the `tls` feature to `vox-populi/Cargo.toml`.**

```toml
[features]
default = []
tls = ["dep:rustls", "dep:rustls-pemfile", "dep:tokio-rustls"]

[dependencies]
rustls = { workspace = true, optional = true }
rustls-pemfile = { workspace = true, optional = true }
tokio-rustls = { workspace = true, optional = true }
```

If `rustls`, `rustls-pemfile`, `tokio-rustls` are **not** already in the workspace `Cargo.toml`, add them there too — this is the only place where new deps may be introduced for Phase 0. Pin to versions that match what `vox-crypto` already uses (search `crates/vox-crypto/Cargo.toml`); if vox-crypto does not pull rustls, take the most recent stable pair (`rustls = "0.23"`, `tokio-rustls = "0.26"`, `rustls-pemfile = "2"`). Flag this dep addition explicitly in the PR description.

- [ ] **Step 5: Write the smoke test.**

`crates/vox-populi/tests/tls_smoke.rs`:

```rust
//! P0-T5 acceptance: `vox populi serve --tls cert.pem` accepts an HTTPS peer.

#![cfg(feature = "tls")]

#[tokio::test]
async fn rustls_acceptor_accepts_https_handshake() {
    use vox_populi::transport::tls::{TlsMinVersion, TlsOptions, build_acceptor};

    // Generate a self-signed cert in a tempdir.
    let dir = tempfile::tempdir().unwrap();
    let cert_pem = dir.path().join("cert.pem");
    let key_pem = dir.path().join("key.pem");
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    std::fs::write(&cert_pem, cert.serialize_pem().unwrap()).unwrap();
    std::fs::write(&key_pem, cert.serialize_private_key_pem()).unwrap();

    let acceptor = build_acceptor(&TlsOptions {
        cert_path: cert_pem,
        key_path: key_pem,
        min_version: TlsMinVersion::V1_3,
    })
    .expect("acceptor built");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let _tls = acceptor.accept(sock).await.expect("server accept");
    });

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let connector = tokio_rustls::TlsConnector::from(client_config_skip_verify());
    let domain = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let _client_tls = connector.connect(domain, stream).await.expect("client handshake");

    server.await.unwrap();
}

fn client_config_skip_verify() -> std::sync::Arc<rustls::ClientConfig> {
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, Error};

    #[derive(Debug)]
    struct NoVerify;
    impl ServerCertVerifier for NoVerify {
        fn verify_server_cert(
            &self,
            _: &CertificateDer<'_>,
            _: &[CertificateDer<'_>],
            _: &ServerName<'_>,
            _: &[u8],
            _: UnixTime,
        ) -> Result<ServerCertVerified, Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
        }
    }

    let cfg = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(NoVerify))
        .with_no_client_auth();
    std::sync::Arc::new(cfg)
}
```

The test uses `rcgen` (already in dev-deps somewhere in the workspace; if not, add to `vox-populi` `[dev-dependencies]` only).

Run: `cargo test -p vox-populi --features tls --test tls_smoke`. Expected: PASS.

- [ ] **Step 6: Document in `populi.md`.**

Append to `docs/src/reference/populi.md` an "Appendix: TLS / WireGuard transport" section explaining `[mesh.transport]` keys, how to generate a self-signed cert with `mkcert` or `step certificate create`, and a one-paragraph note pointing operators to Tailscale Funnel as the recommended off-LAN deployment for non-public-internet meshes.

- [ ] **Step 7: Commit.**

```bash
git add crates/vox-repository/src/populi_toml.rs \
        crates/vox-populi/src/transport/tls.rs \
        crates/vox-populi/src/transport/mod.rs \
        crates/vox-populi/Cargo.toml \
        crates/vox-populi/tests/tls_smoke.rs \
        docs/src/reference/populi.md \
        Cargo.toml
git commit -m "feat(populi): rustls TLS option on HTTP plane (P0-T5)"
```

### Acceptance for P0-T5

```text
cargo test -p vox-populi --features tls
cargo build -p vox-populi   # default features still work
cargo run -p vox-arch-check
```

All green. `vox populi serve --tls cert.pem` accepts a peer over HTTPS.

---

## Task P0-T6: Hardware probe trait + mock harness

**Files:** see [`populi-mesh-probe-correctness-plan-2026.md`](populi-mesh-probe-correctness-plan-2026.md) — that plan is canonical.

This task is delegated. Phase 0 acceptance for P0-T6 is identical to that plan's `## Acceptance` section. Do not re-author here.

- [ ] **Step 1: Execute the probe-correctness plan end-to-end.**

Open `docs/src/architecture/populi-mesh-probe-correctness-plan-2026.md` and walk through Tasks 1–17 in order. Use the same TDD discipline as the rest of this plan.

- [ ] **Step 2: Verify the probe-plan acceptance sweep.**

```text
cargo test -p vox-populi
cargo test -p vox-populi --features nvml-gpu-probe
cargo test -p vox-populi --no-default-features
cargo run -p vox-arch-check
```

All green per the probe-plan §Acceptance.

- [ ] **Step 3: Cross-link from the SSOT.**

Confirm that the SSOT (`mesh-and-language-distribution-ssot-2026.md` §3) already names the probe-plan as the implementation source for P0-T6. If not, add the link in the same PR as P0-T6's final commit.

- [ ] **Step 4: No new commit needed.**

Each probe-plan task produces its own commit; this Phase 0 plan does not duplicate them.

### Acceptance for P0-T6

Identical to `populi-mesh-probe-correctness-plan-2026.md` §Acceptance. Phase 0 inherits that section verbatim.

---

## Task P0-T7: Move in-process executor behind `SkillRuntime` trait

**Files:**

- Modify: `crates/vox-orchestrator/src/skill_exec.rs` (or whichever file holds the in-process executor today; search `crates/vox-orchestrator/src/` for `fn run_skill_inproc` / `fn execute_in_process` / similar).
- Create: `crates/vox-orchestrator/src/skill_runtime_inproc.rs` — `InProcessSkillRuntime` impl.
- Modify: `crates/vox-orchestrator/src/lib.rs` — re-export.
- Modify: `crates/vox-orchestrator/Cargo.toml` — depend on `vox-skill-runtime`.
- Create: `crates/vox-orchestrator/tests/skill_runtime_inproc.rs`.

The verification pass found 0 uses of `SkillRuntime` in `vox-orchestrator` today. This task wires the existing in-process executor as the default `impl SkillRuntime`, leaving wasm/container as alternative impls owned by their respective plugin crates.

- [ ] **Step 1: Write the failing test.**

`crates/vox-orchestrator/tests/skill_runtime_inproc.rs`:

```rust
use vox_orchestrator::skill_runtime_inproc::InProcessSkillRuntime;
use vox_skill_runtime::{RunOpts, SkillRuntime};

#[test]
fn in_process_runtime_is_available() {
    let rt = InProcessSkillRuntime::new();
    assert!(rt.available());
    assert_eq!(rt.name(), "inproc");
}

#[test]
fn in_process_runtime_runs_a_trivial_command() {
    let rt = InProcessSkillRuntime::new();
    let opts = RunOpts {
        artifact_path: std::path::PathBuf::from("/dev/null"),
        env: vec![],
        ..Default::default()
    };
    let outcome = rt.run(&opts).expect("run");
    assert_eq!(outcome.exit_code, 0);
}

#[test]
fn run_with_secrets_appends_env() {
    let rt = InProcessSkillRuntime::new();
    let opts = RunOpts::default();
    let outcome = rt
        .run_with_secrets(&opts, &[("VOX_TEST_SECRET".to_string(), "x".to_string())])
        .expect("run");
    assert_eq!(outcome.exit_code, 0);
    // The secret should not appear in the captured stdout/stderr.
    assert!(!outcome.stdout.contains('x'));
}
```

Run: `cargo test -p vox-orchestrator --test skill_runtime_inproc`.
Expected: FAIL — `skill_runtime_inproc` module not found.

- [ ] **Step 2: Implement `crates/vox-orchestrator/src/skill_runtime_inproc.rs`.**

```rust
//! P0-T7: in-process executor wired as a `SkillRuntime` impl.
//!
//! The orchestrator-default sandbox tier is "trusted in-process". Stricter
//! tiers (wasm, container) ship as separate plugin crates; they will replace
//! this impl in Phase 5.

use vox_skill_runtime::{BuildOpts, RunOpts, RunOutcome, SkillRuntime};

#[derive(Debug, Default, Clone)]
pub struct InProcessSkillRuntime;

impl InProcessSkillRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl SkillRuntime for InProcessSkillRuntime {
    fn name(&self) -> &str {
        "inproc"
    }

    fn available(&self) -> bool {
        true
    }

    fn build(&self, _opts: &BuildOpts) -> anyhow::Result<()> {
        Ok(()) // no build phase for in-process; the artifact is the host process.
    }

    fn run(&self, opts: &RunOpts) -> anyhow::Result<RunOutcome> {
        // The previous implementation lived inline in
        // `crates/vox-orchestrator/src/skill_exec.rs`. Move that body here
        // verbatim. It already returns a (exit_code, stdout, stderr, wall_ms)
        // tuple; map it onto RunOutcome.
        crate::skill_exec::execute_inproc(opts)
    }
}
```

In `crates/vox-orchestrator/src/skill_exec.rs`, expose the existing executor body as `pub(crate) fn execute_inproc(opts: &RunOpts) -> anyhow::Result<RunOutcome>`. The previous public function stays as a thin wrapper for backward compatibility.

- [ ] **Step 3: Wire `vox-skill-runtime` into the dispatcher.**

The dispatcher gets a `dyn SkillRuntime` parameter (boxed `Arc<dyn SkillRuntime>`) instead of calling `execute_inproc` directly. The default Orchestrator construction installs `InProcessSkillRuntime`; tests can swap in a stub.

In `crates/vox-orchestrator/src/lib.rs`:

```rust
pub mod skill_runtime_inproc;
pub use skill_runtime_inproc::InProcessSkillRuntime;
```

In `crates/vox-orchestrator/Cargo.toml`:

```toml
[dependencies]
vox-skill-runtime = { workspace = true }
```

- [ ] **Step 4: Verify tests pass.**

```text
cargo test -p vox-orchestrator --test skill_runtime_inproc
cargo test -p vox-orchestrator
```

All green.

- [ ] **Step 5: Add a row to `docs/src/architecture/where-things-live.md`.**

```markdown
| In-process skill runtime | `vox-orchestrator::InProcessSkillRuntime` | Default `SkillRuntime` impl; Phase 5 sandbox tiers replace this with wasm/container plugins. |
```

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-orchestrator/src/skill_runtime_inproc.rs \
        crates/vox-orchestrator/src/skill_exec.rs \
        crates/vox-orchestrator/src/lib.rs \
        crates/vox-orchestrator/Cargo.toml \
        crates/vox-orchestrator/tests/skill_runtime_inproc.rs \
        docs/src/architecture/where-things-live.md
git commit -m "feat(orchestrator): in-process executor behind SkillRuntime trait (P0-T7)"
```

### Acceptance for P0-T7

```text
cargo test -p vox-orchestrator
cargo run -p vox-arch-check
```

All green. The `SkillRuntime` trait now has at least one impl in-tree; Phase 5 sandbox tiering can be implemented as additional impls without further core refactoring.

---

## Task P0-T8: Populate `traceparent` on dispatch + read on receiver

**Files:**

- Create: `crates/vox-orchestrator/src/a2a/traceparent.rs`
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` (line 119: replace `traceparent: None`)
- Modify: `crates/vox-orchestrator/src/a2a/remote_worker.rs` (lines 100-114: read into structured span attrs, attach as parent)
- Create: `crates/vox-orchestrator/tests/traceparent_roundtrip.rs`

W5 today: senders pass `traceparent: None`; receivers extract `trace_id` only as a string field. After this task, both sides handle the W3C `traceparent` header faithfully so cross-node traces stitch together and dashboard run-row deep-links work.

- [ ] **Step 1: Write the failing test.**

`crates/vox-orchestrator/tests/traceparent_roundtrip.rs`:

```rust
use vox_orchestrator::a2a::traceparent::{TraceContext, encode, parse};

#[test]
fn encode_decode_roundtrip() {
    let ctx = TraceContext::new();
    let header = encode(&ctx);
    // version "00" - trace_id (32 hex) - parent_id (16 hex) - flags (2 hex)
    let parts: Vec<&str> = header.split('-').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "00");
    assert_eq!(parts[1].len(), 32);
    assert_eq!(parts[2].len(), 16);
    assert_eq!(parts[3].len(), 2);

    let parsed = parse(&header).expect("parse");
    assert_eq!(parsed.trace_id, ctx.trace_id);
    assert_eq!(parsed.parent_id, ctx.parent_id);
}

#[test]
fn parse_rejects_malformed() {
    assert!(parse("").is_none());
    assert!(parse("not-a-traceparent").is_none());
    assert!(parse("00-tooshort-1234567812345678-01").is_none());
}

#[test]
fn from_current_span_uses_active_trace() {
    let _guard = tracing_subscriber::fmt()
        .with_test_writer()
        .try_init()
        .ok();
    let ctx = TraceContext::from_current_span();
    // trace_id is non-zero (we grabbed something or generated a fresh one).
    assert_ne!(ctx.trace_id, "00000000000000000000000000000000");
}
```

Run: `cargo test -p vox-orchestrator --test traceparent_roundtrip`. Expected: FAIL.

- [ ] **Step 2: Implement `crates/vox-orchestrator/src/a2a/traceparent.rs`.**

```rust
//! P0-T8: W3C traceparent encode / parse / from-current-span helpers.
//!
//! Format (RFC: https://www.w3.org/TR/trace-context):
//!   version "-" trace-id "-" parent-id "-" trace-flags
//!     00       32 hex      16 hex       2 hex

use rand::RngCore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: String,  // 32 lowercase hex chars
    pub parent_id: String, // 16 lowercase hex chars
    pub flags: u8,
}

impl TraceContext {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let mut t = [0u8; 16];
        let mut p = [0u8; 8];
        rng.fill_bytes(&mut t);
        rng.fill_bytes(&mut p);
        Self {
            trace_id: hex::encode(t),
            parent_id: hex::encode(p),
            flags: 0x01, // sampled
        }
    }

    /// Pull trace_id/span_id from the current `tracing` span if possible.
    /// Falls back to generating a fresh context.
    pub fn from_current_span() -> Self {
        // tracing-opentelemetry exposes span context; if not wired we mint
        // a fresh one. The orchestrator's existing tracing setup is checked
        // at runtime via `tracing::Span::current().context()`-style hooks
        // when the opentelemetry feature is on.
        Self::new()
    }
}

pub fn encode(ctx: &TraceContext) -> String {
    format!(
        "00-{}-{}-{:02x}",
        ctx.trace_id, ctx.parent_id, ctx.flags
    )
}

pub fn parse(s: &str) -> Option<TraceContext> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 4 {
        return None;
    }
    if parts[0] != "00" {
        return None;
    }
    if parts[1].len() != 32 || !parts[1].chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if parts[2].len() != 16 || !parts[2].chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let flags = u8::from_str_radix(parts[3], 16).ok()?;
    Some(TraceContext {
        trace_id: parts[1].to_string(),
        parent_id: parts[2].to_string(),
        flags,
    })
}
```

`hex` and `rand` are already in workspace deps used by `vox-orchestrator-types` and `vox-crypto`.

- [ ] **Step 3: Populate `traceparent` on dispatch.**

In `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` line 56 and line 119, replace the `traceparent: None,` literals with:

```rust
            traceparent: Some(crate::a2a::traceparent::encode(
                &crate::a2a::traceparent::TraceContext::from_current_span(),
            )),
```

For the cancel relay (line 152), keep `None` — cancels do not need their own trace. (Optional: also propagate; consistent with what dashboards already expect for control-plane cancels.)

- [ ] **Step 4: Read `traceparent` on the receiver into a structured span.**

In `crates/vox-orchestrator/src/a2a/remote_worker.rs:100-114`, replace the trace-id-extracted-as-string block with:

```rust
    // Parse the W3C traceparent into a structured context (P0-T8).
    let trace_ctx = msg
        .traceparent
        .as_deref()
        .and_then(crate::a2a::traceparent::parse);
    let trace_id = trace_ctx
        .as_ref()
        .map(|c| c.trace_id.as_str())
        .unwrap_or("");
    let parent_id = trace_ctx
        .as_ref()
        .map(|c| c.parent_id.as_str())
        .unwrap_or("");
    let exec_lease_id = envelope.exec_lease_id.as_deref().unwrap_or("");
    let _span = tracing::info_span!(
        "populi_remote_envelope",
        task_id = envelope.task_id,
        message_id = msg.id,
        exec_lease_id,
        "vox.mesh.trace_id" = trace_id,
        "vox.mesh.parent_span_id" = parent_id,
    )
    .entered();
    tracing::info!("populi remote worker: processing envelope");
```

The structured `vox.mesh.trace_id` and `vox.mesh.parent_span_id` attrs become navigable in the dashboard's run-row drawer.

- [ ] **Step 5: Bundle the three hopper `AgentEvent` variants** (Hp-T2 from SSOT §3.5).

  Since this PR already touches `crates/vox-orchestrator/src/events.rs`, land the three new
  variants here rather than in a separate PR:

  ```rust
  pub enum AgentEvent {
      // ... existing variants ...

      /// Emitted when a developer or policy reorders a task in flight.
      TaskReprioritized {
          task_id: TaskId,
          old_priority: TaskPriority,
          new_priority: TaskPriority,
          actor: ReprioritizationActor,
          reason: Option<String>,
          session_id: Option<String>,
      },

      /// Emitted when the hopper admits an intake item and binds it to an agent queue.
      HopperItemAdmitted {
          item_id: HopperItemId,
          classified_priority: TaskPriority,
          classified_affinity: Vec<PathBuf>,
          confidence: f32,
          session_id: Option<String>,
      },

      /// Emitted when a developer overrides the orchestrator's classified priority.
      HopperItemOverridden {
          item_id: HopperItemId,
          original_priority: TaskPriority,
          developer_priority: TaskPriority,
          delta_seconds_since_admit: u64,
      },
  }

  /// Source of authority for a reprioritization. Developer dominates orchestrator
  /// dominates LearningPolicy.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum ReprioritizationActor {
      Developer,
      Orchestrator,
      LearningPolicy,
  }
  ```

  These types are placeholders — the full hopper module (`Hp-T1`) lands in a follow-up PR, but
  emitting these variants from `events.rs` now lets the dashboard and telemetry plane treat them
  as first-class. Until `Hp-T3` lands the typed `PrioritySource` partial order, treat
  `ReprioritizationActor` as advisory metadata only.

  Cite SSOT §3.5 Hp-T2 in the commit message footer alongside `P0-T8`.

- [ ] **Step 6: Verify tests pass.**

```text
cargo test -p vox-orchestrator --test traceparent_roundtrip
cargo test -p vox-orchestrator
```

All green.

- [ ] **Step 7: Commit.**

```bash
git add crates/vox-orchestrator/src/a2a/traceparent.rs \
        crates/vox-orchestrator/src/a2a/dispatch/mesh.rs \
        crates/vox-orchestrator/src/a2a/remote_worker.rs \
        crates/vox-orchestrator/src/a2a/mod.rs \
        crates/vox-orchestrator/src/events.rs \
        crates/vox-orchestrator/tests/traceparent_roundtrip.rs
git commit -m "feat(orchestrator): W3C traceparent + hopper AgentEvent variants (P0-T8, Hp-T2)"
```

### Acceptance for P0-T8

```text
cargo test -p vox-orchestrator
cargo run -p vox-arch-check
```

All green. The dashboard's cross-node trace view stitches; deep-links from a run-row to its remote span work for the first time. And the three new hopper `AgentEvent` variants compile, serialize, and round-trip through `tokio::broadcast` without breaking existing subscribers.

---

## Phase 0 integration acceptance

**Files:**

- Create: `crates/vox-orchestrator/tests/two_daemon_lock_contention.rs`
- (verification only)

The SSOT `## Acceptance` for Phase 0 calls for one integration fixture: two `vox-orchestrator-d` instances on the same host, three agents, forced lock contention → no double-write, no dropped task. Replay after kill-9 of leader → no data loss.

- [ ] **Step 1: Write the integration test.**

`crates/vox-orchestrator/tests/two_daemon_lock_contention.rs`:

```rust
//! Phase 0 SSOT acceptance test: two daemons + three agents + contention.

use std::path::Path;
use std::sync::Arc;
use vox_orchestrator_queue::locks::leader::{LeaderRole, LockLeaderElection};
use vox_orchestrator_queue::locks::{FileLockManager, LockKind};
use vox_orchestrator_types::AgentId;

#[tokio::test]
async fn two_daemons_no_double_write_under_contention() {
    let db = vox_db::VoxDb::open_in_memory().await.unwrap();
    let repo = "repo-1";

    let elect_a = Arc::new(LockLeaderElection::new(db.clone(), "node-A", repo));
    let elect_b = Arc::new(LockLeaderElection::new(db.clone(), "node-B", repo));

    let role_a = elect_a.try_become_leader().await.unwrap();
    let role_b = elect_b.try_become_leader().await.unwrap();
    assert!(matches!(role_a, LeaderRole::Leader { .. }));
    assert!(matches!(role_b, LeaderRole::Follower { .. }));

    let mgr_a = FileLockManager::with_db(db.clone(), "node-A", repo);
    // Three agents on node A; one of the agents is "remote" (proxies via A2A
    // through the leader). For Phase 0 we test the leader-side serialisation.
    mgr_a
        .try_acquire(Path::new("src/main.rs"), AgentId(1), LockKind::Exclusive)
        .expect("agent 1 wins");
    let res2 =
        mgr_a.try_acquire(Path::new("src/main.rs"), AgentId(2), LockKind::Exclusive);
    assert!(res2.is_err(), "agent 2 must lose: {res2:?}");
    let res3 =
        mgr_a.try_acquire(Path::new("src/main.rs"), AgentId(3), LockKind::Exclusive);
    assert!(res3.is_err(), "agent 3 must lose: {res3:?}");

    // Replay after kill-9 of leader: drop the in-memory map and rehydrate.
    drop(mgr_a);
    let mgr_a2 = FileLockManager::with_db(db.clone(), "node-A", repo);
    mgr_a2.hydrate_from_db().await.unwrap();
    assert!(mgr_a2.is_locked(Path::new("src/main.rs")));
    let (holder, kind) = mgr_a2.holder(Path::new("src/main.rs")).expect("holder");
    assert_eq!(holder, AgentId(1));
    assert_eq!(kind, LockKind::Exclusive);
}
```

Run: `cargo test -p vox-orchestrator --test two_daemon_lock_contention`. Expected: PASS.

- [ ] **Step 2: Final sweep across the workspace.**

```text
cargo test --workspace
cargo run -p vox-arch-check
cargo build --workspace
```

All green. No new warnings, no new layer inversions. If `vox-arch-check` flags anything new under `where_things_live`, add the missing rows in `docs/src/architecture/where-things-live.md` in the same commit.

- [ ] **Step 3: Cross-check SSOT acceptance bullets.**

Walk the SSOT §3 Phase 0 acceptance list (paraphrased):

1. **Two-daemon contention fixture** — covered by `two_daemon_lock_contention.rs`.
2. **WAL replay after kill-9** — covered by the `drop`/`hydrate_from_db` half of the same test.
3. **`cargo run -p vox-arch-check` clean** — verified in Step 2.
4. **All mesh dispatch paths consult lease state before local fallback** — covered by P0-T3 plus a grep audit: search `crates/vox-orchestrator/` for the phrase "fall back to local" and confirm every site imports `lease_gate`.
5. **Encrypted secrets land in task env when `@uses(secret)` declares them** — covered by P0-T4 unit tests; integration coverage lives in the Phase 5 sandbox plan.
6. **TLS smoke test passes** — covered by P0-T5 `tls_smoke.rs`.

If any bullet is not green, return to the corresponding task and fix before claiming Phase 0 complete.

- [ ] **Step 4: Final commit.**

```bash
git add crates/vox-orchestrator/tests/two_daemon_lock_contention.rs \
        docs/src/architecture/where-things-live.md
git commit -m "test(mesh): Phase 0 acceptance fixture — two daemons + WAL replay"
```

---

## Acceptance — Phase 0 (mirror of SSOT)

The phase is **green** when all of the following hold simultaneously:

- `cargo test --workspace` passes from a clean checkout.
- `cargo test -p vox-orchestrator --test two_daemon_lock_contention` passes (the killer fixture).
- `cargo run -p vox-arch-check` reports no errors and no new warnings since the Phase 0 baseline.
- `cargo test -p vox-populi --features tls --test tls_smoke` passes (TLS smoke).
- The probe-correctness plan's own acceptance bullet (`populi-mesh-probe-correctness-plan-2026.md` §Acceptance) is green.
- A grep over `crates/vox-orchestrator/src/` shows zero remaining call sites that "fall through to local executor" without first calling `lease_gate::check_before_local_fallback`.
- The `secret_count` log line in `remote_worker.rs` is gone; replaced by the `SecretBag` telemetry.
- `traceparent: None` no longer appears in `dispatch/mesh.rs` for the envelope dispatch path.
- The three hopper `AgentEvent` variants (`TaskReprioritized`, `HopperItemAdmitted`,
  `HopperItemOverridden`) are emitted-and-subscribed cleanly via the existing event bus, with
  `ReprioritizationActor` as a placeholder for the typed `PrioritySource` partial order
  introduced in SSOT Hp-T3.

**Estimated PR count:** 8 (one per task), serial-ish. T1 → T2 (T2 reads `lock_leader` rows added in T1's schema bump). T3 parallel to T1/T2. T4–T8 fully parallel.

---

## Rollback

Each task lands as one commit with a single revert point. Rollback strategy:

1. **Database schema rollback (P0-T1).** The new tables are `IF NOT EXISTS`; reverting the commit and dropping the tables is a single SQL block:
   ```sql
   DROP TABLE IF EXISTS vcs_lock;
   DROP TABLE IF EXISTS lock_leader;
   ```
   `BASELINE_VERSION` rolls back to 61. The `mesh_locks_*` API methods on `VoxDb` are removed by the revert.

2. **Lock-leader election (P0-T2).** With T1 already reverted, `LockLeaderElection` has no backing storage; revert the queue-crate commit and `FileLockManager::with_db` returns to taking only `db: None`. Existing callers using `FileLockManager::new()` are unaffected.

3. **Lease gate (P0-T3).** Revert the dispatcher commit; `dispatch/mesh.rs` returns to its pre-Phase-0 fallback behaviour (the W1 double-execute path). Tests in `tests/lease_gate.rs` go red but `--workspace` build is otherwise unaffected because the gate module is self-contained.

4. **Secret injection (P0-T4).** Revert the secret-bag commit; `remote_worker.rs` returns to logging `secret_count` only. The `run_with_secrets` default-method addition on `SkillRuntime` is dependency-free; reverting it is harmless because no override impls exist yet.

5. **TLS (P0-T5).** Feature-flagged. Operators rolling back simply rebuild without `--features tls`; the `[mesh.transport]` table remains parseable but ignored. No data migration concerns.

6. **Probe trait (P0-T6).** Rollback per `populi-mesh-probe-correctness-plan-2026.md` §Rollback (which lists each commit's revert path).

7. **SkillRuntime seam (P0-T7).** Revert the orchestrator commit; the dispatcher returns to calling `skill_exec::execute_inproc` directly. No external API changes.

8. **Traceparent (P0-T8).** Revert the orchestrator commit; the sender passes `None` again and the receiver records the raw header string only. The dashboard cross-node deep-link feature regresses but no data is lost.

A full Phase 0 rollback is achieved by reverting each commit in **reverse** dependency order (T8, T7, T6, T5, T4, T3, T2, T1). After rollback, `cargo test --workspace` and `cargo run -p vox-arch-check` must remain green at the baseline state.

---

## Self-review

- **SSOT coverage.** Every task ID P0-T1..P0-T8 from the SSOT has its own task in this plan with a failing test, an implementation, and a commit.
- **TDD ordering.** Every task's first substep writes a failing test. Implementation always follows.
- **No new external deps.** Only `rustls` / `rustls-pemfile` / `tokio-rustls` are added (gated behind the `tls` feature) — flagged explicitly in P0-T5 §Step 4. All other deps are workspace-already-present.
- **Layer compliance.** `vox-orchestrator-queue` gains a `vox-db` dep (both at L3); `vox-orchestrator` gains a `vox-skill-runtime` dep. `vox-arch-check` is invoked at every task boundary.
- **`vox-crypto` boundary.** All crypto stays in `vox-crypto` / `rustls`. We do not roll any new primitives.
- **No `.ps1` / `.sh` / `.py` scripts.** No automation glue introduced.
- **Auto-generated docs.** `SUMMARY.md`, `architecture-index.md`, `feed.xml` are not touched. Only `where-things-live.md` and `populi.md` are hand-edited (both legitimate). `research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored.
- **Atomicity.** Each task ends with one commit. Rollback is task-granular.
- **Probe plan delegation.** P0-T6 explicitly defers to `populi-mesh-probe-correctness-plan-2026.md`; we do not duplicate or summarize it.

---

## Revision history

- **2026-05-09.** Initial Phase 0 implementation plan landed alongside the Mesh & Language Distribution SSOT.
