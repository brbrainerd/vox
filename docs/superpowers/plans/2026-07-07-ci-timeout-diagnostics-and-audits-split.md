# CI Timeout Diagnostics + Audits Nightly Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make timeout-vs-cancellation obvious from the log alone on the 5 self-hosted per-PR gate jobs, and move the Audits job's two proven-heaviest steps (feature-matrix, workspace all-features check — ~19 of the ~25–35 minutes observed) to a new nightly workflow so the PR-blocking job's timeout can come back down from 35 to 20 minutes.

**Architecture:** Two small composite GitHub Actions (`job-timing-start`, `job-timing-report`) bookend each of the 5 target jobs (`guards-fast`, `lints`, `compiler-gates`, `tests`, `audits`) in `ci.yml`, emitting a `::notice`/`::warning` annotation comparing elapsed wall-clock time to that job's own timeout budget. Separately, the all-features-check bash logic is extracted verbatim into a `.vox` script (matching this repo's existing CI-script convention), which a new standalone nightly workflow (`feature-audits-nightly.yml`) calls alongside `vox ci feature-matrix`; both steps are then deleted from `ci.yml`'s Audits job, whose `timeout-minutes` drops from 35 to 20.

**Tech Stack:** GitHub Actions (composite actions, `on: schedule` cron), Bash (`run:` steps), Vox scripting language (`process.run`, existing `scripts/ci/*.vox` conventions).

**Reference spec:** `docs/superpowers/specs/2026-07-07-ci-timeout-diagnostics-and-audits-split-design.md`

---

## Task 1: `job-timing-start` composite action

**Files:**
- Create: `.github/actions/job-timing-start/action.yml`

- [ ] **Step 1: Write the composite action**

```yaml
# Composite action: record this job's wall-clock start time so the paired
# job-timing-report action (last step of the job) can compute elapsed time
# against the job's own timeout-minutes budget. First step of the job —
# no inputs/outputs.
#
# Why: GitHub Actions reports `conclusion: cancelled` identically whether a
# self-hosted job was killed by its own timeout-minutes or cancelled for an
# unrelated reason (fleet outage, concurrency-group cancel, manual cancel).
# Diagnosing which one happened previously required manually pulling
# started_at/completed_at via `gh api .../actions/jobs/<id>` and subtracting
# by hand. See docs/superpowers/specs/2026-07-07-ci-timeout-diagnostics-and-audits-split-design.md.
name: Job Timing Start
description: Records the job's start epoch for job-timing-report to diff against.
runs:
  using: composite
  steps:
    - shell: bash
      run: echo "JOB_TIMING_START_EPOCH=$(date +%s)" >> "$GITHUB_ENV"
```

- [ ] **Step 2: Verify the YAML is well-formed**

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && ruby -ryaml -e "YAML.load_file('.github/actions/job-timing-start/action.yml'); puts 'OK'"`

(If `ruby` isn't available, use whatever YAML-capable interpreter is on the machine — Python's `import yaml` also works: `python -c "import yaml; yaml.safe_load(open('.github/actions/job-timing-start/action.yml')); print('OK')"`. Either is fine; this step only needs to catch a malformed-YAML typo before commit.)

Expected: `OK` printed, no exception.

- [ ] **Step 3: Commit**

```bash
git add .github/actions/job-timing-start/action.yml
git commit -m "feat(ci): add job-timing-start composite action"
```

---

## Task 2: `job-timing-report` composite action

**Files:**
- Create: `.github/actions/job-timing-report/action.yml`

- [ ] **Step 1: Write the composite action**

```yaml
# Composite action: report elapsed wall-clock time against this job's own
# timeout-minutes budget. Last step of the job, `if: always()` at the call
# site so it runs whether the job succeeded, failed, or was
# cancelled/timed out — this is what distinguishes "ran out the clock" from
# a fast real failure/cancellation without hand-computing timestamps after
# the fact.
#
# IMPORTANT (post-adversarial-review): elapsed-time-as-percentage-of-budget
# is NOT by itself evidence of a timeout — a job can legitimately succeed at
# 99% of its budget on a contended fleet. `job-status` (GitHub's own rolling
# job status: success/failure/cancelled) is the real signal for "did this
# actually get cancelled" — the branching below is gated on it first, and
# `pct` only refines the message *within* the already-cancelled case.
name: Job Timing Report
description: Emits an elapsed-vs-budget notice/warning annotation for the calling job.
inputs:
  budget-minutes:
    description: >
      This job's own `timeout-minutes:` value. Actions has no runtime
      read-back of a job's own timeout in the expression context, so this
      is a literal the job author keeps in sync with `timeout-minutes:`
      by hand (same as that value already is).
    required: true
  job-status:
    description: >
      Pass `${{ job.status }}` from the calling workflow — GitHub's own
      rolling status of the job so far (success/failure/cancelled).
    required: true
