# Local-First CI: Resilient Enforcement + Estate Tuning — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the merge gate fleet-independent, move expensive per-PR work to merge_group/nightly (for wall-clock, not cost), cut/merge redundant checks, add feedback-loop guards, and only then enforce the local-first runner policy.

**Architecture:** Sequenced so the safety-critical resiliency precondition (B) lands before the strict flip (F). Reuses the existing `full-ci` label idiom for tiering (no new mechanism). Every destructive cut has a verify-then-act gate. vox is PUBLIC → hosted minutes are free; **no change is justified by cost** — only wall-clock + feedback latency + resilience.

**Tech Stack:** GitHub Actions YAML, branch-protection API (`gh api`), Rust (`vox-cli-ci`, `vox-cli`), `vox ci` subcommands.

**Branch/worktree:** Land on a fresh branch off `main` (recommended — this is a CI-policy change reviewable on its own), or fold into PR #404. Build a fresh release binary before any push so the strict gate is exercised, not `--no-verify`-bypassed.

**Verified facts (hand-checked against the live repo, 2026-06-28):**
- Sole required context: `["Check, Build, and Test (Rust)"]` = `ci-summary` job, `ci.yml:1287`, `runs-on: [self-hosted, linux, x64]`, 2-min aggregator. **Invariant 1 is FALSE today.**
- `main-merge-queue` ruleset = active. Fleet down → queue never drains.
- ci.yml full `nextest --workspace` runs **only on merge_group** (lines 984/994); per-PR is targeted/affected.
- `ci-fallback-hosted.yml` = `workflow_dispatch`-only, not a required context.

---

## File Structure

| File | Responsibility | Workstream |
|------|----------------|-----------|
| `.github/workflows/ci.yml` | `ci-summary`→hosted; relocate advisory scans; per-PR voxup step | B, D |
| `.github/workflows/ci-fallback-hosted.yml` | nightly + fleet-down trigger, required-equivalent name | B |
| `.github/workflows/cross-platform-check.yml` | keep per-PR cargo check; defer heavy legs; drop os_compat/push | C |
| `.github/workflows/gui-cross-build.yml` | Win/macOS → merge_group | C |
| `.github/workflows/compile-matrix.yml` | cut Win/macOS jobs | C |
| `.github/workflows/codeql.yml` | drop pull_request (merge_group + weekly) | C |
| `.github/workflows/mobile-e2e-android.yml` | drop pull_request → merge_group + nightly | C |
| `.github/workflows/distribution-parity.yml` | cut after replacement step lands | D |
| `crates/vox-cli-ci/src/runner_policy_check.rs` | strict tests + "merge_group-only ≠ required" rule | B, F |
| `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:613` | strict flip (CI side only) | F |
| `crates/vox-cli/src/commands/ci/pre_push.rs` | keep advisory; add env-doctor first | F, G |
| `crates/vox-cli/src/commands/ci/` (new `env_doctor.rs`) | system-dep probe | G |
| `docs/src/ci/{github-hosted-exceptions,runner-contract,compute-placement}.md` | registry + enforcement note + runbook | A, B |

---

## Workstream B — Resiliency precondition (HARD-BLOCKS Workstream F)

### Task B1: Move the required aggregator to a hosted runner

**Files:** Modify `.github/workflows/ci.yml:1287` (`ci-summary` job)

- [ ] **Step 1: Confirm the required context identity**

Run: `gh api repos/vox-foundation/vox/branches/main/protection --jq '.required_status_checks.contexts'`
Expected: `["Check, Build, and Test (Rust)"]`. If it lists anything else, STOP and re-plan — the rest of B assumes this exact single context.

- [ ] **Step 2: Flip `ci-summary` to hosted**

In `ci.yml`, the `ci-summary` job — change ONLY its runner (keep `name:`, `needs:`, `if: always()`, steps unchanged):

```yaml
  ci-summary:
    name: Check, Build, and Test (Rust)
    needs: [guards-fast, lints, compiler-gates, tests, audits]
    if: always()
    # Hosted so the SOLE required context is fleet-independent (compute-placement.md
    # Invariant 1). The 5 heavy needs stay self-hosted; this 2-min aggregator only
    # reads their results, so it must not itself require the workstation.
    runs-on: ubuntu-latest
    timeout-minutes: 5
```

