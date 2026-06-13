# CI Anti-Stacking & Caching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Tasks within a phase marked **[PARALLEL-SAFE]** touch disjoint files and may be dispatched concurrently; tasks marked **[SERIAL]** must land in order.

**Goal:** Stop CI/CD runs from stacking up on the 4-runner self-hosted fleet, and make the required gate fast and cache-effective, so a burst of pushes drains in minutes instead of hours.

**Architecture:** Three levers, in priority order: (1) **cut demand** — move non-essential workflows off the per-PR / per-push hot path to nightly / post-merge / path-filtered tiers, shrinking per-PR fan-out from 16 workflows to ~6 core gates; (2) **bound contention** — a shared concurrency budget for non-required self-hosted workflows plus enforced merge-queue serialization of `main`; (3) **make the required gate cheap** — build `vox-cli` once and share it as an artifact across the 5 required jobs instead of 5× rebuild, and harden caching (sccache + restore-keys). Raw runner count is a weak lever (host is already at 24/32 threads), so demand reduction dominates.

**Tech Stack:** GitHub Actions YAML, self-hosted runner autoscaler (`crates/vox-cli/src/commands/ci/runner_scale.rs`), sccache (baked into `infra/ci-runner/Dockerfile`), `actions/cache@v5`, `actions/upload-artifact`/`download-artifact`.

---

## Verified Baseline (from investigation 2026-06-13)

**Runner fleet:** `MAX_RUNNERS = 4` (`crates/vox-cli/src/commands/ci/runner_scale.rs:37`), 6 CPU + 6.5 GB each, ephemeral (one job per runner), 30-min idle reap (`:41`). Image bakes Rust 1.95.0 + sccache (`SCCACHE_DIR=/cache/sccache`, `RUSTC_WRAPPER=sccache`) on a shared `vox-ci-runner-cache` volume (`infra/ci-runner/Dockerfile:35,57,60`).

**Per-PR fan-out (16 workflows):** ci, ci-fallback-hosted, codeql, compile-matrix, cr-l-gates, cr-l8-corpus-feedback, cross-platform-check, docs-quality, gitleaks, link_checker, mutation-pr, ssot-drift, ts-emit-noemit, vox-mental-tracker, vox-visus-audit, mobile-eas-build, mobile-e2e-android.

**`ci.yml` job expansion:** 14 base jobs + 24-entry `all-features-matrix` = **~38 jobs per run** (`ci.yml:1043-1070`). Required gate `ci-summary` (name `Check, Build, and Test (Rust)`) needs only `[guards-fast, lints, compiler-gates, tests, audits]` (`ci.yml:790`). Each of those 5 jobs independently runs `cargo build -p vox-cli` (`ci.yml:88,110,318,431,520,728`) — **no shared binary artifact**.

**Concurrency:** every PR/push workflow has `concurrency` + `cancel-in-progress: true`, grouped by `${{ github.workflow }}-${{ github.ref }}` (`ci.yml:12-14`). This dedupes repeated pushes to the *same* branch only — not across branches, not across workflows.

**Merge queue:** `merge_group` trigger is wired (`ci.yml:10`) but GitHub branch-protection "Require merge queue" enforcement is **unverified** — Task 0.2 confirms.

**Existing docs to update:** `docs/src/ci/runner-autoscaling.md`, `docs/src/ci/runner-contract.md`, `docs/src/ci/rcicd-coverage-cost-matrix-2026.md`, `docs/src/ci/github-hosted-exceptions.md`.

---

## File Structure

