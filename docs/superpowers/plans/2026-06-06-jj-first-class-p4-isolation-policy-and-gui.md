# P4 — Orchestrator Multi-Agent Isolation Policy + GUI Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lift the multi-agent VCS substrate built in P0–P3 into a **user-governable isolation
policy**. Give the orchestrator a typed three-way **isolation strategy** (the §5.1 model), make the
chosen strategy *actually constrain agents* by wiring it into the **existing** scope-guard,
`FileLockManager`, and `WorkspaceManager`/`JjActorHandle` machinery (not a parallel reimplementation),
and surface live strategy + per-agent assignment + conflicts through a new **GUI VCS panel** over the
existing `/api/v2/<surface>` REST + `/v1/ws` conventions — all overridable via `OrchestratorConfig`.

**Architecture:** The strategy is a **decision**, not an engine. A new `IsolationStrategy` enum +
per-agent `IsolationPlan` (in `vox-orchestrator`) is *chosen* from the data the orchestrator already
computes (`overlapping_paths()` / the affinity map) and the config default, then *enforced* by routing
through machinery that already exists:

- **SharedBranch** → make the `FileLockManager` result authoritative at submit time (today
  `task_submit.rs:382` discards it with `let _ =`); disjoint write-sets proceed in parallel, overlaps
  are denied/queued.
- **SplitChanges** → each agent gets its own jj change via the `JjActorHandle` already injected into
  `WorkspaceManager` (`workspace.rs:167`); merge-back records conflicts-as-data
  (`workspace_merge_json` → `record_overlap_conflicts`, already wired in P1).
- **SeparateBranches** → bind each workspace to its own `BranchName` via the existing
  `AgentWorkspace::set_bound_branch` (`workspace.rs:115`).

The GUI panel reuses the `repository` surface slot (already in the registry,
`surface-registry.v1.yaml:431`) by adding **new REST handlers** to the same dashboard router
(`dashboard_api.rs:581 router()`) and a **new WS topic** in the same multiplexed `/v1/ws`
(`http_gateway/mod.rs:234`), modeled exactly on the scientia surface
(`get_scientia_queue` + `scientia.queue.changed`).

**Tech Stack:** Rust, `serde`, `vox-orchestrator`, `vox-orchestrator-queue` (`FileLockManager`),
`vox-orchestrator-types` (`FileAffinity`/`BranchName`), `vox-vcs` (`JjActorHandle`),
`vox-orchestrator-mcp` (axum HTTP gateway), React/TypeScript (`vox-gui`), `vox ci gui-surface-registry`.