runs:
  using: composite
  steps:
    - shell: bash
      env:
        BUDGET_MINUTES: ${{ inputs.budget-minutes }}
        JOB_STATUS: ${{ inputs.job-status }}
      run: |
        set -euo pipefail
        start="${JOB_TIMING_START_EPOCH:-}"
        if [ -z "$start" ]; then
          echo "::warning title=Job Timing::job-timing-start action did not run first; skipping report"
          exit 0
        fi
        now=$(date +%s)
        elapsed_s=$(( now - start ))
        elapsed_m=$(( elapsed_s / 60 ))
        elapsed_rem_s=$(( elapsed_s % 60 ))
        budget_s=$(( BUDGET_MINUTES * 60 ))
        pct=$(( elapsed_s * 100 / budget_s ))
        summary="Elapsed ${elapsed_m}m${elapsed_rem_s}s of ${BUDGET_MINUTES}m budget (${pct}%)"
        if [ "$JOB_STATUS" != "cancelled" ]; then
          echo "::notice title=Job Timing::${summary}"
        elif [ "$pct" -ge 90 ]; then
          echo "::warning title=Job Timing::${summary} — LIKELY HIT THIS JOB'S OWN timeout-minutes, not an external cancellation"
        else
          echo "::warning title=Job Timing::${summary} — cancelled well before budget exhausted; LIKELY AN EXTERNAL CANCELLATION (fleet event, concurrency-group supersede, manual cancel), not this job's own timeout"
        fi
```

- [ ] **Step 2: Verify the arithmetic/branching logic in isolation**

The composite action's shell logic can't run standalone (it needs `$GITHUB_ENV`/`inputs.*` context), so verify the exact same arithmetic by extracting it into a throwaway local script with mocked values covering all four branches (success-at-high-elapsed must NOT say timed out — that's the exact false positive this design was fixed to avoid):

```bash
cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix"
for case in "success_high" "cancelled_low" "cancelled_high" "missing"; do
  echo "=== case: $case ==="
  if [ "$case" = "missing" ]; then
    start=""
    JOB_STATUS="cancelled"
  else
    now_test=$(date +%s)
    case "$case" in
      success_high)  start=$(( now_test - 1485 )); JOB_STATUS="success" ;;    # 24m45s elapsed, but SUCCEEDED
      cancelled_low) start=$(( now_test - 300 ));  JOB_STATUS="cancelled" ;;  # 5 min elapsed, cancelled early
      cancelled_high) start=$(( now_test - 1495 )); JOB_STATUS="cancelled" ;; # 24m55s elapsed, cancelled near budget
    esac
  fi
  BUDGET_MINUTES=25
  if [ -z "$start" ]; then
    echo "::warning title=Job Timing::job-timing-start action did not run first; skipping report"
    continue
  fi
  now=$(date +%s)
  elapsed_s=$(( now - start ))
  elapsed_m=$(( elapsed_s / 60 ))
  elapsed_rem_s=$(( elapsed_s % 60 ))
  budget_s=$(( BUDGET_MINUTES * 60 ))
  pct=$(( elapsed_s * 100 / budget_s ))
  summary="Elapsed ${elapsed_m}m${elapsed_rem_s}s of ${BUDGET_MINUTES}m budget (${pct}%)"
  if [ "$JOB_STATUS" != "cancelled" ]; then
    echo "::notice title=Job Timing::${summary}"
  elif [ "$pct" -ge 90 ]; then
    echo "::warning title=Job Timing::${summary} — LIKELY HIT THIS JOB'S OWN timeout-minutes, not an external cancellation"
  else
    echo "::warning title=Job Timing::${summary} — cancelled well before budget exhausted; LIKELY AN EXTERNAL CANCELLATION (fleet event, concurrency-group supersede, manual cancel), not this job's own timeout"
  fi
done
```

Expected output (four cases, one per branch):
```
=== case: success_high ===
::notice title=Job Timing::Elapsed 24m45s of 25m budget (99%)
=== case: cancelled_low ===
::warning title=Job Timing::Elapsed 5m0s of 25m budget (20%) — cancelled well before budget exhausted; LIKELY AN EXTERNAL CANCELLATION (fleet event, concurrency-group supersede, manual cancel), not this job's own timeout
=== case: cancelled_high ===
::warning title=Job Timing::Elapsed 24m55s of 25m budget (99%) — LIKELY HIT THIS JOB'S OWN timeout-minutes, not an external cancellation
=== case: missing ===
::warning title=Job Timing::job-timing-start action did not run first; skipping report
```

The `success_high` case is the one that matters most: 99% elapsed but `JOB_STATUS=success` must print the plain `::notice` with no "timed out" language at all. If it doesn't, the gate ordering in the composite action is wrong — fix it before proceeding, this is the exact false positive the whole design exists to eliminate.

- [ ] **Step 3: Commit**

```bash
git add .github/actions/job-timing-report/action.yml
git commit -m "feat(ci): add job-timing-report composite action"
```

---

## Task 3: Wire timing steps into `guards-fast`

**Files:**
- Modify: `.github/workflows/ci.yml` (job `guards-fast`, starts at the `  guards-fast:` line — locate via `grep -n "^  guards-fast:" .github/workflows/ci.yml` since line numbers shift as earlier tasks land)

- [ ] **Step 1: Add `job-timing-start` as the first step**

Find this exact block (unique via the `name: Guards (fast)` line):

```yaml
  guards-fast:
    name: Guards (fast)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    # Raised from 15: the added GTK/dbus apt-get install + pnpm install (for the
    # GUI honesty gate) can eat most of a 15-min budget on a cold self-hosted
    # runner before the guards themselves even start.
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v7
      - uses: actions/download-artifact@v8
