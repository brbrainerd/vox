# P1 — Wire the Multi-Agent Safety Substrate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn three already-built-but-dormant safety primitives into live enforcement: lock the MCP
write path, make scope enforcement `Strict` by default, and record real conflicts when agent
workspaces overlap on merge — so two agents on one branch produce a *recorded conflict*, not a clobber.

**Architecture:** Pure wiring of existing code; **no `vox-vcs`/jj-lib dependency** (P1 is independent of
P0). Each change is fronted by a unit-testable pure function so we test logic without standing up a
full `Orchestrator`/`ServerState`. Verified facts (file:line) come from the 2026-06-05 audit in the
source spec.

**Tech Stack:** Rust, `cargo test`, existing `FileLockManager` / `ScopeGuard` / `ConflictManager`.

**Source spec:** [`docs/superpowers/specs/2026-06-05-jj-first-class-vcs-design.md`](../specs/2026-06-05-jj-first-class-vcs-design.md) §2.1, §5, §6, §8 (P1).

**Pruned from P1:** "snapshot-on-write via backend" — the backend doesn't exist until P2, and
snapshots already bracket every task (`task_submit.rs:401`, success/fail paths). Moved to P2/P6.

---

## File Structure

| File | Responsibility |
|---|---|
| Create `crates/vox-orchestrator-mcp/src/lock_guard.rs` | Write-path lock check (mirrors `scope_guard.rs`) |
| Modify `crates/vox-orchestrator-mcp/src/dispatch.rs:84` | Call `lock_guard::check_lock` after `check_scope` |
| Modify `crates/vox-orchestrator-mcp/src/scope_guard.rs` | Make `WRITE_TOOLS` / `PATH_ARG_KEYS` `pub(crate)` |
| Modify `crates/vox-orchestrator-mcp/src/lib.rs` | `mod lock_guard;` |
| Modify `crates/vox-orchestrator/src/config/impl_default.rs:41` | Default scope enforcement → `Strict` |
| Create `crates/vox-orchestrator/src/merge_conflicts.rs` | Pure `record_overlap_conflicts` |
| Modify `crates/vox-orchestrator/src/lib.rs` | `pub mod merge_conflicts;` |
| Modify `crates/vox-orchestrator/src/json_vcs_facade.rs:119` | Detect + record conflicts before `destroy_workspace` |

---

### Task 1: Lock the MCP write path

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/lock_guard.rs`
- Modify: `crates/vox-orchestrator-mcp/src/scope_guard.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs:84`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`

Background (verified): the live write path is gated only by `scope_guard::check_scope`
(`dispatch.rs:84`); `FileLockManager` is **not** consulted there (`task_submit.rs:382` discards its
result). `try_acquire` is **re-entrant for the holder** but `Err(LockConflict)` for anyone else
(`lease.rs:113`), so calling it at the write path *is* the enforcement.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-orchestrator-mcp/src/lock_guard.rs` with just the test first:

```rust
#[cfg(test)]
mod tests {
    use super::evaluate_lock;
    use vox_orchestrator_queue::locks::FileLockManager;