> Note: with the needs self-hosted, a fleet outage still fails the needs → ci-summary reports failure on hosted. B1 alone makes the *aggregator* drainable; B2 provides the actual green-during-outage path. Both are required.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run the required ci-summary aggregator on a hosted runner

The sole branch-protection context 'Check, Build, and Test (Rust)' was
self-hosted, so the merge gate hard-depended on the workstation
(compute-placement.md Invariant 1, false in YAML). Move the 2-min
aggregator to ubuntu-latest; its heavy needs stay self-hosted.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task B2: Make `ci-fallback-hosted.yml` a reachable, required-equivalent valve

**Files:** Modify `.github/workflows/ci-fallback-hosted.yml`

- [ ] **Step 1: Read the current triggers + gate job name**

Run: `sed -n '1,60p' .github/workflows/ci-fallback-hosted.yml`
Identify the `on:` block (currently `workflow_dispatch:` only) and the gate job's `name:`.

- [ ] **Step 2: Add reachable triggers + matching context name**

Change `on:` to add a nightly schedule and a label-gated PR trigger; rename the gate job's `name:` to the required context so a green fallback run satisfies branch protection during an outage:

```yaml
on:
  workflow_dispatch:
  schedule:
    - cron: '0 6 * * *'   # Nightly hosted mirror — recent portable green signal on main
  pull_request:
    types: [labeled, synchronize, reopened]
```

In the gate job, gate the PR path on the label and rename the context:

```yaml
  gate:
    name: Check, Build, and Test (Rust)   # required-equivalent: satisfies branch protection
    if: github.event_name != 'pull_request' || contains(github.event.pull_request.labels.*.name, 'fleet-down')
    runs-on: ubuntu-latest
    timeout-minutes: 45
```

> Effect: applying the `fleet-down` label to a PR makes the hosted fallback report the
> required context green, unblocking merges when the self-hosted fleet is down. The nightly
> schedule keeps a recent portable signal on main.

- [ ] **Step 3: Validate + commit**

Run: `docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:latest .github/workflows/ci-fallback-hosted.yml` → no errors.