```

Replace with:

```yaml
  guards-fast:
    name: Guards (fast)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    # Raised from 15: the added GTK/dbus apt-get install + pnpm install (for the
    # GUI honesty gate) can eat most of a 15-min budget on a cold self-hosted
    # runner before the guards themselves even start.
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/job-timing-start
      - uses: actions/download-artifact@v8
```

- [ ] **Step 2: Add `job-timing-report` as the last step**

Find this exact block (the job's final step, immediately before `  lints:`):

```yaml
      - name: Normative docs — @query is GET (no POST /api/query claims)
        if: needs.setup.outputs.docs_changed == 'true'
        shell: bash
        run: |
          set -euo pipefail
          if rg -n "POST /api/query" docs/src/reference docs/src/api docs/src/architecture docs/src/how-to; then
            echo "docs must not describe @query as POST /api/query"
            exit 1
          fi

  lints:
```

Replace with:

```yaml
      - name: Normative docs — @query is GET (no POST /api/query claims)
        if: needs.setup.outputs.docs_changed == 'true'
        shell: bash
        run: |
          set -euo pipefail
          if rg -n "POST /api/query" docs/src/reference docs/src/api docs/src/architecture docs/src/how-to; then
            echo "docs must not describe @query as POST /api/query"
            exit 1
          fi

      - if: always()
        uses: ./.github/actions/job-timing-report
        with:
          budget-minutes: "20"
          job-status: ${{ job.status }}

  lints:
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "feat(ci): wire job-timing steps into guards-fast job"
```

---

## Task 4: Wire timing steps into `lints`

**Files:**
- Modify: `.github/workflows/ci.yml` (job `lints`)

- [ ] **Step 1: Add `job-timing-start` as the first step**

Find this exact block (unique via the `name: Lints (...)` line, which only appears once in the file):

```yaml
  lints:
    name: Lints (clippy + rustdoc + TOESTUB-scoped + hakari + grammar)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v7
      - uses: actions/download-artifact@v8
```

Replace with:

```yaml
  lints:
    name: Lints (clippy + rustdoc + TOESTUB-scoped + hakari + grammar)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/job-timing-start
      - uses: actions/download-artifact@v8
```

- [ ] **Step 2: Add `job-timing-report` as the last step**

Find this job's final step (immediately before `  compiler-gates:`):

```yaml
      - name: Grammar export check (EBNF/GBNF/Lark/JSON-Schema non-empty)
        run: ./target/debug/vox --quiet ci grammar-export-check

  compiler-gates:
```

Replace with:

```yaml
      - name: Grammar export check (EBNF/GBNF/Lark/JSON-Schema non-empty)
        run: ./target/debug/vox --quiet ci grammar-export-check

      - if: always()
        uses: ./.github/actions/job-timing-report
        with:
          budget-minutes: "20"
          job-status: ${{ job.status }}

  compiler-gates:
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "feat(ci): wire job-timing steps into lints job"
```

---

## Task 5: Wire timing steps into `compiler-gates`

**Files:**
- Modify: `.github/workflows/ci.yml` (job `compiler-gates`)

- [ ] **Step 1: Add `job-timing-start` as the first step**

Find this exact block (unique via the `name: Compiler gates (...)` line):

```yaml
  compiler-gates:
    name: Compiler gates (WebIR + projection + golden examples + benches)
    needs: setup
    if: ${{ needs.setup.outputs.full == 'true' || needs.setup.outputs.affects_compiler == 'true' }}
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v7
      - uses: actions/download-artifact@v8
```

Replace with:

```yaml
  compiler-gates:
    name: Compiler gates (WebIR + projection + golden examples + benches)
    needs: setup
    if: ${{ needs.setup.outputs.full == 'true' || needs.setup.outputs.affects_compiler == 'true' }}
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/job-timing-start
      - uses: actions/download-artifact@v8
```

- [ ] **Step 2: Add `job-timing-report` as the last step**

Find this job's final step (immediately before `  tests:`):

```yaml
      - name: Golden examples fmt idempotency gate (A.12)
        shell: bash
        run: |
          set -euo pipefail
          find examples/golden -name "*.vox" -type f -print0 | while IFS= read -r -d '' f; do
            ./target/debug/vox --quiet fmt "$f"
          done
          if ! git diff --exit-code examples/golden/; then
            echo "vox fmt is not idempotent or examples have unformatted changes."
            exit 1
          fi

  tests:
