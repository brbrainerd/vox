# Release Pipeline Verified Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Get `release-binaries.yml` and `release-installers.yml` to actually produce a real GitHub Release with real, downloadable, per-platform assets — proven with one disposable test tag, not assumed from a green checkmark.

**Architecture:** No new components. Three targeted changes to existing CI config (a runner-pinning switch + its required exceptions-table update, already-committed WiX fix carried over), verified end-to-end against real GitHub Actions runs the local toolchain cannot simulate.

**Tech Stack:** GitHub Actions YAML, `cargo-wix`, `gh` CLI, this repo's `vox ci` guard suite (`ssot-drift`, `runner-policy-check`).

**Spec:** `docs/superpowers/specs/2026-08-23-release-pipeline-verified-design.md`

## Execution Note (post-critique)

Tasks 1's line-number citations below were written before a 6-track parallel
critique found them already stale (real numbers: 32/178/200, not 30/150/172)
and surfaced several additional real issues — see the spec's "Critique
Findings & Resolutions" section for the authoritative list. All of Task 1's
work, PLUS the critique's fixes (prerelease/make_latest safety flags,
build-windows-msi's timeout, and a new release-installers.yml publish job
wiring up previously-discarded MSI/deb/brew artifacts) have already been
implemented and locally verified (`vox ci ssot-drift` exit 0) as of this
note. Task 1's steps below are kept as a record of the original intent, not
as literal to-do items — read the spec's critique section and the actual
git diff before re-deriving anything from this task's stale line numbers.

## Global Constraints

- Every GitHub-hosted `runs-on` requires a matching row in `docs/src/ci/github-hosted-exceptions.md`, verified by `vox ci runner-policy-check --strict` (part of `vox ci ssot-drift`). The workflow change and the table update MUST land in the same commit — AGENTS.md documents this exact pattern for other SSOT pairs, and the gate enforces it here too.
- No `sleep`-loop polling of GitHub Actions runs. Push, then check status a bounded number of times with real time between checks, per this session's standing CI-watching guard.
- The `v0.0.0-test` tag is disposable. Delete it and its GitHub Release once verification is complete, unless told to keep it.
- Do not touch `release-gui.yml` — explicitly out of scope (deferred by user decision).

---

### Task 1: Switch `release-binaries.yml`'s self-hosted jobs to `ubuntu-latest`, update the exceptions table in the same commit

**Files:**
- Modify: `.github/workflows/release-binaries.yml:29-30` (matrix entry), `:150` (`dist-verify` `runs-on`), `:172` (`publish` `runs-on`)
- Modify: `docs/src/ci/github-hosted-exceptions.md:17` (the `release-binaries.yml` row)
- Test: none (CI-config-only; verified via `vox ci ssot-drift` in Task 2)

**Interfaces:**
- Consumes: nothing from earlier tasks (first task)
- Produces: a `release-binaries.yml` with zero `self-hosted` references, ready for Task 2's local verification

- [ ] **Step 1: Confirm the current self-hosted references (baseline, before editing)**

Run: `grep -n "self-hosted" .github/workflows/release-binaries.yml`

Expected output (3 lines, matching current file state):
```
30:            runs_on: '["self-hosted","linux","x64"]'
150:    runs-on: [self-hosted, linux, x64]
172:    runs-on: [self-hosted, linux, x64]
```

If line numbers differ from this baseline, the file has changed since this plan was written — re-read the file before proceeding rather than editing blind.

- [ ] **Step 2: Edit the matrix entry**

In `.github/workflows/release-binaries.yml`, change:
```yaml
          - target: x86_64-unknown-linux-gnu
            runs_on: '["self-hosted","linux","x64"]'
```
to:
```yaml
          - target: x86_64-unknown-linux-gnu
            runs_on: '["ubuntu-latest"]'
```

- [ ] **Step 3: Edit the `dist-verify` job's `runs-on`**

Change:
```yaml
  dist-verify:
    name: dist verification (fat LTO)
    runs-on: [self-hosted, linux, x64]
```
to:
```yaml
  dist-verify:
    name: dist verification (fat LTO)
    runs-on: ubuntu-latest
```

Also update the comment immediately above this job (currently explains the self-hosted 14GB budget assumption) so it doesn't misdescribe the runner after the switch — replace "the fleet image ships the pinned 1.96.0 toolchain and rust-toolchain.toml is authoritative" reasoning for skipping a `rust-toolchain` step with a note that `ubuntu-latest` does NOT ship a pinned Rust toolchain, so this job now needs an explicit `dtolnay/rust-toolchain@master` step (see Step 5 — this is a real behavior change, not just a label swap).

- [ ] **Step 4: Edit the `publish` job's `runs-on`**

Change:
```yaml
  publish:
    name: Publish GitHub release
    runs-on: [self-hosted, linux, x64]
```
to:
```yaml
  publish:
    name: Publish GitHub release
    runs-on: ubuntu-latest
```

- [ ] **Step 5: Add the missing Rust toolchain step to `dist-verify`**

