---
title: "Version Control as a Vox Language Feature — a Jujutsu-Native Multi-Agent VCS"
description: "Research: make snapshotting/version-control a first-class Vox language primitive, backed by Jujutsu's model, with an orchestrator that lets N agents work safely on one branch. Grounded in the existing CAS/oplog substrate."
category: "Architecture SSOTs"
---

# Version Control as a Vox Language Feature — a Jujutsu-Native Multi-Agent VCS

**Status:** research / design proposal (2026-06-05). No code yet. Grounded in a 3-pronged
read-only audit of the current orchestrator VCS, the multi-agent concurrency model, and the
compiler's language-extension surface (file:line citations throughout).

## 0. The one-sentence thesis

Vox has **already, accidentally, reimplemented Jujutsu's model in memory** — content-addressed
auto-snapshots, an operation log with undo/redo, conflict-as-data, and an n-way content merge —
but it is (a) disconnected from any real repository, (b) not exposed to the language, and (c) the
multi-agent safety layer that would make it shine is **built but unwired**. The highest-leverage
move is therefore not to build a VCS from scratch; it is to **connect what exists, back it with
real `jj`, and lift it into the language and the orchestrator.**

---

## 1. Biggest wins (calculated, ranked by leverage)

| # | Win | Who benefits | Why it's big | Cost (grounded) |
|---|-----|--------------|--------------|------------------|
| 1 | **N agents safely on one branch** via conflict-as-data | orchestrator, maintainers | Removes the worktree-per-agent tax (disk, cold builds, merge ceremony). Two agents touching the same file produce a *recorded conflict*, not a data race or a hard block. | Wire 3 existing-but-dormant primitives (`ConflictManager`, `ContentMerge`, `AgentWorkspace.overlapping_paths`) into the write path. **No new VCS engine.** |
| 2 | **Automatic, semantic checkpoints** as a language effect | Vox-script authors, external apps | Every `@mutation` / `uses fs,db` boundary auto-snapshots. Time-travel/undo becomes a language guarantee, not a discipline. Mirrors the existing `db.*` interpreter-state model exactly. | `RepoStore` on the interpreter (copy `DbStore`), 1 effect kind, 1 capability row. |
| 3 | **`jj`-grade undo/operation-log for the whole agent session** | maintainers, AI operators | `jj op log` + `jj undo` semantics over the orchestrator's oplog → "undo the last 6 agent operations across 9 files" in one move. The oplog already exists; today it's CAS-only. | Back the oplog with `jj` operations; expose via GUI. |
| 4 | **No staging area, no detached-HEAD, no rebase-fear** for humans editing Vox | maintainers + external Vox users | Adopt jj's working-copy-is-a-commit ergonomics for the repo holding Vox code. Eliminates the #1 Git friction class (see §2). | Real `jj` integration (today it's 2 default-off shell-outs). |
| 5 | **Orchestrator-decided isolation policy** (same-change / separate-change / separate-branch) with full GUI + config control | users, maintainers | The orchestrator already computes `FileAffinity` per task; let it *choose* the VCS strategy per workload instead of one-size-fits-all worktrees. | New policy enum + GUI surface; reuses `affinity_map`. |

The leverage is unusually high because **most of the substrate is already written** (see §3) — the
work is integration + a thin language layer, not a green-field VCS.

---

## 2. Why Jujutsu (grounded in real user experience, not marketing)

Jujutsu (`jj`) is a Git-compatible VCS (it can use a Git repo as its backing store). What its
users actually report — distilled from community write-ups, Steve Klabnik's widely-shared
tutorial, Google's internal adoption (jj's primary author works there), and the recurring
"I can't go back to git" testimonials — and why each property is *disproportionately* valuable for
an **AI-agent orchestrator**:

1. **The working copy IS a commit.** There is no staging area and no "dirty working tree" state.
   Every edit is already a (mutable) commit. → *Agent relevance:* agents never need to `git add`/
   `git commit`; their edits are continuously, automatically versioned. Vox's `SnapshotStore`
   already encodes exactly this model (`snapshot.rs:1-5` cites it explicitly).