```

Replace with:

```yaml
      - name: Golden examples fmt idempotency gate (A.12)
        shell: bash
        run: |
          set -euo pipefail
          find examples/golden -name "*.vox" -type f -print0 | while IFS= read -r -d '' f; do
            ./target/debug/vox --quiet fmt "$f"
          done
          if ! git diff --exit-code examples/golden/; then
            echo "vox fmt is not idempotent or examples have unformatted changes."
            exit 1
          fi

      - if: always()
        uses: ./.github/actions/job-timing-report
        with:
          budget-minutes: "20"
          job-status: ${{ job.status }}

  tests:
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "feat(ci): wire job-timing steps into compiler-gates job"
```

---

## Task 6: Wire timing steps into `tests`

**Files:**
- Modify: `.github/workflows/ci.yml` (job `tests`)

- [ ] **Step 1: Add `job-timing-start` as the first step**

Find this exact block (unique via the `name: Tests (...)` line):

```yaml
  tests:
    name: Tests (nextest + llvm-cov + doctests)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v7
      - uses: actions/download-artifact@v8
```

Replace with:

```yaml
  tests:
    name: Tests (nextest + llvm-cov + doctests)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v7
      - uses: ./.github/actions/job-timing-start
      - uses: actions/download-artifact@v8
```

- [ ] **Step 2: Add `job-timing-report` as the last step**

Find this job's final step (immediately before `  audits:`):

```yaml
      - name: Upload LLVM coverage artifacts
        if: (success() || failure()) && needs.setup.outputs.full == 'true' && needs.setup.outputs.rust_changed == 'true'
        uses: actions/upload-artifact@v7
        with:
          name: llvm-cov
          path: |
            target/llvm-cov-summary.json
            target/llvm-cov-lcov.info

  audits:
```

Replace with:

```yaml
      - name: Upload LLVM coverage artifacts
        if: (success() || failure()) && needs.setup.outputs.full == 'true' && needs.setup.outputs.rust_changed == 'true'
        uses: actions/upload-artifact@v7
        with:
          name: llvm-cov
          path: |
            target/llvm-cov-summary.json
            target/llvm-cov-lcov.info

      - if: always()
        uses: ./.github/actions/job-timing-report
        with:
          budget-minutes: "30"
          job-status: ${{ job.status }}

  audits:
```

Note: this job's budget is `"30"` (its `timeout-minutes: 30`), not `"20"` like the other four — double-check against the job's own `timeout-minutes:` line before committing, don't copy-paste `"20"` here.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "feat(ci): wire job-timing steps into tests job"
```

---

## Task 7: Wire timing steps into `audits`

**Files:**
- Modify: `.github/workflows/ci.yml` (job `audits`)

- [ ] **Step 1: Add `job-timing-start` as the first step**

At this point in the plan, `audits` still has its original `name:` and `timeout-minutes: 35` (Task 10, later, is what changes those). Find this exact block:

```yaml
  audits:
    name: Audits (TOESTUB-full + build-timings + feature-matrix + completion + mens-gate)
    needs: setup
    runs-on: [self-hosted, linux, x64]
```

This block is followed by the multi-line explanatory comment about the 35-minute timeout (from the earlier commit `b490f8570b`) and then `timeout-minutes: 35` / `steps:` / `- uses: actions/checkout@v7` / `- uses: actions/download-artifact@v8`. Insert the new step right after checkout:

```yaml
      - uses: actions/checkout@v7
      - uses: ./.github/actions/job-timing-start
      - uses: actions/download-artifact@v8
```

- [ ] **Step 2: Add `job-timing-report` as the last step**

Find this job's final step:

```yaml
      - name: Populi CI gate matrix (single manifest)
        if: needs.setup.outputs.full == 'true'
        run: ./target/debug/vox --quiet ci mens-gate --profile ci_full
```

Replace with:

```yaml
      - name: Populi CI gate matrix (single manifest)
        if: needs.setup.outputs.full == 'true'
        run: ./target/debug/vox --quiet ci mens-gate --profile ci_full

      - if: always()
        uses: ./.github/actions/job-timing-report
        with:
          budget-minutes: "35"
          job-status: ${{ job.status }}
```