```bash
git add .github/workflows/ci-fallback-hosted.yml
git commit -m "ci(fallback): make hosted fallback reachable + required-equivalent

Nightly schedule + a fleet-down-labelled PR trigger, and rename the gate
job to the required context name so a green fallback run satisfies branch
protection during a fleet outage. Was workflow_dispatch-only (theater).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task B3: Enforce "merge_group-only jobs are never required contexts"

**Files:** Test + impl in `crates/vox-cli-ci/src/runner_policy_check.rs`; document in `docs/src/ci/runner-contract.md`

- [ ] **Step 1: Write the failing test**

Append to `mod tests` in `runner_policy_check.rs`:

```rust
    #[test]
    fn detects_merge_group_only_job() {
        // A job that only runs on merge_group (no pull_request reachability) must be
        // flaggable so it is never wired as a required branch-protection context.
        let yml = "on:\n  merge_group:\njobs:\n  heavy:\n    runs-on: ubuntu-latest\n";
        assert!(workflow_is_merge_group_only(yml));
        let pr = "on:\n  pull_request:\n  merge_group:\njobs:\n  j:\n    runs-on: ubuntu-latest\n";
        assert!(!workflow_is_merge_group_only(pr));
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo nextest run -p vox-cli-ci detects_merge_group_only_job --profile ci`
Expected: FAIL — `workflow_is_merge_group_only` not defined.

- [ ] **Step 3: Implement the predicate**

Add to `runner_policy_check.rs`:

```rust
/// True when a workflow's `on:` triggers include `merge_group` but NOT `pull_request`,
/// meaning its jobs never report on PRs and must never be branch-protection required
/// contexts (a required-but-skipped context leaves the merge queue permanently pending).
pub fn workflow_is_merge_group_only(text: &str) -> bool {
    let has_merge_group = text.lines().any(|l| l.trim_start().starts_with("merge_group:"));
    let has_pull_request = text.lines().any(|l| l.trim_start().starts_with("pull_request:"));
    has_merge_group && !has_pull_request
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo nextest run -p vox-cli-ci detects_merge_group_only_job --profile ci`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli-ci/src/runner_policy_check.rs
git commit -m "feat(ci): detect merge_group-only workflows (queue-deadlock guard)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task B4: Merge-queue break-glass runbook

**Files:** Modify `docs/src/ci/runner-contract.md` (new §)

- [ ] **Step 1: Add the runbook section**

Append to `runner-contract.md`:

```markdown
## Merge-queue break-glass (fleet outage)

The `main-merge-queue` ruleset is active and serializes every merge through a `merge_group`
ci.yml run on the self-hosted fleet. The admin bypass (`enforce_admins=false`) does NOT
apply inside a required merge queue. If the fleet is down:

1. **Preferred:** label the PR `fleet-down` → `ci-fallback-hosted.yml` reports the required
   context `"Check, Build, and Test (Rust)"` green on hosted infra; merge normally.
2. **If the queue is wedged:** temporarily set the `main-merge-queue` ruleset to
   `enforcement: evaluate` (or disable it) via
   `gh api -X PUT repos/vox-foundation/vox/rulesets/<id> ...`, merge, then re-enable.
3. Restore the fleet (`vox ci runner-scale` / autoscaler) and remove the `fleet-down` label.
```

- [ ] **Step 2: Doc-lint + commit**

Run: `cargo run -q -p vox-doc-pipeline -- --lint-only`
```bash
git add docs/src/ci/runner-contract.md
git commit -m "docs(ci): merge-queue break-glass runbook for fleet outages

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Workstream C — Per-PR → merge_group/nightly tiering

> **Precondition (run once before any C task):**
> `gh api repos/vox-foundation/vox/branches/main/protection --jq '.required_status_checks.contexts'`
> must NOT contain any `cross-platform-check` / `gui-cross-build` / `compile-matrix`
> Win/macOS context. Verified today = only `["Check, Build, and Test (Rust)"]`. If that
> changes, abort C — demoting a required context to merge_group-only deadlocks the queue.

### Task C1: cross-platform-check — keep cheap per-PR check, defer heavy legs

**Files:** Modify `.github/workflows/cross-platform-check.yml`

- [ ] **Step 1: Revert the static self-hosted-Linux matrix edit from this session to the label idiom**

The matrix currently has win/mac/self-hosted legs running every event. Keep all three legs in the matrix, but gate the EXPENSIVE steps and drop the redundant triggers. First, in `on:`, remove the `push: branches: [main]` trigger (merge_group already gates main):

```yaml
on:
  pull_request:
    paths: [ ... keep existing ... ]
  merge_group:
  schedule:
    - cron: '0 4 * * 1'
  workflow_dispatch:
```

- [ ] **Step 2: Keep `cargo check` unconditional; gate the heavy legs**

The `cargo check (workspace) — every event` step (line ~77) stays unconditional on ALL matrix legs — this is the only per-PR `#[cfg(windows)]`/`#[cfg(macos)]` compile proof. Add an `if:` to the platform-sensitive nextest + merge-queue clippy/nextest steps so the Win/macOS legs do only the cheap `cargo check` on PRs:

```yaml
      - name: nextest (platform-sensitive crates) — non-PR or Linux
        if: runner.os == 'Linux' || github.event_name != 'pull_request'
        run: |
          cargo nextest run -p vox-config
          cargo nextest run -p vox-cli-core
          cargo nextest run -p vox-populi --lib
```

(The merge_group-only clippy/nextest steps at lines ~96-102 already carry `if: github.event_name == 'merge_group'` — leave them.)

- [ ] **Step 3: Remove the per-merge `os_compat.py` step**

Delete the `Run portability scanner (os_compat.py)` + `Upload portability report` steps (lines ~104-116). Coverage is preserved by the weekly `os-compat-report.yml`. Add a one-line comment pointing there.

- [ ] **Step 4: Validate + commit**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check` → OK.

```bash
git add .github/workflows/cross-platform-check.yml
git commit -m "ci(cross-platform): keep per-PR cargo check, defer heavy legs to merge_group

Win/macOS legs do only the cheap per-PR cargo check (the sole per-PR
cfg(windows) compile proof); full clippy+nextest stay merge_group-only.
Drop the redundant push:main trigger and the per-merge os_compat.py
(weekly os-compat-report.yml covers it). Coverage deferred: full Win/macOS
test run moves from per-PR to merge_group + weekly.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task C2: gui-cross-build — Win/macOS to merge_group

**Files:** Modify `.github/workflows/gui-cross-build.yml`

- [ ] **Step 0: Read the file and branch the implementation**

Run: `sed -n '1,80p' .github/workflows/gui-cross-build.yml`
- If it is ONE matrixed job: add a job-level `if:` is not enough (it would skip the Linux leg too). Instead gate per-leg via a `full-ci`-style label OR split the matrix include by event. Prefer: keep Linux leg per-PR, add `if: github.event_name != 'pull_request' || contains(github.event.pull_request.labels.*.name, 'full-ci')` on the Win/macOS-only steps.
- If it is N discrete jobs (like compile-matrix): add `if: github.event_name != 'pull_request'` to the Win/macOS jobs only.

Implement whichever matches the actual structure. Do NOT assume C1's shape.

- [ ] **Step 1: Validate + commit**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check` → OK.

```bash
git add .github/workflows/gui-cross-build.yml
git commit -m "ci(gui-cross-build): Win/macOS Tauri legs off per-PR (merge_group)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task C3: compile-matrix — cut Win/macOS jobs

**Files:** Modify `.github/workflows/compile-matrix.yml`

- [ ] **Step 1: Delete the two hosted jobs**

Remove `compile-help-windows` and `compile-help-macos` jobs entirely. Their `vox compile --help` smoke is subsumed by cross-platform-check's per-PR `cargo check --workspace`. Keep `compile-help-linux` (self-hosted). No `merge_group` trigger is added (avoids the deadlock class).

- [ ] **Step 2: Validate + commit**

```bash
git add .github/workflows/compile-matrix.yml
git commit -m "ci(compile-matrix): cut Win/macOS help-smoke jobs (subsumed by cross-platform cargo check)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task C4: codeql + mobile-e2e-android off per-PR

**Files:** Modify `.github/workflows/codeql.yml`, `.github/workflows/mobile-e2e-android.yml`

- [ ] **Step 1: codeql — drop pull_request**

In `codeql.yml` `on:`, remove the `pull_request:` block; keep `push: branches:[main]`, `schedule:` (weekly), `merge_group:` (add if absent), `workflow_dispatch:`. CodeQL's 60-min Rust analysis leaves the per-PR critical path; main + merge_group + weekly retain coverage.

- [ ] **Step 2: mobile-e2e-android — drop pull_request**

In `mobile-e2e-android.yml` `on:`, replace `pull_request:` with `merge_group:` + a nightly `schedule:` (mirror `mobile-e2e-ios.yml`'s cadence — read it first to match).

- [ ] **Step 3: Validate + commit**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check` → OK.

```bash
git add .github/workflows/codeql.yml .github/workflows/mobile-e2e-android.yml
git commit -m "ci: codeql + android-e2e off per-PR (merge_group + schedule)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Workstream D — Test cuts (verify-then-act)

### Task D1: Cut distribution-parity with a per-PR replacement step

**Files:** Modify `.github/workflows/ci.yml`; delete `.github/workflows/distribution-parity.yml`

- [ ] **Step 1: VERIFY the merge_group workspace nextest covers the test**

Run: `cargo nextest run -p voxup --test distribution_parity --profile ci`
Expected: PASS (the test exists at `crates/voxup/tests/distribution_parity.rs`). Confirm ci.yml's merge_group `cargo nextest run --workspace --profile ci` (line 994) includes `voxup` (it is not in any `--exclude`). If `voxup` is excluded, STOP — do not cut.

- [ ] **Step 2: Add a per-PR replacement step to ci.yml `tests` job**

So PR-time signal survives on the fleet (merge_group nextest is full-only), add to the `tests` job a path-filtered fast step:

```yaml
      - name: Distribution SSOT parity (voxup) — fast PR signal
        if: needs.setup.outputs.affects_contracts == 'true' || needs.setup.outputs.full == 'true'
        run: cargo nextest run -p voxup --test distribution_parity --profile ci
```

- [ ] **Step 3: Delete the standalone workflow**

```bash
git rm .github/workflows/distribution-parity.yml
```

- [ ] **Step 4: Remove its exception/registry references**

If `distribution-parity.yml` appears in `github-hosted-exceptions.md`, remove that row.

- [ ] **Step 5: Validate + commit**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check` → OK.

```bash
git add .github/workflows/ci.yml docs/src/ci/github-hosted-exceptions.md
git commit -m "ci: fold distribution-parity into ci.yml per-PR tests; cut standalone hosted workflow

voxup distribution_parity runs in ci.yml merge_group workspace nextest;
add a path-filtered per-PR step so fast signal survives on the fleet, then
remove the cold-build hosted workflow.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task D2: Relocate advisory scans out of the merge-gate hot path

**Files:** Modify `.github/workflows/ci.yml`, `.github/workflows/bench-nightly.yml`

- [ ] **Step 1: Identify the `continue-on-error` advisory steps in ci.yml `tests`/`audits`**

Run: `grep -n "continue-on-error" .github/workflows/ci.yml`
Target the advisory scans named in the audit (crate-build-audit, plugin-candidacy, build-bench, graphify-freshness, cargo-outdated) — confirm each is `continue-on-error: true` (non-blocking) before moving it.

- [ ] **Step 2: Move them to bench-nightly.yml**

Cut each confirmed advisory step from ci.yml and add it to `bench-nightly.yml` (already self-hosted, nightly). Keep the exact command. Leave any that are NOT `continue-on-error` (those are real gates).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml .github/workflows/bench-nightly.yml
git commit -m "ci: relocate advisory scans from merge-gate to bench-nightly

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Workstream E — Speedups

### Task E1: sccache hit-rate telemetry

**Files:** Modify `.github/workflows/ci.yml` (setup, lints, compiler-gates, tests, audits jobs)

- [ ] **Step 1: Add a stats step to each heavy job**

After the build/test steps in each of the 5 jobs, add:

```yaml
      - name: sccache stats
        if: always()
        run: sccache --show-stats || true
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: emit sccache --show-stats on heavy jobs (hit-rate telemetry)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task E2: Split guards-fast (keep deterministic guards blocking)

**Files:** Modify `.github/workflows/ci.yml`

- [ ] **Step 1: Read the guards-fast job**

Run: `grep -n "guards-fast" .github/workflows/ci.yml` then read the job. Identify slow members (cargo-deny, cargo-audit, cargo-shear, `.vox` audits, plugin-abi-parity `--build`).

- [ ] **Step 2: Extract slow members into a non-required `guards-slow` job**

Create a parallel `guards-slow` job (self-hosted) NOT in `ci-summary` needs. Move the slow steps there. Keep deterministic fast guards (line-endings, BOM, manifest, ssot-drift, config gates) in `guards-fast`.

- [ ] **Step 3: Verify guards-slow is not a required context**

Run: `gh api repos/vox-foundation/vox/branches/main/protection --jq '.required_status_checks.contexts'` → still only `["Check, Build, and Test (Rust)"]`. `guards-slow` must not be added to `ci-summary` needs.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: split guards-slow off the blocking guards-fast path

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Workstream G — Feedback-loop / coverage backfill

### Task G1: `vox ci env-doctor` — system-dep probe

**Files:** Create `crates/vox-cli/src/commands/ci/env_doctor.rs`; wire into `cmd_enums.rs`, `run_body.rs`, `pre_push.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/tests/env_doctor_test.rs`:

```rust
#[test]
fn env_doctor_reports_each_required_dep() {
    // The probe must check exactly the deps ci.yml installs, from one SSOT list.
    let report = vox_cli::commands::ci::env_doctor::probe();
    let names: Vec<&str> = report.iter().map(|d| d.name.as_str()).collect();
    for dep in ["libdbus-1", "glib-2.0", "gtk+-3.0", "webkit2gtk-4.1"] {
        assert!(names.contains(&dep), "missing probe for {dep}");
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo nextest run -p vox-cli --test env_doctor_test --profile ci`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the probe**

Create `env_doctor.rs`:

```rust
//! `vox ci env-doctor` — probe the system libraries CI installs, so a missing dep
//! fails fast LOCALLY instead of after a CI round-trip (the libdbus/GTK class).

pub struct DepStatus {
    pub name: String,
    pub present: bool,
}

/// SSOT list — must match the apt packages installed in .github/workflows/ci.yml.
const PKG_CONFIG_DEPS: &[&str] = &[
    "libdbus-1", "glib-2.0", "gtk+-3.0", "webkit2gtk-4.1",
    "libsoup-3.0", "javascriptcoregtk-4.1",
];

pub fn probe() -> Vec<DepStatus> {
    PKG_CONFIG_DEPS
        .iter()
        .map(|name| {
            // On non-Linux (dev workstation is Windows) pkg-config is absent; report
            // present=true so the probe is a no-op off the CI platform.
            let present = if cfg!(target_os = "linux") {
                std::process::Command::new("pkg-config")
                    .args(["--exists", name])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            } else {
                true
            };
            DepStatus { name: (*name).to_string(), present }
        })
        .collect()
}

/// CLI entry: print a table, return Err if any Linux dep is missing.
pub fn run() -> anyhow::Result<()> {
    let report = probe();
    let mut missing = Vec::new();
    for d in &report {
        println!("{:<24} {}", d.name, if d.present { "ok" } else { "MISSING" });
        if !d.present { missing.push(d.name.clone()); }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(
            "env-doctor: missing system deps: {}. Install: sudo apt-get install -y libdbus-1-dev pkg-config libglib2.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev",
            missing.join(", ")
        )
    }
}
```

- [ ] **Step 4: Wire the subcommand**

Add `EnvDoctor` to the `CiCmd` enum (`cmd_enums.rs`) and dispatch in `run_body.rs`:
```rust
CiCmd::EnvDoctor => super::env_doctor::run(),
```
Add `pub mod env_doctor;` to the ci module root.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo nextest run -p vox-cli --test env_doctor_test --profile ci`
Expected: PASS.

- [ ] **Step 6: Run env-doctor first in pre-push (advisory)**

In `pre_push.rs`, add an early step that calls `env_doctor::probe()` and prints warnings (do NOT hard-fail on the Windows workstation — it's a no-op there; on Linux CI mirror it surfaces missing deps).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/ci/env_doctor.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/pre_push.rs crates/vox-cli/tests/env_doctor_test.rs
git commit -m "feat(ci): vox ci env-doctor — probe CI system deps locally

Closes the libdbus/GTK env-only-failure class: a missing system lib now
fails fast in pre-push instead of after a CI round-trip.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task G2: Per-PR GUI vitest lane (verify-then-act)

**Files:** Possibly `.github/workflows/ci.yml`

- [ ] **Step 1: VERIFY whether the unit vitest suite runs in any workflow**

Run: `grep -rn "vitest" .github/workflows/ crates/vox-gui/ui/package.json` and check whether ci.yml's GUI steps run `vitest run` (vs only Playwright e2e + `test:ingest`).
- If a `vitest run` lane already exists → SKIP this task (no gap).
- If NOT → proceed to Step 2.

- [ ] **Step 2: Add a per-PR GUI unit lane**

Add to ci.yml (or the existing GUI job) a step gated on `needs.setup.outputs.affects_gui == 'true'`:

```yaml
      - name: GUI unit tests (vitest) + typecheck
        if: needs.setup.outputs.affects_gui == 'true'
        working-directory: crates/vox-gui/ui
        run: |
          pnpm install --frozen-lockfile
          pnpm exec vitest run
          pnpm exec tsc --noEmit
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(gui): per-PR vitest + tsc lane on GUI-affecting PRs

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Workstream A — Register hosted exceptions

### Task A1: Complete the exception registry + fix the enforcement note

**Files:** Modify `docs/src/ci/github-hosted-exceptions.md`

- [ ] **Step 1: Add rows for still-unregistered hosted workflows**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check` to list current offenders. Add table rows for each remaining one (`version-tag-guard.yml`, `workflow-lint.yml`, and any others surfaced), each citing its `compute-placement.md` rationale (tag-only release guard / neutral-infra lint).

- [ ] **Step 2: Update the enforcement note (line ~40)**

Change `**Enforcement:** ... default advisory; --strict to fail.` to:

```markdown
**Enforcement:** `vox ci runner-policy-check` runs `--strict` inside `vox ci ssot-drift`
(and therefore CI) — an unregistered GitHub-hosted `runs-on` FAILS the gate. The fast
pre-push tier runs it advisory-but-loud. Register genuine exceptions above; placement
rationale lives in [compute-placement.md](compute-placement.md).
```

- [ ] **Step 3: Verify clean against the full estate**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check --strict; echo "exit=$?"`
Expected: `exit=0`. Paste the output into the PR description.

- [ ] **Step 4: Commit**

```bash
git add docs/src/ci/github-hosted-exceptions.md
git commit -m "docs(ci): complete hosted-exception registry + enforcement note

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Workstream F — Strict flip (LAST; CI-side only)

### Task F1: Flip `--strict` in ssot-drift only, keep pre-push advisory

**Files:** Test in `runner_policy_check.rs`; modify `run_body_helpers/docs.rs:613`; LEAVE `pre_push.rs:1084` as `false`

- [ ] **Step 1: Confirm B preconditions are met**

Run:
```bash
gh api repos/vox-foundation/vox/branches/main/protection --jq '.required_status_checks.contexts'
```
Assert the required context is satisfiable on hosted infra (B1: ci-summary hosted; B2: fallback required-equivalent). If B is not merged, STOP — do not flip strict.

- [ ] **Step 2: Write the strict-direction tests**

Append to `mod tests` in `runner_policy_check.rs`:

```rust
    #[test]
    fn strict_errors_on_unregistered_hosted() {
        let tmp = std::env::temp_dir().join(format!("rpc-{}", std::process::id()));
        let wf = tmp.join(".github/workflows");
        std::fs::create_dir_all(&wf).unwrap();
        std::fs::create_dir_all(tmp.join("docs/src/ci")).unwrap();
        std::fs::write(tmp.join(EXCEPTIONS_DOC),
            "| Workflow | Runner | Reason |\n|--|--|--|\n").unwrap();
        std::fs::write(wf.join("rogue.yml"),
            "jobs:\n  j:\n    runs-on: ubuntu-latest\n").unwrap();
        assert!(run(&tmp, false).is_ok());   // advisory tolerates
        assert!(run(&tmp, true).is_err());   // strict rejects
        std::fs::remove_dir_all(&tmp).ok();
    }
```

- [ ] **Step 3: Run it to verify it passes (signature already supports strict)**

Run: `cargo nextest run -p vox-cli-ci strict_errors_on_unregistered_hosted --profile ci`
Expected: PASS (exercises existing `run(root, strict)`).

- [ ] **Step 4: Flip the CI-side call site ONLY**

In `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:613`, change:

```rust
    let _ = vox_cli_ci::runner_policy_check::run(root, false);
```
to:
```rust
    // Enforced on the CI side: ssot-drift fails on unregistered hosted runners.
    // NOTE: ssot-drift is reachable from the fast pre-push tier; that propagation is
    // intentional and bounded. We deliberately do NOT flip pre_push.rs's standalone
    // step_runner_policy_check (it stays advisory) so the gate is never silently
    // bypassed by the known stale-binary --no-verify pattern.
    vox_cli_ci::runner_policy_check::run(root, true)?;
```

Confirm the enclosing function returns `Result<()>` and still ends `Ok(())`.

- [ ] **Step 5: LEAVE pre_push.rs advisory**

Verify `pre_push.rs:1084` still reads `run(root, false)`. Do not change it. (Optional: upgrade its log line to a loud warning.)

- [ ] **Step 6: Build + verify the real path**

Run: `cargo build -p vox-cli && cargo run -q -p vox-cli -- ci ssot-drift; echo "exit=$?"`
Expected: `exit=0` (the registry is complete from Workstream A; the rest of ssot-drift already passes on this branch).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli-ci/src/runner_policy_check.rs crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs
git commit -m "feat(ci): enforce local-first runner policy in ssot-drift (CI-side strict)

Flips runner-policy-check to --strict inside ssot-drift only. The fast
pre-push step stays advisory so the gate is never bypassed by the known
stale-binary --no-verify pattern. Preconditioned on Workstream B
(required gate is now fleet-independent).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

### Task F2: Update runner-contract + AGENTS rules

**Files:** Modify `docs/src/ci/runner-contract.md`, `AGENTS.md`

- [ ] **Step 1: runner-contract.md §Local-first → enforced (CI side)**

Update the heading and enforcement paragraph to state: enforced via `--strict` inside
`ssot-drift`/CI; fast pre-push advisory-but-loud; deliberate hosted jobs registered per
`compute-placement.md`; the required context is fleet-independent (ci-summary hosted) with
`ci-fallback-hosted` as the reachable outage valve.

- [ ] **Step 2: AGENTS.md §"Run CI locally first"**

Change the advisory-drift line to: enforced via `ssot-drift` strict; register exceptions in
`github-hosted-exceptions.md`; note pre-push stays advisory.

- [ ] **Step 3: Doc-lint + commit**

Run: `cargo run -q -p vox-doc-pipeline -- --lint-only && cargo run -q -p vox-cli -- ci check-links`
```bash
git add docs/src/ci/runner-contract.md AGENTS.md
git commit -m "docs(ci): document enforced (CI-side) local-first policy + resilient gate

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## FINAL: review + push

- [ ] **Step 1: Build a fresh release binary** so the strict gate is exercised, not `--no-verify`-bypassed: `cargo build --release -p vox-cli` (or `cargo install --locked --path crates/vox-cli --force`). This kills the stale-binary `graphify` ssot-drift false-positive.
- [ ] **Step 2: Dispatch a code-reviewer agent** over the full B→F diff. Confirm: required context is hosted; fallback reachable + required-equivalent name; no merge_group-only job is a required context; strict flipped ONLY in ssot-drift; every cut has its replacement; no cost/minute framing in any message.
- [ ] **Step 3: Push** (pre-push should now pass without `--no-verify`). If it still trips on an unrelated stale guard, run `vox ci ssot-drift` manually post-push to prove the new gate is green and note it in the PR.
- [ ] **Step 4: Verify branch protection post-merge** — `ci-summary` reports on hosted; apply a `fleet-down` label to a test PR and confirm `ci-fallback-hosted` reports the required context.

---

## Execution via Workflow (sub-agents)

Orchestrate with one `Workflow` call, respecting the hard dependency B→F:

```
phase('B: resiliency')   // sequential within B (B1..B4), but B is a barrier before F
  B = pipeline([B1,B2,B3,B4])         // each gates the next where they share ci.yml
phase('C+D+E+G: improvements')        // independent of each other → parallel()
  await parallel([C1,C2,C3,C4, D1,D2, E1,E2, G1,G2])   // distinct files; G1 is Rust TDD
phase('Verify')                        // barrier: runner-policy-check --strict clean + ssot-drift exit 0
phase('A+F: register + strict')        // sequential, gated on Verify + B
  A1 ; F1 ; F2
phase('Review')                        // code-reviewer over the whole diff
```

Notes: subagents are read-only in the worktree sandbox ([[feedback_subagents_readonly_in_sandbox]]) — run the Workflow from the writable main worktree, or have agents emit edits the main session applies. B and F are safety-critical: do them with the main session in the loop, not fully autonomous.

---

## Self-Review

- **Spec coverage:** every spec workstream (A–G) maps to tasks here; resiliency guardrails 1–7 are realized in B1 (guardrail 1), B2 (2, 6), B3 (3), C (4), B4 (5), and the "nothing GPU on merge_group" note (7, enforced by not adding such triggers).
- **Verified vs unverified:** load-bearing claims (required-context identity, ci-summary runner, merge-queue active, merge_group-only nextest) hand-verified and cited. Destructive cuts (D1 distribution-parity, G2 vitest) carry explicit VERIFY steps before action.
- **No cost framing:** all commit messages cite wall-clock/feedback/resilience, never minutes.
- **Correctness:** strict flipped only in `docs.rs:613` (not `pre_push.rs:1084`); double-invocation called out as intentional+bounded; matrix replaced by label idiom / job-`if:`; C2 reads-then-branches; compile-matrix cut (no merge_group-required deadlock).
- **Type consistency:** `run(root, strict)`, `workflow_is_merge_group_only`, `env_doctor::{probe, run, DepStatus}` used consistently across tasks.