2. **Conflicts are first-class data, not a blocked state.** A commit can *contain* a conflict; you
   keep working and resolve later. Merges and rebases never refuse to proceed. → *Agent relevance:*
   **this is the killer feature for multi-agent work.** Two agents editing the same file yields a
   commit-with-conflict that a third pass (or a human) resolves — instead of a lock-wait, a clobber,
   or a failed task. Vox already has the type for this (`ContentMerge`, `ConflictManager`) — unwired.
3. **The operation log (`jj op log`) + `jj undo`.** Every repo mutation (commit, rebase, even
   `jj undo` itself) is an operation you can undo/redo. The reflog's obscurity is replaced by a
   first-class, navigable history-of-history. → *Agent relevance:* "roll back the last N agent
   operations" becomes trivial and *safe*. Vox's `OpLog` is the same idea, CAS-backed today.
4. **Anonymous, cheap, automatic change tracking.** No named branches required; stable **change IDs**
   (distinct from commit IDs) survive rebases. → *Agent relevance:* the orchestrator can let agents
   produce many ephemeral changes that auto-rebase onto each other without branch bookkeeping.
5. **Rebase-anywhere / edit-anywhere.** You can edit any commit in history and descendants
   auto-rebase. → *Agent relevance:* an agent can amend an earlier step of a plan and the rest of
   the work re-flows automatically.

**The Git pain points users cite that jj removes** (each is a real, repeated complaint): the staging
area mental model, "detached HEAD" confusion, fear of interactive rebase, merge conflicts that
*block* all progress, `git stash` juggling, and reflog spelunking after a mistake. For an
orchestrator driving many agents in parallel, **#2 (conflicts as data) and #3 (operation-log undo)
are the two that change what is architecturally possible** — they let many writers coexist on one
line of history without the coordination overhead Git forces.

Honest counterweights (not marketing): `jj` is younger, its CLI/UX is still stabilizing, the Rust
**library API (`jj-lib`) is explicitly unstable** and `#[doc(hidden)]`-heavy (which is *why* Vox's
wrapper shells out to the binary — see §3), Git-hosting interop is via the Git backend (fine), and
team-wide adoption needs the `jj` binary present. These argue for: **depend on the `jj` binary at
runtime, keep our own stable abstraction in front of it, and treat `jj-lib` as optional/aspirational.**

---

## 3. What Vox already has (the grounded substrate)

The audit's headline: **Vox's real version control today is a custom in-memory engine in
`vox-orchestrator`, and it is jj-shaped — but it touches neither git nor jj.** The git/jj adapters
are thin, read-mostly, and largely stubbed.

### 3.1 The real engine (REAL, default-on)
- **`SnapshotStore`** (`crates/vox-orchestrator/src/snapshot.rs:92`) — in-memory, content-addressed
  (SHA3-256, deduplicated) blob store + per-file snapshots. Its own doc frames it as jj's
  "working copy is a commit" and says it "eliminates the need for agents to `git add`/`git commit`."
- **`OpLog`** — operation log with undo/redo; entries carry predecessor hash, change_id, and
  fs/db snapshots before+after. Mirrored to `VoxDb`.
- **`vcs_ops.rs`** (`orchestrator/vcs_ops.rs`) — `capture_snapshot`, `record_operation`,
  `undo_operation`/`redo_operation`, `restore_fs_snapshot` (rewrites the working tree from CAS blobs).
- **Auto-save is already live:** snapshots bracket every task — submit (`task_submit.rs:401`),
  success (`complete/success/mod.rs:407`), failure (`complete/fail.rs:280`), and MCP compiler-tool
  use (`vox-orchestrator-mcp/compiler_tools.rs:106`). "Save as you go" exists — into the CAS/oplog,
  not git/jj.
- **`AgentWorkspace`** (`workspace.rs:81`) — per-agent diff overlay on a base snapshot, with
  `overlapping_paths()` (`workspace.rs:222`) to detect two agents touching the same file.
- **jj-style primitives, hand-rolled, calling zero jj-lib:** `ContentMerge` (n-way `Merge<T>`
  with Git-marker materialization, `jj_backend.rs:46`) and `OperationDag` (Kahn topo-sort,
  `jj_backend.rs:160`).