| File | Responsibility | Tasks |
|------|----------------|-------|
| `.github/workflows/codeql.yml` | drop PR trigger, keep push:main + weekly | 1.1 |
| `.github/workflows/gitleaks.yml` | drop PR, keep push:main + daily; add paths | 1.2 |
| `.github/workflows/link_checker.yml` | drop PR, keep push:main + nightly; markdown paths | 1.3 |
| `.github/workflows/cross-platform-check.yml` | drop PR, keep weekly schedule | 1.4 |
| `.github/workflows/setup-e2e.yml` | drop PR, keep push:main + nightly | 1.5 |
| `.github/workflows/ssot-drift.yml` | keep push:main; add manifest paths filter | 1.6 |
| `.github/workflows/vox-visus-audit.yml` | confirm advisory/non-blocking; move off PR | 1.7 |
| `.github/workflows/deploy-hetzner.yml` | add Rust paths filter | 2.1 |
| `.github/workflows/ml_data_extraction.yml` | narrow over-broad paths | 2.2 |
| `.github/workflows/ci.yml` | shared vox-cli artifact; concurrency; caching | 3.1, 4.1, 5.1, 5.2 |
| All non-required self-hosted PR workflows | shared concurrency budget header | 3.1 |
| `crates/vox-cli/src/commands/ci/runner_scale.rs` | optional capacity/demand tuning | 6.1 |
| `docs/src/ci/rcicd-coverage-cost-matrix-2026.md` | record new tiering | 7.2 |

---

## Phase 0 — Baseline & Safety (SERIAL — do first)

### Task 0.1: Capture the current fan-out baseline

**Files:**
- Create: `docs/src/ci/anti-stacking-baseline-2026-06-13.md`

- [ ] **Step 1: Measure current per-event fan-out**

Run (records how many workflows + jobs a single PR and a single push to main trigger today):

```bash
cd <repo>
# Count workflows that trigger on pull_request and on push:main
python - <<'PY'
import glob, re, os
pr=[]; push=[]
for f in sorted(glob.glob('.github/workflows/*.yml')):
    s=open(f,encoding='utf-8',errors='replace').read(); n=os.path.basename(f)
    if re.search(r'^\s*pull_request:',s,re.M): pr.append(n)
    if re.search(r'^\s*push:',s,re.M) and 'branches: [main]' in s.replace("'",'').replace('"',''): push.append(n)
print('PR workflows:',len(pr)); print('\n'.join('  '+x for x in pr))
print('push:main workflows:',len(push)); print('\n'.join('  '+x for x in push))
PY
```

Expected: ~16 PR, ~15 push (the numbers this plan reduces).

- [ ] **Step 2: Record the baseline doc with YAML frontmatter**

Create `docs/src/ci/anti-stacking-baseline-2026-06-13.md` starting with:

```markdown
---
title: CI Anti-Stacking Baseline (2026-06-13)
description: Pre-change fan-out and runner-contention baseline used to measure the anti-stacking work.
category: ci
---

# CI Anti-Stacking Baseline (2026-06-13)

- Self-hosted runners: 4 (MAX_RUNNERS, runner_scale.rs:37)
- Per-PR workflow fan-out: 16
- Per-push-to-main workflow fan-out: 15
- ci.yml jobs per run: ~38 (14 base + 24 all-features matrix)
- Required gate jobs each rebuild vox-cli (5× redundant build)
- Target after this plan: per-PR fan-out ≤ 8; required gate single shared vox-cli build
```

- [ ] **Step 3: Commit**

```bash
git add docs/src/ci/anti-stacking-baseline-2026-06-13.md
git commit -m "docs(ci): capture anti-stacking fan-out baseline (2026-06-13)"
```

### Task 0.2: Verify and (if needed) enable merge-queue serialization on `main`

**Files:** none (GitHub config + report)

- [ ] **Step 1: Check whether "Require merge queue" is enabled**

Run:

```bash
gh api repos/vox-foundation/vox/branches/main/protection --jq '{required_status_checks: .required_status_checks.contexts, merge_queue: .required_merge_queue}' 2>&1 || \
gh api repos/vox-foundation/vox/rulesets --jq '.[] | {name, target, enforcement}' 2>&1
```

Expected: shows whether a merge-queue ruleset exists. `merge_group` is already wired in `ci.yml:10`.

- [ ] **Step 2: If not enabled, flag for the human (do NOT change branch protection autonomously)**

Merge queue serializes `main` so each batch is verified once — this is the single biggest cure for cancel-thrash from rapid direct pushes. Enabling it is a repo-admin action. If disabled, record in the baseline doc:

