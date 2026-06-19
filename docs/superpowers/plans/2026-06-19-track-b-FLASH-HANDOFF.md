# Gemini 3.5 Flash Handoff Prompt — Track B (Release + Nightly Automation)

> Copy everything inside the fenced block below into Antigravity / Gemini 3.5 Flash.
> It is self-contained but assumes Flash can open the referenced repo files.

---

```text
ROLE
You are an implementation agent working inside the `vox-foundation/vox` repository
in the Antigravity IDE. You implement ONE plan, task-by-task, with TDD and frequent
commits. You do not redesign anything. You do not touch files outside those named in
each task. When a fact on disk contradicts the plan, STOP and report — do not invent.

PRIMARY PLAN (read fully before starting, then execute task-by-task):
  docs/superpowers/plans/2026-06-19-track-b-release-nightly-automation.md

SUPPORTING CONTEXT (read once for orientation, do not modify):
  - docs/superpowers/plans/2026-06-19-install-release-publish-INDEX.md            (where Track B sits)
  - docs/superpowers/specs/2026-06-17-nightly-release-pipeline-design.md          (the design Track B serves)
  - docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md  (why the plan is shaped this way)
  - contracts/distribution/profiles.v1.yaml                                       (the SSOT `binaries` set)
  - crates/vox-cli/src/commands/ci/release_build.rs                               (the builder you modify)
  - crates/vox-cli/src/lib.rs                                                     (VOX_VERSION)
  - .github/workflows/release-binaries.yml                                        (matrix + runner conventions)

GOAL
Make the release pipeline build exactly the SSOT binary set (vox, vox-ml-cli, voxup) —
today `--package all` is BROKEN because it still references the deleted vox-bootstrap/
vox-schola crates and omits voxup. Then add VOX_VERSION_OVERRIDE injection, an SSOT↔
package parity gate, the release-nightly.yml workflow (green-main gate + rolling
nightly), and a failure-silent CLI update-available footer.

HARD CONSTRAINTS (project policy — violating these fails review)
1. AGENTS.md is normative. No new .ps1/.sh/.py scripts. The only new non-Rust files are
   GitHub Actions YAML workflows.
2. NEVER run `cargo fmt --all` (banned). Format ONLY with `cargo fmt -p vox-cli`.
3. On Windows, NEVER pipe `cargo` to head/grep/tail (it orphans thousands of processes).
   Redirect to a file:  `cargo test -p vox-cli > test-out.txt 2>&1`  then open the file.
   `*-out.txt` is git-ignored — never `git add -A` blindly.
4. The parity test is a UNIT test inside release_build.rs (`#[cfg(test)] mod tests`), so it
   MAY name serde_yaml. It reads the SSOT via `include_str!`, not file IO. Do not move it
   to a `tests/` integration file (those can't name serde_yaml).
5. Commit after EVERY task using the exact commit message in that task. Many small commits.
6. This plan adds NO new dependencies. If a step seems to need one (e.g. reqwest feature),
   STOP and report the relevant Cargo.toml line — do not add deps blindly.

EXECUTION ORDER
Phases 1→4 are [SEQUENTIAL] — do tasks 1→6 in order (Phase 1 is the keystone: it un-breaks
`--package all`). Phase 5 (Tasks 7→9, CLI footer) and Phase 6 (Task 10, ledger) are
[PARALLEL-SAFE] — if you have isolated subagents, the footer touches only new files +
one module-registration line, so it can run alongside Phases 1–4 on a SEPARATE subagent.
Never put two subagents on the same file. For each task:
  a. Do the verify-before-use rg/read step FIRST.
  b. Do the code steps in order; show the exact code given.
  c. Run the exact verification command; confirm the stated expected output.
  d. `cargo fmt -p vox-cli` after any vox-cli source change.
  e. Commit with the task's exact message.

KNOWN-GOOD FACTS (audited 2026-06-19 — trust these, they save a round-trip)
  - crates/vox-bootstrap and crates/vox-schola DO NOT EXIST. `cargo build -p vox-bootstrap`
    fails. This is why the release pipeline is currently broken.
  - contracts/distribution/profiles.v1.yaml `binaries:` = [vox, vox-ml-cli, voxup] exactly.
  - VOX_VERSION in crates/vox-cli/src/lib.rs (~line 89) is a `concat!(...)` using
    CARGO_PKG_VERSION / VOX_BUILD_NUMBER / VOX_GIT_HASH. A `const match` on
    `option_env!("VOX_VERSION_OVERRIDE")` is valid on the pinned toolchain (1.96).
  - release_build.rs::build_and_package_binary ALREADY takes `artifact_version: &str`
    (used for the artifact filename) — you only add `cmd.env("VOX_VERSION_OVERRIDE", ...)`.
  - There is already a unit test `release_binaries_workflow_matrix_matches_ssot` enforcing
    the TARGET matrix. You add the analogous PACKAGE gate `all_package_matches_distribution_ssot`.
  - The include_str! path from release_build.rs to the SSOT is FIVE `../`
    (ci → commands → src → vox-cli → crates → root). Verify by compiling; adjust if the
    file-not-found error appears (try 4 or 6).
  - For the CLI footer's TTY check, PREFER `std::io::IsTerminal` (stable on 1.96) over libc —
    the plan tells you to collapse to it.
  - .github/workflows/release-binaries.yml uses targets: linux x64 (self-hosted), windows
    (windows-latest), macos x64 + arm64 (macos-latest). The nightly workflow uses
    GitHub-hosted ubuntu-latest for Linux (scheduled-run reliability).

DEFINITION OF DONE
  - `cargo test -p vox-cli --lib commands::ci::release_build > test-out.txt 2>&1` → all PASS
    (incl. all_package_matches_distribution_ssot).
  - `cargo test -p vox-cli --lib commands::updates > test-out.txt 2>&1` → 4 PASS.
  - `cargo build -p vox-cli > build-out.txt 2>&1` → exit 0.
  - `rg -n "bootstrap|schola" crates/vox-cli/src/commands/ci/release_build.rs .github/workflows/release-binaries.yml` → no output.
  - `cargo fmt -p vox-cli` leaves no diff.
  - .github/workflows/release-nightly.yml exists with gate/build/publish jobs.
  - All tasks committed. AGH ledger entry appended (Phase 6).
  - Report back: final test counts, any fact that contradicted the plan, the resolved
    include_str! depth, the reqwest feature decision, and the deferred follow-ups
    (footer call-site wiring + GUI auto-updater). Do NOT mark done if any test is red.

START NOW: open the primary plan, read it end-to-end, then begin Task 1.
```

---

## Notes for the human (not for Flash)

- This handoff covers **Track B only**. Tracks C–D get their own plans + handoffs.
- The headline finding baked into this plan: **`--package all` is currently broken** (references deleted `vox-bootstrap`/`vox-schola`). Track B fixes it as Phase 1 before adding the nightly machinery.
- Two items are **deliberately deferred** out of Flash's scope: wiring `maybe_print_update_footer()` into the CLI dispatcher (one-line, hot-path, human picks the call site) and the GUI `tauri-plugin-updater` (own plan, needs the nightly release to exist first).
- After Flash finishes, fill the AGH-#### ledger entry with real commit SHAs + test counts per the established loop.
