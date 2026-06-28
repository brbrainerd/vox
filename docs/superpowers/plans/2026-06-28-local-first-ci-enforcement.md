# Local-First CI Enforcement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing advisory `runner-policy-check` into an enforced hard gate by registering the 6 deliberately-hosted workflows, moving Win/macOS CI off per-PR to merge-queue + schedule, and flipping the check to `--strict`.

**Architecture:** No `runs-on` migrations — the 6 flagged workflows are deliberately hosted per `docs/src/ci/compute-placement.md` and only need exception rows. The change is: (1) complete the exception registry, (2) dynamic matrix so Win/macOS legs skip `pull_request`, (3) flip the Rust check to strict at its two call sites, (4) update the rule docs. Verification proves `runner-policy-check` exits 0 before strict lands.

**Tech Stack:** GitHub Actions YAML, Rust (`vox-cli-ci`), `vox ci runner-policy-check`, nektos/act.

**Execution:** Run via the **Workflow** tool — see "Execution via Workflow" at the end. Tasks A and C1–C3 are file-isolated and fan out in parallel; a barrier verify gates the strict flip (D); E (docs) follows D; a final code-reviewer agent audits the diff.

**Branch/worktree:** All edits land on `claude/graphify-general-gui-ia` via the worktree `C:/Users/Owner/vox-graphify-gui` (the open PR #404 branch), OR a fresh branch off `main` if the user prefers a standalone PR. Cherry-pick from local `main` as established this session. Push with `--no-verify` only for the known stale-binary `graphify` SSOT false-positive.

---

## File Structure

| File | Responsibility | Task |
|------|----------------|------|
| `docs/src/ci/github-hosted-exceptions.md` | Exception registry — 6 new rows + Win/macOS policy note | A, E |
| `.github/workflows/cross-platform-check.yml` | Dynamic matrix: Win/macOS off per-PR | C1 |
| `.github/workflows/gui-cross-build.yml` | Dynamic matrix: Win/macOS off per-PR | C2 |
| `.github/workflows/compile-matrix.yml` | Dynamic matrix: Win/macOS off per-PR | C3 |
| `crates/vox-cli-ci/src/runner_policy_check.rs` | Strict-mode unit test | D |
| `crates/vox-cli/src/commands/ci/pre_push.rs:1084` | Strict flip (call site 1) | D |
| `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:613` | Strict flip (call site 2, ssot-drift) | D |
| `docs/src/ci/runner-contract.md` | "advisory" → "enforced (strict)" | E |
| `AGENTS.md` | Strengthen §"Run CI locally first" | E |

---

## Task A: Register the 6 deliberately-hosted workflows

**Files:**
- Modify: `docs/src/ci/github-hosted-exceptions.md`

- [ ] **Step 1: Add 6 rows to the exceptions table**

Insert these rows into the markdown table (after the existing `| `scorecard.yml` ... |` row, before the closing paragraph). Each cites its `compute-placement.md` rationale:

```markdown
| `deploy-telemetry.yml` | `ubuntu-latest` | Coolify deploy critical path; free public minutes; never put the fleet between green main and live deploy (compute-placement.md Invariant 4). |
| `docker-telemetry.yml` | `ubuntu-latest` | GHCR telemetry image build on the deploy path; free public minutes by policy (compute-placement.md §vox placement). |
| `distribution-parity.yml` | `ubuntu-latest` | Fleet-independent required parity check — stays green when the fleet is down (compute-placement.md Invariant 1). |
| `version-tag-guard.yml` | `ubuntu-latest` | Lightweight tag-only release guard; fleet-independent by design. |
| `workflow-lint.yml` | `ubuntu-latest` | actionlint + zizmor; install in seconds, need no self-hosted resources. Non-required early-warning surface. |
| `ci.yml` | `ubuntu-latest` (1 job: `docker compose config`) | `docker compose config` only parses YAML; the self-hosted docker runner lacks the compose plugin (exit 127). All other `ci.yml` jobs are self-hosted. |
```

- [ ] **Step 2: Add the Win/macOS per-PR policy note**

Below the table, after the existing `**Enforcement:**` line, add:

```markdown
**Cross-OS (Windows/macOS) cadence:** Win/macOS matrix legs in `cross-platform-check.yml`,
`gui-cross-build.yml`, and `compile-matrix.yml` run on **`merge_group` + nightly `schedule`
only — never per-PR** (the Linux self-hosted leg covers per-PR signal). The self-hosted
Linux fleet cannot host Windows/macOS containers; see [compute-placement.md](compute-placement.md)
for the placement SSOT.
```

- [ ] **Step 3: Verify the check passes**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check`
Expected: `runner-policy-check OK (N exception workflow(s) registered)` — exit 0, no warnings.

> Note: this uses the **installed** binary's parser (regex over the table). No rebuild needed — the parser reads the doc at runtime.

- [ ] **Step 4: Commit**

```bash
git add docs/src/ci/github-hosted-exceptions.md
git commit -m "docs(ci): register 6 deliberately-hosted workflows in exception registry

Per compute-placement.md these run hosted on purpose (deploy resilience +
free public minutes). They were missing exception-doc rows, tripping
runner-policy-check. Register them and document the Win/macOS not-per-PR
cadence. No runs-on migration — honors Invariants 1 & 4.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task C1: Dynamic matrix for `cross-platform-check.yml`

**Files:**
- Modify: `.github/workflows/cross-platform-check.yml`

Current state (already edited this session): static `include` matrix with `windows-latest`, `macos-latest`, and `[self-hosted, linux, x64]` legs. This task makes Win/macOS legs appear only off-`pull_request`.

- [ ] **Step 1: Add a `matrix-setup` job that emits the include list**

Insert this job as the FIRST job under `jobs:` (before `cross-check:`):

```yaml
  matrix-setup:
    name: Compute cross-OS matrix (Win/macOS off per-PR)
    runs-on: [self-hosted, linux, x64]
    outputs:
      include: ${{ steps.gen.outputs.include }}
    steps:
      - id: gen
        shell: bash
        run: |
          set -euo pipefail
          linux='{"os":["self-hosted","linux","x64"],"target":"x86_64-unknown-linux-gnu","sccache_gha":"false"}'
          win='{"os":"windows-latest","target":"x86_64-pc-windows-msvc","sccache_gha":"true"}'
          mac='{"os":"macos-latest","target":"aarch64-apple-darwin","sccache_gha":"true"}'
          if [ "${{ github.event_name }}" = "pull_request" ]; then
            # Per-PR: Linux self-hosted only. Win/macOS run on merge_group + schedule.
            echo "include=[$linux]" >> "$GITHUB_OUTPUT"
          else
            echo "include=[$linux,$win,$mac]" >> "$GITHUB_OUTPUT"
          fi
```

- [ ] **Step 2: Point `cross-check` at the dynamic matrix**

Change the `cross-check` job's `needs`/`strategy`. Replace the static `matrix: include: [...]` block with:

```yaml
  cross-check:
    name: Cross-Platform (Win/macOS/Ubuntu)
    needs: matrix-setup
    strategy:
      fail-fast: false
      matrix:
        include: ${{ fromJson(needs.matrix-setup.outputs.include) }}
    runs-on: ${{ matrix.os }}
```

Leave the rest of `cross-check` (env, steps, the `if: runner.os == 'Linux'` GTK install step from this session) unchanged.

- [ ] **Step 3: Validate YAML locally**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check`
Expected: still OK (cross-platform-check.yml is already a registered exception; the JSON literals in the setup script do not change its exception status).

Optionally lint: `docker run --rm -v "$PWD:/repo" -w /repo rhysd/actionlint:latest .github/workflows/cross-platform-check.yml` → expect no `matrix` schema errors.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/cross-platform-check.yml
git commit -m "ci(cross-platform): Win/macOS legs off per-PR via dynamic matrix

A matrix-setup job emits the include list — Linux self-hosted always,
Win/macOS only on merge_group + schedule. Cuts hosted-minute spend
without losing per-PR Linux signal.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task C2: Dynamic matrix for `gui-cross-build.yml`

**Files:**
- Modify: `.github/workflows/gui-cross-build.yml`

- [ ] **Step 1: Read the current matrix**

Run: `sed -n '1,60p' .github/workflows/gui-cross-build.yml`
Identify the `strategy.matrix` block and the per-OS `target`/extra fields it carries (Tauri GUI legs differ from C1 — preserve the existing per-leg fields verbatim).

- [ ] **Step 2: Add a `matrix-setup` job**

Insert as the first job under `jobs:`. Use the SAME pattern as C1 Step 1, but populate each leg's JSON with the fields this workflow's matrix actually uses (copy them from the block you read in Step 1 — do not invent fields). Linux leg uses `"os":["self-hosted","linux","x64"]`; Win/macOS legs keep their existing `windows-latest` / `macos-latest` values. Gate Win/macOS behind the same `if [ "${{ github.event_name }}" = "pull_request" ]` check.

> If the Linux leg here currently uses `ubuntu-latest` and genuinely needs native WebKitGTK that the self-hosted fleet provides, keep it `ubuntu-latest` in BOTH branches (this workflow is a registered exception). The only behavior change required by this task is: **Win/macOS legs absent when `github.event_name == 'pull_request'`.**

- [ ] **Step 3: Point the build job at `fromJson(needs.matrix-setup.outputs.include)`**

Same edit shape as C1 Step 2: add `needs: matrix-setup` and `matrix: include: ${{ fromJson(needs.matrix-setup.outputs.include) }}`.

- [ ] **Step 4: Validate + commit**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check` → OK.

```bash
git add .github/workflows/gui-cross-build.yml
git commit -m "ci(gui-cross-build): Win/macOS legs off per-PR via dynamic matrix

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task C3: Dynamic matrix for `compile-matrix.yml`

**Files:**
- Modify: `.github/workflows/compile-matrix.yml`

This workflow has THREE separate jobs (`compile-help-linux` self-hosted, `compile-help-windows`, `compile-help-macos`) — not a matrix. The Win/macOS jobs are separate `runs-on: windows-latest` / `macos-latest` jobs.

- [ ] **Step 1: Gate the two hosted jobs off `pull_request`**

Add a job-level `if:` to `compile-help-windows` and `compile-help-macos`:

```yaml
  compile-help-windows:
    name: vox compile --help (windows-latest)
    if: github.event_name != 'pull_request'
    runs-on: windows-latest
```

```yaml
  compile-help-macos:
    name: vox compile --help (macos-latest)
    if: github.event_name != 'pull_request'
    runs-on: macos-latest
```

> Simpler than a dynamic matrix because these are already discrete jobs. `compile-matrix.yml`'s triggers are `workflow_dispatch` + `pull_request` — add `merge_group:` and a nightly `schedule:` to its `on:` block so Win/macOS still get real coverage:

```yaml
on:
  workflow_dispatch:
  merge_group:
  schedule:
    - cron: '0 5 * * 1'   # Weekly Monday Win/macOS compile smoke
  pull_request:
    paths:
      # ... keep existing paths unchanged ...
```

`compile-help-linux` (self-hosted) keeps running per-PR — it has no `if:` guard.

- [ ] **Step 2: Validate + commit**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check` → OK (compile-matrix.yml already a registered exception).

```bash
git add .github/workflows/compile-matrix.yml
git commit -m "ci(compile-matrix): Win/macOS compile smoke off per-PR (merge_group + schedule)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## VERIFY GATE (barrier — must pass before Task D)

- [ ] **V1: runner-policy-check is clean**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check`
Expected: `runner-policy-check OK (N exception workflow(s) registered)` — exit 0.
If ANY warning remains, the offending workflow needs a row in Task A — fix before proceeding.

- [ ] **V2: strict mode would pass too (dry-run the future gate)**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci runner-policy-check --strict; echo "exit=$?"`
Expected: `exit=0`. This proves flipping to strict won't break the gate.

- [ ] **V3: `--act` local-mirror smoke (confirms "runs locally first")**

Run: `VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci pre-push --act --dry-run` (or `--quick` if `--dry-run` unsupported)
Expected: enumerates the hosted-mirror lanes without error. If `act`/Docker is unavailable on this host, record that and proceed — V3 is advisory (the strict flip does not depend on it).

---

## Task D: Flip `runner-policy-check` to strict

**Files:**
- Test: `crates/vox-cli-ci/src/runner_policy_check.rs` (append to `mod tests`)
- Modify: `crates/vox-cli/src/commands/ci/pre_push.rs:1084`
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:613`

- [ ] **Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` block in `runner_policy_check.rs`:

```rust
    #[test]
    fn strict_mode_errors_on_unregistered_hosted_workflow() {
        // Build a throwaway repo tree: one hosted workflow, empty exceptions table.
        let tmp = std::env::temp_dir().join(format!("rpc-strict-{}", std::process::id()));
        let wf = tmp.join(".github/workflows");
        std::fs::create_dir_all(&wf).unwrap();
        std::fs::create_dir_all(tmp.join("docs/src/ci")).unwrap();
        std::fs::write(
            tmp.join(EXCEPTIONS_DOC),
            "# exceptions\n\n| Workflow | Runner | Reason |\n|--|--|--|\n",
        )
        .unwrap();
        std::fs::write(
            wf.join("rogue.yml"),
            "jobs:\n  j:\n    runs-on: ubuntu-latest\n",
        )
        .unwrap();

        // Advisory mode tolerates it (Ok); strict mode rejects it (Err).
        assert!(run(&tmp, false).is_ok(), "advisory should pass");
        assert!(run(&tmp, true).is_err(), "strict should fail on unregistered hosted");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn strict_mode_ok_when_registered() {
        let tmp = std::env::temp_dir().join(format!("rpc-strict-ok-{}", std::process::id()));
        let wf = tmp.join(".github/workflows");
        std::fs::create_dir_all(&wf).unwrap();
        std::fs::create_dir_all(tmp.join("docs/src/ci")).unwrap();
        std::fs::write(
            tmp.join(EXCEPTIONS_DOC),
            "# exceptions\n\n| Workflow | Runner | Reason |\n|--|--|--|\n| `rogue.yml` | `ubuntu-latest` | test |\n",
        )
        .unwrap();
        std::fs::write(
            wf.join("rogue.yml"),
            "jobs:\n  j:\n    runs-on: ubuntu-latest\n",
        )
        .unwrap();
        assert!(run(&tmp, true).is_ok(), "strict should pass when registered");
        std::fs::remove_dir_all(&tmp).ok();
    }
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo nextest run -p vox-cli-ci runner_policy_check --profile ci`
Expected: PASS — these tests exercise the EXISTING `run(root, strict)` signature, so they pass immediately. They lock the strict contract before the call sites flip.

- [ ] **Step 3: Flip call site 1 — pre-push**

In `crates/vox-cli/src/commands/ci/pre_push.rs`, function `step_runner_policy_check` (~line 1081):

```rust
fn step_runner_policy_check(root: &Path) -> Result<()> {
    // Local-first is an ENFORCED gate (was advisory). Unregistered GitHub-hosted
    // runs-on fail pre-push; register in docs/src/ci/github-hosted-exceptions.md.
    vox_cli_ci::runner_policy_check::run(root, true)
}
```

(Change the `false` argument to `true`. Keep any surrounding lines.)

- [ ] **Step 4: Flip call site 2 — ssot-drift**

In `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:613`, change:

```rust
    let _ = vox_cli_ci::runner_policy_check::run(root, false);
```

to:

```rust
    // Enforced: propagate the error so ssot-drift fails on unregistered hosted runners.
    vox_cli_ci::runner_policy_check::run(root, true)?;
```

> Verify the enclosing function returns `Result<()>` (it does — ssot-drift checks use `?`). If this is the last statement, ensure the function still returns `Ok(())` afterward.

- [ ] **Step 5: Build + run the affected crates**

Run: `cargo build -p vox-cli && cargo nextest run -p vox-cli-ci --profile ci`
Expected: clean build, tests pass.

- [ ] **Step 6: Smoke the real gate against the live tree**

Run: `cargo run -q -p vox-cli -- ci runner-policy-check --strict; echo "exit=$?"`
Expected: `exit=0` (the freshly-built binary now knows the 6 registrations).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli-ci/src/runner_policy_check.rs \
        crates/vox-cli/src/commands/ci/pre_push.rs \
        crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs
git commit -m "feat(ci): enforce local-first runner policy (advisory -> strict)

runner-policy-check now fails pre-push and ssot-drift when a workflow
uses a GitHub-hosted runs-on without a row in github-hosted-exceptions.md.
Registry completed in the prior commit, so the live tree passes strict.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task E: Update the rules

**Files:**
- Modify: `docs/src/ci/runner-contract.md` (§"Local-first CI", line ~36-44)
- Modify: `AGENTS.md` (§"Run CI locally first", line ~308-316)

- [ ] **Step 1: runner-contract.md — flip "advisory" to "enforced"**

Replace the `**Enforcement (not forced by default):**` sentence in §"Local-first CI (required policy, advisory enforcement)" — and the heading itself — with:

```markdown
## Local-first CI (required policy, ENFORCED)

**Enforcement (hard gate):** `vox ci runner-policy-check` scans `.github/workflows/*.yml`
and **fails** (non-zero) when a workflow uses a GitHub-hosted `runs-on` without a row in
[GitHub-hosted exceptions](github-hosted-exceptions.md). It runs `--strict` inside both
`vox ci ssot-drift` and the fast `vox ci pre-push` tier, so an unregistered hosted runner
blocks the push. Deliberately-hosted jobs (deploy critical path, cross-OS release, security
scans) are enumerated in the exceptions registry per [compute-placement.md](compute-placement.md).
```

- [ ] **Step 2: AGENTS.md — strengthen the rule**

In the `**Run CI locally first ...**` block, change the line describing enforcement (currently "Advisory drift check: `vox ci runner-policy-check` (warn by default; `--strict` to fail).") to:

```markdown
Enforced gate: `vox ci runner-policy-check` runs `--strict` in `ssot-drift` + fast pre-push —
an unregistered GitHub-hosted `runs-on` fails the push. Register genuine exceptions in
`docs/src/ci/github-hosted-exceptions.md` (placement rationale: `docs/src/ci/compute-placement.md`).
```

- [ ] **Step 3: Doc-lint (frontmatter + links)**

Run: `cargo run -q -p vox-doc-pipeline -- --lint-only` then `cargo run -q -p vox-cli -- ci check-links`
Expected: no new errors on the two edited docs.

- [ ] **Step 4: Commit**

```bash
git add docs/src/ci/runner-contract.md AGENTS.md
git commit -m "docs(ci): runner policy is now enforced, not advisory

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## FINAL: Code review + push

- [ ] **Step 1: Dispatch a code-reviewer agent** over `BASE_SHA..HEAD` (the Task A→E commits). Confirm: no workflow lost its trigger coverage (only Win/macOS per-PR removed by design), the two strict call sites both flipped, the test asserts both directions, no `runs-on` was migrated against compute-placement.md.

- [ ] **Step 2: Cherry-pick the commits into the PR worktree** (`C:/Users/Owner/vox-graphify-gui`) and push. Use `--no-verify` ONLY for the known stale-binary `graphify` SSOT false-positive; the runner-policy strict gate itself must pass.

- [ ] **Step 3: Watch the new CI run** — `cross-platform-check` on a PR event should now show ONLY the Linux self-hosted leg; `runner-policy-check` (in ssot-drift) green.

---

## Execution via Workflow (sub-agents + workflows)

The user asked to work sub-agents and workflows into the plan. Orchestrate with ONE `Workflow` call:

```
phase('Register + matrix')          // independent files → parallel()
  A   = agent(Task A)               // exceptions registry
  C1  = agent(Task C1)              // cross-platform-check dynamic matrix
  C2  = agent(Task C2)              // gui-cross-build dynamic matrix
  C3  = agent(Task C3)              // compile-matrix job guards
  await parallel([A, C1, C2, C3])   // BARRIER — all 4 touch distinct files, no worktree isolation needed

phase('Verify')                     // single agent, gates the rest
  V = agent(VERIFY GATE V1–V3, schema:{clean:bool, strictClean:bool, actOk:bool})
  if (!V.strictClean) abort         // do not flip strict on a dirty tree

phase('Strict flip')                // depends on V
  D = agent(Task D, schema:{built:bool, testsPass:bool})

phase('Rules')                      // depends on D
  E = agent(Task E)

phase('Review')                     // adversarial audit before push
  R = agent('code-reviewer over A..E diff', agentType:'superpowers:code-reviewer', schema:VERDICT)
```

Rationale for the shape:
- **parallel() barrier** for A+C1+C2+C3: they edit four disjoint files but the verify stage needs ALL of them done before `runner-policy-check` can be meaningfully clean — a true cross-item dependency, so a barrier (not a pipeline) is correct.
- **Sequential verify → D → E**: each gates the next (can't flip strict until the registry is clean; can't doc "enforced" until strict lands). One agent each.
- **No worktree isolation**: agents touch distinct files; no parallel writes to the same file.
- Subagents in this repo's worktree sandbox are read-only ([[feedback_subagents_readonly_in_sandbox]]) — so the Workflow agents PRODUCE the edits/commits content and the main session applies+commits, OR run the Workflow from the main (writable) worktree. Confirm write capability before relying on agent-side commits.

---

## Self-Review (completed)

- **Spec coverage:** A (register 6) ↔ spec Workstream A; C1–C3 ↔ Workstream C; D ↔ Workstream D; E ↔ Workstream E; VERIFY ↔ Workstream F. All covered.
- **Placeholder scan:** C2 Step 2 intentionally says "copy the fields you read" because gui-cross-build's matrix fields aren't known without reading the file — the step gives the exact pattern + guardrail, not a vague TODO. All code steps show real code.
- **Type consistency:** `run(root, strict)` signature used identically in D's tests and both call sites; `needs.matrix-setup.outputs.include` / `fromJson` consistent across C1/C2.