```markdown
## ACTION REQUIRED (repo admin)
Enable GitHub merge queue on `main` (Settings → Branches → main → Require merge queue),
required check: "Check, Build, and Test (Rust)". The ci.yml merge_group trigger is already wired.
```

- [ ] **Step 3: Commit the note**

```bash
git add docs/src/ci/anti-stacking-baseline-2026-06-13.md
git commit -m "docs(ci): record merge-queue enforcement status + admin action"
```

---

## Phase 1 — Cut Per-PR Fan-Out (the biggest lever)

> Each task removes a non-essential workflow from the per-PR hot path while preserving coverage via post-merge (`push: main`) and/or a scheduled nightly. **[PARALLEL-SAFE]** — every task edits a distinct file.

### Task 1.1: Tier CodeQL off PRs [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/codeql.yml` (the `on:` block)

- [ ] **Step 1: Read current trigger**

Run: `sed -n '1,30p' .github/workflows/codeql.yml`
Expected: `on: { pull_request: branches:[main], push: branches:[main], schedule: <weekly> }`

- [ ] **Step 2: Remove the `pull_request:` trigger**

Edit the `on:` block to keep only `push: branches: [main]`, the weekly `schedule:`, and `workflow_dispatch:`. Delete the `pull_request:` key and its sub-lines. Result:

```yaml
on:
  push:
    branches: [main]
  schedule:
    - cron: '23 4 * * 1'   # weekly, keep existing cron value
  workflow_dispatch:
```

Rationale: weekly scan + every-merge scan covers regressions; per-PR CodeQL on a hosted runner is redundant security noise.

- [ ] **Step 3: Validate YAML**

Run: `python -c "import yaml,sys; yaml.safe_load(open('.github/workflows/codeql.yml')); print('ok')"`
Expected: `ok`

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/codeql.yml
git commit -m "ci(codeql): drop per-PR scan; keep push:main + weekly schedule"
```

### Task 1.2: Tier gitleaks off PRs + add paths [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/gitleaks.yml`

- [ ] **Step 1: Read current trigger** — `sed -n '1,30p' .github/workflows/gitleaks.yml` (expected PR + push:main + daily schedule, no paths).

- [ ] **Step 2: Remove `pull_request:`; keep push:main + daily schedule**

```yaml
on:
  push:
    branches: [main]
  schedule:
    - cron: '0 5 * * *'   # daily, keep existing value
  workflow_dispatch:
```

Rationale: the daily scan + every-merge scan covers the PR window; secrets that land are caught at merge, not buried under per-PR noise.

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/gitleaks.yml')); print('ok')"
git add .github/workflows/gitleaks.yml
git commit -m "ci(gitleaks): drop per-PR scan; keep push:main + daily schedule"
```

### Task 1.3: Tier link_checker to nightly + markdown paths [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/link_checker.yml`

- [ ] **Step 1: Read** — `sed -n '1,30p' .github/workflows/link_checker.yml` (PR + push:main, no paths).

- [ ] **Step 2: Drop PR, keep push:main path-filtered to markdown, add nightly**

```yaml
on:
  push:
    branches: [main]
    paths:
      - '**/*.md'
      - '.lycheeignore'
      - '.github/workflows/link_checker.yml'
  schedule:
    - cron: '0 6 * * *'   # nightly full link sweep
  workflow_dispatch:
```

Rationale: external link checks are flaky and non-blocking by nature; nightly + markdown-only-on-merge is sufficient.

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/link_checker.yml')); print('ok')"
git add .github/workflows/link_checker.yml
git commit -m "ci(links): drop per-PR; markdown-only push:main + nightly sweep"
```

### Task 1.4: Tier cross-platform-check to weekly-only [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/cross-platform-check.yml`

- [ ] **Step 1: Read** — `sed -n '1,30p' .github/workflows/cross-platform-check.yml` (PR path-filtered + weekly schedule; runs windows-latest + macos-latest).

- [ ] **Step 2: Remove `pull_request:`; keep the existing weekly `schedule:` + `workflow_dispatch:`**

```yaml
on:
  schedule:
    - cron: '0 7 * * 1'   # weekly, keep existing value
  workflow_dispatch:
```

