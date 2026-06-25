# Untangle & Land the `voxmens-split-c-followups` Divergence

> **Status:** Remediation plan. Source: AGH-0012 review found the branch is a multi-session kitchen-sink, arch-check RED, 281 commits ahead of `origin/main` with nothing merged.
> **Handoff model (audited against [`gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md)):** this plan is **split by who can safely do each part**. Gemini 3.5 Flash under Antigravity executes ONLY the **mechanical, atomic, verifiable** tasks tagged **`[GEMINI-SAFE]`** (commit known file-sets, run gates, emit a report). Every task that requires **judgment** — deciding which of 281 commits belong to which initiative, conflict resolution, integration-strategy choice, entanglement teardown — is tagged **`[STOP-HANDBACK]`**: Gemini must NOT attempt it; it stops and emits the §Z handback for a human/Claude. Rationale: the limitations doc forbids open-ended reasoning, long-horizon judgment, and no-checkpoint surgery from this model — git history surgery on a kitchen-sink is the worst case for all three.

## A0. Gemini execution rules (for the `[GEMINI-SAFE]` tasks only)
- **Prerequisite (human, before dispatch):** confirm concurrent sessions are quiesced and the tree is stable. Gemini cannot judge this; do not dispatch onto a live tree.
- Atomic-green-commit; verify-before-use (`rg`/`git status` before any path op); two-strike then STOP+handback.
- **PATH-SCOPED git only.** NEVER `git add -A`/`git add .` — the index may hold foreign staged files. Stage by explicit path; if `git status` shows unexpected staged files you did not create, **STOP-HANDBACK** (do not commit them).
- **No history rewrite** (`rebase`, `reset --hard`, `push --force`, `filter-branch`) — all of that is `[STOP-HANDBACK]`. Gemini's git verbs are limited to: `status`, `log`, `diff`, `add <path>`, `commit`, `bundle`, `tag`, `cherry`.
- **Never weaken a gate** to force-green (`forbidden_pattern` stays `error`; no `--warn-only`/`|| true`). If a gate is red for reasons your task didn't introduce → STOP-HANDBACK.
- On reaching ANY `[STOP-HANDBACK]` task, or on a two-strike failure, emit the §Z handback and halt.

**Goal:** Get the work on `voxmens-split-c-followups` safely into `origin/main` — first by making the branch genuinely green, then by re-slicing the 281-commit divergence into reviewable, thematic PRs (with the VoxMens hub-and-spoke as one clean slice), without losing any work or breaking `main`.

## 0. The real state (audited 2026-06-19)
- `origin/main` contains **none** of: Plan A (`domain_profiles` base/method/router), Plan B (`spoke_base_resolver`, `train_bases`, `route_by_signal`), the Split B **arch-check guard engine** (`exempt_tests`/`cfg_test_line_mask`), or F1 (`training_selection`).
- The branch is **281 commits ahead of `origin/main`**; **40 files dirty**, **33 staged** in the index by concurrent sessions; `agy_ledger.rs` being live-written.
- **arch-check is RED**: 27 `forbidden_pattern` violations (25 test-code false-positives the guard engine would clear + 2 real). The branch has the guard *config* (`exempt_tests = true` in `layers.toml`) but not the *engine* code.
- Multiple initiatives are interleaved on this one branch: VoxMens hub-and-spoke (A→B→C→F1 + guard), model-pool, soft-HITL, telemetry, agy delegation, GUI, etc. Shared files (`pipeline.rs`, `layers.toml`, `domain_profiles.rs`, `GEMINI.md`) are edited by several.

**Implication:** a per-commit cherry-pick of "the VoxMens commits" onto `main` is infeasible — the VoxMens files depend on non-VoxMens changes to shared files. The branch must be stabilized as a whole, then re-sliced.

## A. Operating rules
- **Never operate on a moving tree.** Phase 0 freezes concurrent sessions first; nothing else proceeds until `git status` is stable.
- **Back up before any history surgery** (Phase 0): a bundle + tag, per the repo's established practice (`preserve/local-main-*` tags + `*.bundle`). Losing 281 commits of unmerged work is the worst case — guard against it first.
- **Never weaken a gate** to force-green (the AGH-0008 / AGH-0012 lesson). arch-check must be made green by *fixing*, not downgrading `forbidden_pattern`.
- **No `git push --force` to `main`**; integration is via PRs.
- Prefer additive, reversible steps; verify after each.

## Phase 0 — Freeze & back up (do FIRST)  [0.1 HUMAN PREREQ; 0.2-0.3 GEMINI-SAFE]
- [ ] **0.1 Quiesce concurrent sessions.** Confirm no other agent/session is committing to `voxmens-split-c-followups` (check `git status` twice, 30s apart — file set must be identical). If still moving, STOP and coordinate; do not proceed on a live tree.
- [ ] **0.2 Snapshot the index/worktree.** The index has 33 staged files from other sessions. Decide with the owner whether those staged changes are intended commits; if unowned, `git stash --staged`-equivalent is unsafe across sessions — instead just **back up the whole state**:
  - `git bundle create ../voxmens-untangle-20260619.bundle --all`
  - `git tag preserve/voxmens-split-c-followups-20260619` (annotated).
  Record both paths. This is the rollback anchor.
- [ ] **0.3 Capture the dirty/staged file list** to a scratch file (`git status --short > /tmp/untangle-status.txt`) so nothing is lost track of.

## Phase 1 — Make the branch genuinely green (no gate-weakening)  [GEMINI-SAFE]
The branch can't be merged or sliced while arch-check is red. Land the missing guard work that's already (uncommitted) in the working tree.
- [ ] **1.1 Commit the guard fixes PATH-SCOPED** (do NOT `git add -A` — the index has 33 foreign staged files). Stage only:
  - `crates/vox-arch-check/src/forbidden_patterns.rs` (the `exempt_tests` engine + `cfg_test_line_mask` + its unit test)
  - `crates/vox-cli/src/commands/graphify/mod.rs` (raw-git → `vox_git::read_only`)
  - `crates/voxup/src/shell.rs` (annotate the cfg(windows) powershell spawn)
  Then `git commit` (only those three). *(If a concurrent session already committed equivalents, skip — verify with `git status` on those paths first.)* These are the Split B guard work missing on this branch (equivalent to `ccc37615f7`).
- [ ] **1.2 Confirm `forbidden_pattern = "error"` and `exempt_tests = true`** survive in `layers.toml` (concurrent edits kept toggling these). If a concurrent edit removed them, re-add — do not leave `error` without the engine, or vice-versa.
- [ ] **1.3 arch-check green:** `CARGO_TARGET_DIR=target/iso cargo run -p vox-arch-check` → **0 `[ERROR]`** (warnings OK), exit 0. If violations remain, fix the root cause (annotate cfg-gated sites, route raw execs); never downgrade the guard.
- [ ] **1.4 Build + test gate:** `cargo check --workspace` (or the changed crates) compiles; `cargo test -p vox-populi -p vox-ml-cli` green; `vox ci spoke-check` exit 0. Paste outputs. This establishes a trustworthy "branch is green" baseline — the thing the AGH-0012 report claimed but couldn't back.

## Phase 2 — Choose the integration strategy  [STOP-HANDBACK: judgment]
281 commits / multiple initiatives is too large for one review. Two viable paths (pick with the owner):
- [ ] **2.A Re-slice into thematic PRs (recommended).** Reuse the prior playbook ([memory] "PR re-slice → 9-PR stack #377-385"): group the divergence by initiative (VoxMens hub-and-spoke, model-pool, soft-HITL, telemetry, agy, GUI, …) into <140-file thematic PRs off `origin/main`. Each PR must build + pass CI independently. Note: CodeRabbit skips stacked PRs (manual `@coderabbitai review`); intermediate PRs may not build alone → plan FF-merge order.
- [ ] **2.B Single integration merge.** If the team accepts one large integration: open ONE PR `voxmens-split-c-followups → main`, rely on the Phase 1 green gate + full CI. Faster, far harder to review; only if the divergence is genuinely one coherent release.

**Recommendation:** 2.A. 281 unmerged commits across ≥5 initiatives is exactly the kitchen-sink anti-pattern the repo has paid for before.

## Phase 3 — Produce the clean VoxMens slice  [STOP-HANDBACK: entanglement teardown]
Within 2.A, the VoxMens hub-and-spoke is one slice. Build it as a focused, self-contained PR:
- [ ] **3.1 Identify the VoxMens commit set.** `git log origin/main..HEAD --oneline -- mens/ crates/vox-populi/src/mens/ crates/vox-ml-cli/src/commands/mens/ crates/vox-corpus/ docs/superpowers/plans/2026-06-1*-voxmens-* docs/superpowers/specs/2026-06-19-voxmens-* docs/superpowers/antigravity-handoff-ledger.md crates/vox-arch-check/src/forbidden_patterns.rs` — this is the candidate slice (Plan A, B, convergent C, F1, guard, ledger). Expect interleaving with shared-file edits.
- [ ] **3.2 Assess shared-file entanglement.** For `pipeline.rs`, `domain_profiles.rs`, `layers.toml`, `mod.rs` — do the VoxMens edits depend on non-VoxMens edits in the same files? If yes (likely), a pure cherry-pick won't apply cleanly. Prefer a **squash-style reconstruction**: branch off `origin/main`, bring the *final* VoxMens file states via `git checkout voxmens-split-c-followups -- <voxmens paths>`, then `cargo check` and resolve the deps that the VoxMens files need (any shared types/crates introduced by other initiatives must either be included or stubbed out of the VoxMens slice).
- [ ] **3.3 Make the slice compile + pass on its own** off `origin/main`: `cargo test -p vox-populi -p vox-ml-cli`, `vox ci spoke-check`, `cargo run -p vox-arch-check` all green. If the VoxMens files pull in a non-VoxMens dependency (e.g. a model-pool type), decide: include that dependency's commit in the slice, or refactor the coupling out. Document any such coupling.
- [ ] **3.4 Open the VoxMens PR** with the AGH-0012 ledger entries as the narrative; request review.

## Phase 4 — Land the remainder + close out  [STOP-HANDBACK: PR + judgment]
- [ ] **4.1** Work the other thematic slices (model-pool, soft-HITL, telemetry, agy, GUI) the same way, FF-merging in dependency order.
- [ ] **4.2** Once all slices are merged, confirm `origin/main` == the union (no lost commits): `git cherry origin/main voxmens-split-c-followups` should show nothing unique remaining (or only intentionally-dropped commits).
- [ ] **4.3** Delete the kitchen-sink branch only after the backup bundle (0.2) is confirmed and all work is on `main`. Add an AGH ledger note recording the untangle.
- [ ] **4.4 Prevent recurrence:** institute branch isolation going forward (one initiative per branch off current `origin/main`) — this is ledger §B-3, repeatedly violated. Consider a CI/pre-push warning when a branch exceeds N commits or N initiatives ahead of `main`.

## B. Green boundary (untangle complete)
- Backup bundle + tag exist (0.2).
- The branch is genuinely arch-check-green (guard engine landed, no gate weakened) + tests/spoke-check green (Phase 1).
- The divergence is merged to `origin/main` — as thematic PRs (2.A) or one integration PR (2.B) — with the VoxMens hub-and-spoke as a clean, self-contained slice (Phase 3).
- `git cherry` confirms no work lost; kitchen-sink branch retired; recurrence guard noted (4.4).

## Z. Handback statement (Gemini emits this; USER copy-pastes it to the reviewer)
On completion of the `[GEMINI-SAFE]` tasks, OR on reaching ANY `[STOP-HANDBACK]` gate, OR on a two-strike failure: emit EXACTLY the block below, filled in, as your FINAL message. Paste real command output/exit codes — do not summarize. The user copy-pastes this back to Claude Code for review.
```
=== VOXMENS UNTANGLE — HANDBACK (paste to reviewer) ===
plan: docs/superpowers/plans/2026-06-19-voxmens-branch-untangle.md
status: completed-gemini-safe | stopped-at <task id> | two-strike-stop <task id>
branch: <name>   ahead_of_main: <git rev-list --count origin/main..HEAD>
backup (Phase 0.2, REQUIRED before any commit): bundle=<path>  tag=<name>
commits_made (sha + subject): <list or NONE>
files_committed (path-scoped — list EVERY one): <list or NONE>
unexpected_staged_files_seen: <list or NONE>   # if any, you must NOT have committed them
gates (paste real exit/counts, not prose):
  arch-check: exit=<n>  forbidden_pattern_violations=<n>
  cargo test -p vox-populi -p vox-ml-cli: <N passed / M failed>
  vox ci spoke-check: exit=<n>
gate_weakening_done: NONE        # must be NONE — if not, say exactly what and why
judgment_gate_reached: <which [STOP-HANDBACK] task + why you stopped, or NONE>
two_strike_failures: <task + what failed twice, or NONE>
open_questions_for_reviewer: <list or NONE>
=== END HANDBACK ===
```

## C. Scope NOT included
- Building a `vox ci handoff-ledger` lint or a "branch divergence" CI gate (4.4 is a recommendation; implement under its own initiative).
- Re-running a real `--features gpu` micro-train (the GPU-free effect-proof from the F1 plan stands as the CI substitute).
