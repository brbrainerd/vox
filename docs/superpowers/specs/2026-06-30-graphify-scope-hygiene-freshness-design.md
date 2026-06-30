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

- `.gitignore` respected by default → `.gitignore` becomes the **single source of truth** for
  exclusions; the hand-maintained list is deleted. New worktree/build patterns are excluded
  automatically as `.gitignore` evolves.
- Hidden directories skipped by default → `.claude/` (and other dotdirs) drop out, killing the
  worktree pollution.
- Keep the existing extension filter (`rs/ts/tsx/js/jsx/py`) — only the directory-pruning
  source changes.
- **Determinism:** build single-threaded and **sort** the resulting path list before returning,
  so `graph.json` is stable across rebuilds on the same tree.

Behavior changes to record:
- Gitignored + hidden dirs vanish from the graph; counts become deterministic.
- Non-cache parts of `.vox/` may now be visited, but the extension filter ignores `.vox` files,
  so there is no functional change today (and it is forward-compatible with SP-4 `.vox`
  extraction). `**/.vox/cache/` stays excluded via `.gitignore`.

**Test:** a tempdir containing `.gitignore` with `dist/`, a `src/a.rs`, and a `dist/b.js`;
assert `walk_source_files` returns exactly `[src/a.rs]`. A second case with a hidden `.work/`
dir asserts hidden dirs are skipped.

### Part B — Freshness automation (Tasks 2-4)

`refresh --auto` becomes safe to run unattended on a timer:

1. **Concurrency lock (Task 2).** Acquire an advisory lockfile in
   `.vox/cache/graphify/` (e.g. `refresh.lock` holding the PID) before a rebuild; if already
   held, skip with a logged message. Prevents a scheduled run from stacking on a manual rebuild
   or CI. Released on completion (and stale-lock tolerant: ignore a lock whose PID is dead).
2. **Stop worktree-drift thrash (Task 3).** With `scope_path:"."`, `worktree_drift` fires on
   any uncommitted file, so an hourly task would rebuild continuously while editing. Change the
   **auto path only** so `worktree_drift` *alone* maps to `RefreshAction::Skip`; rebuild still
   triggers on `git_drift`, `ttl_expired`, `lexical_lag`, or `graph_missing`. Manual
   `vox graphify rebuild` remains forceful (unchanged). This keeps the freshness signal honest
   in `status` (still *reported* as drift) while not thrashing the auto rebuild.
3. **Hidden child processes (Task 3).** Ensure rebuild-spawned children use `CREATE_NO_WINDOW`
   via the existing `quiet_command` helper so a scheduled run never flashes consoles on Windows.
4. **Trigger (Task 4, documented host step).** A one-time Windows Task Scheduler registration
   runs `vox graphify refresh --auto` every 60 minutes, hidden, whether-or-not-logged-on. Per
   AGENTS.md (VoxScript-only automation; no new `.ps1/.sh/.py`) the task invokes the `vox`
   binary directly — no wrapper script. The spec/docs provide the exact
   `schtasks`/`Register-ScheduledTask` command; the user runs it once. This is host-only, like
   the existing CI autoscaler task.

## Components & boundaries

- `walk_source_files` (`vox-graph-reader/src/rebuild.rs`): one focused function; swap the walker,
  keep the signature `(&Path) -> Vec<PathBuf>`. Consumers unchanged.
- Refresh lock + auto-path staleness mapping (`vox-cli/src/commands/graphify/mod.rs`): the lock
  helper and the `refresh_action`-for-auto adjustment live next to the existing
  `refresh`/`refresh_action` code. `refresh_action` keeps its current behavior for any non-auto
  caller; the auto path gets the worktree_drift-skip via a dedicated wrapper or an `auto: bool`
  parameter so the change is explicit and testable.

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

- **Hygiene:** the two tempdir walk tests above (gitignored dir excluded; hidden dir skipped;
  output sorted).
- **Lock:** unit test — acquire lock, assert a second acquire returns "held/skip"; assert a lock
  with a dead PID is reclaimable.
- **Auto staleness mapping:** unit test on the auto wrapper — `["worktree_drift"]` → Skip;
  `["git_drift"]` → Rebuild; `["worktree_drift","git_drift"]` → Rebuild. Assert the non-auto
  `refresh_action` is unchanged for `["worktree_drift"]` (still its current value).
- **Determinism:** rebuild a fixture tree twice; assert identical node/edge counts (regression
  guard against the non-determinism finding).

## Scope boundary (files)

- `crates/vox-graph-reader/src/rebuild.rs` — `walk_source_files` (Part A) + a determinism test.
- `crates/vox-graph-reader/Cargo.toml` — add `ignore = { workspace = true }`; drop `walkdir` if
  unused elsewhere in the crate.
- `crates/vox-cli/src/commands/graphify/mod.rs` — refresh lock + auto-path worktree_drift skip +
  `quiet_command` for spawns; tests.
- `docs/` (e.g. a how-to under `docs/src/how-to/`) — the one-time Task Scheduler registration
  command, with required frontmatter.

No new automation scripts, no GUI, no `.mcp.json`, no extractor/schema change.