Use `"35"` here (this job's *current* `timeout-minutes` value, before Task 10 lowers it to 20 later in this plan) — Task 10 updates both the `timeout-minutes:` line and this `budget-minutes:` value together, so they never drift out of sync even mid-plan.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "feat(ci): wire job-timing steps into audits job"
```

---

## Task 8: Extract the all-features-check logic to `scripts/ci/all_features_check.vox`

**Files:**
- Create: `scripts/ci/all_features_check.vox`

- [ ] **Step 1: Write the script**

```vox
// vox:caps fs env process
// scripts/ci/all_features_check.vox
//
// Workspace-wide `cargo check --all-features` compile check. Moved off the
// PR-blocking Audits job to the nightly `feature-audits-nightly.yml`
// workflow — this check's own real cost is ~10+ minutes and it catches
// latent-bug-class issues (features nobody enables together in normal
// development), not immediate PR-authored regressions. See
// docs/superpowers/specs/2026-07-07-ci-timeout-diagnostics-and-audits-split-design.md.
//
// `--all-features` unconditionally enables every workspace crate's `cuda`
// feature (candle-core/cuda etc.), which requires nvcc, and every crate's
// `metal` feature, which requires macOS. Neither is available on this
// self-hosted Linux fleet, so both are excluded below. vox-codegen's
// `standalone` feature is also excluded — see its own Cargo.toml comment:
// it exists only so `#[cfg(feature = "standalone")]` guards in its
// `#[path]`-embedded vox-codegen-ts/src/mod.rs are known cfgs, and
// force-enabling it via --all-features breaks the embedded (default) build.
//
// Deliberately NOT carried over from the original ci.yml bash: the
// FULL-vs-affected-crates-only (`P_ARGS`) branch. That branch existed so a
// small PR touching only a few crates wouldn't pay for a full-workspace
// compile. This script only runs nightly now (never per-PR), and a nightly
// cron has no PR diff at all — there is no "small PR" case here, so this
// always does the full workspace check unconditionally. Not a lost feature.

fn nvcc_available() to bool {
    let res_opt = process.run("nvcc", ["--version"]);
    if res_opt is null {
        return false;
    }
    return res_opt.unwrap().code == 0;
}

fn is_darwin() to bool {
    let res_opt = process.run("uname", ["-s"]);
    if res_opt is null {
        return false;
    }
    let res = res_opt.unwrap();
    return res.code == 0 && res.stdout.trim() == "Darwin";
}

fn main() {
    let mut args = ["check", "--workspace", "--exclude", "vox-gui", "--exclude", "vox-codegen"];

    // Crates with their own `cuda` feature: vox-plugin-mens-candle-cuda,
    // vox-quantize, vox-plugin-speech, vox-populi (mens-candle-qlora-cuda),
    // vox-speech. vox-ml-cli's `mens-candle-cuda` feature forwards into
    // vox-populi's cuda feature via `vox-populi/mens-candle-qlora-cuda`, so
    // it needs excluding too or --all-features re-enables it as a dependency.
    if not nvcc_available() {
        args = args.push("--exclude");
        args = args.push("vox-plugin-mens-candle-cuda");
        args = args.push("--exclude");
        args = args.push("vox-quantize");
        args = args.push("--exclude");
        args = args.push("vox-plugin-speech");
        args = args.push("--exclude");
        args = args.push("vox-populi");
        args = args.push("--exclude");
        args = args.push("vox-speech");
        args = args.push("--exclude");
        args = args.push("vox-ml-cli");
    }

    // Crates with their own `metal` feature: vox-plugin-mens-candle-metal,
    // vox-quantize. (vox-quantize may already be excluded above by the cuda
    // branch — cargo tolerates the same --exclude package appearing twice.)
    if not is_darwin() {
        args = args.push("--exclude");
        args = args.push("vox-plugin-mens-candle-metal");
        args = args.push("--exclude");
        args = args.push("vox-quantize");
    }

    args = args.push("--all-features");

    print("Running: cargo " + args.join(" "));
    let res_opt = process.run("cargo", args);
    if res_opt is null {
        log.error("all_features_check: could not spawn cargo");
        process.exit(1);
    }
    let res = res_opt.unwrap();
    process.exit(res.code);
}
```

- [ ] **Step 2: Verify the script runs and produces the expected excluded-crate list**

Redirect to a file rather than piping through `head` — piping (`cmd | head -5`) would make a later `$?` check report `head`'s exit code, not the actual command's, silently defeating the exit-code check in the next step:

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && ./target/debug/vox --quiet run --interp scripts/ci/all_features_check.vox > /tmp/all_features_check_verify.log 2>&1; echo "exit code: $?"`

This is the full `--all-features` workspace check (several minutes even with a warm sccache; this exact argument list has already been verified to pass cleanly earlier in this session via manual `cargo tree -i cudarc`/`-i objc2` checks showing neither package resolves) — let it run to completion in the foreground, or via `run_in_background: true` if the executor supports it, since it's a multi-minute command.

Expected: `exit code: 0`

Then confirm the constructed argument list matches expectations:

Run: `head -1 /tmp/all_features_check_verify.log`

Expected (this machine has no `nvcc` and is not Darwin, so both exclude branches fire):
```
Running: cargo check --workspace --exclude vox-gui --exclude vox-codegen --exclude vox-plugin-mens-candle-cuda --exclude vox-quantize --exclude vox-plugin-speech --exclude vox-populi --exclude vox-speech --exclude vox-ml-cli --exclude vox-plugin-mens-candle-metal --exclude vox-quantize --all-features
```

- [ ] **Step 3: Commit**

```bash
git add scripts/ci/all_features_check.vox
git commit -m "feat(ci): extract all-features-check logic to scripts/ci/all_features_check.vox"
```

---

## Task 9: Create the nightly workflow

**Files:**
- Create: `.github/workflows/feature-audits-nightly.yml`

- [ ] **Step 1: Write the workflow**

```yaml
# Nightly feature-combination audits: the vox-cli feature matrix and the
# workspace-wide `--all-features` compile check. Moved off the PR-blocking
# Audits job (see docs/superpowers/specs/2026-07-07-ci-timeout-diagnostics-and-audits-split-design.md)
# because these two steps alone accounted for ~19 of the ~25-35 minutes that
# job was observed taking, and they catch latent-bug-class issues (features
# nobody enables together in normal development) rather than immediate
# PR-authored regressions — a 24h feedback loop is an acceptable trade for
# not blocking every PR on ~19 extra minutes.
name: Feature Audits (nightly)

on:
  workflow_dispatch:
  schedule:
    # 47 4 * * * UTC — offset from the other three nightlies (03:17, 05:17,
    # and qwen35-native-nightly's own schedule) to avoid runner contention.
    - cron: "47 4 * * *"

env:
  CARGO_TERM_COLOR: always
  # Self-hosted runner image sets RUSTC_WRAPPER; repeat here for non-container jobs.
  RUSTC_WRAPPER: sccache
  SCCACHE_DIR: /cache/sccache
  # W1: sccache caches nothing under incremental compilation (cargo's dev/test
  # default) — without this pinned, sccache 0%-hits. MUST be the quoted string
  # "0" (not the bare number 0): crates/vox-cli/src/commands/ci/sccache_workflow_guard.rs's
  # `all_sccache_workflows_pin_incremental` test scans every file in
  # .github/workflows/ for the exact literal `\n  CARGO_INCREMENTAL: "0"` and
  # fails the whole workspace test suite if a workflow turns sccache on at the
  # top-level env without it — this is not a style nicety, it's a mechanically
  # enforced requirement.
  CARGO_INCREMENTAL: "0"

jobs:
  feature-audits:
    name: feature-matrix + workspace all-features check
    runs-on: [self-hosted, linux, x64]
    timeout-minutes: 40
    steps:
      - uses: actions/checkout@v7

      - name: Install system deps (dbus/GTK for wry/keyring/soup on Linux)
        run: sudo apt-get update -y && sudo apt-get install -y libdbus-1-dev pkg-config libglib2.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-feature-audits-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-feature-audits-
            ${{ runner.os }}-cargo-

      - name: Build vox-cli
        run: cargo build -p vox-cli --locked --features completion-toestub,extras-ludus,ars,coderabbit

      - name: vox-cli feature matrix
        run: ./target/debug/vox --quiet ci feature-matrix

      - name: Workspace-wide all-features check
        run: ./target/debug/vox --quiet run --interp scripts/ci/all_features_check.vox
```

- [ ] **Step 2: Verify the workflow YAML is well-formed**

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && python -c "import yaml; yaml.safe_load(open('.github/workflows/feature-audits-nightly.yml')); print('OK')"`

Expected: `OK`

- [ ] **Step 3: Run the real sccache-pinning guard test against this new file**

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && cargo test -p vox-cli --lib sccache_workflow_guard:: 2>&1 | tail -20`

Expected: all tests pass, including `all_sccache_workflows_pin_incremental` (this test scans every file in `.github/workflows/`, so it will now include the new `feature-audits-nightly.yml` — if `CARGO_INCREMENTAL: "0"` isn't exactly right in Step 1, this is what catches it before push instead of a real CI failure downstream).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/feature-audits-nightly.yml
git commit -m "feat(ci): add nightly feature-matrix + all-features-check workflow"
```

---

## Task 10: Trim the `audits` job in `ci.yml` and lower its timeout

**Files:**
- Modify: `.github/workflows/ci.yml` (job `audits`)

- [ ] **Step 1: Remove the `feature-matrix` and `Workspace-wide all-features check (A.4)` steps**

Find this exact block:

```yaml
      - name: vox-cli feature matrix + no vox_orchestrator imports
        run: ./target/debug/vox --quiet ci feature-matrix && ./target/debug/vox --quiet ci no-vox-orchestrator-import

      - name: Workspace-wide all-features check (A.4)
        shell: bash
        run: |
          set -euo pipefail
          FULL="${{ needs.setup.outputs.full }}"
          P_ARGS="${{ needs.setup.outputs.affected_p_args }}"
          # `--all-features` unconditionally enables every workspace crate's
          # `cuda` feature (candle-core/cuda etc.), which requires nvcc — this
          # step had no guard for that, so it always failed on fleets without
          # the CUDA toolkit installed. `command -v nvcc` alone isn't reliable
          # here — some fleet runners have a stale/broken PATH entry that
          # resolves but fails when actually invoked. Run the same check
          # cudarc's build.rs does. Crates with their own `cuda` feature:
          # vox-plugin-mens-candle-cuda, vox-quantize, vox-plugin-speech,
          # vox-populi (mens-candle-qlora-cuda), vox-speech. vox-ml-cli's
          # `mens-candle-cuda` feature forwards into vox-populi's cuda
          # feature via `vox-populi/mens-candle-qlora-cuda`, so it needs
          # excluding too or --all-features re-enables it as a dependency.
          CUDA_EXCLUDE=()
          if ! nvcc --version >/dev/null 2>&1; then
            CUDA_EXCLUDE=(--exclude vox-plugin-mens-candle-cuda --exclude vox-quantize --exclude vox-plugin-speech --exclude vox-populi --exclude vox-speech --exclude vox-ml-cli)
          fi
          # Same problem for the `metal` feature: it links Apple's Metal
          # framework via objc2/candle-core, which cannot compile off macOS at
          # all (no runtime-check fallback like nvcc). This job always runs on
          # self-hosted Linux, so always exclude the crates with their own
          # `metal` feature: vox-plugin-mens-candle-metal, vox-quantize.
          METAL_EXCLUDE=()
          if [ "$(uname -s)" != "Darwin" ]; then
            METAL_EXCLUDE=(--exclude vox-plugin-mens-candle-metal --exclude vox-quantize)
          fi
          # vox-codegen declares a `standalone` feature that is intentionally
          # never enabled (see its Cargo.toml comment): it's only there so the
          # `#[cfg(feature = "standalone")]` guards in the `#[path]`-embedded
          # vox-codegen-ts/src/mod.rs are known cfgs. --all-features force-
          # enables it anyway, which makes the embedded copy take the
          # standalone branch (`use vox_codegen::...`) while compiled inside
          # vox-codegen itself, an unresolved-import error. It has no other
          # non-default feature, so excluding it loses no real coverage.
          if [ "$FULL" = "true" ]; then
            cargo check --workspace --exclude vox-gui --exclude vox-codegen "${CUDA_EXCLUDE[@]}" "${METAL_EXCLUDE[@]}" --all-features
          elif [ -n "$P_ARGS" ]; then
            # shellcheck disable=SC2086
            cargo check $P_ARGS --all-features
          else
            echo "No affected crates — skipping all-features check."
          fi

      - name: Optional CUDA feature compile (Oratio + Populi Candle, when nvcc exists)
```

Replace with:

```yaml
      - name: no vox_orchestrator imports
        run: ./target/debug/vox --quiet ci no-vox-orchestrator-import

      - name: Optional CUDA feature compile (Oratio + Populi Candle, when nvcc exists)
```

(The `feature-matrix` and `all-features check` invocations are deleted — both now run nightly via `feature-audits-nightly.yml`. `no-vox-orchestrator-import` stays on the PR path since it's a fast, unrelated guard that happened to be chained onto the same step by `&&` — it gets its own step now instead of losing its per-PR coverage as a side effect of this split.)

- [ ] **Step 2: Lower `timeout-minutes` from 35 to 20, and update the comment**

Find:

```yaml
    name: Audits (TOESTUB-full + build-timings + feature-matrix + completion + mens-gate)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    # This job's true steady-state runtime (TOESTUB validation, safety-inventory,
    # hardcoded-values-audit, completion-audit, detect-rules-bench, build-timings,
    # feature-matrix cargo checks, the all-features check, cuda-features, and the
    # corpus_prep + mens-mix step) was never actually observed before — every run
    # on this branch failed on an earlier, now-fixed bug first. Once those were
    # fixed, this job started reaching the 25-minute ceiling under normal
    # self-hosted-runner contention (repeatedly landing at ~25m15s, i.e. cut off
    # mid-step by the timeout, not a real hang). 35m matches the headroom given to
    # other substantial jobs in this workflow (30/40m).
    timeout-minutes: 35
```

Replace with:

```yaml
    name: Audits (TOESTUB-full + build-timings + completion + mens-gate)
    needs: setup
    runs-on: [self-hosted, linux, x64]
    # feature-matrix and the workspace all-features check moved to the nightly
    # feature-audits-nightly.yml workflow (see
    # docs/superpowers/specs/2026-07-07-ci-timeout-diagnostics-and-audits-split-design.md)
    # — they alone accounted for ~19 of the ~25-35 minutes this job was
    # observed taking. Remaining steps' summed observed time is ~15 minutes,
    # plus two previously-unmeasured tail steps (corpus_prep/mens-mix, Populi
    # gate matrix) whose cost this job had never actually reached before all
    # of the above was fixed. 20m budgets that ~15m plus headroom for the two
    # unknowns and normal runner contention; the job-timing-report step
    # (below) will show on the first several real runs whether this needs
    # one more data-driven adjustment.
    timeout-minutes: 20
```

- [ ] **Step 3: Update the `job-timing-report` call (added in Task 7) to match the new budget**

Task 7 added this job's `job-timing-report` step using `budget-minutes: "35"` (the value at the time). Now that Step 2 above changed `timeout-minutes` to `20`, this must change too or the timing report will compare elapsed time against the wrong (stale) budget. Find:

```yaml
      - if: always()
        uses: ./.github/actions/job-timing-report
        with:
          budget-minutes: "35"
          job-status: ${{ job.status }}
```

Replace with:

```yaml
      - if: always()
        uses: ./.github/actions/job-timing-report
        with:
          budget-minutes: "20"
          job-status: ${{ job.status }}
```

- [ ] **Step 4: Verify the resulting YAML is well-formed and the job's step list makes sense**

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && python -c "import yaml; d = yaml.safe_load(open('.github/workflows/ci.yml')); j = d['jobs']['audits']; print('OK, audits timeout-minutes =', j['timeout-minutes']); print('OK, job-timing-report budget-minutes =', j['steps'][-1]['with']['budget-minutes'])"`

Expected:
```
OK, audits timeout-minutes = 20
OK, job-timing-report budget-minutes = 20
```

If the two numbers don't match, Step 3 above wasn't applied — this is exactly the drift this step exists to catch.

Then confirm no leftover reference to the removed steps:

Run: `grep -n "feature-matrix\|Workspace-wide all-features check" .github/workflows/ci.yml`

Expected: no matches inside the `audits:` job block (the string `feature-matrix` may still legitimately appear in the job's `name:` line if Step 2 above wasn't applied correctly — re-check that the `name:` was updated to drop `+ feature-matrix` too).

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "fix(ci): trim audits job (feature-matrix + all-features-check moved to nightly), lower timeout 35m -> 20m"
```

---

## Task 11: Full-file sanity pass

**Files:**
- Read-only verification of: `.github/workflows/ci.yml`, `.github/workflows/feature-audits-nightly.yml`, `.github/actions/job-timing-start/action.yml`, `.github/actions/job-timing-report/action.yml`, `scripts/ci/all_features_check.vox`

- [ ] **Step 1: Confirm all 5 target jobs have exactly one `job-timing-start`, one `job-timing-report`, and one `job-status` input**

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && grep -c "uses: ./.github/actions/job-timing-start" .github/workflows/ci.yml && grep -c "uses: ./.github/actions/job-timing-report" .github/workflows/ci.yml && grep -c "job-status: \${{ job.status }}" .github/workflows/ci.yml`

Expected: all three commands print `5`. If `job-timing-report`'s count is `5` but `job-status`'s count is less, one of the 5 call sites is missing the `job-status` input (it would silently fall through to the `required: true` validation failing at runtime, not at commit time — catch it here instead).

- [ ] **Step 2: Confirm each `job-timing-report`'s `budget-minutes` matches its job's `timeout-minutes`**

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && python -c "
import yaml
d = yaml.safe_load(open('.github/workflows/ci.yml'))
for name in ['guards-fast', 'lints', 'compiler-gates', 'tests', 'audits']:
    job = d['jobs'][name]
    tm = job['timeout-minutes']
    report_step = [s for s in job['steps'] if s.get('uses') == './.github/actions/job-timing-report'][0]
    bm = report_step['with']['budget-minutes']
    match = 'OK' if str(tm) == str(bm) else 'MISMATCH'
    print(f'{name}: timeout-minutes={tm} budget-minutes={bm} {match}')
"`

Expected: every line prints `OK` (guards-fast=20, lints=20, compiler-gates=20, tests=30, audits=20 after Task 10). Fix any `MISMATCH` inline before proceeding — this is a real bug (the report would compare elapsed time against the wrong budget), not a style nit.

- [ ] **Step 3: Run the repo's existing workflow-lint tooling locally if available**

Run: `cd "C:/Users/Owner/AppData/Local/Temp/vox-guihonesty-fix" && (command -v actionlint >/dev/null 2>&1 && actionlint .github/workflows/ci.yml .github/workflows/feature-audits-nightly.yml) || echo "actionlint not installed locally — the repo's own Workflow Lint CI job (.github/workflows/workflow-lint.yml) will run actionlint + zizmor automatically on this push, per its existing on: pull_request paths filter for .github/workflows/**"`

Expected: either real actionlint output (fix anything it flags that isn't a pre-existing repo-wide advisory finding — cross-check against `docs/src/ci` or the workflow-lint.yml comments for what's already known/accepted), or the fallback message confirming CI will check it remotely.

- [ ] **Step 4: Push and watch the PR's own CI run for real confirmation**

This plan's tasks are all local/mechanical; the real end-to-end validation is the next actual PR run, where the `job-timing-report` annotations should now appear in the Actions UI for `guards-fast`/`lints`/`compiler-gates`/`tests`/`audits`, and the `audits` job (now missing its two heaviest steps) should complete well inside its new 20-minute budget.
