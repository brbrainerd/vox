---
title: "Multi-Repository Workspace Support Spec (2026-07-31)"
description: "Design for letting the Vox GUI operate against multiple local repositories (not just the Vox repo itself): a workspaces table, a folder-picker/switcher GUI surface, and an honest verdict on the git-worktree-vs-jj isolation question, grounded in what is actually implemented today (jj-lib is a real optional dependency behind vox-vcs, current_dir()-based single-repo binding everywhere else)."
category: "Architecture SSOTs"
status: "draft"
training_eligible: false
---

# Multi-Repository Workspace Support Spec (2026-07-31)

> **Scope note.** This is a *spec*, not an implementation. No feature code was written to
> produce it. Every claim about current state below was checked against source, not carried
> over from planning docs or agent memory — several memory-note claims about the jj integration
> turned out to be stale (§1.3).

## 1. Current state (verified by reading source)

### 1.1 Repo binding: single implicit repo, no workspace concept

Vox's own tooling (GUI backend, orchestrator, CLI) has **no multi-workspace scaffolding today**.
Every path that needs "which repo am I operating on" resolves it the same way: the process's
current working directory.

- `crates/vox-orchestrator/src/bootstrap.rs:50` — falls back to `std::env::current_dir().ok()`.
- `crates/vox-orchestrator/src/gate.rs:106` — `if let Ok(cwd) = std::env::current_dir() { ... }`.
- `crates/vox-orchestrator/src/groups.rs:145-212` — `detect_from_repository_layout(repo_root: &Path)`
  takes a single `repo_root` passed in from the caller's cwd; there is no notion of *which*
  repo among several.
- `crates/vox-gui/src/commands/workspace_town.rs:109` — `let root = std::env::current_dir()...`.
  ("Workspace Town" here is a GUI feature name, not a multi-repo concept — it operates on the
  one root the process happens to be running in.)
- `crates/vox-gui/src/commands/harness.rs` and ten other `commands/*.rs` files match
  `current_dir`/`repo_path`/`project_path`, all following the same single-cwd pattern.

There is no `workspace_id` (or equivalent) threaded through vox-db, vox-orchestrator, or
vox-gui commands. **This is additive-with-plumbing, not purely additive**: introducing a second
concurrently-open repo means every one of these `current_dir()` call sites needs an explicit
workspace root passed in instead of relying on process cwd — see §3.2 for the honest scope of
that migration.

### 1.2 GUI: no folder picker / project switcher exists, but the Tauri capability is present

- `crates/vox-gui/ui/package.json:26` already depends on `@tauri-apps/plugin-dialog@^2.7.1`.
- Grepping `crates/vox-gui/ui/src` for folder-picker / open-dialog / switch-project / recent-project
  UI turns up **zero matches**. The dependency is present (likely pulled in for some other
  file-dialog use, e.g. import/export) but there is no folder-picker, workspace-switcher, or
  recent-projects UI wired to it today.
- Conclusion: the client-side capability to open a native folder picker is one `open()` call
  away (the plugin is already a dependency), but the GUI has never used it for repo selection,
  and the backend has nothing to receive a chosen path into.

### 1.3 jj/jujutsu integration: real optional dependency, not vaporware — but confined to vox-vcs and not wired into orchestrator isolation yet

Correcting the memory note ("jj-as-first-class-VCS initiative... plan-only"): that characterization
is **stale**. As of this pass:

- `Cargo.toml:317` (workspace root) — `jj-lib = { version = "0.42", default-features = false }` is
  a real, versioned workspace dependency, not a comment or TODO.
- `crates/vox-vcs/Cargo.toml` — real crate, feature-gated: `jj = ["dep:jj-lib", "dep:tokio", "dep:futures"]`,
  with the crate description stating it is "the single home for all `jj_lib::` calls."
- `crates/vox-vcs/src/` contains real, substantial source: `jj_backend.rs` (952 lines),
  `jj_actor.rs` (576 lines), `backend.rs` (149-line `VcsBackend` trait abstraction),
  `cas_fallback.rs`, `types.rs` — not stub files.
- `Cargo.lock` resolves both `jj-lib` and `jj-lib-proc-macros` as real locked dependencies
  (lines 6670, 6717) — this compiles today, it is not aspirational.
