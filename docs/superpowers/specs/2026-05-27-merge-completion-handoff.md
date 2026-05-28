---
title: "Handoff: Complete the PR #92 → main merge (2026-05-27)"
description: "Audit + execution playbook for completing the PR #92 merge into main without losing local-only commits, parallel-session WIP, or remote work. Includes verified ref deltas, predicted 21-file conflict surface, and per-file resolution direction."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-27"
training_eligible: false
training_rationale: "Operational session-pause handoff tied to a specific merge incident; not source-of-truth content."
schema_type: "TechArticle"
---

# Handoff: Complete the PR #92 → main merge (2026-05-27)

**Pause invoked at:** 2026-05-27 22:10 EDT
**Audit window covered:** 2026-05-25 16:13 EDT → 2026-05-27 22:10 EDT (~54 hours)
**Audit basis:** raw `git` verified facts only — no assumptions
**Goal state per user:** PR #92 cleanly merged into `main`; local and remote synced; no work lost from either side; conflicts resolved by intelligent feature-combining (not picking one side at random)

## §1 — Verified state right now

### 1.1 Refs

| Ref | Local tip | Origin tip | Ahead | Behind |
|---|---|---|---|---|
| `main` | `1fc21d81f7` | `13e0bc38da` | **40** | 0 |
| `cc_bdesktop2/jovial-buck-e93ac0` (PR #92) | `d55e1fe712` (my merge from 2 days ago) | `76033de293` | **76** | 0 |
| `cc_bdesktop2/naughty-dirac-825348` (PR #90) | `7ddebeecca` | `7ddebeecca` | 0 | 0 |

**Three local-only realities to harmonize:**

1. **40 commits on local `main` that aren't on origin.** Substantive feature work (see §1.2).
2. **76 commits on PR #92 branch that aren't on origin's PR #92 branch.** This is the merge commit `d55e1fe712` (which itself includes 75 commits from origin/main as of 2 days ago, plus my own work).
3. **My merge commit `d55e1fe712` is now STALE** — it was based on main at `9f0a54f80e`, but main has moved by 39 more commits on remote and 40 more on local. The merge would need to be re-done or extended.

### 1.2 Local `main` unpushed work (40 commits)

Categories of work in the 40 unpushed local main commits:

| Category | Examples (most recent first) |
|---|---|
| **vox-arch-check perf** | `1fc21d81f7` Phase 2 cache, `f8bf3160f3` eliminate redundant cargo metadata call, `46a945ff77` Phase 1 build jobs tuning |
| **Test-suite tooling** | `490b6ce3ee` test-baseline.v1 schema, `d2c85564af` test-suite perf+gate tiers plan, `8545a8b08c` test-suite perf design |
| **CR-L corpus work** | `52484bb3b7` CR-L4 plan-fidelity corpus, `dd2155e5e5` CR-L3 repair corpus (15 fixtures, 5 bug classes), `1ce450acf0` CR-L8 corpus-feedback 2026-Q2 |
| **Phase D telemetry** | `eec63fa6e5` org-policy hard-off + build failure telemetry, `03311b3f3e` retry_attempt + per-fallback ErrorEvents, `3d427589fa` StderrDebugSink |
| **Eval / typeck** | `96d8529786` TupleLit + DecimalLit, `ee02ea58a6` `?` operator, `899cf5ebd0` regex.find typeck, `8d045d7d60` regex namespace (is_match/captures/find), `fca689a833` pop() fix + dict/Map + 20 stdlib methods, `f32b72d2b7` chr() + complete HumanEval to 164/164 |
| **CR-L1 humaneval** | `0937627004` real harness P2.3, `d7e85e8f09` MV corpus, `6dd301701f` print/join + expand humaneval to 100, `b6ed82509c` 3 interp bug fixes + 75 problems |
| **V0.6 fixes** | `b0fdfc36ce` CR-L5 serde flip + CR-L6 retirement-audit + humaneval corpus hash + A-5 arity fix, `11e10843e2` CR-L1 count 164 + CLI catalog baseline |
| **Parser** | `f9bf7ec5fa` `_` wildcard in for-in + idiomatic range() patterns |
| **Marquee** | `f5d95032dd` P3.5 slot-2 + slot-3 real fixtures (CR-P1) |
| **Various** | `2f024fd0d6` golden corpus updates, `8ba4b3d45c` close 19 golden-corpus gaps → 62/62 |

**No subset of this work can be safely dropped.** Every commit is forward-progress and forms a coherent stack.

### 1.3 Origin `main` work not yet in my PR's merge base

39 commits on origin/main since my last merge base (`9f0a54f80e`), of which the most consequential for the upcoming merge:

- `b81ef6991e fix(v0.6): complete @endpoint hard-remove across all test fixtures and contracts` — **@endpoint retirement is now LANDED.** PR #92 had `@endpoint(kind: query)` style markers in places; those are now invalid main-side.
- `13e0bc38da feat(vox-cli): add 'vox check --strict' flag (CR-L2 gate)`
- `fba8371661 feat(vox-code-audit): Phase J.19 — import_cycles CR-L gate (rule 51)`
- `2326c8245f docs(ssot): mark Phase G, J, E, F complete; update marquee manifest`
- `201d39d4dc feat(phase-ef): lock 53/53 script corpus baseline; Phase E+F complete`

These ARE in local `main` already (since local is 0 behind remote). They just aren't in PR #92's branch yet.

### 1.4 Main tree's uncommitted state (`C:/Users/Owner/vox`)

**25 files modified/added uncommitted.** This is parallel-session WIP that should be cleared (committed or stashed) before any merge into main.

Notable items:

- `M crates/vox-arch-check/src/forbidden_patterns.rs` + `M crates/vox-arch-check/src/main.rs` + `A crates/vox-arch-check/tests/helpers/{fixture,mod}.rs` + `M crates/vox-arch-check/tests/integration.rs` — vox-arch-check test refactor in progress
- `M Cargo.lock` — dep state churn
- `M crates/vox-cli/Cargo.toml` + `M crates/vox-cli/src/commands/ci/{cmd_enums,pre_push,run_body}.rs` — vox-cli ci command changes
- `A contracts/reports/retirement/2026-05-28.json` — already a 2026-05-28-dated report (the parallel session is operating ahead of today's date according to my audit clock; or testing report-name normalization)
- `A docs/src/architecture/_screenshots/mobile-bakeoff-2026/research-sources.md` + `A docs/src/architecture/mobile-target-evaluation-2026.md` — mobile target evaluation in progress
- `A CUsersOwnervoxtmp_print_test.vox` — looks like a Windows-pathing accident; should be removed
- `A .claude/scheduled_tasks.lock` — Claude harness lock; usually `.gitignore`'d but somehow tracked

### 1.5 PR mergeable status (per `gh pr view`)

- **PR #92**: `mergeStateStatus: DIRTY, mergeable: CONFLICTING` — GitHub's view of the PR (head still at `76033de293` because we haven't pushed `d55e1fe712`)
- **PR #90**: `mergeStateStatus: DIRTY, mergeable: CONFLICTING` — substantially stale
- PR #70 (mental-tracker), PR #68 (vox-mobile): still open, still stale (2026-05-08); out of scope here

## §2 — Predicted conflict surface for the PR #92 merge

**21 files** will conflict when merging current `main` into PR #92 branch (or vice versa). Run:

```bash
git merge-tree --write-tree --messages d55e1fe712 main | grep "^CONFLICT"
```

### 2.1 Conflict inventory with resolution direction

| # | File | Conflict type | Recommended resolution | Why |
|---|---|---|---|---|
| 1 | `AGENTS.md` | content | Take main (post-@endpoint retirement) + carry forward any PR-only sections that don't overlap | Main has the latest @endpoint hard-remove documentation; PR is from before that landed |
| 2-3 | `apps/marquee/chat/{Vox.toml, src/main.vox}` | add/add | Take main (it has the P3.5 slot fixtures) | PR added these as scaffolds; main now has real fixtures (commit `f5d95032dd`) |
| 4-5 | `apps/marquee/todo-auth/{Vox.toml, src/main.vox}` | add/add | Same as above — take main | Same rationale |
| 6 | `contracts/eval/humaneval-vox/manifest.v1.yaml` | content | Take main (164/164 manifest from `f32b72d2b7`) | Main expanded humaneval to 164 problems; PR is from earlier |
| 7 | `contracts/eval/plan-fidelity/manifest.v1.yaml` | content | Take main (`52484bb3b7` plan-fidelity corpus) | Main has the CR-L4 minimum-viable; PR predates |
| 8 | `contracts/eval/repair-corpus/manifest.v1.yaml` | content | Take main (`dd2155e5e5` repair-corpus 15 fixtures) | Main has CR-L3; PR predates |
| 9 | `contracts/marquee/manifest.v1.yaml` | content | Take main (`2326c8245f` Phase J marquee manifest update) | Main has the latest manifest |
| 10 | `contracts/reports/corpus-feedback/2026-Q2.json` | add/add | Take main (`1ce450acf0` Q2 report) | Main has the bootstrap report |
| 11 | `crates/vox-audit/src/lib.rs` | content | **HYBRID** — PR's `same_day_canonical_with_panel` (7-day lookback) is the documented fix for evidence preservation. Take PR's version BUT verify main hasn't refactored elsewhere in the file. | Subtle: main may have moved this code. Read both sides before resolving. |
| 12 | `crates/vox-audit/src/subcommands/humaneval.rs` | add/add | Take main (`0937627004` real harness P2.3) | Main has the actual implementation; PR may have a stub or older variant |
| 13 | `crates/vox-audit/src/subcommands/mod.rs` | content | **HYBRID** — both sides added pub mod lines; union both sets | Standard pattern from prior merge — see [`2026-05-24-multi-agent-work-loss-audit.md`](2026-05-24-multi-agent-work-loss-audit.md) precedent |
| 14 | `crates/vox-audit/src/subcommands/stubs.rs` | content | Take main if it has retired the stub for `humaneval` (since main moved it to real impl) | Read the diff; align with whichever side has the cleaner stub set |
| 15 | `crates/vox-compiler/src/lexer/token.rs` | content | **HYBRID** — both sides likely added different new tokens. Union the additions; verify no overlapping name | Common pattern: lexer changes from independent feature branches |
| 16 | `crates/vox-compiler/src/parser/descent/decl/head.rs` | content | **HARD** — `@endpoint` retirement on main likely touched this. PR may have `@endpoint(kind: ...)` parse logic that's been fully removed on main. Take main's version (the retirement is final). | User directive established: `@endpoint` is being retired. Main is the canonical post-retirement state. |
| 17 | `crates/vox-compiler/src/parser/descent/mod.rs` | content | Take main (alignment with #16) | Same retirement context |
| 18 | `crates/vox-compiler/src/typeck/checker/expr.rs` | content | **HARD** — likely overlapping changes in typeck. Read both sides. If conflict is in `@endpoint` typeck logic, take main. Otherwise hybrid based on the specific lines. | Compiler hot spot from prior merge precedent |
| 19 | `crates/vox-compiler/src/typeck/diagnostics.rs` | content | **HYBRID** — both sides likely added new diagnostic codes. Union the additions | Standard pattern |
| 20 | `crates/vox-compiler/tests/snapshots/diagnostic_snapshots__rust_import_dup_diagnostic_payload_snapshot.snap` | content | Take main — snapshot files reflect ground truth of current compiler output. If main's snapshot is the current accepted state, PR's is stale. | Re-run snapshots after the merge to verify |
| 21 | `docs/src/architecture/v1-release-criteria.md` | content | **HYBRID** — both sides likely added CR-L/CR-A status updates. Hand-merge to preserve both sets of completion notes | Both sides have been making real progress; both deserve credit in the file |

### 2.2 Conflict count comparison

| Merge | Date | Conflict count | My then-conflict-resolutions |
|---|---|---|---|
| First merge (`40a7985455`) | 2026-05-23 | 18 files | Documented in [`2026-05-24-multi-agent-work-loss-audit.md`](2026-05-24-multi-agent-work-loss-audit.md) |
| Second merge (`d55e1fe712`) | 2026-05-25 | 7 files | Documented in merge commit body |
| **Third merge (pending)** | 2026-05-27 (now) | **21 files predicted** | This handoff doc's §2.1 |

The conflict count is growing because main is moving fast and PR #92 has been static for 2+ days.

## §3 — What's changed since the previous handoff

The prior handoff (`docs/superpowers/specs/2026-05-25-pr-harmonization-pause-handoff.md`, my commit `76033de293`) is now superseded. Specifically:

| Prior claim | Now |
|---|---|
| "Main: 22 commits unpushed" | 40 commits unpushed (parallel session added 18 more) |
| "PR #92 vs main: ~7 file conflicts" | PR #92 vs main: 21 file conflicts |
| "Predicted Cargo.lock, vox-arch-check, builtin_registry, …" conflicts | Those resolved in `d55e1fe712`; NEW conflicts now in `apps/marquee/*`, audit subcommands, compiler parser (post-@endpoint retirement), eval manifests |
| "User said: real-code over vox:skip" | Still applies, but the specific blocks affected may have moved |
| "User said: @query canonical, prefer main where in doubt" | **CONFIRMED on remote** — `b81ef6991e` is the full `@endpoint` hard-remove. User's prior direction now materialized as code on main. |
| "PR #90: 9 ahead, 269 behind" | Still 9 ahead, but more behind now (~300+); same conflict situation magnified |
| "Stale PRs #70, #68" | Still stale, no movement |

## §4 — Missing context to proceed safely (FLAG before executing)

These questions need answers from the human or from inspecting other-session artifacts before executing the merge:

1. **Is the parallel session done with the 40 local-only main commits?** If not, they may add commits 41, 42, … while the merge is in flight. Watch for `git fetch origin` showing main has advanced beyond what we captured.
2. **Should main tree's 25 uncommitted files be committed before pushing main?** Some look like intentional WIP (vox-arch-check tests, mobile-target eval doc). Others look accidental (the `CUsersOwnervoxtmp_print_test.vox` Windows-pathing artifact; possibly `.claude/scheduled_tasks.lock`).
3. **Discard my old merge commit `d55e1fe712`, or build on top of it?** Both are technically viable (see §5). A clean restart from PR #92's pre-merge state (`7b94673241`) gives a fresh perspective; building on `d55e1fe712` preserves the 7 resolutions I already did.
4. **Is the user driving PR #90 themselves, or is it owned by another agent?** PR #90's content (domain rename + scripts/show) is orthogonal to PR #92. Same merge dance needed independently.
5. **MENS tasks (#2, #3)** — still pending from the original task list. Out of scope for this merge but should be retired or re-prioritized after the merge lands.

## §5 — Recommended execution plan

Two viable strategies. **Recommend Strategy A** (clean restart) for clarity.

### Strategy A: Discard `d55e1fe712`, fresh merge from PR's pre-merge tip

This is cleaner because most of `d55e1fe712`'s value (the 75 origin/main commits it pulled in) is already in local main via the parallel session's pulls. The 7 conflict resolutions inside `d55e1fe712` are worth preserving as REFERENCE (commit body has them documented) but redoing them against the new main is OK.

**Phase 0 — Stabilize main tree (do this from the main tree, not PR worktree):**

```bash
cd /c/Users/Owner/vox

# Review the 25 uncommitted files
git status

# Decide: commit each, stash each, or rm each
# Suggested:
#   - Commit the intentional WIP (vox-arch-check tests, mobile-target doc, etc.) as separate commits with clear messages
#   - rm the accidental Windows-pathing artifact (CUsersOwnervoxtmp_print_test.vox)
#   - decide what to do with .claude/scheduled_tasks.lock (probably gitignore it then remove from index)

# Confirm clean:
git status   # expect: nothing modified except untracked artifacts
```

**Phase 1 — Push local main (40 commits) to origin:**

```bash
# from the main tree
cd /c/Users/Owner/vox
git push origin main
# Pre-push hooks will run. If any fail, address before continuing.
# 40 commits is a lot but fast-forward; no force needed since behind=0.
```

**Phase 2 — Reset PR #92 branch to pre-merge state, then fresh merge:**

```bash
# from the PR worktree
cd /c/Users/Owner/vox/.claude/worktrees/jovial-buck-e93ac0

# Confirm current state
git rev-parse HEAD          # d55e1fe712 (the stale merge to discard)
git log --oneline -3
# d55e1fe712 merge: integrate origin/main into PR #92 (harmonization pass 2)
# 76033de293 docs(handoff): PR #92/#90 harmonization pause snapshot
# 9f0a54f80e <main commit, the merge base>

# Reset to the pre-merge tip (preserve the handoff doc commit):
git reset --hard 76033de293

# Now merge in the now-current main (which has all the work):
git fetch origin
git merge origin/main --no-commit --no-ff
# Expect: 21 conflicts per §2.1
```

**Phase 3 — Resolve the 21 conflicts using §2.1's direction:**

For each conflict, follow §2.1's recommended resolution. The pattern from prior merges:

- For "Take main" conflicts: `git checkout --theirs <file>` then `git add <file>`
- For "Take PR" conflicts: `git checkout --ours <file>` then `git add <file>`
- For HYBRID conflicts: open in editor, union additions, ensure no semantic regressions

Stop at any conflict that's genuinely ambiguous (where §2.1 says HARD); surface it for review before continuing.

**Phase 4 — Verify build + commit the merge:**

```bash
cargo check --workspace --quiet 2>&1 | grep -E "^error" | grep -v "vox-gui\|frontendDist"
# Expect: no errors (vox-gui Tauri issue is unrelated environment problem)

git commit -m "merge: integrate origin/main into PR #92 (harmonization pass 3)" \
           -m "<resolution notes per §2.1>"
```

**Phase 5 — Push PR + merge via GitHub:**

```bash
git push origin cc_bdesktop2/jovial-buck-e93ac0
# Wait for GitHub to recompute mergeability (~30s)
gh pr view 92 --repo vox-foundation/vox --json mergeable,mergeStateStatus

# Once MERGEABLE:
gh pr merge 92 --repo vox-foundation/vox --merge --delete-branch=false

# Pull origin/main into local main:
cd /c/Users/Owner/vox
git pull origin main --ff-only
```

### Strategy B: Build on top of `d55e1fe712`

Same outcome, different mechanic: `git merge origin/main` from the current d55e1fe712 tip. Resolves the 21 conflicts in a SECOND merge commit on top of the existing merge.

**Pros:** preserves the 7 conflict resolutions from the prior merge as their own commit
**Cons:** Two merge commits in PR #92's history; messier graph

If you take this path, the resolution direction in §2.1 still applies — just over a different base.

## §6 — Conflict-resolution playbook (rapid execution)

For each file in §2.1, here's the exact mechanic:

```bash
# Files where main wins outright (most of #2.1):
for f in AGENTS.md \
         apps/marquee/chat/Vox.toml \
         apps/marquee/chat/src/main.vox \
         apps/marquee/todo-auth/Vox.toml \
         apps/marquee/todo-auth/src/main.vox \
         contracts/eval/humaneval-vox/manifest.v1.yaml \
         contracts/eval/plan-fidelity/manifest.v1.yaml \
         contracts/eval/repair-corpus/manifest.v1.yaml \
         contracts/marquee/manifest.v1.yaml \
         contracts/reports/corpus-feedback/2026-Q2.json \
         crates/vox-audit/src/subcommands/humaneval.rs \
         crates/vox-compiler/src/parser/descent/decl/head.rs \
         crates/vox-compiler/src/parser/descent/mod.rs \
         crates/vox-compiler/tests/snapshots/diagnostic_snapshots__rust_import_dup_diagnostic_payload_snapshot.snap ; do
  git checkout --theirs "$f" && git add "$f"
done

# HYBRID files (inspect each):
for f in crates/vox-audit/src/lib.rs \
         crates/vox-audit/src/subcommands/mod.rs \
         crates/vox-audit/src/subcommands/stubs.rs \
         crates/vox-compiler/src/lexer/token.rs \
         crates/vox-compiler/src/typeck/checker/expr.rs \
         crates/vox-compiler/src/typeck/diagnostics.rs \
         docs/src/architecture/v1-release-criteria.md ; do
  echo "=== $f ==="
  # open editor, hand-merge per §2.1 guidance
done
```

## §7 — Items to prune from the prior session's task list

Several items are now stale and should be retired before continuing:

1. **Task #4 "Integrate origin/main into PR #92 (strategy TBD)"** — was marked completed for pass-2 (`d55e1fe712`); now needs reopening as pass-3. Rename to "Complete PR #92 merge via Strategy A/B (§5)".
2. **Prior handoff at `2026-05-25-pr-harmonization-pause-handoff.md`** — superseded by this doc.
3. **"Plan C: push main → GH PR-merge handles it"** — partially still relevant (Phase 1 + 5 of §5 align with this), but the "GH PR-merge handles it" claim is now invalid since PR #92 will still need conflict resolution after main is pushed.
4. **MENS tasks #2 and #3** — still valid but explicitly OUT of scope for this merge. Defer to a focused MENS session after the merge lands.

## §8 — Critical risk register

1. **Parallel session may still be committing.** The 40 local-only main commits are recent (most recent: `1fc21d81f7` at 2026-05-27 21:48, ~22 min before this audit). They may add commit 41 mid-merge. Mitigation: re-`git fetch origin` after any pause; check for new commits before pushing.
2. **The `@endpoint` retirement is irreversible.** Main commit `b81ef6991e` deleted `@endpoint` from all test fixtures and contracts. PR #92 references to `@endpoint(kind: query)` in any form WILL conflict and main wins (per user authority).
3. **Snapshot tests may need re-baselining.** PR #92 has its own snapshots; main has its own. After resolution, run `cargo test --workspace -- --include-ignored` and update snapshots that drift; commit those updates as a separate "test: rebaseline snapshots after harmonization pass 3" commit.
4. **Cwd-bug from prior sessions can recur.** Always chain `cd <worktree-path> && <git cmd>` per the precedent in [`2026-05-24-multi-agent-work-loss-audit.md`](2026-05-24-multi-agent-work-loss-audit.md) §9. Never trust Bash's cwd persistence between tool calls.
5. **Pre-push hooks may fail on `main` push.** 40 commits at once is a large batch. If pre-push lint or test-runner trips, the symptom is a `gh push` exit non-zero. Triage by fixing forward (additional commit on main) rather than amending/force-pushing.

## §9 — Highest-value next steps (prioritized)

1. **Resolve Phase 0** (main tree's 25 uncommitted files). Without this, the merge can't safely complete. ETA: 15-30 min depending on what's intentional vs. accidental.
2. **Phase 1 push of local main** (40 commits). Gets remote and local main aligned; gives PR #92 a stable target. ETA: 5 min + pre-push hook duration.
3. **Phase 2-5 of Strategy A** (the full PR #92 merge). ETA: 1-3 hours depending on how many of the 21 conflicts turn out to be HARD.
4. **PR #90 hand-off** to its owner (a different agent) — write a `gh pr comment` on PR #90 pointing them at this handoff for the same mechanic.
5. **Stale PR closure** — close PRs #70 and #68 with a comment explaining why (out of date, work since landed elsewhere, or owner decision).
6. **Resume MENS work** (Tasks #2, #3) as a dedicated follow-up session.

## §10 — Files referenced in this handoff

- [`docs/superpowers/specs/2026-05-24-multi-agent-work-loss-audit.md`](2026-05-24-multi-agent-work-loss-audit.md) — original forensic record
- [`docs/superpowers/specs/2026-05-25-pr-harmonization-pause-handoff.md`](2026-05-25-pr-harmonization-pause-handoff.md) — prior handoff (now superseded by this doc)
- This file: `docs/superpowers/specs/2026-05-27-merge-completion-handoff.md`

---

**Authoring note:** This handoff doc is on PR #92's branch
(`cc_bdesktop2/jovial-buck-e93ac0`). It will land on main when PR #92
merges (via either Strategy A or B). Until then, it lives as part of
the PR's contents and is visible at the PR's "Files changed" view on
GitHub.
