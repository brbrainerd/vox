---
title: "Handoff: Windows box → faster PC (post-M3)"
description: "What landed, what is still open, and what was too slow to finish on the Windows machine — written for a Claude Code session picking this up elsewhere."
category: "Contributor Guides"
status: "current"
---

# Handoff: Windows box → faster PC (post-M3)

Written 2026-09-04, at the point where `chat-harness-bugfix-and-completion` merges into
`main`. This is the "what to do next, and what this machine could not finish" note. For
*getting the repo onto a Mac at all*, see
[`2026-09-04-macbook-clone-handoff.md`](2026-09-04-macbook-clone-handoff.md) — that doc
covers the clone, the toolchain pin, and the sccache/Xcode situation, and is not repeated
here.

## State at handoff

`main` carries Phase G plus Phase M (M0–M3). The M3 "Surface" work — Pareto-frontier
reporting for `vox model scoreboard` and `vox model explain` — is complete, reviewed, and
green:

- Full suite: **8872 passed, 0 failed** under `cargo nextest run --workspace --lib`.
- `vox ci pre-push --complete`: all gates green.
- Six plan tasks, each independently reviewed; a whole-branch review whose 7 findings were
  fixed and re-reviewed clean; then a merge from `main` and a further review of that.

Design record: [`ADR 046`](../src/adr/046-pareto-frontier-reporting.md). Plan (with its
Withdrawn Scope and Blocked Follow-Ups):
[`2026-09-03-m3-surface-pareto-reporting.md`](plans/2026-09-03-m3-surface-pareto-reporting.md).

## Two environment traps that cost this machine hours

Both are **local environment**, not repo defects. Hitting them and misreading them as code
failures is the single easiest way to waste a day here.

### 1. A stale local model catalog silently fails two selection tests

`ModelRegistry::new()` seeds from an embedded bootstrap catalog **plus**
`~/.vox/cache/model-catalog.v1.json`. When that cache is stale, these two fail with
`a model exists`:

- `models::select::tests::select_with_empty_policy_falls_through_to_cascade`
- `models::select::tests::select_with_premium_alias_honors_alias_when_intelligence_high`

Proven by moving the cache aside: both pass, and the full suite goes 8872/8872. Neither
test nor `select.rs` is touched by any recent branch. **If you see these two fail, move
`~/.vox/cache/model-catalog.v1.json` aside before debugging anything.**

### 2. `process_which_finds_system_executable` needs System32 on PATH

`vox-actor-runtime` asserts `cmd.exe` resolves. Some shells here launch without
`C:\Windows\System32` on `PATH`, so `vox_process_which` correctly returns `None` and the
test fails. Not a defect. On a Mac this is the `sh` branch and does not arise.

## Use nextest, not bare `cargo test`

`cargo test --workspace` runs tests in-process-parallel and produces **four** spurious
failures from shared process globals (env vars, cwd, temp dirs) in `vox-search`,
`vox-orchestrator-driver`, `vox-secrets`, `vox-populi`. All four pass single-threaded.
CI uses nextest (process-per-test), which is why CI never sees them. `cargo-nextest` was
installed on this box; install it early on the new one.

Also: bare `cargo test` **fail-fasts**, so one failure hides every later crate. Always
`--no-fail-fast`.

## Why this machine was slow (and what the faster PC changes)

The dominant cost was not compute — it was **disk**. `C:` hit **0 bytes free three times**
during this work. Each time the culprit was `target/debug/incremental`, which reached
**43.8 GB** in the main checkout and **53.3 GB** in one worktree. Deleting only the
incremental caches reclaimed 57 GB the first time, without forcing peer sessions into a
full rebuild the way `cargo clean` would.

Suggestion for the new machine: consider `CARGO_INCREMENTAL=0` for long agent-driven runs.
It is a real tradeoff, not a free win — it makes each individual rebuild slower, and buys
back the disk that was actually stopping work here. On a box with plenty of free space,
leave incremental on and just watch `target/*/incremental`.

A second effect compounds the rebuild cost, though more narrowly than it first appears.
`vox-build-meta` emits `cargo:rerun-if-changed=.git/HEAD` and `.git/refs/` and injects
`VOX_GIT_HASH`, so **a commit invalidates the build scripts of the three crates that use
it** — `vox-cli`, `vox-gui`, `vox-arch-check` — not the whole workspace. That is still
expensive in practice, because `vox-cli` is the crate every `vox ci ...` invocation runs
(its debug binary is ~158 MB to link). So a commit-then-`vox ci`-verify loop pays a
`vox-cli` relink every iteration. Batch commits before verifying. Concretely, on this box:

| Step | Cold | Warm |
|---|---|---|
| `cargo clippy --workspace --all-targets` | **57 min** | ~3 min |
| `vox ci pre-push --complete` | exceeded its own 25-min cap | ~22 min |
| `cargo nextest run --workspace --lib` | ~12 min build + ~5 min tests | ~5 min |

The 57-minute clippy run **passed**, but blew `pre-push`'s internal 25-minute wall-clock
cap, which reports as a failure. Do not chase that as a defect — re-run warm. Part of that
cold cost was a one-time `vox-gui` release sidecar build, which every fresh worktree pays
once (`vox run scripts/gui-build.vox`).