**Source spec:** [`vcs-as-vox-language-feature-jujutsu-2026.md`](../../src/architecture/vcs-as-vox-language-feature-jujutsu-2026.md)
§5 (§5.1 three strategies, §5.2 conflict-as-data, §5.3 make enforcement real, §5.4 GUI + config) and
§6 (P3 row: "three-strategy selector, conflict surface, op-log/undo in the Vox GUI, with config
defaults").
**Depends on:** P0 (`vox-vcs`), P1 (overlap detection + `record_conflict` wiring:
`merge_conflicts.rs`, `workspace_merge_json`), P2 (`JjActorHandle` / `spawn_jj_actor`), P3 (`repo.*` /
`Vcs` effect — only loosely; this phase is orchestrator + GUI, not the language). Independent of P4's
sibling "decorators + auto-snapshot" line (spec §6's later P4).

> **Naming note:** the spec calls the three strategies "shared change / per-agent change /
> separate branches". This plan uses the enum variant names **`SharedBranch`**, **`SplitChanges`**,
> **`SeparateBranches`** (the task prompt's spelling). They map 1:1 to §5.1 items 1/2/3 respectively.

> **Honest scoping (read before executing):**
> - The `jj` feature is **on by default** but the orchestrator compiles `--no-default-features` too
>   (`vox-orchestrator/Cargo.toml:35` `jj = ["vox-vcs/jj", "runtime"]`). The `JjActorHandle` field on
>   `WorkspaceManager` is `#[cfg(feature = "jj")]` (`workspace.rs:166`). **All jj-touching enforcement
>   in this plan (SplitChanges/SeparateBranches branch creation) must be `#[cfg(feature = "jj")]`-gated
>   with a graceful no-jj fallback to SharedBranch lock semantics.** The strategy *model* + *config* +
>   SharedBranch lock enforcement are feature-independent.
> - There is **no `vox.toml`**; project config is in-language → app contract (`app_contract.rs:92`).
>   The §5.4 "in-language `@config`" surface is **out of scope** for P4 — this plan adds the strategy
>   to `OrchestratorConfig` (the real, existing config struct the daemon/GUI read/write through
>   `config_handle()`, `accessors.rs:139`). The in-language `@config` projection is a follow-up; Task 8
>   notes it explicitly rather than faking it.
> - `JjActorHandle` exposes `snapshot/changes/diff/undo/conflicts/resolve/push` (`jj_actor.rs`,
>   `backend.rs:35`) but **has no `create_branch`/`bind_branch` command**. SeparateBranches needs one.
>   The plan **adds that command (Task 5)** with a test — it does not pretend it exists. If adding a jj
>   branch op proves to exceed the slice, Task 5 degrades SeparateBranches to "bound `BranchName`
>   recorded in `AgentWorkspace` only" (metadata, already supported by `set_bound_branch`) and reports
>   DONE_WITH_CONCERNS.

---

## File Structure

| File | Responsibility |
|---|---|
| Create `crates/vox-orchestrator/src/isolation.rs` | `IsolationStrategy` enum, `IsolationPlan`, `choose_strategy()` decision fn, per-agent assignment map |
| Modify `crates/vox-orchestrator/src/lib.rs` | `pub mod isolation;` + re-export `IsolationStrategy`/`IsolationPlan` |
| Modify `crates/vox-orchestrator/src/config/orchestrator_fields.rs` | Add `isolation_strategy_default` + `isolation_per_agent` fields |
| Modify `crates/vox-orchestrator/src/config/impl_default.rs` | Default the new fields |
| Modify `crates/vox-orchestrator/src/orchestrator.rs` | Hold an `IsolationPolicy` handle (Arc<RwLock<…>>) + accessor |
| Modify `crates/vox-orchestrator/src/orchestrator/accessors.rs` | `isolation_policy_handle()` accessor |
| Modify `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs` | Make the discarded lock result (`:382`) authoritative under SharedBranch |
| Modify `crates/vox-orchestrator/src/scope.rs` | Default `Strict` for multi-agent runs (helper) — §5.3(b) |
| Modify `crates/vox-orchestrator/src/workspace.rs` | `JjActorHandle` branch-create call for SeparateBranches; per-agent change for SplitChanges |
| Modify `crates/vox-vcs/src/jj_actor.rs`, `backend.rs`, `lib.rs` | Add `create_branch` actor command + `VcsBackend::create_branch` (Task 5) |
| Modify `crates/vox-orchestrator/src/json_vcs_facade.rs` | `isolation_status_json()` — strategy + per-agent + active conflicts bundle |
| Modify `crates/vox-orchestrator-mcp/src/http_gateway/dashboard_api.rs` | `get_vcs_isolation` (GET) + `post_vcs_isolation_strategy` (POST) handlers + `router()` rows |
| Modify `crates/vox-orchestrator-mcp/src/http_gateway/scientia_feed.rs` (or new `vcs_feed.rs`) | `vcs.isolation.changed` WS topic constant + publish hook |
| Create `crates/vox-gui/ui/src/components/surfaces/Repository/IsolationPanel.tsx` | The strategy/assignment/conflicts panel + strategy selector |
| Modify `crates/vox-gui/ui/src/components/surfaces/Repository/RepositoryView.tsx` | Mount `IsolationPanel` |
| (regenerate) `contracts/gui/surface-registry.v1.yaml` + `surfaceRegistry.generated.ts` | Only if the `repository` surface row changes; via `vox ci gui-surface-registry --write` (NEVER hand-edit) |

---

### Task 1: `IsolationStrategy` model + `choose_strategy()` decision

**Files:** Create `crates/vox-orchestrator/src/isolation.rs`; Modify `crates/vox-orchestrator/src/lib.rs`.

This is pure logic with no engine/jj dependency — fully unit-testable in isolation, like
`merge_conflicts.rs`.

- [ ] **Step 1: Write the failing test.** Create `isolation.rs` with the test module first:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disjoint_write_sets_pick_shared_branch() {
        // Two agents, no overlapping write paths -> the cheap shared-branch strategy.
        let s = choose_strategy(/* predicted_overlap */ 0, /* long_running */ false,
                                IsolationStrategy::SharedBranch);
        assert_eq!(s, IsolationStrategy::SharedBranch);
    }

    #[test]
    fn overlap_escalates_to_split_changes() {
        let s = choose_strategy(3, false, IsolationStrategy::SharedBranch);
        assert_eq!(s, IsolationStrategy::SplitChanges);
    }

    #[test]
    fn long_running_prefers_separate_branches() {
        let s = choose_strategy(0, true, IsolationStrategy::SharedBranch);
        assert_eq!(s, IsolationStrategy::SeparateBranches);
    }

    #[test]
    fn config_default_is_honored_when_no_signal_overrides() {
        // An explicit non-default config default wins absent overlap/long-running signal.
        let s = choose_strategy(0, false, IsolationStrategy::SeparateBranches);
        assert_eq!(s, IsolationStrategy::SeparateBranches);
    }
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-orchestrator isolation` (undefined symbols).

- [ ] **Step 3: Implement.** Prepend to `isolation.rs`:
```rust
//! Multi-agent isolation strategy (spec §5.1). The orchestrator *chooses* a
//! strategy per workload from predicted overlap + task duration + config; this
//! module is the decision + per-agent assignment record. Enforcement lives in
//! `task_submit.rs` (locks) and `workspace.rs` (changes/branches).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::AgentId;

/// The three orchestrator-chosen isolation strategies (spec §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolationStrategy {
    /// §5.1(1) Shared change, file-partitioned. All agents on one jj change;
    /// the `FileLockManager` grants single-writer leases per file. Default for
    /// disjoint write-sets — zero branch/worktree overhead.
    #[default]
    SharedBranch,
    /// §5.1(2) Per-agent change, auto-rebased. Each agent gets its own jj change
    /// off the same base; merge-back records conflicts-as-data.
    SplitChanges,
    /// §5.1(3) Separate branches — classic isolation, cheap because jj branches
    /// are anonymous and rebasing is conflict-tolerant.
    SeparateBranches,
}

/// Per-workload + per-agent assignment of strategies. Chosen by the orchestrator,
/// overridable by config and (P4 GUI) by the user.
#[derive(Debug, Clone, Default)]
pub struct IsolationPlan {
    /// Strategy applied when an agent has no explicit override.
    pub default: IsolationStrategy,
    /// Per-agent overrides (config or GUI driven).
    pub per_agent: HashMap<AgentId, IsolationStrategy>,
}

impl IsolationPlan {
    /// Resolve the effective strategy for `agent`.
    pub fn strategy_for(&self, agent: AgentId) -> IsolationStrategy {
        self.per_agent.get(&agent).copied().unwrap_or(self.default)
    }
    /// Set (or clear, with `None`) a per-agent override.
    pub fn set_override(&mut self, agent: AgentId, strategy: Option<IsolationStrategy>) {
        match strategy {
            Some(s) => { self.per_agent.insert(agent, s); }
            None => { self.per_agent.remove(&agent); }
        }
    }
}

/// Choose a strategy for a workload (spec §5.1: "a function of predicted overlap,
/// task duration, and user policy — and is fully overridable").
///
/// `predicted_overlap` is the count of write-paths the new task shares with any
/// active agent (from `overlapping_paths()` / the affinity map). `config_default`
/// is the user/GUI-set baseline that wins absent a stronger signal.
pub fn choose_strategy(
    predicted_overlap: usize,
    long_running: bool,
    config_default: IsolationStrategy,
) -> IsolationStrategy {
    if long_running {
        IsolationStrategy::SeparateBranches
    } else if predicted_overlap > 0 {
        IsolationStrategy::SplitChanges
    } else {
        config_default
    }
}
```

- [ ] **Step 4: Register.** In `lib.rs`: `pub mod isolation;` + `pub use isolation::{IsolationStrategy, IsolationPlan};` (place near the existing `pub use scope::{...}` re-export at `lib.rs:333`).

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator isolation`.

- [ ] **Step 6: Commit.**
```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/isolation.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): IsolationStrategy model + choose_strategy (spec §5.1)"
```

---

### Task 2: Config surface — strategy default + per-agent overrides

**Files:** Modify `crates/vox-orchestrator/src/config/orchestrator_fields.rs`;
`crates/vox-orchestrator/src/config/impl_default.rs`.

`OrchestratorConfig` (`orchestrator_fields.rs:18`) is `#[serde(deny_unknown_fields, default)]`, so new
fields **must** also be defaulted in `impl_default.rs:14` or `Default` won't compile.

- [ ] **Step 1: Failing test.** Add to a tests module in `orchestrator_fields.rs` (or wherever config
roundtrip tests live — grep `OrchestratorConfig::default()` for the existing test home):
```rust
#[test]
fn isolation_strategy_default_is_shared_branch() {
    let c = OrchestratorConfig::default();
    assert_eq!(c.isolation_strategy_default, crate::isolation::IsolationStrategy::SharedBranch);
    assert!(c.isolation_per_agent.is_empty());
}

#[test]
fn isolation_strategy_roundtrips_through_serde() {
    let mut c = OrchestratorConfig::default();
    c.isolation_strategy_default = crate::isolation::IsolationStrategy::SeparateBranches;
    let json = serde_json::to_string(&c).unwrap();
    let back: OrchestratorConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.isolation_strategy_default, crate::isolation::IsolationStrategy::SeparateBranches);
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-orchestrator isolation_strategy` (field undefined).

- [ ] **Step 3: Implement.** Add to `OrchestratorConfig` (group with the existing scope field at
`orchestrator_fields.rs:109`):
```rust
    /// Default multi-agent isolation strategy (spec §5.1). Default: SharedBranch.
    #[serde(default)]
    pub isolation_strategy_default: crate::isolation::IsolationStrategy,
    /// Per-agent isolation strategy overrides (numeric agent id → strategy).
    /// Keyed by raw u64 string for TOML/JSON friendliness (matches the
    /// agent_id-as-string convention in `json_vcs_facade.rs`).
    #[serde(default)]
    pub isolation_per_agent: std::collections::HashMap<u64, crate::isolation::IsolationStrategy>,
```
Add the matching lines to `impl_default.rs` (near `scope_enforcement: ScopeEnforcement::default()` at
`impl_default.rs:41`):
```rust
            isolation_strategy_default: crate::isolation::IsolationStrategy::default(),
            isolation_per_agent: std::collections::HashMap::new(),
```

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-orchestrator isolation_strategy`.

- [ ] **Step 5: Commit.**
```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/config/orchestrator_fields.rs crates/vox-orchestrator/src/config/impl_default.rs
git commit -m "feat(orchestrator): OrchestratorConfig isolation strategy default + per-agent overrides (spec §5.4)"
```

---

### Task 3: `IsolationPolicy` handle on the orchestrator + accessor

**Files:** Modify `crates/vox-orchestrator/src/orchestrator.rs`;
`crates/vox-orchestrator/src/orchestrator/accessors.rs`.

The orchestrator already holds shared state behind `Arc<RwLock<…>>` (e.g. `scope_guard` at
`orchestrator.rs:94`, `conflict_manager`). The live `IsolationPlan` belongs alongside them so the
submit path *and* the GUI REST handler read/write the same instance.

- [ ] **Step 1: Failing test.** Add to the orchestrator tests (mirror an existing
`Orchestrator::new(OrchestratorConfig::default())` test):
```rust
#[test]
fn isolation_policy_seeds_from_config_default() {
    let mut cfg = crate::config::OrchestratorConfig::default();
    cfg.isolation_strategy_default = crate::isolation::IsolationStrategy::SplitChanges;
    let orch = Orchestrator::new(cfg);
    let plan = crate::sync_lock::rw_read(&*orch.isolation_policy_handle());
    assert_eq!(plan.default, crate::isolation::IsolationStrategy::SplitChanges);
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-orchestrator isolation_policy_seeds`.

- [ ] **Step 3: Implement.**
  - Add a field to the `Orchestrator` struct: `pub(crate) isolation_policy: std::sync::Arc<std::sync::RwLock<crate::isolation::IsolationPlan>>` (mirror the `scope_guard` field decl at `orchestrator.rs:94`).
  - In `Orchestrator::new`, seed it from config: `default = config.isolation_strategy_default`, and pre-load `per_agent` from `config.isolation_per_agent` (mapping `u64` → `AgentId`).
  - Add accessor to `accessors.rs` (mirror `conflict_manager_handle` at `accessors.rs:410`):
```rust
    /// Live multi-agent isolation policy (strategy default + per-agent overrides).
    pub fn isolation_policy_handle(
        &self,
    ) -> std::sync::Arc<std::sync::RwLock<crate::isolation::IsolationPlan>> {
        std::sync::Arc::clone(&self.isolation_policy)
    }
```

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-orchestrator isolation`.

- [ ] **Step 5: Commit.**
```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/orchestrator.rs crates/vox-orchestrator/src/orchestrator/accessors.rs
git commit -m "feat(orchestrator): live IsolationPolicy handle seeded from config"
```

---

### Task 4: Enforcement — SharedBranch makes the lock authoritative (§5.3a/b)

**This is the headline enforcement task.** Today `task_submit.rs:382` is
`let _ = self.lock_manager.try_acquire(&fa.path, agent_id, lock_kind);` — the lock result is
**discarded**, so SharedBranch parallelism has no teeth. The pre-queue `PolicyEngine::check_before_queue`
(`task_submit.rs:359`) already *checks* locks/scope; this task makes the **acquire** authoritative
under the SharedBranch strategy.

**Files:** Modify `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs`;
`crates/vox-orchestrator/src/scope.rs`.

- [ ] **Step 1: Failing test.** Add an integration test (the submit path is `async`; mirror the
existing submit tests — grep `check_before_queue` / `submit` tests for the harness). Assert: with
`isolation_strategy_default = SharedBranch`, two agents requesting an **exclusive write** on the *same*
path cannot both hold it — the second submit returns `OrchestratorError::LockConflict` (today it
silently double-"acquires"):
```rust
#[tokio::test]
async fn shared_branch_denies_concurrent_writers_to_same_file() {
    let mut cfg = OrchestratorConfig::for_testing();
    cfg.isolation_strategy_default = IsolationStrategy::SharedBranch;
    cfg.scope_enforcement = ScopeEnforcement::Strict;
    let orch = Orchestrator::new(cfg);
    // agent 1 submits a task writing "shared.rs" -> Ok, holds the exclusive lock.
    // agent 2 submits a task writing "shared.rs" -> Err(LockConflict).
    // (Use the same submit entrypoint the existing submit tests drive.)
}
```

- [ ] **Step 2: Run → FAIL.** Today the second acquire is discarded → both "succeed".

- [ ] **Step 3: Implement.** In `task_submit.rs`, read the effective strategy
(`self.isolation_policy_handle()` → `strategy_for(agent_id)`), and replace the discard at `:382`:
```rust
    // Under SharedBranch the file lock is AUTHORITATIVE (spec §5.3a): a failed
    // exclusive acquire on a contested path is a hard conflict, not best-effort.
    // SplitChanges / SeparateBranches tolerate overlap (conflicts recorded later),
    // so they keep the best-effort acquire.
    for fa in file_manifest {
        if fa.access == AccessKind::Write {
            match self.lock_manager.try_acquire(&fa.path, agent_id, LockKind::Exclusive) {
                Ok(_) => {}
                Err(e) if strategy == IsolationStrategy::SharedBranch => {
                    return Err(OrchestratorError::LockConflict(e));
                }
                Err(_) => { /* tolerated: overlap becomes a recorded conflict at merge */ }
            }
        }
    }
```
Keep the existing affinity-map + scope `assign_file` block (`:386`) as-is.

- [ ] **Step 4: §5.3(b) Strict-for-multi-agent helper.** Add a small helper in `scope.rs` (TDD it):
```rust
/// §5.3(b): multi-agent runs should default to Strict scope. Returns the
/// enforcement to use given how many agents are active and the configured base.
pub fn multi_agent_enforcement(active_agents: usize, configured: ScopeEnforcement) -> ScopeEnforcement {
    if active_agents > 1 && configured == ScopeEnforcement::Warn {
        ScopeEnforcement::Strict
    } else {
        configured
    }
}
```
with a unit test (`>1 agent + Warn → Strict`; `Disabled stays Disabled`; `single agent unchanged`).
Wire it where `scope_enforcement` is read in the submit path (the `scope_enforcement` binding feeding
`task_submit.rs:356`). **Do not** silently flip `Disabled` → that's an explicit opt-out.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator` (submit + scope).

- [ ] **Step 6: Commit.**
```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs crates/vox-orchestrator/src/scope.rs
git commit -m "feat(orchestrator): SharedBranch makes file lock authoritative + Strict-for-multi-agent (spec §5.3)"
```

> **Note on `batch.rs`:** the batch submit path has the same `scope_guard_lock` pattern
> (`batch.rs:110`) and likely the same discarded-lock shape. Grep it; if it mirrors `task_submit.rs`,
> apply the identical fix in this task (add a sub-step + assertion) — do not leave a second silent
> discard.

---

### Task 5: Enforcement — SplitChanges per-agent change + SeparateBranches branch bind

**Files:** Modify `crates/vox-vcs/src/jj_actor.rs`, `crates/vox-vcs/src/backend.rs`,
`crates/vox-vcs/src/lib.rs`; Modify `crates/vox-orchestrator/src/workspace.rs`.

The `JjActorHandle` already does snapshot/undo (`workspace.rs:298-323`). SeparateBranches needs a
**branch-create** op that does not exist yet — add it (with a test), gated on `#[cfg(feature = "jj")]`.

- [ ] **Step 1: Add the `create_branch` trait method (vox-vcs).** In `backend.rs`, extend `VcsBackend`
with a default-`Unavailable` method (mirror `add_remote` at `backend.rs:49`):
```rust
    /// Create (or move) a named branch/bookmark at the current change. Backends
    /// without a branch concept (CAS) return [`VcsError::Unavailable`].
    async fn create_branch(&mut self, _name: &str) -> Result<(), VcsError> {
        Err(VcsError::Unavailable("backend has no branches".into()))
    }
```

- [ ] **Step 2: Failing test in `jj_actor.rs` (or `jj_backend.rs`).** Against a temp colocated repo,
snapshot a change then create a branch and assert it is visible (via jj-lib's bookmark listing —
resolve the exact read against jj-lib 0.42, consistent with the P2 spike's `git`/bookmark module
usage). Run → FAIL (`create_branch` is the default `Unavailable`).

- [ ] **Step 3: Implement** the `JjBackend::create_branch` (in the `jj_lib::`-confined `jj_backend.rs`)
plus the `Command::CreateBranch { name, reply }` variant + handler in `jj_actor.rs` (mirror the
existing `Push` variant at `jj_actor.rs:89` and its `guarded!`-wrapped handler). Re-export nothing new
(handle method is via the trait).

  **If a jj-lib 0.42 bookmark-create call cannot be resolved within the slice** (the P2 spike noted
  jj's git/bookmark surface is unstable): do NOT fake it. Leave `create_branch` returning
  `Unavailable("jj branch create: <obstacle>")`, and in Task 5b below **degrade SeparateBranches to
  recording the `BranchName` in `AgentWorkspace` only** (metadata via the existing
  `set_bound_branch`, `workspace.rs:115`). Report DONE_WITH_CONCERNS.

- [ ] **Step 4: Wire strategies into `workspace.rs` (Task 5b).** Add a method on `WorkspaceManager`
that, given an `AgentId` + resolved `IsolationStrategy`, does the strategy-specific setup:
  - **SharedBranch** → no-op (locks already enforce; one shared change).
  - **SplitChanges** → `create_change(agent_id, …)` (already exists, `workspace.rs:253`) so each agent
    tracks its own change; merge-back conflict recording is already wired (`workspace_merge_json`).
  - **SeparateBranches** → `#[cfg(feature = "jj")]` call `self.vcs`'s `create_branch("agent/<id>")`
    via `spawn_supervised_infallible` (mirror the snapshot/undo spawns at `workspace.rs:298`), then
    `ws.set_bound_branch(BranchName::parse("agent/<id>")?)`. Without `jj`: just `set_bound_branch`.
  Add a unit test (no-jj build) asserting SeparateBranches records the bound branch on the workspace,
  and (jj build, `#[cfg(feature = "jj")]`, mirror `jj_actor_snapshot_on_merge` at `workspace.rs:417`)
  asserting the branch op runs without panic.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-vcs` and
`cargo test -p vox-orchestrator --features jj workspace` and `cargo test -p vox-orchestrator workspace`
(no-jj fallback path).

- [ ] **Step 6: Commit.**
```bash
cargo fmt -p vox-vcs && cargo fmt -p vox-orchestrator
git add crates/vox-vcs/src/jj_actor.rs crates/vox-vcs/src/backend.rs crates/vox-vcs/src/jj_backend.rs crates/vox-orchestrator/src/workspace.rs
git commit -m "feat(vcs): create_branch op + SplitChanges/SeparateBranches strategy wiring (spec §5.1/§5.3)"
```

---

### Task 6: `isolation_status_json` facade — strategy + per-agent + live conflicts

**Files:** Modify `crates/vox-orchestrator/src/json_vcs_facade.rs`.

The GUI panel needs one JSON bundle. Build it on the same facade that already serves the MCP VCS tools
(`workspace_merge_json` etc.). It must read the **live** `IsolationPlan` (Task 3 handle) and the
**live** `ConflictManager` (`conflict_manager` handle, `accessors.rs:410`; `active_conflicts()` at
`conflicts.rs:187`).

- [ ] **Step 1: Failing test.** Add to the `json_vcs_facade.rs` tests module (mirror
`workspace_status_json_no_workspace` at `:229`):
```rust
#[test]
fn isolation_status_json_reports_default_and_conflicts() {
    let orch = Orchestrator::new(OrchestratorConfig::default());
    let v = isolation_status_json(&orch);
    assert_eq!(v["strategy_default"], "shared_branch");
    assert_eq!(v["per_agent"].as_object().map(|m| m.len()), Some(0));
    assert_eq!(v["active_conflicts"].as_array().map(|a| a.len()), Some(0));
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-orchestrator isolation_status_json`.

- [ ] **Step 3: Implement** `isolation_status_json(orch: &Orchestrator) -> Value` reading
`orch.isolation_policy_handle()` (default + per_agent, serialized with the `snake_case` serde rename
from Task 1) and `orch.conflict_manager_handle()` `active_conflicts()` projected to
`{ id, path, sides: [agent_id…], created_ms }` (mirror the conflict fields in `conflicts.rs:86`). Keep
agent ids as raw-u64 strings for parity with the rest of the facade (see the comment at
`json_vcs_facade.rs:90`).

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-orchestrator isolation`.

- [ ] **Step 5: Commit.**
```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/json_vcs_facade.rs
git commit -m "feat(orchestrator): isolation_status_json facade (strategy + per-agent + live conflicts)"
```

---

### Task 7: REST + WS surface — `/api/v2/vcs/isolation` (GET/POST) + `vcs.isolation.changed`

**Files:** Modify `crates/vox-orchestrator-mcp/src/http_gateway/dashboard_api.rs`; Modify
`crates/vox-orchestrator-mcp/src/http_gateway/scientia_feed.rs` (add a sibling topic const) or create
`crates/vox-orchestrator-mcp/src/http_gateway/vcs_feed.rs`.

Model exactly on `get_scientia_queue` (`dashboard_api.rs:516`) + the `scientia.queue.changed` topic
(`scientia_feed.rs:20`). The router is `dashboard_api.rs:581 router()`, nested under `/api/v2`
(`http_gateway/mod.rs:228`); the WS multiplex is `/v1/ws` (`mod.rs:234`). State is
`gs.server_state.orchestrator` (used throughout, e.g. `dashboard_api.rs:132`).

- [ ] **Step 1: Failing test.** Mirror `tests/api_v2_health_test.rs` (which hits `/api/v2/health` via
`build_app`). Build the gateway app and assert `GET /api/v2/vcs/isolation` returns `200` with an
envelope `{ "v": …, "data": { "strategy_default": "shared_branch", … } }` (the `ok(…)` envelope shape
from `dashboard_api.rs:526`). Run → FAIL (route 404).

- [ ] **Step 2: Implement the GET handler** (mirror `get_scientia_queue`, including
`enforce_dashboard_read(&gs, &connect.0, &headers)` at `dashboard_api.rs:521`):
```rust
/// GET /api/v2/vcs/isolation — live isolation strategy + per-agent + active conflicts.
pub async fn get_vcs_isolation(
    State(gs): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if let Err(e) = enforce_dashboard_read(&gs, &connect.0, &headers) { return e; }
    let v = vox_orchestrator::json_vcs_facade::isolation_status_json(&gs.server_state.orchestrator);
    ok(v)
}
```

- [ ] **Step 3: Implement the POST handler** (mirror the write-side of
`post_dashboard_layout`/`post_mesh_node_kill` at `dashboard_api.rs:592`; use the dashboard *write*
guard if one exists — grep `enforce_dashboard_write` / how `put_dashboard_layout` guards):
```rust
/// POST /api/v2/vcs/isolation/strategy — set the default (and/or per-agent) strategy.
/// Body: { "strategy_default"?: "shared_branch"|"split_changes"|"separate_branches",
///         "agent_id"?: u64, "strategy"?: <same enum>|null }
pub async fn post_vcs_isolation_strategy(/* State, ConnectInfo, headers, Json(body) */) -> Json<Value> {
    // 1) auth guard; 2) parse strategy via serde (the snake_case rename from Task 1);
    // 3) write through orch.isolation_policy_handle() (set `default` and/or `set_override`);
    // 4) publish the WS topic (Step 5); 5) return ok(isolation_status_json(..)).
}
```

- [ ] **Step 4: Register routes.** In `router()` (`dashboard_api.rs:581`), add:
```rust
        .route("/vcs/isolation", get(get_vcs_isolation))
        .route("/vcs/isolation/strategy", post(post_vcs_isolation_strategy))
```

- [ ] **Step 5: WS topic.** Add `pub(crate) const VCS_ISOLATION_CHANGED: &str = "vcs.isolation.changed";`
(sibling to `SCIENTIA_QUEUE_CHANGED` at `scientia_feed.rs:20`) and publish it on the existing WS
broadcast bus from the POST handler (mirror how scientia publishes — grep
`SCIENTIA_QUEUE_CHANGED` publish site). A full background poller is **not** required for P4 (config
changes are user-driven, push-on-write is enough); note that conflict-driven pushes can be a follow-up.

- [ ] **Step 6: Run → PASS.** `cargo test -p vox-orchestrator-mcp` (the new route test + existing
gateway tests). **Verify Cargo edge:** `vox-orchestrator-mcp` already depends on `vox-orchestrator`
(it calls `gs.server_state.orchestrator` everywhere), so no new dependency — confirm with
`cargo build -p vox-orchestrator-mcp`.

- [ ] **Step 7: Commit.**
```bash
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/http_gateway/dashboard_api.rs crates/vox-orchestrator-mcp/src/http_gateway/scientia_feed.rs
git commit -m "feat(gateway): /api/v2/vcs/isolation GET/POST + vcs.isolation.changed WS topic (spec §5.4)"
```

---

### Task 8: GUI panel — strategy + per-agent + conflicts + selector

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/Repository/IsolationPanel.tsx`; Modify
`crates/vox-gui/ui/src/components/surfaces/Repository/RepositoryView.tsx`.

The `repository` surface already exists in the registry (`surface-registry.v1.yaml:431`,
`nav_group: develop`, `nav_icon: branch`, `tier: live_backend`) and renders `RepositoryView.tsx`. P4
**reuses that slot** — no new registry row needed (so no `surface-registry.v1.yaml` regeneration unless
you change the row). The panel fetches `GET /api/v2/vcs/isolation`, subscribes to
`vcs.isolation.changed` over `/v1/ws`, renders strategy + per-agent + live conflicts, and POSTs the
selector. This is frontend (vitest), so the "failing test" is a component test.

- [ ] **Step 1: Failing component test.** Add `IsolationPanel.test.tsx` (mirror the existing vitest
setup — grep `*.test.tsx` under `vox-gui/ui/src` for the harness + a fetch mock pattern). Mock
`fetch('/api/v2/vcs/isolation')` → `{ v:1, data:{ strategy_default:'shared_branch', per_agent:{},
active_conflicts:[] } }`; assert the panel renders "Shared Branch" as the active strategy and shows
"No active conflicts". Run → FAIL (component undefined).

- [ ] **Step 2: Implement `IsolationPanel.tsx`.** A live-backend panel (fetch + WS), not a
command-runner. Render: (a) the current `strategy_default` with a `<select>` of the three strategies
that POSTs `/api/v2/vcs/isolation/strategy` on change; (b) a per-agent table (agent id → effective
strategy, with an override control); (c) an "Active conflicts" list from `active_conflicts` (path +
sides). Re-fetch on the `vcs.isolation.changed` WS message. (Use whatever the codebase's existing live
panels use to read the gateway base URL / WS — grep how `MeshView`/`Scientia` panels fetch `/api/v2/…`
and subscribe to `/v1/ws`; reuse that hook, do not invent a new transport.)

- [ ] **Step 3: Mount it.** In `RepositoryView.tsx`, render `<IsolationPanel />` above or below the
existing harness buttons (keep the existing command buttons — they're still useful).

- [ ] **Step 4: Run → PASS.** Run the GUI unit tests (the project's vitest command — grep
`package.json` `scripts.test` under `crates/vox-gui/ui`).

- [ ] **Step 5: Surface-registry gate.** Run `vox ci gui-surface-registry` (the self-surfacing gate,
`crates/vox-cli/src/commands/ci/gui_surface_registry.rs`). Since the `repository` row is unchanged it
should pass green with no regeneration. **If** you decide a distinct `isolation` view_key is warranted
instead of reusing `repository`, add the row by running `vox ci gui-surface-registry --write` (which
regenerates BOTH `surface-registry.v1.yaml` and `surfaceRegistry.generated.ts` — per the
"never hand-edit auto-generated files" rule) and commit the regenerated artifacts.

- [ ] **Step 6: Commit.**
```bash
git add crates/vox-gui/ui/src/components/surfaces/Repository/
git commit -m "feat(gui): VCS isolation panel — strategy selector + per-agent + live conflicts (spec §5.4)"
```

> **Out of scope, recorded honestly (do NOT fake):** the spec §5.4 *in-language `@config`* isolation
> setting projects through the app contract (`app_contract.rs:92`); P4 ships the `OrchestratorConfig`
> field (Task 2) + GUI control (this task), which is the real, wired surface. The `@config`
> language projection is a follow-up phase and is intentionally not stubbed here. Likewise the op-log /
> undo controls mentioned in §5.4 are already served by the existing MCP `vox_oplog` /
> `oplog_list_json` + `JjActorHandle::undo`; surfacing them in this panel is optional polish, not a P4
> gate — add a read-only op-log tail only if Step 4 is green with time to spare.

---

## Self-Review

- **Spec coverage (§5):** §5.1 three strategies → `IsolationStrategy` enum + `choose_strategy`
  (Task 1) ✓; §5.3(a) lock authoritative → SharedBranch hard-conflict at `task_submit.rs:382`
  (Task 4) ✓; §5.3(b) Strict-for-multi-agent → `multi_agent_enforcement` helper (Task 4) ✓; §5.2
  conflict-as-data → reuses P1's `record_overlap_conflicts`/`workspace_merge_json` (Task 5 SplitChanges,
  surfaced in Tasks 6/8) ✓; §5.4 config → `OrchestratorConfig` fields (Task 2) ✓; §5.4 GUI → REST/WS
  (Task 7) + panel (Task 8) ✓. SeparateBranches branch-create is the one genuinely new VCS op (Task 5)
  with an honest degrade path.
- **Build on P1, don't reinvent:** enforcement routes through the **existing** `FileLockManager`
  (`try_acquire`, `vox-orchestrator-queue/src/locks/mod.rs:106`), `ScopeGuard` (`scope.rs:52`),
  `ConflictManager` (`conflicts.rs:110`), `WorkspaceManager`/`set_bound_branch` (`workspace.rs`), and
  `JjActorHandle` (`jj_actor.rs:294`). No new lock/conflict store is created.
- **Cargo edges:** no new crate dependency — `vox-orchestrator-mcp → vox-orchestrator` already exists
  (Task 7 confirms via build); `vox-orchestrator → vox-vcs` already exists behind `feature = "jj"`
  (`workspace.rs:167`); `create_branch` (Task 5) is added inside `vox-vcs`, the jj-confinement crate.
- **Feature-gating honesty:** every jj-touching path is `#[cfg(feature = "jj")]` with a no-jj fallback
  to SharedBranch/metadata semantics, matching the existing `workspace.rs` cfg pattern — the
  `--no-default-features` build stays green.
- **Auto-generated files:** Task 8 regenerates `surface-registry.v1.yaml` + `surfaceRegistry.generated.ts`
  only via `vox ci gui-surface-registry --write`, never by hand (per repo policy).
- **TDD discipline:** every behavioral task is Write-failing-test → FAIL → implement → PASS → commit.
  Pure-logic tasks (1, parts of 4/6) are plain unit tests; integration tasks (4 submit, 5 jj, 7 gateway,
  8 vitest) cite the existing harness to mirror. No `todo!()`/stub ships; genuinely-deferred surfaces
  (`@config` projection, op-log polish) are called out, not faked.
- **Type/name consistency:** `IsolationStrategy` (`snake_case` serde), `IsolationPlan`,
  `choose_strategy`, `isolation_policy_handle`, `isolation_status_json`, `get_vcs_isolation`,
  `VCS_ISOLATION_CHANGED`, `create_branch` are used identically across Rust + the GUI fetch contract.