### 3.2 The thin/stubbed git+jj layer
- **jj-lib Rust API usage = effectively zero**: one `JJ_LIB_PINNED_VERSION` const + one no-op
  `#[cfg(test)]` print (`jj_backend.rs:245-255`). The module doc lists jj modules "used" — but none
  are called. The `=0.27.0` pin (now `0.42`) existed *only* to hold gix back (confirmed: upgrading
  jj-lib dissolved the gix-0.84 resolution conflict).
- **`JjBridge`** (`jj_backend.rs:264`) shells out to `jj commit` / `jj abandon @-`, **behind the
  default-off `jj-backend` feature** and warn-on-failure — so in a default build it never runs.
- **`vox-git`** uses `gix` for exactly one thing (ahead/behind via `merge_base`, `bridge.rs:235`);
  reads HEAD/refs/remote by **hand-parsing `.git/` files**; runs allowlisted read-only `git` for
  diff/log (`read_cmd.rs`). **`fetch`/`push` are type-only stubs (no implementation exists);
  commit/branch/checkout/merge are absent** (`sync.rs`). Used only by the CodeRabbit reviewer.
- **Backend selection is compile-time only** (`jj-backend` Cargo feature); there is **no runtime
  config/env/settings** to choose a VCS backend.

### 3.3 The multi-agent concurrency model (and its gaps)
- **`FileLockManager`** (`vox-orchestrator-queue/src/locks/`) — single-writer/multi-reader per path,
  in-memory + optional `VoxDb`/Turso fence-token persistence. **But it is consulted only at task
  submit, and the result is discarded** (`task_submit.rs:382` `let _ = ...try_acquire(...)`).
- **`ScopeGuard`** (`scope.rs:52`) — per-agent allowed-path set; **defaults to `Warn`, not
  `Strict`** (`config/impl_default.rs:41`).
- **MCP `scope_guard.rs`** — the *only* hard runtime enforcement: rejects write tools whose path is
  outside an agent's declared `.vox/agents/{id}.md` scope glob.
- **`ConflictManager`** (`conflicts.rs:129`) — records both sides of a contested file with
  `TakeLeft`/`TakeRight`/`DeferToAgent`/`Manual` strategies. **Built, but nothing in the dispatch/
  merge path calls `record_conflict`.**
- **No worktree-per-agent** — all agents share one working tree, isolated only by the in-memory
  overlay/lock/scope layers. (Real `git worktree` is used only by the CodeRabbit reviewer.)
- **Distributed lease** (`a2a/dispatch/lease_gate.rs`) — per-`scope_key` DB lease prevents duplicate
  cross-node execution on the Populi mesh; the closest thing to a real ownership arbiter, but
  per-scope-key, not per-file.

**The gap = the opportunity.** Conflict-as-data, n-way merge, overlap detection, and an oplog all
exist as code. They are not connected to the write path, to each other, or to real jj. Wiring them
is the bulk of the win.

---

## 4. Design: version control as a Vox language primitive

Mirror the **`db.*` subsystem**, which is the proven precedent for a stateful, language-integrated
capability that spans the whole pipeline (lexer → AST → HIR → typeck → lower → interpreter exec →
codegen → packaging). The interpreter already *executes* `db.*` against a per-interpreter store
(`eval/db.rs:38` `DbStore`); a `repo.*` subsystem follows the same shape.

### 4.1 The `repo.*` / `vcs.*` builtin namespace
A namespaced builtin family (sibling to `db.*`, `Browser.*`, `Scrape.*`), dispatched at
`eval/builtins.rs:108 call_builtin_method(obj, method, args, caps)`. The `caps` parameter is already
the runtime capability gate. Proposed surface (intentionally jj-shaped):

```
repo.snapshot(label?)        -> Change      # capture current working state (auto on effect boundaries)
repo.changes()               -> [Change]    # the operation log
repo.diff(a?, b?)            -> Diff
repo.undo() / repo.redo()    -> Change
repo.new()                   -> Change      # start a fresh change (anonymous; jj `new`)
repo.squash(into) / repo.restore(path)
repo.conflicts()             -> [Conflict]  # first-class, from ConflictManager
repo.resolve(path, strategy) -> Resolution
```