Rationale: Windows/macOS hosted runners are slow and expensive; weekly coverage catches platform drift. Per-PR cross-platform is not a required gate.

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/cross-platform-check.yml')); print('ok')"
git add .github/workflows/cross-platform-check.yml
git commit -m "ci(cross-platform): weekly-only; drop per-PR matrix"
```

### Task 1.5: Tier setup-e2e off PRs [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/setup-e2e.yml`

- [ ] **Step 1: Read** — `sed -n '1,30p' .github/workflows/setup-e2e.yml` (push:main + PR + nightly).

- [ ] **Step 2: Remove `pull_request:`; keep push:main + nightly schedule + dispatch.**

```yaml
on:
  push:
    branches: [main]
  schedule:
    - cron: '0 8 * * *'   # nightly, keep existing value
  workflow_dispatch:
```

Rationale: clean-room setup verification rarely regresses per-PR; post-merge + nightly is sufficient.

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/setup-e2e.yml')); print('ok')"
git add .github/workflows/setup-e2e.yml
git commit -m "ci(setup-e2e): drop per-PR; keep push:main + nightly"
```

### Task 1.6: Path-filter ssot-drift [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/ssot-drift.yml`

- [ ] **Step 1: Read** — `sed -n '1,30p' .github/workflows/ssot-drift.yml` (push:main + PR, NO paths; self-hosted).

- [ ] **Step 2: Keep PR + push:main but add a manifest/contract paths filter so it does not run on doc-only or code-only-no-manifest changes**

```yaml
on:
  push:
    branches: [main]
    paths: &ssot_paths
      - 'crates/*/Cargo.toml'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - 'contracts/**'
      - 'catalog.v1.yaml'
      - '.github/workflows/ssot-drift.yml'
  pull_request:
    paths: *ssot_paths
  workflow_dispatch:
```

Rationale: SSOT drift only matters when manifests/contracts/catalog change. Keeps it a gate but stops it consuming a self-hosted slot on unrelated PRs.

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/ssot-drift.yml')); print('ok')"
git add .github/workflows/ssot-drift.yml
git commit -m "ci(ssot-drift): path-filter to manifest/contract changes"
```

### Task 1.7: Make vox-visus-audit non-blocking + off PR [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/vox-visus-audit.yml`

- [ ] **Step 1: Read** — `sed -n '1,40p' .github/workflows/vox-visus-audit.yml` (PR path-filtered, windows-latest, already advisory/continue-on-error).

- [ ] **Step 2: Move to push:main path-filtered + dispatch (drop PR), confirm `continue-on-error: true` remains on the job**

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'crates/vox-browser/**'
      - 'crates/vox-compiler/**'
      - 'crates/vox-cli/**'
      - 'docs/src/**'
  workflow_dispatch:
```

Rationale: it is advisory-only and env-blocked; a Windows runner per PR is pure cost. Keep it running post-merge for trend signal.

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/vox-visus-audit.yml')); print('ok')"
git add .github/workflows/vox-visus-audit.yml
git commit -m "ci(visus): advisory post-merge only; drop per-PR Windows run"
```

---

## Phase 2 — Path-Filter Heavy Workflows (SERIAL after Phase 1; PARALLEL-SAFE within)

### Task 2.1: Path-filter deploy-hetzner [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/deploy-hetzner.yml`

- [ ] **Step 1: Read** — `sed -n '1,30p' .github/workflows/deploy-hetzner.yml` (push:main, NO paths; builds vox-cli in smoke).

- [ ] **Step 2: Add a paths filter so doc-only merges don't trigger a deploy build**

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'crates/**'
      - 'Cargo.lock'
      - 'Dockerfile*'
      - '.github/workflows/deploy-hetzner.yml'
  workflow_dispatch:
```

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/deploy-hetzner.yml')); print('ok')"
git add .github/workflows/deploy-hetzner.yml
git commit -m "ci(deploy): path-filter Hetzner deploy to code changes"
```

### Task 2.2: Narrow ml_data_extraction paths [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/ml_data_extraction.yml`

