---
title: "Release Pipeline Verified — Design"
description: "Get the CLI/voxup/installer distribution pipeline actually producing a real GitHub Release with real downloadable assets, verified end-to-end."
category: "Architecture SSOTs"
status: "draft"
---

# Release Pipeline Verified — Design

## Problem

Every release/distribution workflow in this repository has a 0% historical
success rate:

| Workflow | Runs | Successes |
|---|---|---|
| `release-binaries.yml` | 2 | 0 |
| `release-gui.yml` | 6 | 0 |
| `release-installers.yml` | 2 (5 jobs) | 4/5 jobs, 0 full runs |

Zero GitHub Releases have ever been published on this repo, despite a
`v0.6.0` tag existing. There is no nightly trigger for any release workflow
— `docs/src/architecture/distribution-ssot.md`'s "release + nightly read
`binaries`" claim is aspirational prose, not a running pipeline.

Root causes identified this session:

1. **`release-binaries.yml`'s Linux build + `dist-verify` + `publish` jobs**
   require `runs-on: [self-hosted, linux, x64]`. The last recorded attempt
   (2026-05-26) sat queued for exactly 24:00:00 and was never picked up by a
   runner — a fleet-availability problem, not a code bug.
2. **`release-binaries.yml`'s Windows/macOS build failures** (same date)
   were `cargo build failed for crate vox-bootstrap` — `vox-bootstrap` was
   since deleted and replaced by `voxup` (see AGENTS.md §Retired Surfaces).
   That data is stale; current status against `vox-bootstrap`-free code is
   unknown.
3. **`release-installers.yml`'s `build-windows-msi` job** failed with
   `light.exe` exit 103 (LGHT0103, file-not-found). Root cause: `cargo wix
   --no-build` expects `target/dist/vox.exe` + `vox-compilerd.exe` to
   already exist, but no step ever built them. Fixed in commit `b7fa5274d`
   on `claude/vox-distribution-security-floor` — **unverified**, no local
   WiX toolset to test against.
4. **`release-gui.yml`** compiles for 1–2.5 hours then fails downstream
   with `No artifacts were found` — root cause not yet determined. Out of
   scope for this design (see Non-Goals).

## Goal

A real, verifiable GitHub Release exists with real downloadable assets for
`vox`, `vox-ml-cli`, and `voxup` across Linux (x86_64), Windows
(x86_64-pc-windows-msvc), and macOS (x86_64 + aarch64), plus the
`release-installers.yml` packaging (Windows MSI, macOS Homebrew formula,
Linux `.deb`) — produced by pushing a real tag, checked by fetching the
release from `gh release view` / the GitHub API, not assumed from workflow
"green" status alone.

## Non-Goals

- `release-gui.yml` (Tauri desktop app release) — separate investigation,
  explicitly deferred per user decision. Not touched by this plan.
- A real nightly/scheduled release trigger — not requested; the SSOT doc's
  claim is noted as inaccurate but not fixed here (prose-only fix, separate
  concern from "does the pipeline work").
- Fixing the self-hosted fleet itself — infrastructure/ops, outside this
  agent's access. Worked around by switching affected jobs to GitHub-hosted
  runners instead.

## Decisions (from brainstorming)

1. **Self-hosted → hosted runner switch.** `release-binaries.yml`'s Linux
   build, `dist-verify`, and `publish` jobs move from
   `[self-hosted, linux, x64]` to `ubuntu-latest`. This is a deliberate,
   documented departure from the repo's local-first-CI default (see
   `docs/src/ci/runner-contract.md` §Local-first CI) — the existing
   `docs/src/ci/github-hosted-exceptions.md` row for `release-binaries.yml`
   already says "Linux build lane is self-hosted"; this design changes that
   line and the reasoning column to reflect the new, hosted-only state.
   `vox ci runner-policy-check --strict` (part of `ssot-drift`) enforces
   that every GitHub-hosted `runs-on` has a matching exception row — this
   must be updated in the same commit as the workflow change, or the gate
   fails.
2. **CLI/installers first, GUI deferred.** Confirmed by direct user choice.
3. **Verification is a real tag push**, not a `workflow_dispatch` shim.
   `release-binaries.yml` and `release-installers.yml` have no
   `workflow_dispatch` trigger — only `push: tags: v*`. A disposable test
   tag (`v0.0.0-test`) will be pushed to `origin` to get real, full,
   cross-platform CI signal. The tag (and any GitHub Release it produces)
   is treated as disposable and cleaned up after verification succeeds,
   unless the user asks to keep it as a known-good marker.

## Architecture

No new components. This is a CI-workflow-and-Cargo-config correctness
project: fix what's provably broken in the existing three-workflow release
pipeline (`release-binaries.yml`, `release-installers.yml`, and their
shared `docs/src/ci/github-hosted-exceptions.md` contract), then prove it
with one real, disposable tag push, read back through the GitHub Releases
API rather than assumed from CI's green checkmark alone (workflow "success"
and "a release with real assets exists" are checked separately — the
`publish` job could theoretically report success while
`fail_on_unmatched_files: false` silently drops an expected asset, which is
exactly the kind of false-positive this plan's verification step exists to
catch).

## Testing / Verification Plan

1. **Static, before any push:** `actionlint` against the changed workflow
   YAML (repo already runs this in `.github/workflows/ci.yml` — reuse
   locally if the binary is available; if not, careful manual YAML review
   plus `vox ci ssot-drift`'s `runner_policy_check` step, which is a real,
   runnable local gate for the exceptions-table half of this change).