### 4.2 A `Vcs` effect (capability/permission)
Add a `Vcs` (or parameterized `Vcs(repo)`) effect kind — touching the four parallel sites the audit
identified: `ast/decl/effect.rs:8` (`EffectAnnotation`, `from_keyword`, `as_str`),
`hir/nodes/effect.rs:9` (`HirEffectKind`), the `HirCapability` enum, and the governance rule in
`typeck/effect_check.rs`. **The cheapest single hook:** add `"repo" | "vcs" => Some(HirCapability::Vcs)`
to `stdlib_module_capability` (`effect_check.rs:506`) — then every `repo.*` call is automatically
governed by a `uses vcs` clause and inferred bottom-up (`infer_expr_effects`, `:389`). The
`Mcp(String)` parameterized-effect pattern (`effect.rs:28`) is the precedent for `Vcs(repo_name)`
if you want per-repository scoping. A packaging row goes in
`contracts/capability/runtime-capabilities.v1.yaml` (`required_capabilities.rs:270`).

### 4.3 Automatic snapshotting via decorators + effect boundaries
- A `@versioned` / `@tracked` / `@snapshot` decorator (lexer token at `lexer/token.rs:123`, parser
  at `parser/descent/decl/head.rs`, lowering at `hir/lower/decl.rs`) marks a function/module whose
  mutations are auto-checkpointed — exactly how `@query`/`@mutation`/`@table` work today.
- Even without an annotation, the interpreter can auto-`repo.snapshot()` at every `uses fs`/`uses db`
  effect boundary, because effects are already inferred. This is the "automatic as you go along"
  property the maintainer asked for, delivered by the type system rather than by discipline.

### 4.4 The interpreter `RepoStore` (the heart)
Add `interp.repo: RepoStore` (mirroring `interp.db: DbStore`), holding changes/snapshots/refs and the
oplog. `repo.*` calls lower to a `RepoOpPlan` IR (parallel to `HirDbQueryPlan`) and dispatch through
the existing `opt_plan` interception at `eval/expr.rs:455` to a new `eval/repo.rs::execute_repo_plan`
(parallel to `execute_db_plan`, `eval/db.rs:187`). For **Vox's own development**, `RepoStore` persists
to disk and is backed by the orchestrator's `SnapshotStore`/`OpLog` (and, when present, real `jj`);
for **external programs written in Vox**, it gives every Vox app cheap, automatic, language-level
time-travel of its own working state.

### 4.5 Backend abstraction (fix the compile-time-only gap)
Introduce a runtime-selectable backend trait — `{ InMemoryCas, Jj, Git }` — chosen by config/GUI,
not a Cargo feature. The `InMemoryCas` impl is today's `SnapshotStore`; the `Jj` impl drives the `jj`
binary (replacing the 2 dormant shell-outs with a real adapter and finally giving `vox-git` its
missing commit/branch/fetch/push by delegating to `jj`/`gix`).

---

## 5. The multi-agent orchestrator model (the core ask)

The maintainer's constraint: *keep agents from racing on the same file, but go beyond
"worktree-per-agent" — the orchestrator should decide whether N agents share one branch, split
across branches, or anything in between, with full user control via the Vox GUI and config.*

Jujutsu makes the ambitious option **safe**, because conflicts are data, not blocks. Proposed model:

### 5.1 Three orchestrator-chosen isolation strategies
The orchestrator already computes a per-task **`FileAffinity`** manifest (`Read`/`Write` paths) and
an `affinity_map`. Let it pick, per workload:

1. **Shared change, file-partitioned (default for disjoint file sets).** All agents work on one `jj`
   change/branch. The `FileLockManager` (made authoritative — see 5.3) grants single-writer leases
   per file; agents with disjoint write sets proceed fully in parallel with **zero** branch/worktree
   overhead. This is the big new capability.
2. **Per-agent change, auto-rebased (for overlapping or risky sets).** Each agent gets its own
   anonymous `jj` change off the same base; on completion they auto-rebase/merge. Overlaps that
   can't auto-resolve become **recorded conflicts** (`ConflictManager.record_conflict`) surfaced for
   a resolver pass — never a hard failure.
3. **Separate branches (for long-running or human-review workloads).** Classic isolation, but cheap
   because jj branches are anonymous and rebasing is conflict-tolerant.

The choice is a function of predicted overlap (from `overlapping_paths()` + `affinity_map`), task
duration, and user policy — and is **fully overridable**.