- `crates/vox-orchestrator/Cargo.toml:52-53` gates orchestrator's own `jj` feature: "pulls
  jj-lib... on by default; `--no-default-features` chains through to a lean build with no jj-lib."
- `crates/vox-orchestrator/src/isolation.rs` defines `IsolationStrategy` (SharedBranch /
  SplitChanges / SeparateBranches) explicitly in terms of **jj changes and branches** ("all
  agents on one jj change"; "jj branches are anonymous and rebasing is conflict-tolerant") per
  spec §5.1 — this is the *multi-agent-on-one-repo* isolation model, and it assumes jj as the
  VCS. It is a decision/assignment record only; the module docstring says enforcement lives in
  `task_submit.rs` (locks) and `workspace.rs` (changes/branches) — those were not opened in this
  pass and their completeness is unverified.
- Grep for `worktree` across vox-orchestrator/vox-cli/vox-gui/src surfaces real, unrelated
  git-worktree usage: `vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs` (CI artifact
  worktree garbage collection) and `vox-cli/src/commands/repo_upgrade.rs`. These are **CI/build
  worktree management**, not a "clone the repo N times for N GUI-opened workspaces" feature —
  there is no existing "cloud/remote execution" concept for arbitrary user repos either; the
  `remote_poller.rs` / `remote_worker.rs` / `populi_remote.rs` matches under vox-orchestrator/a2a
  are agent-to-agent task distribution across the mesh, not remote execution of a user-selected
  workspace.
- The plan docs under `docs/src/architecture/` (`vcs-as-vox-language-feature-jujutsu-2026.md`,
  `agentic-vcs-automation-impl-plan-phase{1..5}-2026.md`, `multi-agent-vcs-replication-*-2026.md`)
  were located but **not read in full** for this pass — treat their "shipped" claims as unverified
  until read; only the source files above were confirmed compiling/real.

**Net assessment:** jj-lib is real and load-bearing for *in-repo* multi-agent isolation
(concurrent agents on the Vox repo itself). It is **not** currently used, anywhere found, as a
mechanism for isolating *multiple different user-selected repositories* opened in the GUI — that
is a different axis (workspace-level, not agent-level) that this spec's §4 must design fresh.

### 1.4 vox-db: no session/task table with a path column found

`find crates/vox-db -iname "*migration*"` surfaces `crates/vox-db/src/migration.rs` (the
migration *mechanism*, generic `Migration::new(version, name, up_sql)`) and
`crates/vox-db/src/facade/migrations.rs`. Actual DDL lives in
`crates/vox-db/src/schema/manifest.rs` (`SCHEMA_FRAGMENTS`, `BASELINE_VERSION`, currently `84`)
which assembles per-domain `.sql` files under `crates/vox-db/src/schema/domains/sql/*.sql`
(e.g. `coordination.sql`, `discovery.sql`, `gamification.sql`). A representative fragment
(`discovery.sql`) shows the house style: `CREATE TABLE IF NOT EXISTS <name> (... PRIMARY KEY ...)`
followed by `CREATE INDEX IF NOT EXISTS idx_<table>_<col> ON <table>(<cols>)`, with a one-line
provenance comment above each new table referencing its originating task (e.g. "`-- 84: ... Task
3.4, harness parity plan`" as an inline numbered comment above `BASELINE_VERSION`). No existing
table in the tables searched (`CODEX_API_REQUIRED_TABLES`, etc.) carries a filesystem-path
column bound to a session or task; `agent_sessions` exists but was not opened to confirm its
schema in this pass. There is no pre-existing "workspace path" column to extend — this spec's
table (§2) would be new, not a retrofit.

## 2. Data model: `workspaces` table

Following the manifest style (§1.4): a new domain fragment (or an addition to an existing
small domain, e.g. `coordination.sql`), a `CREATE TABLE IF NOT EXISTS` with `PRIMARY KEY`, one
supporting index, and a `BASELINE_VERSION` bump with a provenance comment — matching exactly how
version 84 ("Task 3.4, harness parity plan") was added.

```sql
-- 85: feat(workspaces): add workspaces table (multi-repository-workspace-support spec)
CREATE TABLE IF NOT EXISTS workspaces (
    id              TEXT    PRIMARY KEY,      -- uuid, generated client-side on first open
    path            TEXT    NOT NULL UNIQUE,  -- canonicalized absolute filesystem path
    display_name    TEXT    NOT NULL,         -- defaults to folder basename, user-editable
    vcs_kind         TEXT    NOT NULL DEFAULT 'unknown', -- 'git' | 'jj' | 'none' | 'unknown'
    is_default      INTEGER NOT NULL DEFAULT 0, -- 1 for the zero-config Vox-repo workspace
    created_ms      INTEGER NOT NULL,
    last_opened_ms  INTEGER NOT NULL,
    archived_ms     INTEGER              -- NULL = active; set on remove-from-list (soft delete)
);

CREATE INDEX IF NOT EXISTS idx_workspaces_last_opened
    ON workspaces(last_opened_ms DESC);
```

`BASELINE_VERSION` bumps to `85`. `vcs_kind` is detected at add-time by checking for `.jj/` vs
`.git` at the chosen path (cheap `Path::exists()` checks — no jj-lib call required just to
detect presence), and re-checked on open in case the user runs `jj git init` on an existing git
repo later.

No `session`/`task` table needs a new `workspace_id` foreign key for *this* spec to land — see
§3.2 for why that's deliberately deferred.

## 3. GUI surface

### 3.1 Folder picker, switcher, persistence

- **Add workspace**: call `@tauri-apps/plugin-dialog`'s `open({ directory: true })` — the
  dependency is already present (§1.2), this is new call-site code, not a new dependency. On
  selection, canonicalize the path, detect `vcs_kind`, insert into `workspaces`, set as active.
- **Switcher**: a dropdown/palette (recent-first, ordered by `last_opened_ms DESC`) sourced from
  `SELECT * FROM workspaces WHERE archived_ms IS NULL`. Selecting one updates `last_opened_ms`
  and sets the active workspace in whatever GUI-session state construct already exists (needs
  identifying at implementation time — not located in this pass).
- **Persistence across restarts**: the table itself is the persistence; on GUI startup, load the
  most-recently-opened non-archived workspace as the active one, or fall back to the default
  workspace (§4) if the table is empty or the stored path no longer exists on disk.

### 3.2 Additive layer vs deeper threading — stated honestly

The **table and picker UI are purely additive** — no existing code path is touched to add them.
**Making the rest of the GUI actually operate against the selected workspace is not additive.**
Per §1.1, every Rust command handler that today calls `std::env::current_dir()` to find "the
repo" would need that replaced with "the active workspace's `path` column," which means:

1. Introducing an active-workspace-id concept in GUI session state (new, not found to exist).
2. Passing that resolved path explicitly into orchestrator bootstrap (`bootstrap.rs:50`),
   `groups.rs::detect_from_repository_layout`, `gate.rs`, `workspace_town.rs:109`, and the other
   ~10 `commands/*.rs` files that pattern-match on `current_dir`/`repo_path` (§1.1), instead of
   letting them default to process cwd.
3. Confirming vox-orchestrator's task/session state (whatever backs `agent_sessions`, not opened
   in this pass) doesn't implicitly assume "one repo per process lifetime" elsewhere.

This spec's phased breakdown (§5) treats (1)-(2) as a distinct, larger phase from the table +
picker, and explicitly does not attempt (3) — flagged as a pre-req investigation, not scoped
work, since `agent_sessions`' schema was never read.

### 3.3 Vox repo as zero-config default

On first run (empty `workspaces` table), seed one row: `path` = the Vox repo root (same
resolution `bootstrap.rs` already does via `current_dir()`/`current_exe()`-relative lookup),
`display_name` = `"Vox"`, `is_default = 1`. This preserves today's zero-config behavior
byte-for-byte for users who never open the switcher — the switcher and multi-repo path are
strictly additive UI that most users may never touch, while `is_default` gives the GUI a
guaranteed fallback if the previously-active workspace's path vanishes.

## 4. Worktree/isolation checkbox: recommendation

**Recommendation: build a VCS-agnostic abstraction now, backed by plain filesystem isolation
(nothing at all — separate `path` rows are already separate directories) for the multi-workspace
case, and explicitly defer git-worktree or jj-workspace-based intra-repo isolation to a later,
separate spec.**

Justification, grounded in §1.3:

- The isolation question this spec was asked to resolve is "should opening N *different*
  repositories in the GUI use git-worktree, defer to jj, or something else." That is a different
  problem from `vox-orchestrator/src/isolation.rs`'s `IsolationStrategy`, which solves
  *concurrent agents inside one already-open repo* (SharedBranch/SplitChanges/SeparateBranches)
  and is explicitly jj-based.
- For N distinct user-chosen repos, no isolation mechanism is needed at all beyond "N distinct
  filesystem paths" — each `workspaces` row already points at an independent directory tree.
  Git-worktree only matters when you want *multiple checkouts of the same repo* running
  concurrently (e.g. so an agent working in workspace A doesn't collide with the user's own
  checkout) — that is a real future need (an agent operating on one of these workspaces while
  the user keeps working in it) but is **not required to ship §2-§3 of this spec**.
- Building git-worktree isolation now would duplicate work: jj-lib is already a real, compiling
  dependency (§1.3) and `vox-vcs::VcsBackend` already exists as an abstraction point
  (`backend.rs`, 149 lines) with a jj-backed implementation. A *second*, git-worktree-specific
  isolation mechanism introduced at the workspace layer would fork the abstraction the codebase
  already invested in, rather than extending it.
- Full jj-only is also wrong to bake in now: not every repo a user opens will be jj-initialized
  (`vcs_kind` may be `'git'` or `'none'`), and `vox-vcs::VcsBackend` per its own crate description
  is meant to be an abstraction over backends, with jj as "(later) the ... engine" alongside
  `CasFallback` — i.e. the crate's own stated design is multi-backend, not jj-exclusive.
- Therefore: extend `vox-vcs::VcsBackend` (already the seam) with whatever operations a future
  "isolate this workspace for agent work" feature needs, selecting git-worktree vs jj-workspace
  vs no-op-per-directory at runtime based on the row's detected `vcs_kind` — but do not build
  that feature now. It is out of scope for "let the GUI open more than one repo," and speccing
  it without reading `task_submit.rs`/`workspace.rs` (unopened in this pass, per §1.3) would be
  exactly the kind of unverified-claim inheritance this spec was asked to avoid.

## 5. Phased task breakdown

| Phase | Task | Files | Gate |
|---|---|---|---|
| **1** | Task 1.1: `workspaces` table migration | New SQL in `crates/vox-db/src/schema/domains/sql/` (new or existing small fragment); `manifest.rs` — add fragment to `SCHEMA_FRAGMENTS`, bump `BASELINE_VERSION` to 85 with provenance comment (§2) | `cargo test -p vox-db` — baseline digest test passes; `workspaces` present after fresh `baseline_sql()` apply |
| **1** | Task 1.2: Workspace repository/CRUD layer | New `crates/vox-db/src/workspaces.rs` (or facade module matching existing per-domain module convention) — insert/list/touch-last-opened/archive | RED: test asserts `list_workspaces()` returns rows ordered by `last_opened_ms DESC`; GREEN: implement |
| **0** | Task 0.1 (spike, added in adversarial-review pass — do this BEFORE Task 1.3, not deferred to implementation time): locate the real vox-gui backend startup path | n/a — read `crates/vox-gui/src/main.rs`'s `fn main`/`tauri::Builder::setup` and whatever `GuiDbPool`/`PersistentDaemon` construction happens there | Written finding, named file:line, feeding Task 1.3's real hook point — the first draft of this spec left this as a during-implementation unknown; a structural review of a sibling plan this session found that pattern reliably produces wrong guesses, so it is pulled forward here as a zero-risk, cheap investigation step instead |
| **0** | Task 0.2 (spike, same rationale): locate or confirm the absence of existing GUI-session active-workspace state | n/a — grep `crates/vox-gui/src` and `crates/vox-gui/ui/src` for any existing per-session "current project"/"active repo" state construct beyond the `current_dir()` patterns already found in §1.1 | Written finding feeding Task 2.2's real state shape |
| **1** | Task 1.1: `workspaces` table migration | New SQL in `crates/vox-db/src/schema/domains/sql/` (new or existing small fragment); `manifest.rs` — add fragment to `SCHEMA_FRAGMENTS`, bump `BASELINE_VERSION` to 85 with provenance comment (§2) | `cargo test -p vox-db` — baseline digest test passes; `workspaces` present after fresh `baseline_sql()` apply |
| **1** | Task 1.2: Workspace repository/CRUD layer | New `crates/vox-db/src/workspaces.rs` (or facade module matching existing per-domain module convention) — insert/list/touch-last-opened/archive | RED: test asserts `list_workspaces()` returns rows ordered by `last_opened_ms DESC`; GREEN: implement |
| **1** | Task 1.3: Zero-config default-workspace seeding | The real startup hook Task 0.1 located — seed default row iff table empty, using the same root resolution as `bootstrap.rs:50` | Test: fresh DB + no prior workspaces → exactly one `is_default=1` row pointing at the Vox repo root |
| **2** | Task 2.1: `add_workspace` Tauri command + folder picker wiring | New `crates/vox-gui/src/commands/workspaces.rs`; `ui/src` component calling `@tauri-apps/plugin-dialog`'s `open({ directory: true })` | Manual/e2e: picking a folder inserts a row with correct `vcs_kind` detection (`.git` vs `.jj` vs neither) |
| **2** | Task 2.2: Workspace switcher UI + active-workspace GUI state | `ui/src` new switcher component; active-workspace state per Task 0.2's finding (new construct only if Task 0.2 confirmed none exists) | Manual: switching updates `last_opened_ms`, persists as active across a GUI restart |
| **3** | Task 3.0 (investigation, sequenced BEFORE 3.1 — ordering risk flagged in adversarial review): Read `agent_sessions` schema + `task_submit.rs`/`workspace.rs` | n/a — spike/investigation task | Written finding: does session/task state assume one-repo-per-process anywhere not yet found. **If yes, Task 3.1's design below must be revisited before implementing it** — the original draft placed this investigation as a same-phase sibling of 3.1 rather than its prerequisite, which risked implementing 3.1 against an assumption 3.0 might overturn. Do not start 3.1 until 3.0's finding is written and reviewed. |
| **3** | Task 3.1: Thread active workspace path into orchestrator entrypoints | `crates/vox-orchestrator/src/bootstrap.rs:50`, `gate.rs:106`, `groups.rs::detect_from_repository_layout` call sites, plus the ~10 `crates/vox-gui/src/commands/*.rs` files matched in §1.1 — replace bare `current_dir()` with an explicit workspace-root parameter | `cargo test -p vox-orchestrator` with a test repo at a non-cwd path — confirms detection/bootstrap works when invoked against an explicitly-passed root, not just cwd |
| — | **Deferred, explicitly out of scope** | Git-worktree- or jj-workspace-backed isolation for concurrent agent access to a GUI-opened workspace (§4) | Not attempted here; requires reading `crates/vox-orchestrator/src/task_dispatch/submit/task_submit.rs`-area code and the unread jj/VCS plan docs listed in §1.3 first |

**Explicit non-goals of this spec:** replicating a workspace to a second machine, remote/cloud
execution against a user-selected repo (no such mechanism exists anywhere per §1.3), and any
per-agent isolation strategy for multi-repo workspaces (§4).

## 6. Prior art (added in adversarial-review pass)

VS Code's multi-root workspace pattern (`showWorkspaceFolderPick`, "Add Folder to Workspace,"
status-bar workspace picker, recent-folders persistence) is the closest established analog to
§3's design, and this spec's persistence model is a deliberate improvement on one of its known
pain points: several VS Code users have filed reports that multi-root workspace-folder selection
is lost across restarts in some configurations (see "Persist last selected workspace folder...
across sessions," [espressif/vscode-esp-idf-extension#1776](https://github.com/espressif/vscode-esp-idf-extension/issues/1776)).
§3.1's design — the `workspaces` table itself as the persistence layer, loading the
`last_opened_ms`-max row on startup — does not have this failure mode by construction. No further
spec change follows from this; noted here as grounding for §3's rationale rather than inventing
the persistence design's justification from scratch. See also:
[Multi-root Workspaces — VS Code docs](https://code.visualstudio.com/docs/editing/workspaces/multi-root-workspaces),
[Adopting Multi Root Workspace APIs — microsoft/vscode wiki](https://github.com/microsoft/vscode/wiki/Adopting-Multi-Root-Workspace-APIs).
