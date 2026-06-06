# Build-Artifact Garbage Collector — Design Spec

**Status:** design / scoping (no code yet)
**Date:** 2026-06-05
**Author:** AI assistant (at maintainer request)
**Motivation:** Per-worktree Cargo `target/` dirs accumulate to hundreds of GB and
periodically fill the disk. A 2026-06-05 manual sweep reclaimed ~268 GB
(53 → 322 GB free on C:). This spec scopes a **safe, automatable** garbage
collector so that never has to be done by hand again.

---

## 1. Problem statement (measured)

- The workspace pulls a ~700–800-crate dependency graph (turso, candle, tantivy,
  wasmtime, tokio, …). A single `target/debug` is **10–90 GB**.
- `.cargo/config.toml` sets `CARGO_TARGET_DIR = { value = "target", relative = true }`.
  Because every git worktree is a full checkout with its own `.cargo/config.toml`,
  this resolves **per-worktree** (`<worktree>/target`) — by design, to avoid build-lock
  contention between concurrent agents (see the corrected comment in that file).
  Consequence: **N worktrees → N full targets.** ~20 Claude worktrees were observed
  holding ~500 GB of `target/` in aggregate.
- Composition of a representative 12 GB target:
  - `incremental/` = **65%** (`.bin` serialized state + `.o`) — *worthless in any
    worktree you are no longer actively iterating in*.
  - `deps/` = **34%** (`.rlib` + `.rmeta`).
  - Debug info is already minimized (deps `debug = 0`, `/DEBUG:NONE` → no `.pdb`),
    so the remaining bloat is structural, not config-fixable.
- The bloat is therefore **duplication across worktrees + incremental cache**, which
  is exactly what a GC (not a config change) should reclaim.

## 2. What already exists (reuse, don't reinvent)

`crates/vox-cli/src/commands/ci/workspace_artifacts/` already implements
`vox ci artifact-audit` and `vox ci artifact-prune`:

- YAML-policy-driven (`contracts/operations/workspace-artifact-retention.v1.yaml`),
  default age thresholds **7 days** (`TransientPolicy`/`ScratchPolicy`).
- `repo_root_stale_target_dirs()` — detects **repo-root** `target-*` / `target_*`
  sprawl dirs.
- Emits typed `ArtifactAuditRow { path, class, bytes, age_days, tracked, delete_candidate, … }`.
- **Explicitly excludes the canonical `target/`** from prune
  (`"canonical Cargo target — not removed by artifact-prune; use cargo clean"`,
  `mod.rs:260`).

**Gaps this spec fills:**
1. No coverage of **per-worktree** target dirs (`.claude/worktrees/<wt>/target`,
   `.worktrees/<wt>/target`, `.coderabbit/worktrees/<wt>/target`).
2. No coverage of the canonical `target/` itself (it is the single largest item and
   is currently hands-off).
3. No **active-build safety gate** — the one rule that makes deletion safe.
4. No **whole-stale-worktree** removal (the bigger reclaim: source + `.git` admin +
   target together).

The GC is an **extension of `artifact-prune`**, sharing its policy file, audit-row
type, age logic, and dry-run/JSON conventions.

## 3. Goals / non-goals

**Goals**
- Reclaim target/worktree disk **without ever breaking an in-flight build or losing
  uncommitted work**.
- Dry-run by default; explicit opt-in to delete; machine-readable report.
- Run unattended on a schedule, and on demand.
- One policy file, auditable thresholds.

**Non-goals**
- Changing the per-worktree target model (intentional; see §1).
- Trimming sccache's own cache (separate concern; can be a later class).
- Touching tracked source or anything not rebuildable.

## 4. The safety model (the heart of the design)

Deleting a `target/` is safe **iff** nothing is building into it right now. The
hard gate, in priority order — a worktree/target is **PROTECTED** (never touched)
when ANY of:

1. **It is the current worktree** (the one the GC runs from).
2. **It is git-`locked`** (`git worktree list --porcelain` → `locked`) — an active
   workflow/agent registration.
3. **An active build process references it.** Snapshot processes named
   `cargo|rustc|rustdoc|lld-link|link.exe|cc1|build-script*|sccache|vox.exe` and map
   each to a worktree by scanning its command line for
   `\\(?:\.claude\\worktrees|\.worktrees|\.coderabbit\\worktrees)\\([^\\]+)` (or the
   repo root). Any worktree that appears is **active → protected.** Use the `sysinfo`
   crate (already a workspace dependency) rather than shelling out, for portability.
4. **It has uncommitted *source* work** (`git status --porcelain` shows tracked
   modifications, or untracked files that are NOT build junk). Build junk
   (`target/`, `build/`, `*.dll`, snapshot/report dirs) does **not** count as dirty.
   Rationale: a dirty worktree is usually a *live agent* mid-task; its target is
   rebuildable, but we protect it to avoid sabotaging in-progress work. (Configurable:
   a `--include-dirty-targets` flag may relax this to clean only the `target/`, never
   the source.)

Beyond the gate, **selection** requires the artifact be **stale**: last-touched
> `max_age_days` (default 7). "Last touched" = newest mtime of a non-`target`,
non-`.git` file in the worktree (NOT the HEAD commit date — a tree can have an old
HEAD but recent edits, and vice-versa).

**Race hardening:** the process snapshot is taken **immediately before each
deletion**, not once up front. Builds last minutes; a fresh snapshot per item plus
the `> max_age_days` staleness requirement makes a start-mid-delete race negligible.

## 5. Collection classes