2. **Real signal:** push `v0.0.0-test`. Watch (bounded checks, not a poll
   loop) `release-binaries.yml` and `release-installers.yml` runs to
   completion.
3. **The actual proof, not the CI checkmark:** once both workflows report
   complete, independently verify via `gh release view v0.0.0-test
   --json assets` (or equivalent) that real, non-empty binary/installer
   assets exist for every declared platform — Linux/Windows/macOS×2
   tarballs+zips for `vox`, `vox-ml-cli`, `voxup`, the Windows MSI, the
   macOS brew formula artifact, and the Linux `.deb`. A workflow reporting
   "success" is necessary but not sufficient proof.
4. **Cleanup:** delete the `v0.0.0-test` tag and its GitHub Release once
   verified (`gh release delete`, `git push origin :refs/tags/v0.0.0-test`)
   — unless the user says to keep it.

## Open Risks

- The WiX fix (commit `b7fa5274d`) is unverified — this plan's tag push is
  its first real test. If it's wrong, `build-windows-msi` will need a
  second iteration.
- `release-binaries.yml`'s Windows/macOS legs haven't run against current
  (post-`vox-bootstrap`) code at all. They may fail for reasons unrelated
  to anything fixed so far — the stale May 26 data gives no current signal.
- Switching `publish`'s runner does not change its `contents: write`
  permission scope, already narrowed in an earlier session (commit
  `711a9eabb`, `release-gui.yml`'s equivalent split) — this design assumes
  `release-binaries.yml`'s existing job-scoped permissions (`build`:
  `contents: read`, `publish`: `contents: write`) are already correctly
  separated (confirmed by reading the workflow file: yes, they are) and
  makes no further permission changes.

## Critique Findings & Resolutions

A 6-track parallel critique (12 agents: 6 findings + 6 adversarial verification
passes) audited this spec/plan against the real codebase before any tag push.
All findings below were independently confirmed REAL. Resolved in the
implementation (not just noted as risk):

1. **Goal-breaking, now fixed:** `release-installers.yml`'s three packaging
   jobs (MSI, deb, brew tarball) built artifacts and discarded them — zero
   upload steps anywhere; the "Upload to Homebrew Tap" step was a stub
   (`echo "Simulating..."`). Added `actions/upload-artifact` to each job and
   a new `publish` job that attaches all three to the tag's GitHub Release.
   **Known remaining gap, explicitly out of scope:** the actual Homebrew tap
   dispatch (to a separate `vox-foundation/homebrew-vox` repo) was never
   implemented and still isn't — the tarball+checksum are now real,
   downloadable release assets, but nothing auto-updates a brew formula.
2. **Safety-critical, now fixed:** neither publish step set `prerelease`/
   `make_latest`, so `softprops/action-gh-release@v3`'s default
   (`make_latest: true`) meant a disposable test tag could become the
   public "latest" release — and `voxup install` (unlike `voxup update`)
   has no version-floor guard, so a real user could install a test build.
   Fixed with a semver-convention rule on both publish jobs: a hyphenated
   tag (`v0.0.0-test`, `v1.2.3-rc1`) is `prerelease: true` /
   `make_latest: false`; a plain tag is a real release and becomes latest.
3. **Confirmed, accepted as noise:** `version-tag-guard.yml` will hard-fail
   on `v0.0.0-test` (tag version `0.0.0-test` != `Cargo.toml`'s `0.6.0`) — a
   third, expected red run on the verification tag push. Not fixed (fixing
   it would mean weakening a real drift-prevention gate for a throwaway
   tag); documented here so it isn't mistaken for pipeline breakage during
   verification.
4. **Confirmed, accepted as noise:** `release-gui.yml` also triggers on any
   `v*` tag push and will run its known-broken, multi-hour, explicitly
   out-of-scope path as an unavoidable side effect of the verification tag.
5. **Fixed:** `build-windows-msi`'s `timeout-minutes` was still 45 after the
   WiX fix (commit `b7fa5274d`) added a real fat-LTO build step — sibling
   jobs doing the identical build were bumped to 180 minutes in a separate,
   prior commit (`70d4dce26`) that predates and didn't touch this job.
   Raised to 180 to match; otherwise the one verification tag push would
   near-certainly burn on a timeout, not a real WiX signal.
6. **Positive confirmation, no change needed:** the WiX
   `CargoTargetBinDir`/custom-profile resolution mechanism itself is
   correct — independently verified against cargo-wix's actual source
   behavior for non-standard profile names.
7. **Plan-text correction:** the plan's Task 2 described
   `runner-policy-check` as validating the exceptions table's *content*
   (workflow-to-runner pairing); it actually only checks whether the
   workflow's filename string appears anywhere in the doc — presence, not
   correctness. Low practical risk (the row already existed, and its text
   was still updated for human accuracy) but the plan's own claim about
   what local verification proves was wrong and is corrected in the plan.
8. **Accepted, unverified risk — no cheaper test exists:** the self-hosted
   fleet's 14GB memory budget was calibrated from a *different* workload
   (`cargo doc`, thin LTO) than this pipeline's fat-LTO release links. Fat-LTO
   peak RSS has never been measured on any runner — the one self-hosted
   attempt queued 24h and never reached the link step. `ubuntu-latest`'s
   nominal 16GB may or may not be enough; there's no way to know without the
   real push this plan already requires. If it OOMs there, that is strictly
   more information than the self-hosted hang ever produced.