### 5.2 Conflict-as-data, wired
Connect the dormant pieces: on every agent write, `repo.snapshot()`; detect overlap via
`overlapping_paths()`/`affinity_map`; on true overlap call `record_conflict` + attempt `ContentMerge`
auto-resolution; only escalate to a resolver agent or human when trivial resolution fails. This
replaces today's advisory locks + clobber risk with jj's "keep going, resolve later" guarantee.

### 5.3 Make enforcement real
Three concrete fixes the audit makes obvious: (a) stop discarding the lock result
(`task_submit.rs:382`) and gate the **actual MCP write path** (`vox-orchestrator-mcp/dispatch.rs`) on
the `FileLockManager`, not just the scope glob; (b) default `ScopeGuard` to `Strict` for multi-agent
runs; (c) call `record_conflict` from the merge-back path (`json_vcs_facade.rs:119`,
`workspace_merge_json`, which today merely destroys the workspace and counts files).

### 5.4 Full user control (GUI + config)
- **Vox GUI:** a VCS surface (the registry-driven sidebar already has the slot pattern; the MCP VCS
  tools `vox_snapshot_list`/`vox_workspace_create`/`vox_oplog` already exist) showing the operation
  log, live conflicts, per-agent changes, and a one-click "isolation strategy" selector + undo.
- **Config:** an in-language `@config` decl / app-contract field (there is no `vox.toml`; project
  config is in-language, projected to the app contract — `app_contract.rs:92`) setting default
  isolation strategy, scope strictness, auto-snapshot granularity, and conflict-escalation policy.

---

## 6. Phased roadmap (grounded, incremental)

- **P0 — Wire the dormant substrate (no new engine).** Snapshot-on-write; `record_conflict` in the
  merge path; make the `FileLockManager` authoritative at the write boundary; default `Strict` scope
  for multi-agent. *Pure integration of existing code.* Unlocks win #1.
- **P1 — Real `jj` backend.** Replace the 2 dormant shell-outs with a runtime-selectable `Jj` backend
  adapter; give `vox-git` its missing commit/branch/fetch/push via `jj`/`gix`. Unlocks wins #3/#4.
- **P2 — Language primitive.** `repo.*` builtins + `Vcs` effect + `RepoStore` interpreter state
  (copy the `db.*` blueprint). Unlocks win #2 for Vox-script authors and external apps.
- **P3 — Orchestrator isolation policy + GUI.** The three-strategy selector, conflict surface, and
  operation-log/undo in the Vox GUI, with config defaults. Unlocks wins #1/#5 fully.
- **P4 — Decorators + auto-snapshot-on-effect.** `@versioned`/`@tracked`; auto-checkpoint at
  inferred `fs`/`db` effect boundaries.

---

## 7. Risks & open questions
- **`jj-lib` instability** → mitigated by depending on the `jj` *binary* + our own stable trait;
  `jj-lib` stays optional/aspirational (matches current reality).
- **`jj` binary as a runtime dependency** → gate the `Jj` backend behind availability detection;
  fall back to `InMemoryCas`.
- **Conflict semantics for non-text files** → `ContentMerge` is line/marker oriented today; binary
  assets need a "last-writer + conflict record" path.
- **Cross-node (Populi mesh) consistency** → the per-`scope_key` distributed lease must compose with
  per-file leasing; define the precedence (mesh lease ⊇ local file lease).
- **Performance of snapshot-on-every-write** → CAS dedup already exists; need a coalescing/debounce
  policy at effect granularity.
- **User mental model** → expose jj vocabulary (change, operation, conflict) consistently in GUI +
  language so the abstraction is learnable, not a leaky reimplementation.

## 8. Bottom line
The biggest calculable win is **multi-agent concurrency without the worktree tax** — N agents on one
line of history, conflicts as recorded data instead of races or blocks — and it is unusually cheap
because the orchestrator already contains a jj-shaped engine and the exact primitives needed
(`SnapshotStore`, `OpLog`, `ConflictManager`, `ContentMerge`, `overlapping_paths`, `FileLockManager`).
They are simply **not connected**. Connect them (P0), back them with real `jj` (P1), and lift them
into the language by copying the `db.*` model (P2) — and version control becomes a Vox guarantee that
saves both maintainers and Vox-script authors the coordination overhead Git imposes, while giving the
orchestrator a genuinely modern, conflict-tolerant multi-agent VCS under full user control.
