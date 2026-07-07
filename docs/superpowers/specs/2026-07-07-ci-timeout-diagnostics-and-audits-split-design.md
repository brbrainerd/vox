# CI Timeout Diagnostics + Audits Job Nightly Split

> Design spec, brainstormed 2026-07-07 on `fix/vox-redact-workspace-version`
> (PR #440), after the "Audits" job was repeatedly (mis)diagnosed as fleet
> flakiness across ~7 CI runs before exact job-timestamp math revealed it was
> actually hitting its own `timeout-minutes: 25` almost exactly every time
> (landing at 25m15–17s elapsed). Goal: (1) make timeout-vs-real-failure
> obvious from the log/summary alone, without hand-computing timestamps, and
> (2) bring the Audits job's per-PR runtime back down by moving its two
> proven-heaviest, most self-contained steps to a nightly schedule instead of
> blocking every PR.

## 0. Problem

Two related but separable problems surfaced investigating PR #440's CI:

1. **Timeout vs. failure is indistinguishable from the log.** When a
   self-hosted job hits its `timeout-minutes` budget, GitHub Actions reports
   `conclusion: cancelled` — the exact same conclusion as a genuine
   external cancellation (fleet outage, concurrency-group cancel, manual
   cancel). Diagnosing which one occurred currently requires manually pulling
   `started_at`/`completed_at` via `gh api .../actions/jobs/<id>` and
   subtracting by hand, then comparing against the job's `timeout-minutes` in
   `ci.yml`. This was done 7+ times by hand this session before the pattern
   was recognized.

2. **The Audits job's real per-PR runtime was never observed until now.**
   Every run on this branch failed on an earlier, now-fixed bug (missing
   CUDA/metal excludes, a broken `vox-codegen` feature, a missing
   `vox-publisher` dependency, a `ModelOutcome` compile error, ~30 broken
   rustdoc links, a `vox-cli` dispatch panic) well before reaching the
   job's later steps. Once all of those were fixed, the job ran end-to-end
   for the first time — and its true steady-state runtime turned out to be
   marginal against (then exceeding) its 25-minute budget under normal
   self-hosted-runner contention. A blind timeout bump (25m → 35m) was tried
   and explicitly rejected: "no job should take twenty five minutes long."
   Investigation (see §1) found two steps responsible for ~19 of the ~25
   minutes, both suitable for nightly cadence without losing real coverage.

## 1. Investigation findings (informing scope)

Timing breakdown from a full Audits job log (`gh api .../actions/jobs/<id>/logs`,
step `##[group]Run ...` timestamps subtracted pairwise):

| Step | Duration | Verdict |
|---|---|---|
| checkout + artifact download + apt-get (dbus/GTK/webkit) | ~2m50s | Keep — required for the job's own later builds to link |
| `validate-toestub-contracts` | ~3m41s | Keep — fast, catches real contract drift |
| safety-inventory / hardcoded-values-audit / completion-audit / detect-rules-bench | ~1m40s combined | Keep — all fast |
| `build-timings --crates` (wall-clock per-crate cargo check lanes) | ~6m48s | Keep — measures build-time regressions per-PR, not moving |
| **`feature-matrix`** (13 sequential `cargo check -p vox-cli --features X` combos) | **~8m29s** | **Move to nightly** |
| **`Workspace-wide all-features check (A.4)`** (`cargo check --workspace --all-features`) | **~10+ min** | **Move to nightly** |
| `cuda-features` (`vox ci cuda-features`) | ~0s (early-exits: "nvcc not found") | Keep — free on this fleet today |
| `corpus_prep.vox` + `mens corpus mix` (strict required sources) | unmeasured (every run died at/before this step) | Keep — cost unknown, no evidence it's heavy |
| `Populi CI gate matrix` | unmeasured (never reached in any captured log) | Keep — cost unknown, no evidence it's heavy |

`feature-matrix` and the all-features check are the two steps this session's
real bugs were actually caught by (a missing CUDA exclude, a mis-cfg'd
`vox-codegen` feature, a missing `vox-publisher` dependency) — moving them to
nightly is a real trade-off, not free. The trade-off is accepted: these are
latent-bug-class issues (pre-existing conditions exposed by unrelated changes
getting the job further than before), not the kind of immediate, PR-authored
regression that must block merge. A nightly cadence catches them within 24h
instead of at merge time.

## 2. Part A — Timeout diagnostic marker

**Scope:** the 5 self-hosted jobs in `ci.yml` that this session's ambiguous
cancellations/failures actually came from: `guards-fast`, `lints`,
`compiler-gates`, `tests`, `audits`. (Not the whole workflow — these are the
five sequential per-PR gate jobs; other self-hosted jobs like `setup` or the
nightly workflows aren't part of the pattern this is fixing.)

**Mechanism:** two small composite actions, each a single `run:` step, no
inputs/outputs beyond what's described:

- `.github/actions/job-timing-start/action.yml` — first step of the job.
  Writes `JOB_TIMING_START_EPOCH=$(date +%s)` to `$GITHUB_ENV`.
- `.github/actions/job-timing-report/action.yml` — **last** step of the job,
  `if: always()` so it runs whether the job succeeded, failed, or was
  cancelled/timed out. Takes one input, `budget-minutes` (a plain string the
  job author sets to match that job's own `timeout-minutes:` — no dynamic
  read-back of the job's own timeout exists in Actions' expression context,
  so this is a manually-kept-in-sync literal, same as the existing
  `timeout-minutes:` value itself). Computes
  `elapsed = now - JOB_TIMING_START_EPOCH`, converts to minutes, computes
  `pct = elapsed / (budget-minutes * 60) * 100`. Emits one of:
  - `pct < 80`: `::notice title=Job Timing::Elapsed {elapsed} of {budget}m budget ({pct}%)`
  - `pct >= 80`: `::warning title=Job Timing::Elapsed {elapsed} of {budget}m budget ({pct}%) — approaching timeout`
  - `pct >= 98`: `::warning title=Job Timing::Elapsed {elapsed} of {budget}m budget ({pct}%) — LIKELY TIMED OUT, not a generic cancellation`

  (`::error` is deliberately not used for the timing report itself — a slow
  job isn't necessarily a failed job, and using `warning`/`notice` avoids the
  timing step itself flipping an otherwise-green job red. The real
  pass/fail signal stays with the job's actual steps.)

**Why composite actions, not copy-pasted `run:` blocks:** 5 jobs × 2 steps
would otherwise duplicate the same shell logic 10 times across `ci.yml`;
composite actions keep it to one source of truth, matching how this repo
already extracts repeated CI logic (e.g. `vox-cli-ci`'s shared guard
functions).

**Interaction with `if: always()` and cancellation:** GitHub Actions does run
`if: always()` steps even when the job is cancelled mid-step (including by
its own `timeout-minutes`), as long as the *step* that's currently running
gets killed first — the always()-steps still execute afterward within
whatever time remains before the job's hard timeout enforcement fully tears
down the runner. This is the same mechanism the existing "Post job cleanup"
steps already rely on (visible in every captured log this session,
including the ones that hit the 25-minute timeout — cleanup steps ran even
then). No new risk introduced.

## 3. Part B — Audits job split

**3a. New file: `.github/workflows/feature-audits-nightly.yml`**

New standalone workflow, matching the existing nightly pattern
(`bench-nightly.yml`, `mutation-nightly.yml`, `qwen35-native-nightly.yml`):
`on: workflow_dispatch` + `schedule: cron` (a time offset from the existing
nightlies to avoid runner contention — existing crons are 03:17 and 05:17
UTC; this one uses `47 4 * * *`), self-hosted runner, own generous
`timeout-minutes: 40` (not blocking anyone, so headroom costs nothing), same
toolchain-install/cache steps as the current Audits job (copied, since this
workflow doesn't depend on `setup`'s artifacts the way PR-triggered jobs do
— it does its own checkout + build). Two jobs steps:
run `./target/debug/vox --quiet ci feature-matrix` (after building the
`vox` binary itself, since this workflow has no `setup` job to inherit an
artifact from) and the workspace all-features check (extracted, see 3b).
Failures show up as a red run in the Actions tab — no new notification
plumbing, consistent with how the other three nightly workflows already
behave (checked: none of them create issues or post to Slack on failure).

**3b. Extract the all-features check logic to a shared script**

The all-features check step's CUDA/metal/vox-codegen exclude logic
(currently ~30 lines inline in `ci.yml`'s Audits job) moves to
`scripts/ci/all_features_check.vox` (or a `vox-cli-ci` subcommand if that's
a better fit for this codebase's existing SSOT-script conventions — decided
during planning, not here) so both the nightly workflow and any future
caller share one source of truth for the exclude list, instead of
duplicating it if the nightly workflow needs the same logic. (The current
single copy in `ci.yml` moves wholesale; nothing is duplicated as an
interim state.)

**3c. Trim `ci.yml`'s Audits job**

Remove the `feature-matrix` and `Workspace-wide all-features check (A.4)`
steps from the Audits job in `ci.yml`. Everything else in that job (TOESTUB
validation, safety-inventory, hardcoded-values-audit, completion-audit,
detect-rules-bench, `build-timings`, `cuda-features`, `corpus_prep.vox` +
`mens corpus mix`, `Populi CI gate matrix`) stays as-is.

**3d. Lower the Audits job's `timeout-minutes`**

From 35 back down. The two removed steps accounted for ~19 of the ~25–35
minutes observed; the remaining steps' summed observed time is ~15 minutes
plus two unmeasured tail steps (`corpus_prep`/`mens-mix`, `Populi gate
matrix`) whose cost is unknown. Set `timeout-minutes: 20` as a reasoned
middle ground (15 measured + headroom for the two unknowns and normal
runner contention) rather than either the old 25 (proven insufficient
pre-split) or an arbitrary guess. This is not claimed to be final — the
timing-report marker from Part A will show, on the first several real runs
post-split, whether 20 has comfortable headroom or needs one more
adjustment, and that adjustment will now be a data-driven one-line change
instead of another round of manual timestamp archaeology.

## 4. Out of scope (explicitly, to prevent scope creep during planning)

- Splitting the Audits job into *parallel* jobs (Approach C considered
  during brainstorming, not chosen) — not part of this spec.
- A persisted timing-history file / trend dashboard (considered during
  brainstorming as the most complete option, not chosen) — not part of this
  spec.
- Investigating or changing `sccache` hit-rate behavior across the
  `feature-matrix` step's 13 sequential builds — irrelevant now that step
  is moving off the PR-blocking path.
- Any change to `corpus_prep.vox`, `mens corpus mix`, or `Populi CI gate
  matrix` themselves — their cost is unmeasured and unproven to be a
  problem; this spec doesn't touch them.
- A GPU-enabled nightly runner for `cuda-features` to actually exercise real
  CUDA compilation — out of scope; it stays a no-op check on this fleet
  either way.