**Work deferred purely for speed:** nothing in M3 was cut. What this box could not afford
was *breadth* of verification — mutation testing (`cargo mutants`), the `--act` Docker
path that reproduces GitHub jobs locally, and any all-features/cross-platform lane. Those
are the obvious first wins on faster hardware.

## Outstanding work

### Parked from the M3 final review (deliberate, both minor)

1. **`budget_recommendation`'s precondition is doc-only.** It takes `recommendable: &[usize]`
   which must come from `recommendable_positions`, but both are `Vec<usize>`, so nothing
   enforces it. A newtype would. Two existing tests pass a raw frontier; their fixtures
   clear both gates, so they pass honestly. Risk if unfixed: a future caller reintroduces
   the defect where `--budget` names a row the table refuses to star.
2. **`explain` computes the frontier over all ranked candidates but prints `.take(5)`.**
   A top-5 row can lose its `[pareto-optimal]` mark to a dominator that is not displayed.
   The behaviour is correct and the *rationale* is now recorded in code
   (`explain.rs`, above the `pareto_frontier` call) — but nothing tells the **CLI reader**,
   who can still see an unmarked row with no visible reason. If you surface it, note that
   the legend deliberately says "no other row", not "no other row shown".

### Found while merging — worth someone's judgment

**The lesson worth carrying:** the branch and `main` each changed compatible-looking code,
git merged both cleanly, and the result **did not compile** — `main` added an exhaustive
`ModelScoreboardRow` literal (`eval_corpus.rs`) while the branch added three fields to that
struct (E0063). A textually clean merge is not a verified merge. After any nontrivial merge
here, **build and run the suite before trusting it**; `vox ci pre-push --complete` will not
catch this class, because it stops at clippy and never builds test targets for the merged
tree.

3. **~~The `#[ignore]` on `legacy_export_covers_all_baseline_tables`~~ — RESOLVED here.**
   Its comment blamed a "suspected turso 0.6.1 execute_batch bug" for three
   `scientia_harness_*` tables, on the premise that `LEGACY_EXPORT_TABLES` named them while
   `sqlite_master` lacked them. The first half was false: the list did not name them (0 hits,
   on `main` too), and `sqlite_master` has all three. Adding those names (plus
   `live_chat_completeness_pending`) makes the test pass; verified repeatedly and reproduced
   independently in review, so the `#[ignore]` was lifted and a real SSOT gate is live again.
   **If the batch-executor symptom ever does return, it now surfaces as a failure rather than
   a skipped test** — which is the outcome to want.
4. **A second, unreferenced `ModelScoreboardRow` exists** at
   `crates/vox-db/src/types/store_types/rows_core.rs:338` — 13 fields, nothing uses it, and
   it does not carry the three columns the live struct gained. Pre-existing and harmless
   today, but exactly the split-brain that produced the semantic merge break described below.
   Deleting it is a small, safe win.
5. **`cargo build -p <single-crate>` can fail where the workspace build succeeds.**
   `vox-gamify` calls `vox_db::paths::local_user_id()`, and `vox_db::paths` is behind the
   `host-integration` feature, which only some other crate enables. Single-package builds
   therefore hit `E0433: cannot find 'paths' in 'vox_db'`. Pre-existing and reproducible on
   `main` without any branch applied. Related in-flight work: `49fb36bc6`.

### From the M3 plan's own Blocked Follow-Ups

6. **Give `success` a real definition** — the prerequisite for everything else. `success`
   currently means "the provider returned a non-error response", and
   `chat_tools/chat/message.rs:1357` hardcodes it `true`. Until this is fixed, the
   reliability axis on the new Pareto surfaces measures provider uptime, not answer quality.
   Wire M2's `completeness_ok` (`bd5c14e05`) into `ModelOutcome.success`.
7. **Decide whether `ModelSelectionEngine` should exist** — dead code with live-looking
   config; `resolve_model_with_registry_fallbacks` has zero production callers.
8. **`novel_explores_so_far` is hardcoded `0`** — `max_concurrent_explorations` never binds.
9. **`vox-config::ExplorationConfig` is a split-brain second parser** of
   `model-routing.v1.yaml`.
10. **`ModelScore.n_calls` is an arbitrary task-category slice** — `vox model explain` hangs
   its rank-confidence suppression and its `[pareto-optimal]` mark on whichever
   `(task_category, strength_tag)` row landed last, because `inject_scoreboard` is keyed by
   `model_id` in the registry API. `explain` discloses this in its output; re-keying would
   ripple into selection.

## Conventions worth carrying over

- **Local gates are the verdict for what they cover; never watch remote checks.** A
  PreToolUse hook blocks `gh pr checks` / `gh run watch` for agent sessions.
- **`vox ci pre-push --complete` does *not* run tests** — it stops at clippy + scoped
  toestub. Run nextest separately; do not read a green `--complete` as a green suite.
- **Never `cargo fmt --all`** on this workspace — it overflows the Windows command line
  (`os error 206`). Use `vox run scripts/fmt.vox`, or `cargo fmt -p <crate>`.
- **`cmd | tail` returns tail's exit code, not cmd's.** Use
  `cmd > /tmp/x.log 2>&1; echo "EXIT=$?"`. This produced two false "silent kill" diagnoses
  before it was caught.