    #[test]
    fn holder_is_reentrant_others_are_rejected() {
        let mgr = FileLockManager::new();
        assert!(evaluate_lock(&mgr, 1, "src/a.rs").is_none(), "agent 1 should acquire");
        assert!(evaluate_lock(&mgr, 1, "src/a.rs").is_none(), "same agent re-entrant");
        assert!(evaluate_lock(&mgr, 2, "src/a.rs").is_some(), "agent 2 must be rejected");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp lock_guard`
Expected: FAIL — `evaluate_lock` not defined.

- [ ] **Step 3: Implement the guard**

Prepend to `crates/vox-orchestrator-mcp/src/lock_guard.rs`:

```rust
//! Write-path lock enforcement. Mirrors `scope_guard.rs` but consults the
//! orchestrator's `FileLockManager`. `try_acquire` is re-entrant for the
//! lock holder and rejects any other agent, which is exactly the gate we want.

use crate::scope_guard::{PATH_ARG_KEYS, WRITE_TOOLS};
use crate::server_state::ServerState;
use std::path::Path;
use vox_orchestrator::types::AgentId;
use vox_orchestrator_queue::locks::{FileLockManager, LockKind};

/// Pure core: try to take an exclusive lock for `agent_id` on `path`.
/// `None` = allowed (acquired or re-entrant); `Some(msg)` = rejected.
pub(crate) fn evaluate_lock(
    lock_manager: &FileLockManager,
    agent_id: u64,
    path: &str,
) -> Option<String> {
    match lock_manager.try_acquire(Path::new(path), AgentId(agent_id), LockKind::Exclusive) {
        Ok(()) => None,
        Err(conflict) => Some(format!(
            "LOCK_CONFLICT: '{path}' is locked by another agent ({conflict:?}). \
             Wait for release or route this write to a non-overlapping path."
        )),
    }
}

/// Returns `Some(rejection)` when a write tool targets a path locked by another agent.
pub fn check_lock(
    state: &ServerState,
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<String> {
    if !WRITE_TOOLS.contains(&tool_name) {
        return None;
    }
    let agent_id = args
        .get("agent_id")
        .or_else(|| args.get("vcs_agent_id"))
        .and_then(|v| v.as_u64())?;
    let path = PATH_ARG_KEYS
        .iter()
        .find_map(|key| args.get(*key).and_then(|v| v.as_str()))?;
    evaluate_lock(&state.orchestrator.lock_manager, agent_id, path)
}
```

(If `FileLockManager`/`LockKind` are re-exported from `vox_orchestrator`, prefer that path. If
`vox-orchestrator-mcp` lacks a direct `vox-orchestrator-queue` dep, add it under
`[dependencies]` — it is already a transitive dep via `vox-orchestrator`.)

- [ ] **Step 4: Make the shared consts visible + register the module**

In `crates/vox-orchestrator-mcp/src/scope_guard.rs`, change the `WRITE_TOOLS` and `PATH_ARG_KEYS`
declarations from private (`const`/`static`) to `pub(crate) const` / `pub(crate) static`.

In `crates/vox-orchestrator-mcp/src/lib.rs`, add next to `mod scope_guard;`:

```rust
mod lock_guard;
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp lock_guard`
Expected: PASS.

- [ ] **Step 6: Wire into the dispatch path**

In `crates/vox-orchestrator-mcp/src/dispatch.rs`, immediately after the existing `check_scope` block
(currently lines 84-87):

```rust
    if let Some(rejection) = crate::scope_guard::check_scope(state, name_canonical, agent_id, &args)
    {
        return Ok(crate::params::ToolResult::<()>::err(rejection).to_json_compact());
    }
```

add:

```rust
    if let Some(rejection) = crate::lock_guard::check_lock(state, name_canonical, &args) {
        return Ok(crate::params::ToolResult::<()>::err(rejection).to_json_compact());
    }
```

- [ ] **Step 7: Build, format, commit**

Run: `cargo build -p vox-orchestrator-mcp` (Expected: compiles)

```bash
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/lock_guard.rs crates/vox-orchestrator-mcp/src/scope_guard.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(mcp): enforce file locks at the write-tool dispatch path"
```

---

### Task 2: Make scope enforcement `Strict` by default

**Files:**
- Modify: `crates/vox-orchestrator/src/config/impl_default.rs:41`

Background (verified): `ScopeEnforcement` defaults to `Warn` (`scope.rs:22`,
`impl_default.rs:41`). `ScopeGuard::check_write` returns `Allowed` for any agent with **no declared
scope** (`scope.rs`), so flipping the default to `Strict` only bites agents that declared a scope and
then wrote outside it — single-agent/unscoped flows are unaffected.

- [ ] **Step 1: Write the failing test**

Add to the test module in `crates/vox-orchestrator/src/config/impl_default.rs` (or the file's existing
`#[cfg(test)] mod tests`):

```rust
#[test]
fn default_scope_enforcement_is_strict() {
    use crate::scope::ScopeEnforcement;
    let cfg = super::OrchestratorConfig::default();
    assert_eq!(cfg.scope_enforcement, ScopeEnforcement::Strict);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator default_scope_enforcement_is_strict`
Expected: FAIL — current default is `Warn`.

- [ ] **Step 3: Change the default**

In `crates/vox-orchestrator/src/config/impl_default.rs:41`, change:

```rust
            scope_enforcement: ScopeEnforcement::default(),
```
to:
```rust
            scope_enforcement: ScopeEnforcement::Strict,
```

- [ ] **Step 4: Run the new test**

Run: `cargo test -p vox-orchestrator default_scope_enforcement_is_strict`
Expected: PASS.

- [ ] **Step 5: Run the full crate suite and fix any `Warn`-assuming tests**

Run: `cargo test -p vox-orchestrator`
Expected: PASS. If a pre-existing test asserted `Warn` behavior (e.g. a scoped agent writing
out-of-scope expecting `Warned` not `Denied`), update that test to construct its `ScopeGuard`/config
with `ScopeEnforcement::Warn` explicitly — the *default* is now `Strict`, but callers can still opt
down. Do not weaken the new default to make a test pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/config/impl_default.rs
git commit -m "feat(orchestrator): default scope enforcement to Strict"
```

---

### Task 3: Record conflicts when workspaces overlap on merge

**Files:**
- Create: `crates/vox-orchestrator/src/merge_conflicts.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs`
- Modify: `crates/vox-orchestrator/src/json_vcs_facade.rs:119`

Background (verified): `workspace_merge_json` today just `destroy_workspace` + counts
(`json_vcs_facade.rs:119`); `ConflictManager::record_conflict` is never called in prod. The
`WorkspaceManager` already exposes `overlapping_paths(a, b)` and `list_workspaces()`.

- [ ] **Step 1: Write the failing test for the pure core**

Create `crates/vox-orchestrator/src/merge_conflicts.rs` with the test first:

```rust
#[cfg(test)]
mod tests {
    use super::record_overlap_conflicts;
    use crate::conflicts::ConflictManager;
    use crate::snapshot::SnapshotId;
    use crate::types::AgentId;
    use std::path::PathBuf;

    #[test]
    fn one_conflict_per_overlapping_path() {
        let mut cm = ConflictManager::new();
        let others = vec![(
            AgentId(2),
            SnapshotId(20),
            vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")],
        )];
        let ids = record_overlap_conflicts(&mut cm, (AgentId(1), SnapshotId(10)), &others);
        assert_eq!(ids.len(), 2);
        assert_eq!(cm.active_conflicts().len(), 2);
    }

    #[test]
    fn no_overlap_records_nothing() {
        let mut cm = ConflictManager::new();
        let ids = record_overlap_conflicts(&mut cm, (AgentId(1), SnapshotId(10)), &[]);
        assert!(ids.is_empty());
        assert!(cm.active_conflicts().is_empty());
    }
}
```

(Import paths for `SnapshotId` / `AgentId` must match those already used in `conflicts.rs` and
`json_vcs_facade.rs` — adjust if the crate re-exports them under a different path.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator merge_conflicts`
Expected: FAIL — `record_overlap_conflicts` not defined.

- [ ] **Step 3: Implement the pure core**

Prepend to `crates/vox-orchestrator/src/merge_conflicts.rs`:

```rust
//! Pure conflict-recording core for workspace merge-back. Kept free of
//! `Orchestrator`/`WorkspaceManager` so it is unit-testable in isolation.

use crate::conflicts::{ConflictId, ConflictManager};
use crate::snapshot::SnapshotId;
use crate::types::AgentId;
use std::path::PathBuf;

/// Record one conflict per overlapping path between the merging agent and each
/// other active workspace. `others` is `(agent, its_base_snapshot, overlap_paths)`.
pub fn record_overlap_conflicts(
    cm: &mut ConflictManager,
    merging: (AgentId, SnapshotId),
    others: &[(AgentId, SnapshotId, Vec<PathBuf>)],
) -> Vec<ConflictId> {
    let (merging_agent, merging_snap) = merging;
    let mut ids = Vec::new();
    for (other_agent, other_snap, paths) in others {
        for path in paths {
            let id = cm.record_conflict(
                path.clone(),
                Some(merging_snap),
                vec![(merging_agent, merging_snap), (*other_agent, *other_snap)],
            );
            ids.push(id);
        }
    }
    ids
}
```

- [ ] **Step 4: Register the module + run the test**

In `crates/vox-orchestrator/src/lib.rs` add (near the other `pub mod` lines):

```rust
pub mod merge_conflicts;
```

Run: `cargo test -p vox-orchestrator merge_conflicts`
Expected: PASS. (If `SnapshotId` is not `Copy`, bind `let merging_snap = merging.1;` and `.clone()`
the side values — adjust until it compiles.)

- [ ] **Step 5: Wire the impure glue into `workspace_merge_json`**

In `crates/vox-orchestrator/src/json_vcs_facade.rs`, replace the body of `workspace_merge_json`
(currently lines 119-135) with:

```rust
/// Merge workspace into mainline (MCP `vox_workspace_merge`).
/// Records overlap conflicts (as data) before destroying the workspace.
pub fn workspace_merge_json(orch: &Orchestrator, agent_id: u64) -> Value {
    let merging = AgentId(agent_id);
    let ws_handle = orch.workspace_manager_handle();
    let mut mgr = crate::sync_lock::rw_write(&*ws_handle);

    // Detect overlaps against every other active workspace, before mutating.
    let merging_base = mgr.get_workspace(merging).map(|w| w.base_snapshot);
    let others: Vec<(AgentId, SnapshotId, Vec<std::path::PathBuf>)> = mgr
        .list_workspaces()
        .iter()
        .filter(|w| w.agent_id != merging)
        .map(|w| (w.agent_id, w.base_snapshot, mgr.overlapping_paths(merging, w.agent_id)))
        .filter(|(_, _, paths)| !paths.is_empty())
        .collect();

    let conflicts_recorded = if let Some(base) = merging_base {
        let mut cm = crate::sync_lock::rw_write(&*orch.conflict_manager);
        crate::merge_conflicts::record_overlap_conflicts(&mut cm, (merging, base), &others).len()
    } else {
        0
    };

    match mgr.destroy_workspace(merging) {
        Some(ws) => {
            let count = ws.modified_count();
            json!({
                "merged": true,
                "files_merged": count,
                "conflicts_recorded": conflicts_recorded,
            })
        }
        None => json!({
            "merged": false,
            "error": "no_active_workspace",
        }),
    }
}
```

Ensure `SnapshotId` is imported at the top of `json_vcs_facade.rs` (it already uses
`SnapshotId(before_id)` at line 34, so the import exists).

- [ ] **Step 6: Add a regression test for the wired behavior**

Add to the `#[cfg(test)]` module in `json_vcs_facade.rs` (or `crates/vox-orchestrator/tests/vcs_test.rs`)
a test that builds an `Orchestrator`, creates two overlapping workspaces, calls
`workspace_merge_json`, and asserts `conflicts_recorded > 0` and
`orch.conflict_manager` has active conflicts. Use the same `Orchestrator` construction helper the
existing `tests/vcs_test.rs` uses (it already exercises `record_conflict` at line 137). Populate the
two workspaces' overlays via the same write API that test uses so `overlapping_paths` returns a path.

Run: `cargo test -p vox-orchestrator vcs_test`
Expected: PASS, with the new assertion green.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/merge_conflicts.rs crates/vox-orchestrator/src/lib.rs crates/vox-orchestrator/src/json_vcs_facade.rs
git commit -m "feat(orchestrator): record overlap conflicts on workspace merge-back"
```

---

## Self-Review

- **Spec coverage (P1 row):** make `FileLockManager` authoritative at the write path ✓ (T1); default
  `ScopeGuard`→`Strict` ✓ (T2); call `record_conflict` from the merge-back path ✓ (T3). jj-lib
  conflict *auto-resolution* is intentionally deferred to P2 (needs the engine) — noted in the spec.
- **Independence:** P1 touches only existing crates; it does **not** depend on P0/`vox-vcs`.
- **Placeholders:** none — every step has real code/commands. The two "adjust import path / if not
  Copy" notes are compile-time confirmations against existing verified types, not missing logic.
- **Type consistency:** `evaluate_lock`/`check_lock`, `record_overlap_conflicts`,
  `ScopeEnforcement::Strict`, `ConflictManager::record_conflict`, `AgentId`, `SnapshotId`,
  `overlapping_paths`, `list_workspaces`, `destroy_workspace`, `modified_count` all match the
  verbatim signatures captured on 2026-06-05.
- **Behavioral win:** after P1, two agents writing the same file → `LOCK_CONFLICT` rejection at the
  write path (T1) and/or a recorded conflict on merge (T3), never a silent clobber.
