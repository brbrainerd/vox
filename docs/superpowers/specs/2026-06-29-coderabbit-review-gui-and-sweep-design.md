---
title: CodeRabbit review — date-scoped importance sweep + GUI panel
date: 2026-06-29
status: approved-design (revised after adversarial codebase audit)
supersedes_draft: the original "bring-forward" framing was wrong; see Audit Correction.
---

# CodeRabbit review: date-scoped importance sweep + GUI panel

## Audit correction (read first)

An adversarial audit against the actual code overturned the original premise:

- **The `coderabbit` CLI module is already on `main`**, gated behind cargo feature
  `coderabbit` (`crates/vox-cli/src/commands/review/coderabbit/**`, registered in
  `review/mod.rs` under `#[cfg(feature = "coderabbit")]`). There is **nothing to
  bring forward**.
- **`vox-db` `external_review` schema/store is already on `main`** at baseline
  schema **v80**, byte-identical to the branch. The branch
  `claude/skill-discovery-engine` is **v77 and 669 commits behind** (missing
  `activity_log`/`history` domains) — it is obsolete; **do not copy from it** (that
  would downgrade the schema and fail the baseline digest check).
- The only files that differ between main and that branch are `config.rs` and
  `limits.rs`; **main is canonical**. The `limits.rs` **bugs still exist on main**.

So this work is **purely additive on `main`**: extend the existing feature-gated
module, fix the limits constants, and add a GUI surface.

## Verified constraints (ground truth)

CodeRabbit's published numbers are self-contradictory across its own pages. The one
cleanly-documented figure is **Pro = 5 PR reviews/hour**. Owner's lived experience on
Pro: **~150 files per PR cap; full line-by-line reviews on every tier (not
summary-only)**. Treat the per-PR cap as **config-driven, default 140** (margin under
150). No adaptive backoff is needed: `pack_oversized_files`
(`semantic_planner/rules.rs:282`) already guarantees no slice exceeds the configured
cap, so an oversized-PR rejection cannot occur by construction.

### `limits.rs` bugs to fix on main (`coderabbit/limits.rs`)

| Constant | Code (main) | Correct |
|---|---|---|
| `files_per_review(Pro)` | 300 | 150 |
| `reviews_per_hour(Pro)` | 8 | 5 |
| `min_delay_between_prs_secs(Pro)` | 450 | 720 (3600/5) |
| `recommended_max_files_per_pr(Pro)` | 250 | 140 |
| `Enterprise` reviews/hour | 12 | 12 (keep; mark `// unverified`) |
| doc comment "(summary only)" on Free | present | wrong; remove |

## What we reuse (unchanged, already on main, behind `coderabbit` feature)

`semantic-submit` (the GUI path; plan-only by default, `--execute` to open PRs),
`historical-submit`, `batch-submit`, `submit`, `ingest`, `db-backfill`, `db-report`,
`db-status`, `learning-sync`, `deadletter-retry`, `tasks`, `wait`; worktree-isolated
PR creation, clean-baseline topology, rate-limit delays (`limits.rs`), resume
run-state (`.coderabbit/run-state.json`), `external_review_*` VoxDB tables.
`stack-submit` stays but is **never wired to the GUI** (confirmed destructive:
`mod.rs:128`).

Key verified facts the new code builds on:
- Collectors are **async**: `collect_changed_files`/`collect_all_files` in
  `semantic_planner/collector.rs`; full-repo-vs-changed branch at `submit.rs:29-37`.
- `SemanticSubmitConfig` in `semantic_planner/types.rs:217` (17 fields;
  `max_files_per_pr: usize`, `tier: CodeRabbitTier`).
- Manifest path: `.coderabbit/semantic-manifest.json` (`manifest.rs:10`).
- Churn source: `git.rs` `DiffEntry.weight = insertions + deletions`.
- **Chunk order is by rule `order`, files reordered by prefix-packing**
  (`types.rs:184`, `rules.rs:282`). Input ranking does NOT control PR order.

## New work (four additive features + GUI wiring)

### 1. Date scope — `--since`
New **async** collector `collect_files_modified_since(repo, since)` in
`collector.rs`, using `git log --since=<date> --name-only --diff-filter=ACMR
--pretty=format:`. Add `since: Option<String>` to `SemanticSubmitConfig` and
`--since <DATE>` to `semantic-submit`; when set it is the candidate set (takes
priority over `--full-repo`/changed-files at `submit.rs:29-37`).

### 2. Importance ranker — `ranker.rs` + `--top`/`--rank-weights`
`score(file) = w_recency·norm(recency) + w_churn·norm(churn) + w_central·norm(central)`