- [ ] **Step 1: Read** — `sed -n '1,30p' .github/workflows/ml_data_extraction.yml` (push:main, very broad paths incl `**/*.vox` and all `docs/src/**`; self-hosted GPU).

- [ ] **Step 2: Narrow to the corpus/compiler inputs that actually change training data**

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'examples/golden/**'
      - 'crates/vox-compiler/**'
      - 'crates/vox-cli/src/commands/corpus/**'
      - 'crates/vox-cli/src/training/**'
      - 'scripts/ci/**'
      - 'contracts/eval/**'
  schedule:
    - cron: '0 4 * * *'   # keep existing nightly
  workflow_dispatch:
```

Rationale: a GPU training run should not fire on a comment edit in `docs/src/` or any `.vox` doc snippet.

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/ml_data_extraction.yml')); print('ok')"
git add .github/workflows/ml_data_extraction.yml
git commit -m "ci(ml): narrow training trigger to corpus/compiler inputs"
```

---

## Phase 3 — Bound Contention (SERIAL)

### Task 3.1: Add a shared concurrency budget to non-required self-hosted workflows

**Files:**
- Modify: `.github/workflows/cr-l8-corpus-feedback.yml`, `mutation-pr.yml`, `vox-mental-tracker.yml`, `compile-matrix.yml` (concurrency group only)

Goal: when a new commit supersedes an old one on the same branch, the *secondary* self-hosted workflows cancel immediately so they don't hold runner slots that the required `ci.yml` gate needs. They already have per-workflow cancel; this widens cancellation so a branch update frees all its secondary self-hosted jobs at once.

- [ ] **Step 1: For each file, set a shared per-ref concurrency group**

Replace each workflow's existing `concurrency:` block with a group keyed to a shared name + ref (so all secondary self-hosted workflows for one branch share a cancellation domain, while `ci.yml` keeps its own dedicated group and is never cancelled by them):

```yaml
concurrency:
  group: selfhosted-secondary-${{ github.ref }}
  cancel-in-progress: true
```

Apply identically to `cr-l8-corpus-feedback.yml`, `mutation-pr.yml`, `vox-mental-tracker.yml`, and `compile-matrix.yml`. **Do NOT** change `ci.yml`'s concurrency group (it must stay independent so the required gate is never collaterally cancelled).

- [ ] **Step 2: Validate each file**

```bash
for f in cr-l8-corpus-feedback mutation-pr vox-mental-tracker compile-matrix; do
  python -c "import yaml; yaml.safe_load(open('.github/workflows/$f.yml')); print('$f ok')"
done
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/cr-l8-corpus-feedback.yml .github/workflows/mutation-pr.yml .github/workflows/vox-mental-tracker.yml .github/workflows/compile-matrix.yml
git commit -m "ci: shared cancellation domain for secondary self-hosted workflows"
```

---

## Phase 4 — Make the Required Gate Cheap (SERIAL; highest caching win)

### Task 4.1: Build `vox-cli` once and share it across the 5 required jobs

**Files:**
- Modify: `.github/workflows/ci.yml` (the `setup`, `guards-fast`, `lints`, `compiler-gates`, `tests`, `audits` jobs)

