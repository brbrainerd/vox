# SP-2 — Graphify Scope Hygiene + Freshness Automation

**Date:** 2026-06-30
**Status:** Design, ready for implementation plan
**Scope:** `vox-graph-reader` (walk), `vox-cli` graphify refresh path; one documented host step
**Predecessor:** SP-1 (directed call queries) — Tasks 1-5 landed on `sp1-directed-call-queries`. This spec fixes the two critical findings from the SP-1 review.

## Problem

The SP-1 review surfaced two issues that make the graph untrustworthy regardless of how good
the query API is:

1. **Scope pollution (non-deterministic).** `walk_source_files` (`rebuild.rs:44-55`) walks with
   `walkdir` and a hand-maintained dir-exclusion list (`.git`, `target`, `.vox`,
   `node_modules`). It misses `worktrees`/`.worktrees`/`dist`, so ~20% of nodes (33,831 /
   168,574 in the measured corpus) come from `.claude/worktrees/` repo copies and built
   `dist/` JS bundles. Because worktree count varies per rebuild, node/edge counts are
   non-deterministic (95,818 → 139,347 → 168,574 observed across one session). The manual list
   already drifted behind `.gitignore`.
2. **No freshness automation.** `vox graphify refresh --auto` and
   `refresh_action(stale_reasons) → Rebuild/Ingest/Skip` exist and work, but nothing invokes
   them. The corpus goes stale (its git_sha is routinely not even an ancestor of HEAD) and only
   a human running the command fixes it.

## Verified facts (the fix is wiring, not building)

- The `ignore` crate is **already a workspace dependency** (`Cargo.toml:332`, `ignore = "0.4.25"`,
  resolved 0.4.26). It is the canonical gitignore-respecting walker (powers ripgrep).
- All polluters are gitignored — `git check-ignore` confirms `.claude/worktrees`, `dist/`,
  `.worktrees/`, `node_modules/`, `target/`, and `**/.vox/cache/`.
- `refresh --auto` already iterates every corpus in the registry by `scope_path`, so external
  repos are a config entry, not new code.

## Goal

Make the graph **deterministic + clean** (hygiene) and **fresh without human action**
(automation), reusing existing machinery, with no Vox-specific hardcoding.

### Non-goals (deferred)

- Cross-harness `.mcp.json` registration (SP-3).
- Receiver-type / method-call resolution (SP-4a) and dataflow (SP-4b).
- Incremental (changed-files-only) graph updates — the existing per-file blake3 cache already
  makes a full rebuild cheap enough; a true incremental graph diff is out of scope.

## Design

### Part A — Scope hygiene (Task 1; ship first)

Replace the `walkdir` walk in `walk_source_files` with the `ignore` crate's `WalkBuilder`:

- `.gitignore` respected via `require_git(false)` → `.gitignore` becomes the **single source of
  truth** for exclusions; the hand-maintained list is deleted. `require_git(false)` is
  load-bearing: it makes `.gitignore` apply even in a checkout without `.git` (an external
  target repo) — verified against ignore-0.4.25's own `gitignore_allowed_no_git` unit test.
- Hidden directories skipped by default (`hidden(true)`) → `.claude/`, `.github/`, `.vox/`,
  `.worktrees/` and other dotdirs drop out, killing the worktree pollution.
- **`target`/`node_modules` are NOT dotdirs**, so `hidden(true)` does not skip them and they are
  excluded only if `.gitignore` lists them. To keep the old hardcoded guarantee for sub-scopes /
  external repos whose `.gitignore` may not cover them, a `filter_entry` prunes `target` and
  `node_modules` by name (belt-and-suspenders).
- Keep the existing extension filter (`rs/ts/tsx/js/jsx/py`) — only the directory-pruning
  source changes.
- **Determinism:** use `sort_by_file_path` (honored by the serial `.build()`) so `graph.json` is
  stable across rebuilds on the same tree.

Behavior changes to record:
- Gitignored + hidden dirs vanish from the graph; counts become deterministic.
- **Over-exclusion verified safe:** an audit (`git ls-files` + `git check-ignore` over 4,499
  tracked source files) found the new walk drops exactly **6** files — all
  `crates/vox-gui/ds/.design-sync/previews/*.tsx` design-system preview fixtures (0.133%), which
  have no inbound imports and call only into the already-walked `@vox-axis/limes` package. No
  tracked source exists under `.github`/`.config`/`.claude/skills`, and no force-added gitignored
  source exists. No carve-out is needed.
- Non-cache parts of `.vox/` are skipped because `.vox` is itself a dotdir (`hidden(true)`),
  matching the old behavior; this is forward-compatible with SP-4 `.vox` extraction (which would
  add `.vox` explicitly when ready).

**Test:** a tempdir containing `.gitignore` with `dist/`, a `src/a.rs`, and a `dist/b.js`;
assert `walk_source_files` returns exactly `[src/a.rs]`. A second case with a hidden `.work/`
dir asserts hidden dirs are skipped.

### Part B — Freshness automation (Tasks 2-4)

`refresh --auto` becomes safe to run unattended on a timer:

1. **Concurrency lock (Task 2).** An advisory `refresh.lock` in the corpus dir under
   `.vox/cache/graphify/`. A lock whose mtime is < 1h old is treated as held → the caller skips
   (no PID-liveness syscall; cross-platform via `std::fs`). Older/unreadable mtime → stale →
   reclaimed (self-heals after a `kill -9`/power loss where no in-process cleanup can run). An
   **RAII Drop guard** releases the lock on normal return, on `?` early-return, AND on
   panic-unwind, so a crashed rebuild does not wedge the corpus until the 1h reclaim. Applied to
   BOTH the `refresh --auto` rebuild arm and the manual `vox graphify rebuild` handler so the two
   cannot race each other. The lock is advisory: the check→write window has a benign TOCTOU race,
   mitigated by the scheduler's `MultipleInstances IgnoreNew` (only a hand-run rebuild could race,
   and both writes are deterministic).
2. **No worktree-drift thrash — ALREADY CORRECT (verified, no change).** It might seem an hourly
   task would rebuild constantly because `scope_path:"."` makes `worktree_drift` fire on any
   uncommitted file. But `refresh_action` (`graphify/mod.rs:121-130`) already rebuilds only on
   `graph_missing`/`graph_corrupt`/`git_drift`/`ttl_expired` and re-ingests only on
   `lexical_lag` — `worktree_drift` alone falls through to `RefreshAction::Skip`. So the auto
   path does not thrash on uncommitted edits today. The plan adds a regression test locking this
   behavior (`["worktree_drift"]` → Skip); no logic change.
3. **Trigger (Task 3, documented host step).** A one-time Windows Task Scheduler registration
   runs `vox graphify refresh --auto` every 60 minutes, **hidden** and whether-or-not-logged-on.
   The trigger MUST set `-RepetitionDuration` (a large finite value, e.g. 3650 days — NOT
   `[TimeSpan]::MaxValue`, which overflows the serializer on some builds); `-RepetitionInterval`
   alone does not repeat reliably. Repo root resolves from the task's `-WorkingDirectory` (cwd
   walk-up, `resolve.rs:14`; there is no `--repo` flag), with `VOX_REPO_ROOT` as the documented
   override. Per AGENTS.md (VoxScript-only automation; no new `.ps1/.sh/.py`) the task invokes the
   `vox` binary directly — no wrapper script. Host-only, matching the existing `VoxCIRunnerScale`
   autoscaler precedent (also Windows Task Scheduler).
4. **Cross-platform equivalents (documented, not built).** The registration command is
   Windows-only, but per the repo's cross-platform SSOT and external-repo direction the spec/plan
   document the Linux/macOS equivalents running the identical `vox graphify refresh --auto`:
   systemd timer (`OnUnitActiveSec=1h`, `Persistent=true`), cron (`0 * * * *`), or launchd
   (`StartInterval=3600`), each with the repo as working dir. The Task 2 lock is pure `std::fs`,
   so the no-overlap guarantee carries to all of them with no per-OS work.

## Components & boundaries

- `walk_source_files` (`vox-graph-reader/src/rebuild.rs`): one focused function; swap the walker,
  keep the signature `(&Path) -> Vec<PathBuf>`. Consumers unchanged.
- Refresh lock (`vox-cli/src/commands/graphify/mod.rs`): a small `with_graph_lock(cache_dir, ||
  …)` helper lives next to the existing `refresh`/`refresh_action` code and wraps both rebuild
  call sites (the `refresh --auto` `Rebuild` arm and the manual `GraphifyCmd::Rebuild` handler).
  `refresh_action` is unchanged.

## Edge cases & correctness

- **Empty/clean tree:** walk returns deterministically sorted (possibly empty) list — fine.
- **Stale lock (dead PID):** treated as not-held; the new run proceeds and overwrites the lock.
- **Corpus fresh:** `refresh --auto` already Skips; no rebuild, no lock churn.
- **Only worktree_drift stale:** auto path Skips (no thrash); `status` still reports the drift so
  a human/agent can rebuild on demand.
- **Scheduled run during active edit/CI:** lock prevents overlap; worst case one run is skipped
  and the next interval catches up.
- **`.gitignore` absent (external repo without one):** `ignore` walks everything except hidden
  dirs; acceptable, and the corpus owner can add a `.gitignore`.

## Testing

- **Hygiene:** a tempdir walk test (no `git init`) asserting `dist/` is excluded by `.gitignore`,
  `.hidden/` by `hidden(true)`, and `node_modules/` by `filter_entry` **without** a gitignore
  rule for it, with sorted output `[src/a.rs, src/b.rs]`.
- **Lock:** unit test — acquire lock, assert a second acquire returns "held/skip"; assert a lock
  with a dead PID is reclaimable.
- **Staleness mapping (regression guard, no logic change):** unit test on the existing
  `refresh_action` — `["worktree_drift"]` → Skip; `["git_drift"]` → Rebuild;
  `["worktree_drift","git_drift"]` → Rebuild; `["lexical_lag"]` → Ingest. Locks that the auto
  path never rebuilds on uncommitted-edit drift alone.
- **Determinism:** rebuild a fixture tree twice; assert identical node/edge counts (regression
  guard against the non-determinism finding).

## Scope boundary (files)

- `crates/vox-graph-reader/src/rebuild.rs` — `walk_source_files` (Part A) + a determinism test.
- `crates/vox-graph-reader/Cargo.toml` — add `ignore = { workspace = true }`; drop `walkdir` if
  unused elsewhere in the crate.
- `crates/vox-cli/src/commands/graphify/mod.rs` — `with_graph_lock` helper wrapping both rebuild
  call sites; `refresh_action` regression test. (No `refresh_action` logic change.)
- `docs/` (e.g. a how-to under `docs/src/how-to/`) — the one-time Task Scheduler registration
  command, with required frontmatter.

No new automation scripts, no GUI, no `.mcp.json`, no extractor/schema change.
