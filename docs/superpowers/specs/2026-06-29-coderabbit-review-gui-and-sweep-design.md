---
title: CodeRabbit review — bring-forward, date-scoped importance sweep, and GUI panel
date: 2026-06-29
status: approved-design
---

# CodeRabbit review: bring-forward + date-scoped sweep + GUI panel

## Problem

A complete CodeRabbit integration exists but is **stranded** on branch
`claude/skill-discovery-engine` (worktree `wt-skill-discovery-engine`): 33 commits
ahead of `main`, 669 behind, never PR'd. None of it is on `main`. We want to:

1. Bring it forward to `main`.
2. Periodically (date-controlled) review the **most important, recently-modified**
   files in the repo, sliced under CodeRabbit's per-PR file cap, as discrete PRs so
   CodeRabbit reviews each slice from scratch.
3. Surface it as a review option in the vox GUI.
4. Handle the API key (it is GitHub-side; minimal work needed).

## Verified constraints (ground truth)

CodeRabbit's published numbers are inconsistent across its own docs and aggregators
(free = "150 files" vs "200 files/hour"; Pro = "300" with no per-PR cap stated; a
separate ~3000-file processing ceiling). The only cleanly-documented figure is
**Pro = 5 PR reviews/hour**. Owner's lived experience on Pro: **~150 files per PR
cap, full line-by-line reviews on every tier (not summary-only)**.

**Design consequence:** do not trust hardcoded vendor constants. The per-PR cap is
**config-driven (default 140, margin under 150)** with **adaptive backoff** — if a
PR is bounced/cancelled as too large, halve the slice and retry.

### Bugs in existing `limits.rs` (fix during bring-forward)

| Constant | Code | Correct |
|---|---|---|
| `files_per_review(Pro)` | 300 | 150 |
| `reviews_per_hour(Pro)` | 8 | 5 |
| `min_delay_between_prs_secs(Pro)` | 450 | 720 (3600/5) |
| `recommended_max_files_per_pr(Pro)` | 250 | 140 |
| `Enterprise` reviews/hour | 12 | 12 (keep; unverified, leave note) |
| doc comment "summary only" (Free) | present | wrong; remove |

## What we reuse (≈95%, already built)

`vox review coderabbit` subcommands, unchanged: `semantic-submit` (the GUI path),
`historical-submit`, `batch-submit`, `submit`, `ingest`, `db-backfill`, `db-report`,
`db-status`, `learning-sync`, `deadletter-retry`, `tasks`, `wait`. Plus the
worktree-isolated PR creation, clean-baseline topology, rate-limit delays, resume
run-state, and the `external_review_*` VoxDB schema/store + `vox-code-audit/review`.

`stack-submit` stays but is **never wired to the GUI** (destructive/deprecated).

## New work (four small pieces)

### 1. Date scope — `--since`
New collector `collect_files_modified_since(repo, since: &str) -> Vec<String>` in
`semantic_planner/collector.rs`, using `git log --since=<date> --name-only
--diff-filter=ACMR` (drop deletions). Wire `--since <DATE>` into `semantic-submit`.
When set, it replaces the default changed-files collection as the candidate set.

### 2. Importance ranker — `ranker.rs`
`score(file) = w_recency·norm(recency) + w_churn·norm(churn) + w_central·norm(centrality)`

- recency: commits-since / days-since from `git log --since`.
- churn: insertions+deletions (reuse existing `DiffEntry.weight` / numstat).
- centrality: degree of the file's node in the existing graphify graph
  (`graphify-out/`). **Degrades gracefully**: if no graph present, drop the term and
  renormalize over the remaining two.
- Weights: equal by default (`1/1/1`), overridable via `Vox.toml`
  `[review.coderabbit.ranking]` and `--rank-weights r,c,g`.

Add `--top N` to `semantic-submit`: rank candidates, keep top N, then hand to the
existing semantic chunker (so slices still group semantically and stay ≤cap).
Ranking determines **which files** and **slice order** (highest-importance PRs first).

### 3. Adaptive backoff
In the per-chunk submit loop: detect a CodeRabbit "too large / cancelled" outcome
(via `wait`/`ingest` signal or PR comment marker). On detection, split the chunk in
half and resubmit the halves; record the split in run-state. Bounded to 2 splits per
chunk, then surface a warning rather than looping.

### 4. GUI panel + Tauri commands
New vox-gui route "CodeRabbit review". Three Tauri commands that **shell out** to the
CLI (no review logic in TS):

- `coderabbit_plan(since, cap, rank_weights, dry) -> Manifest JSON`
  → `semantic-submit --since … --top … --max-files-per-pr … --plan`
- `coderabbit_run(...) -> run-state JSON` → same with `--execute`
- `coderabbit_report(repo) -> findings + slice status JSON` → `db-report --json`

GUI renders the manifest (slice list + importance bars + status), the 5/hr budget,
and the findings summary. Status colors map to CDS git-state roles. Respects the
existing vox-gui ("Limes") token system; uses CDS accent/pro roles where they map.

**Token (key) handling:** read-only. GUI shows FORGE_TOKEN present/absent via
`vox-secrets` (`SecretId::ForgeToken`). **No key-editor UI** — CodeRabbit auth is a
GitHub App install + one secret already managed via Clavis (`github-ci`). Link to
docs for setup.

## Bring-forward plan

Copy-forward (not rebase) onto a fresh branch off current `main`, in a **worktree**
(the vox-broker shim breaks `cargo` in the main dir):

- `crates/vox-cli/src/commands/review/coderabbit/**`
- `crates/vox-db` `external_review` schema + store + tests
- `crates/vox-code-audit/src/review/**`
- contracts: `coderabbit-semantic-groups.v1.yaml`, registry/catalog entries
- `crates/vox-cli/tests/coderabbit_e2e.rs`

Fix the `limits.rs` bugs in the same pass. Rebuild, run `coderabbit_e2e` + `clippy -p`
the touched crates, then PR.

## Testing

- Keep `coderabbit_e2e` (DB ingest + dataset export).
- New: `ranker.rs` unit test (deterministic ordering for a fixed file/churn/graph
  fixture; graceful-degrade case with no graph).
- New: `collect_files_modified_since` test against a fixture repo.
- New: one integration test for worktree PR-manifest creation (currently zero
  coverage) — manifest-only (no live GitHub) to stay hermetic.
- Backoff: unit test that an oversized signal halves the chunk and stops at 2 splits.

## Cadence

Manual / on-demand: documented
`vox review coderabbit semantic-submit --since <DATE> --top <N> --execute`.
No scheduler. Add one later only if it earns its keep.

## Explicitly out of scope (skipped)

- Key-management UI (GitHub-side).
- Scheduler / cron.
- A new `sweep` subcommand (extend `semantic-submit` instead).
- Wiring `stack-submit` to the GUI.