Today each of the 5 required jobs runs `cargo build -p vox-cli` (`ci.yml:88,110,318,431,520,728`) — 5 cold-ish builds across 5 ephemeral runners. Build it once in `setup`, upload `target/debug/vox` as an artifact, download + `chmod +x` in each downstream job. The corrected direct-binary invocation form (`./target/debug/vox --quiet ci <sub>` — note: NO `--` before the subcommand; see fix in #277/#278) is already on main.

- [ ] **Step 1: In the `setup` job, after `cargo build -p vox-cli`, upload the binary**

Add after the build step (around `ci.yml:88`):

```yaml
      - name: Upload vox-cli binary
        uses: actions/upload-artifact@v4
        with:
          name: vox-cli-debug
          path: target/debug/vox
          retention-days: 1
          if-no-files-found: error
```

- [ ] **Step 2: In each of guards-fast, lints, compiler-gates, tests, audits — replace `cargo build -p vox-cli` with artifact download**

For each of the five jobs, replace the `- name: Build vox-cli\n  run: cargo build -p vox-cli` step with:

```yaml
      - name: Download vox-cli binary
        uses: actions/download-artifact@v4
        with:
          name: vox-cli-debug
          path: target/debug
      - name: Make vox-cli executable
        run: chmod +x target/debug/vox
```

Add `needs: setup` to any of these jobs that does not already depend on `setup` (guards-fast/lints/compiler-gates/tests/audits already restore `needs.setup.outputs.cache-key`, so they already `needs: setup`).

- [ ] **Step 3: Validate YAML + confirm no `cargo build -p vox-cli` remains in the 5 required jobs**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"
# Expect only the setup job to still build vox-cli:
grep -n 'cargo build -p vox-cli' .github/workflows/ci.yml
```
Expected: a single match inside the `setup` job.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: build vox-cli once in setup, share via artifact to required jobs"
```

---

## Phase 5 — Caching Hardening (SERIAL after Phase 4; PARALLEL-SAFE within)

### Task 5.1: Confirm sccache is active for ci.yml Rust jobs [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/ci.yml` (top-level `env:` only if needed)

The runner image sets `RUSTC_WRAPPER=sccache` + `SCCACHE_DIR=/cache/sccache` as image ENV (`infra/ci-runner/Dockerfile:35,60`), so self-hosted jobs already inherit it. This task verifies and makes it explicit/observable.

- [ ] **Step 1: Add an sccache stats step to the `tests` job (observability, non-fatal)**

After the test run in the `tests` job, add:

```yaml
      - name: sccache stats (non-fatal)
        if: always()
        run: sccache --show-stats || echo "sccache not active on this runner"
```

- [ ] **Step 2: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"
git add .github/workflows/ci.yml
git commit -m "ci: surface sccache hit-rate stats in tests job"
```

### Task 5.2: Add restore-keys fallback to caches missing them [PARALLEL-SAFE]

**Files:**
- Modify: `.github/workflows/ci.yml` (web-vite-build-smoke `:849-856`, all-features-matrix `:1074-1081`)

Two cache blocks have an exact `key:` but no `restore-keys:`, so a Cargo.lock bump = 100% cold miss.

- [ ] **Step 1: Add restore-keys to web-vite cache** (`ci.yml:849-856`)

```yaml
      - name: Cache Cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-web-vite-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-web-vite-
            ${{ runner.os }}-cargo-
```

- [ ] **Step 2: Add restore-keys to all-features-matrix cache** (`ci.yml:1074-1081`)

```yaml
      - name: Cache Cargo
        uses: actions/cache@v5
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-matrix-${{ matrix.crate }}-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-matrix-${{ matrix.crate }}-
            ${{ runner.os }}-cargo-
```

- [ ] **Step 3: Validate + commit**

```bash
python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"
git add .github/workflows/ci.yml
git commit -m "ci: add restore-keys fallback to web-vite + matrix caches"
```

---

## Phase 6 — Runner Capacity Tuning (SERIAL; OPTIONAL — only if Phases 1–5 insufficient)

### Task 6.1: Re-evaluate MAX_RUNNERS against host headroom

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs:34-37`

Currently 4 runners × 6 CPU = 24 of 32 host threads. After demand reduction (Phases 1–2), 4 may suffice. Only raise if the queue still backs up.

- [ ] **Step 1: Write a failing test asserting the intended ceiling**

In the existing test module of `runner_scale.rs`, add:

```rust
#[test]
fn max_runners_matches_host_budget() {
    // 5 runners × 5 CPU = 25 threads, leaves 7 for host/WSL — only adopt if measured stable.
    assert_eq!(MAX_RUNNERS, 5);
    assert_eq!(RUNNER_CPUS, 5);
}
```

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p vox-cli max_runners_matches_host_budget` → FAIL (currently 4/6).

- [ ] **Step 3: Update constants** (`runner_scale.rs:34-37`): `MAX_RUNNERS = 5`, `RUNNER_CPUS = 5`, `RUNNER_MEM = "5200m"` (5 × 5.2 GB ≈ 26 GB). Adjust only if host has the RAM.

- [ ] **Step 4: Run test + clippy**

```bash
cargo test -p vox-cli max_runners_matches_host_budget
cargo clippy -p vox-cli -- -D warnings
```
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "ci(runner): raise fleet to 5×5cpu after demand reduction"
```

> **NOTE:** This is host-resource-bound. If Phases 1–5 already drain the queue, SKIP this task and `log()` that it was intentionally skipped (no silent cap change).

---

## Phase 7 — Verification & Rollout (SERIAL — last)

### Task 7.1: Whole-workflow YAML + actionlint validation

- [ ] **Step 1: Validate every workflow parses**

```bash
for f in .github/workflows/*.yml; do
  python -c "import yaml,sys; yaml.safe_load(open('$f')); " || echo "BAD: $f"
done
echo "all parsed"
```

- [ ] **Step 2: Re-measure fan-out vs baseline (re-run Task 0.1 Step 1 script)**

Expected: per-PR workflows ≤ 8 (down from 16); per-push-to-main unchanged or slightly down. Confirm the required gate set (`ci.yml`, `mutation-pr` path-filtered, `cr-l-gates` path-filtered, `cr-l8` path-filtered, `ts-emit` path-filtered, `docs-quality` path-filtered, `vox-mental-tracker` path-filtered, `mobile-*` path-filtered) is intact.

### Task 7.2: Update the cost matrix doc

**Files:**
- Modify: `docs/src/ci/rcicd-coverage-cost-matrix-2026.md`

- [ ] **Step 1: Update each retiered workflow's row** (codeql, gitleaks, link_checker, cross-platform-check, setup-e2e, ssot-drift, vox-visus-audit, deploy-hetzner, ml_data_extraction) to its new trigger tier, and add a "2026-06-13 anti-stacking" note. Keep the existing frontmatter intact.

- [ ] **Step 2: Commit**

```bash
git add docs/src/ci/rcicd-coverage-cost-matrix-2026.md
git commit -m "docs(ci): record anti-stacking re-tiering in cost matrix"
```

### Task 7.3: Open the PR; its own CI is the proof

- [ ] **Step 1: Push the branch and open a PR**

```bash
git push -u origin <branch>
gh pr create --base main --title "ci: anti-stacking + caching — cut per-PR fan-out, share vox-cli build" --body "<summary of phases>"
```

- [ ] **Step 2: Observe the PR's own CI fan-out**

The PR itself should now trigger ~8 workflows instead of 16 (CodeQL/gitleaks/link_checker/cross-platform/setup-e2e/visus no longer fire on PR). The required `Check, Build, and Test (Rust)` gate should run the 5 required jobs with a single shared `vox-cli` build. Confirm green, then merge (admin if merge-queue not yet enabled).

---

## Self-Review

**Spec coverage:** ✅ "don't want CICD to stack up" → Phases 1–3 (cut fan-out 16→~8, shared cancellation, merge-queue). "cache its results" → Phase 4 (shared vox-cli artifact) + Phase 5 (sccache stats, restore-keys). "supposed to have that already" → root-caused: cancel-in-progress only dedupes same-branch; merge-queue may be unenforced (Task 0.2); too many workflows on the hot path; 5× redundant builds. Capacity (Phase 6) covers "drain quickly."

**Placeholder scan:** ✅ Every task has exact files, current→target YAML, validate + commit commands. Cron values are marked "keep existing value" where the precise existing cron must be read first (Task steps include the read command).

**Type consistency:** ✅ Concurrency group name `selfhosted-secondary-${{ github.ref }}` used identically across Task 3.1 files; artifact name `vox-cli-debug` consistent between upload (4.1 Step 1) and download (4.1 Step 2); `ci-summary` required-needs set referenced consistently.

**Coordination risk (flagged):** This edits CI while a parallel multi-session swarm is active and the repo's CI was just repaired. Land Phase 0 + Phase 1 first (additive trigger reductions, lowest risk), verify fan-out drops, then Phases 3–5. Phase 6 is optional/skippable. Each task is its own commit so partial rollback is trivial.