The self-hosted fleet image ships a pre-installed, pinned Rust toolchain (per the job's original comment: "No rust-toolchain step: the fleet image ships the pinned 1.96.0 toolchain"). `ubuntu-latest` does not. Without this step, `dist-verify`'s `cargo build` will use whatever Rust GitHub's stock Ubuntu image happens to have (unpinned, version drift risk) or fail outright if none is preinstalled.

Add, as the step immediately after `actions/checkout@v7` in the `dist-verify` job:
```yaml
      - name: Install Rust toolchain
        # Pinned, not @stable: shipped artifacts must be built by the same compiler
        # CI gates use (rust-toolchain.toml). @stable also imports each new
        # release's lint wave — see AGENTS.md §Perennial Bug Patterns.
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"
```

(This exact step already exists in the `build` job's Windows/macOS legs in this same file — copy its style, not just its shape, for consistency.)

- [ ] **Step 6: Confirm no self-hosted references remain**

Run: `grep -n "self-hosted" .github/workflows/release-binaries.yml`

Expected: no output (empty match).

- [ ] **Step 7: Update the exceptions table row**

In `docs/src/ci/github-hosted-exceptions.md`, change line 17 from:
```
| `release-binaries.yml` | `windows-latest`, `macos-latest` (matrix) | Publish tagged Windows/macOS binaries; Linux build lane is self-hosted. |
```
to:
```
| `release-binaries.yml` | `windows-latest`, `macos-latest`, `ubuntu-latest` (matrix + dist-verify + publish) | Publish tagged binaries for all platforms. Linux build, fat-LTO dist-verify, and the release-publish step moved off the self-hosted fleet 2026-08-23 — the fleet's availability for tag-triggered release runs is unverified (a 2026-05-26 run queued 24h with no runner pickup) and this pipeline needs to be provably reliable, not best-effort. |
```

- [ ] **Step 8: Commit**

```bash
git add .github/workflows/release-binaries.yml docs/src/ci/github-hosted-exceptions.md
git commit -m "fix(ci): move release-binaries.yml off the self-hosted fleet

Linux build, dist-verify, and publish all required [self-hosted, linux,
x64] and none of the three self-hosted-dependent jobs have ever completed
in a recorded run -- the most recent attempt (2026-05-26) queued for
exactly 24:00:00 with no runner pickup, GitHub's platform ceiling killing
it rather than any job-level timeout firing.

Switched all three to ubuntu-latest, added the Rust toolchain install step
dist-verify was implicitly relying on the fleet image for (it has none),
and updated the github-hosted-exceptions.md row in the same commit --
runner-policy-check fails otherwise."
```

---

### Task 2: Verify the runner-switch change passes the repo's own local gates

**Files:**
- None modified (verification only)

**Interfaces:**
- Consumes: Task 1's committed changes
- Produces: confidence the exceptions-table pairing is correct before spending a real tag push on it

- [ ] **Step 1: Build the `vox` binary needed to run `vox ci` locally, if not already built in this worktree**

Run: `ls target/debug/vox.exe target/release/vox.exe 2>&1 | grep -v cannot`

If neither exists:
Run: `cargo build -p vox-cli --bin vox` (background this — it takes 15-40 minutes on a fresh worktree; do not block on it synchronously)

- [ ] **Step 2: Run the runner-policy guard**

Run: `target/debug/vox.exe ci ssot-drift` (or `target/release/vox.exe`, whichever was built)

Expected: no `Error: ... runner-policy-check` or `Error: ... unregistered GitHub-hosted runs-on` line. If this specific check fails, re-read Task 1 Step 7 — the table row's workflow-name/runner-list pairing likely doesn't match what the guard expects; do not proceed to Task 3 until this passes.

- [ ] **Step 3: Sanity-check the YAML is syntactically valid**

Run: `actionlint .github/workflows/release-binaries.yml` if `actionlint` is available locally (`which actionlint`); if not installed, do a careful manual re-read of the diff instead — do not skip this check silently.

Expected: no errors.

---

### Task 3: Push the disposable verification tag and watch both workflows to completion

**Files:**
- None modified

**Interfaces:**
- Consumes: Task 1's pushed commit (must be on a branch reachable from the tag)
- Produces: two completed (not necessarily successful) GitHub Actions runs to inspect in Task 4

- [ ] **Step 1: Confirm the branch with Task 1's commit is pushed to origin**

Run: `git push origin <branch-name>` (the branch this plan's commits landed on)

- [ ] **Step 2: Create and push the disposable tag from that branch's tip**

```bash
git tag v0.0.0-test
git push origin v0.0.0-test
```

- [ ] **Step 3: Confirm both tag-triggered workflows actually started**

Run: `gh run list --workflow release-binaries.yml --limit 1 --json databaseId,status,headSha`
Run: `gh run list --workflow release-installers.yml --limit 1 --json databaseId,status,headSha`

Expected: both return a run whose `headSha` matches the tagged commit and `status` is `queued` or `in_progress`. If either is missing, the tag push didn't trigger it — check the workflow's `on: push: tags:` pattern against the exact tag name before re-tagging.

- [ ] **Step 4: Wait for both runs to complete — bounded checks, not a loop**

These jobs have historically taken 15 minutes to several hours. Check status no more than once every 15-20 minutes using:

`gh api repos/vox-foundation/vox/actions/runs/<run-id> -q '.status, .conclusion'`

Do not sleep-loop this in the same tool call. Between checks, do other useful work (review the workflow diff again, prep Task 4's verification commands) or end the turn and resume on the next user message / a legitimate external trigger. Continue only once `.status` is `completed` for both runs.

---

### Task 4: Verify real, non-empty release assets exist — not just a green checkmark

**Files:**
- None modified

**Interfaces:**
- Consumes: Task 3's completed runs
- Produces: a pass/fail verdict on the actual goal (a real distributable release), independent of workflow-reported success

- [ ] **Step 1: Confirm a GitHub Release was actually created**

Run: `gh release view v0.0.0-test --json assets,tagName`

Expected: valid JSON, not a "release not found" error. If this fails even though both workflows reported success, that is itself a bug to report — it means `publish`'s `fail_on_unmatched_files: false` (in the existing `softprops/action-gh-release@v3` step) silently swallowed a missing-files condition. Do not treat workflow-green as sufficient; this step is the actual test.

- [ ] **Step 2: Confirm every expected asset is present and non-empty**

Run: `gh release view v0.0.0-test --json assets -q '.assets[].name'`

Expected, at minimum, one file matching each of these patterns (exact version/target substitution aside):
```
vox-v0.0.0-test-x86_64-unknown-linux-gnu.tar.gz
vox-v0.0.0-test-x86_64-pc-windows-msvc.zip
vox-v0.0.0-test-x86_64-apple-darwin.tar.gz
vox-v0.0.0-test-aarch64-apple-darwin.tar.gz
vox-ml-cli-v0.0.0-test-*.{tar.gz,zip}  (one per target)
voxup-v0.0.0-test-*.{tar.gz,zip}       (one per target)
checksums.txt
sbom.spdx.json                          (best-effort, continue-on-error — absence here is a warning, not a failure)
```

For `release-installers.yml`'s outputs (published separately — check whether it uploads to the same tag's release or a separate one; read `release-installers.yml`'s publish step to confirm before assuming), confirm:
```
<something>.msi        (from build-windows-msi)
<something>.deb         (from build-linux-deb)
<a homebrew formula artifact>  (from publish-macos-brew)
```

For any missing asset, do not guess why — pull that specific job's log (`gh api repos/vox-foundation/vox/actions/jobs/<job-id>/logs --allow-escape-sequences`) and read the actual failure before reporting a verdict.

- [ ] **Step 3: Spot-check at least one asset's integrity**

Run: `gh release download v0.0.0-test --pattern 'checksums.txt' --dir /tmp/release-verify` then confirm the file is non-empty and contains real SHA256-looking lines (`grep -cE '^[0-9a-f]{64}  '`).

- [ ] **Step 4: Report the verdict**

State plainly, per asset category: which platforms/binaries genuinely produced a working, downloadable artifact, and which (if any) did not — with the specific job/log evidence for each failure. This is the actual deliverable of this plan; do not summarize as "the pipeline works" without this per-asset breakdown.

---

### Task 5: Clean up the disposable tag and release

**Files:**
- None modified

**Interfaces:**
- Consumes: Task 4's completed verification
- Produces: no leftover test artifacts on the shared repo

- [ ] **Step 1: Confirm with the user before deleting anything**

Per this session's standing rule on destructive/hard-to-reverse shared-state actions: do not delete the tag or release without a fresh confirmation at this point in execution, even though this plan pre-authorizes the tag's creation as disposable — deletion is the irreversible half of that lifecycle and deserves its own checkpoint, especially if Task 4 found real bugs worth leaving the evidence up for a moment.

- [ ] **Step 2: Delete the release and tag (only after confirmation)**

```bash
gh release delete v0.0.0-test --yes
git push origin :refs/tags/v0.0.0-test
git tag -d v0.0.0-test
```

- [ ] **Step 3: Commit any follow-up fixes discovered in Task 4 as their own task/plan**

If Task 4 found real, fixable bugs (e.g., a missing asset from a specific platform leg), do not fold ad-hoc fixes into this plan's tasks — write a short follow-up plan or task list scoped to exactly what broke, following the same "verify against the real GitHub API, not the checkmark" discipline this plan used.

---

## Self-Review Notes

- **Spec coverage:** Task 1 covers Decision 1 (runner switch). Task 3 covers Decision 3 (real tag verification). Task 2 is new (not explicit in the spec) but necessary — it's the local-gate check the spec's "Testing / Verification Plan" step 1 describes. Task 4 covers the spec's "actual proof, not the CI checkmark" requirement directly. Task 5 covers the spec's cleanup requirement. The WiX fix (spec's item 3) is already committed (`b7fa5274d`) — no task re-does it; Task 3's real tag push is its first verification, called out explicitly in the spec's Open Risks.
- **Known gap, deliberately not a task:** `release-binaries.yml`'s Windows/macOS legs have not run against current code at all (stale May 26 data only). This plan does not pre-fix anything there speculatively — Task 4 will surface whatever's actually broken, and Task 5 Step 3 hands that off as a scoped follow-up rather than guessing now.