- recency: commits-touching-file since `since` (from `git log --since --name-only`).
- churn: insertions+deletions since `since` (from `git log --since --numstat`).
- centrality: file-aggregated node degree from the **existing `vox-graph-reader`
  crate** — `GraphifyReader::from_value(graph.json)` then sum `god_nodes(n)` degrees
  per file; node ids are `file::symbol`, split on `::`, normalize a leading
  `.claude/worktrees/<seg>/` prefix. Graph path resolved via
  `vox_config::paths::REPO_GRAPHIFY_REPO_CODE_GRAPH_DIR` (never a hard-coded `.vox/`
  literal). **Loaded only when
  `w_central > 0`.** Any missing-file/parse case → the centrality term is omitted.
  **VERIFIED 2026-06-29** against the real 342MB cache: centrality covers only **39% of
  tracked files** (cache is 84% stale worktree paths). Uncovered files are therefore
  **imputed at the median of covered candidates, NOT zero** — absence must be neutral,
  never a penalty (zeroing would wrongly sink 61% of files). Coverage % is **logged each
  run** with a hint to regenerate the graph (`vox graph`). Consequence: with today's
  stale graph, recency+churn carry the real signal and centrality is a light tiebreaker;
  it strengthens only after a fresh main-scoped graph.
- Weights default equal (`1/1/1`); override via `--rank-weights r,c,g` and
  `Vox.toml [review.coderabbit]`.
- `--top N`: rank candidates, keep highest-scoring N, then plan as usual.

vox-cli gains a `vox-graph-reader` dependency **inside the `coderabbit` feature**.

### 3. Importance-first PR order (post-plan chunk re-sort)
Because the planner orders chunks by rule `order`, add an **opt-in** re-sort: when
ranking is active, compute each chunk's aggregate score (mean of its files' scores)
and re-sort the manifest chunks descending before submission, so the most important
slices become the earliest PRs (and are the ones that land first under the 5/hr
budget). Behind `--rank-order` (default on when `--top`/`--rank-weights`/`--since`
set). Pure manifest post-processing; does not touch grouping.

### 4. GUI panel + async Tauri commands (in `crates/vox-gui/src/`)
Mirror existing patterns exactly:
- `coderabbit_plan(since, cap, rank_weights) -> manifest JSON`: async, shells out via
  `app.shell().sidecar("vox").args(["review","coderabbit","semantic-submit","--since",
  …,"--max-files-per-pr",…,"--rank-weights",…])` (NO `--execute`, NO `--plan`), then
  reads `.coderabbit/semantic-manifest.json` (CWD-relative is safe — existing
  commands use `std::env::current_dir()`).
- `coderabbit_run_async(...) -> {task_id, status:"running"}`: returns immediately,
  `tokio::spawn`s the `--execute` run (hours, rate-limited), emits progress via
  `app_handle.emit("coderabbit://progress", …)`. Mirrors `commands/research.rs`
  `start_research_async` + `commands/control_plane.rs`.
- `coderabbit_report() -> {run_state, db_status}`: reads `.coderabbit/run-state.json`
  (slice/PR statuses) + `db-status --json` (findings totals).
- `coderabbit_token_present() -> bool`: `FORGE_TOKEN`/`GITHUB_TOKEN` presence via
  `vox_secrets::resolve_secret(...)` (never direct `std::env::var` — secrets policy).

Register in `crates/vox-gui/src/main.rs` `generate_handler!`. Frontend uses raw
`invoke(...)` (no tauri-specta) + `listen("coderabbit://progress")`. Route added via
`ui/src/lib/navigation.ts` (`PARENT_CHILD_MAP`/`NAV_LABELS`) + the `View` union in
`ui/src/App.tsx`. Panel matches the approved mockup; uses existing vox-gui ("Limes")
tokens; status pills map to PR/CR states. **Token handling is read-only** — no
key-editor (CodeRabbit auth is a GitHub App install + the existing `ForgeToken`
secret).

### 5. Sidecar feature wiring (gap)
The GUI sidecar is `target/release/vox` (`tauri.conf.json` `externalBin`). `coderabbit`
is **not** a default feature, so the bundled binary must be built with
`--features coderabbit`, and all coderabbit tests/clippy must pass `--features
coderabbit`. Document the build step; if a sidecar build script exists, add the flag
there.

## Testing (all with `--features coderabbit`)
- `limits.rs` unit tests updated to the corrected constants (TDD: edit expectations
  first, watch fail, fix).
- `collect_files_modified_since` test against a temp 2-commit repo.
- `ranker::rank_files` test: churn dominates ordering; centrality term dropped when
  graph absent (deterministic).
- `file path extraction` test: `a/b.rs::sym` → `a/b.rs`; worktree-prefixed id
  normalized.
- chunk re-sort test: higher-aggregate-score chunk sorts first.
- Existing `coderabbit_e2e` still green.

## Cadence
Manual / on-demand: `vox review coderabbit semantic-submit --since <DATE> --top <N>
--execute`. No scheduler.

## Explicitly out of scope (skipped, with reason)
- Bring-forward / schema copy — **already on main** (audit).
- Adaptive backoff — impossible-by-construction given the cap + packer.
- Key-management UI — GitHub-side.
- Scheduler — manual chosen.
- New `sweep` subcommand — extend `semantic-submit`.
- Wiring `stack-submit` to the GUI — destructive.