| Class | What | Default action | Notes |
|---|---|---|---|
| `repo-root-target-sprawl` | `target-*`/`target_*` at repo root | prune if stale | **exists today** |
| `canonical-target` | `<root>/target` | prune if stale **and** no active build | new; today it's excluded |
| `worktree-target` | `<wt>/target` for each `git worktree` | prune if stale + PASSES gate §4 | new; the main win |
| `stale-worktree` | whole worktree dir | `git worktree remove` (+ `rm -rf` fallback, `prune`) if stale + clean + unlocked + not current | new; reclaims source+admin+target |
| `sccache-cache` | sccache store | (future) `sccache --trim`/size cap | out of scope v1 |

Worktree enumeration covers **all three roots** — `.claude/worktrees/`,
`.worktrees/`, `.coderabbit/worktrees/` — since each can hold large stale trees
(observed: a 2-month-old CodeRabbit tree).

## 6. CLI surface (extend `vox ci artifact-prune`)

Add flags rather than a new command:

```
vox ci artifact-audit  [--include-worktrees] [--json]
vox ci artifact-prune  [--include-worktrees] [--remove-stale-worktrees]
                       [--include-dirty-targets] [--max-age-days N]
                       [--yes] [--json]
```

- **Dry-run is the default** for `artifact-prune` with the new classes: it prints the
  plan (path, class, bytes, age, protect-reason or delete-reason) and a reclaim total,
  and deletes nothing without `--yes`.
- `--remove-stale-worktrees` gates the destructive whole-tree class separately from
  target-only pruning.
- Honors the existing YAML policy; new keys below.

### Policy additions (`workspace-artifact-retention.v1.yaml`)
```yaml
worktree_targets:
  max_age_days: 7
  roots: [".claude/worktrees", ".worktrees", ".coderabbit/worktrees"]
  protect_dirty: true          # §4.4
stale_worktrees:
  max_age_days: 7
  require_clean: true
  require_unlocked: true
canonical_target:
  enable_prune: false          # opt-in; off by default (it's your main dev target)
```

## 7. Implementation notes

- **Language/placement:** the heavy logic (process scan, git plumbing, fs walk,
  byte accounting) lives in Rust under the existing
  `ci/workspace_artifacts/` module — it already owns the audit-row type, policy
  loader, and age helpers. Per the repo's VoxScript-first policy, the *scheduled
  glue* is a thin `scripts/target-gc.vox` that shells `vox ci artifact-prune
  --include-worktrees --yes` (no business logic in the script).
- **`sysinfo`** for the active-build gate (cross-platform, already a dep — avoids a
  brittle `wmic`/`tasklist`/`ps` shell-out).
- **Windows file locks:** a running test binary (`target/debug/deps/*.exe`) or held
  DLL cannot be deleted; `rm`-equivalent must continue past locked files and report
  the target as `PARTIAL` rather than failing. A `PARTIAL` result also implies "something
  is still using it" → treat as a soft protect and skip next time.
- **`git worktree remove`** frequently fails on Windows with *"Directory not empty"*
  (leftover untracked build files / handles); fall back to recursive remove +
  `git worktree prune`.
- **voxup hard-link interaction:** the canonical/cargo `vox` binaries are hard-linked
  (one inode, two paths — see the voxup forwarder). GC must never delete those; they
  live in `~/.vox/bin` / `~/.cargo/bin`, outside any `target/`, so they are naturally
  out of scope — but call it out so a future "binary cache" class doesn't break it.
- **Telemetry:** emit a summary event (worktrees scanned, protected w/ reason,
  reclaimed bytes, free-space before/after) so scheduled runs are auditable.

## 8. Scheduling

- On-demand: `vox ci artifact-prune --include-worktrees` (dry-run) then `--yes`.
- Unattended: a scheduled routine (Windows Task Scheduler entry, or the agent
  `schedule`/`/loop` mechanism) running the `scripts/target-gc.vox` glue, e.g. nightly,
  **dry-run-to-log on weekdays, `--yes` on a weekly cadence**, so a human can eyeball
  the plan before the first destructive run.
- A **free-space trigger** is the highest-value mode: run `--yes` automatically only
  when free space drops below a threshold (e.g. < 80 GB), so cleanup happens exactly
  when needed and never otherwise.

## 9. Edge cases / open questions

- **effort-style large-but-recent trees:** 30 GB trees touched 6 days ago are kept by
  the 7-day rule. Correct, but means the policy, not size, drives reclaim. Expose
  `--max-age-days` for manual aggressive sweeps.
- **Shared-target alternative:** explicitly rejected (lock contention across concurrent
  agents). Documented here so it isn't re-proposed.
- **Incremental-only prune:** a cheaper middle option — delete just
  `<wt>/target/*/incremental/` (the 65%) while keeping `deps/` so a rebuild is fast.
  Worth a `--incremental-only` mode; **open question** whether to make it the default
  for *active-but-idle* worktrees (would need the gate to distinguish "building now"
  from "agent attached but idle").
- **Decision needed:** should `canonical-target` prune ever be on by default, or always
  opt-in? (Recommended: opt-in — it's the user's primary dev target.)
- **Decision needed:** dirty-target policy — protect entirely (safe, default) vs
  clean-target-only-never-source (more reclaim). 

## 10. Recommended first slice

1. Add `worktree-target` class + the §4 safety gate (sysinfo process scan, locked,
   current, dirty) to `artifact-audit`/`artifact-prune`, dry-run default, behind
   `--include-worktrees`. This alone would have safely reclaimed the bulk this session.
2. Add the policy keys (§6) + telemetry summary.
3. Thin `scripts/target-gc.vox` + a free-space-triggered scheduled run (§8).

`stale-worktree` removal and the `--incremental-only` mode are fast-follows once the
gate is proven.
