# Repo Consolidation → One Clean `main` (branches, stashes, PRs)

> **Operator: Claude Code (Opus) or a human — NOT Gemini/a weak model.** This is judgment-heavy release surgery (base selection, per-conflict winner picks, triage verdicts, PR slicing). The limitations doc forbids exactly this from a weak model. Only the *gate runs* (build/test/arch/CI) are mechanically delegable.
> **Design source:** approved brainstorm (this session). **Status:** implementation plan.

**Goal:** Collapse the entire unmerged divergence — **30+ local / 122 remote branches, 48 stashes, open PRs** — into **one clean `main` synced to remote**, every change reviewed by CodeRabbit and landed as **separation-of-concerns PRs at ≤~140 files each**, with **last-3-day work prioritized**, and all consolidated branches/stashes/PRs retired. No work lost (everything bundled first).

**Architecture:** Audit everything → build the **union on an `integration/` branch** (one base + layered LIVE branches + recovered stashes, per-conflict picks surfaced to the owner) → make it fully green → **re-slice the integration→main diff into ≤140-file thematic PRs** → CodeRabbit-review + merge each in dependency order → retire branches/stashes/PRs → install a recurrence guard.

**Tech stack:** `git`, `gh` (GitHub CLI), `vox ci` gates, `cargo`, CodeRabbit (`@coderabbitai review`).

---

## A. Operating rules (non-negotiable)
- **Freeze first (Phase 0). Never operate on a moving tree.** The owner confirmed the swarm is stopped — re-verify with two `git status` samples 30s apart before each destructive phase.
- **Back up before ANY deletion or history op.** Bundle `--all` + tag every branch + snapshot every stash to a ref. Nothing is deleted until it's in the bundle AND confirmed merged.
- **Never weaken a gate** (`forbidden_pattern` stays `error`; no `--warn-only`/`|| true`/`--no-verify`). Green = fix, not silence (AGH-0008 lesson).
- **No `git push --force` to `main`.** main only advances via reviewed PR merges.
- **Path-scoped git only** where a working tree is dirty; never `git add -A` into a foreign index.
- **Verify after every step**; one logical change at a time; frequent commits on the integration branch.
- **≤~140 files per PR**; CodeRabbit skips stacked PRs → trigger manually with `@coderabbitai review` on each.

## Phase 0 — Freeze & TOTAL backup
- [ ] **0.1** Confirm swarm stopped: `git status` identical across two samples 30s apart; no branch-switch under you. If moving, STOP.
- [ ] **0.2 Bundle everything:** `git bundle create ../vox-consolidation-<date>.bundle --all`. Verify: `git bundle verify ../vox-consolidation-<date>.bundle`. Record the path. **This is the master archive.**
- [ ] **0.3 Tag every branch:** for each local+remote branch, `git tag preserve/<sanitized-branch>-<date> <branch>`. Script it; confirm `git tag -l 'preserve/*' | wc -l` ≥ branch count.
- [ ] **0.4 Snapshot every stash to a ref** (stashes are NOT in `--all` by default once dropped): for each `stash@{N}`, `git tag preserve/stash-<N>-<date> stash@{N}` (a stash is a commit-ish). Now `git bundle ... --all` re-run includes them, OR they're tag-anchored. Re-run 0.2 after tagging so the bundle contains the stash refs. Confirm 48 stash tags exist.
- [ ] **0.5** Record the inventory to `graphify-out/consolidation/inventory.txt`: `git stash list`, `git for-each-ref refs/heads refs/remotes`, `gh pr list --state open`.

## Phase 1 — Audit: the contribution map (deterministic; the "see all changes")
- [ ] **1.1 Branch contribution map.** For each local branch AND each remote branch not mirrored locally, compute (no LLM): ahead-count (`git rev-list --count origin/main..<b>`), **unique commits** (`git cherry origin/main <b>` — count lines starting `+`; equivalence-aware, NOT raw rev-list), unique files (`git diff --name-only origin/main...<b>`), last-commit date, and a **verdict**: `LIVE-unique` / `SUPERSEDED` (all `+` commits also appear equivalently on another branch) / `DUPLICATE` (divergent reimpl of a base feature) / `DEAD` (0 unique `+` commits). Flag branches with commits in the **last 3 days** as `PRIORITY`. Write `graphify-out/consolidation/branch-map.md` (table sorted by verdict then priority).
- [ ] **1.2 Stash map.** For each of the 48 stashes: `git stash show -p stash@{N} --stat`, and test whether its diff is already represented on its origin branch / the integration target (`git stash show -p stash@{N} | git apply --check` against the candidate base). Verdict per stash: `LIVE-unique` (apply-clean + not already present) / `SUPERSEDED` / `CONFLICTS` (needs manual). Write `graphify-out/consolidation/stash-map.md`.
- [ ] **1.3 Open-PR triage.** `gh pr list --state open --json number,title,headRefName,changedFiles`. Currently: **#388** (dependabot rust-deps, 1 file) — decide merge-or-close. Record any others.
- [ ] **1.4** Present `branch-map.md` + `stash-map.md` to the owner. **[OWNER CHECKPOINT]** — the maps drive every later keep/drop decision.

## Phase 2 — Pick the integration base + open the integration branch
- [ ] **2.1 [OWNER DECISION, informed by the map]** Choose the base = the most-complete, green-able branch (candidates from the audit: `crate-build-spine-hardening` 316, `telemetry-track-f` 305, `auto-gui-debug-plans` 291 [has the arch-check guard]). Prefer the one that (a) is already arch-check-green, (b) carries the most `LIVE-unique` mass, (c) minimizes downstream conflicts.
- [ ] **2.2** `git switch -c integration/main-consolidation-<date> <base>`. All consolidation happens here; `main` is never directly edited.
- [ ] **2.3** Baseline-green the integration branch: `cargo run -p vox-arch-check` (0 errors; `forbidden_pattern=error`), `cargo test` (changed crates), `vox ci pre-push`. If red, fix root cause (e.g., the VoxMens guard 3-file fix if the base lacks it). Record the green baseline.

## Phase 3 — Layer all LIVE branches + recover unique stashes
For each branch the map marked `LIVE-unique` (PRIORITY/last-3-days first), in dependency order:
- [ ] **3.1** Compute its unique delta vs the integration branch (`git cherry`, `git diff --name-only`).
- [ ] **3.2** Bring it in: prefer `git merge --no-ff <branch>` for branches that share lineage; use `git cherry-pick <unique-shas>` for small disjoint deltas. **On a `DUPLICATE`-divergent conflict, STOP and surface BOTH versions to the owner; the owner picks the winner** (per the approved conflict rule). Resolve, `git add <paths>`, continue.
- [ ] **3.3** After each branch: re-green (arch-check 0, tests, `vox ci pre-push`). Never proceed red. Commit the integration step.
- [ ] **3.4** Recover `LIVE-unique` stashes (Phase 1.2): `git stash apply stash@{N}` onto the integration branch, resolve, commit. `CONFLICTS` stashes → surface to owner.
- [ ] **3.5** When all LIVE work is layered: full gate — `cargo test` (workspace or affected), `cargo run -p vox-arch-check` (0), `vox ci pre-push`, and a representative `vox ci` full run. **[OWNER CHECKPOINT]**: integration branch = the intended union, green.

## Phase 4 — Re-slice integration→main into ≤140-file PRs (separation of concerns)
The diff `origin/main...integration/main-consolidation` is the total change set. Slice it by concern.
- [ ] **4.1** Enumerate the change set: `git diff --name-only origin/main...HEAD | wc -l` and group by top-level concern (e.g. `crates/vox-gui/**`, `crates/vox-populi+vox-ml-cli (mens)`, `crates/vox-orchestrator*`, `crates/vox-db*`, `docs/**`, `contracts/**`, `voxup`, telemetry, agy). Target **≤~140 files/slice**; split a concern that exceeds it.
- [ ] **4.2** For each slice, create `slice/<concern>-<date>` off **current `origin/main`** and bring only that concern's files: `git checkout integration/main-consolidation-<date> -- <paths>`; `cargo check`/test the slice; if it needs files from another not-yet-merged slice, record the dependency (FF-merge order) or fold them together. (Reconcile with the pre-existing `stack/02..09` branches — reuse if still valid, else supersede.)
- [ ] **4.3** Prioritize: build the **last-3-day PRIORITY slices first** (some may already be merged elsewhere — confirm with `git cherry origin/main` before re-submitting).
- [ ] **4.4** Push each slice; `gh pr create` with a clear separation-of-concerns title + body (link the branch-map row).

## Phase 5 — CodeRabbit review + merge to main
- [ ] **5.1** On each PR: `@coderabbitai review` (CodeRabbit skips stacked PRs, so trigger manually). Wait for review.
- [ ] **5.2** Address findings (`receiving-code-review` skill: verify before implementing, push back on wrong suggestions). Re-green; re-review if substantive.
- [ ] **5.3** Merge in dependency order once green + CI passing + review resolved. `main` advances only via these merges. Re-base/re-target later slices after each merge so each PR's diff stays ≤140 and conflict-free.
- [ ] **5.4** After all slices merged: `main` == the consolidated union, synced to remote, green, reviewed. Verify: `git rev-list --count main..integration/main-consolidation` == 0 (nothing left on integration that isn't on main).

## Phase 6 — Retire branches, stashes, PRs (only after backup + merge confirmed)
- [ ] **6.1 Branches:** for every branch, `git cherry origin/main <branch>` → if **no unique `+` commits remain**, delete (`git branch -D <local>`; `git push origin --delete <remote>`). For the 122 remote branches, script the cherry-check; delete only zero-unique ones; list any with residual unique work for a final owner decision. (Pre-push hook runs on `--delete` → prune stale worktrees first: `git worktree prune`.)
- [ ] **6.2 Stashes:** drop every stash whose work is now on `main` (`git stash drop stash@{N}`); the `preserve/stash-*` tags + bundle remain the archive. Surface any still-unrecovered `CONFLICTS` stash to the owner before dropping.
- [ ] **6.3 PRs:** merge or close #388 (dependabot) and any other open PRs per 1.3; close PRs whose branches were consolidated (reference the consolidating PR).
- [ ] **6.4 Worktrees:** remove the `worktree-*`/`stack/*` scaffolding once their work is merged (`git worktree remove`, `git branch -D`).
- [ ] **6.5** Final state check: `gh pr list --state open` empty (or only intentional), `git for-each-ref refs/heads refs/remotes/origin | wc -l` collapsed to the keep-set, `main` green + synced.

## Phase 7 — Recurrence guard (prevent the next 122-branch swarm)
- [ ] **7.1** Policy: one initiative per branch, branched off **current** `origin/main`, merged within N days (AGENTS.md §B-3 / ledger lesson).
- [ ] **7.2** A `vox ci` (or pre-push) **warning** when the current branch is >N commits ahead of `origin/main` or older than D days — early signal before a branch becomes a 300-commit kitchen-sink. (Scope the lint under its own task; reference here.)
- [ ] **7.3** Record the consolidation in the handoff ledger + an architecture note (what merged, what was archived-and-dropped, the bundle path).

## B. Self-review (plan author)
- All branches → main: Phases 1–6 (audit → layer → slice → merge → retire). ✓
- Stashes: 0.4 backup, 1.2 map, 3.4 recover, 6.2 drop. ✓
- Open PRs: 1.3 triage, 6.3 close/merge. ✓
- One clean main synced to remote: Phase 5.4. ✓
- CodeRabbit-reviewed: Phase 5.1–5.3. ✓
- ≤140-file separation-of-concerns PRs: Phase 4. ✓
- Last-3-day priority: 1.1 flag, 4.3 first. ✓
- No work lost: Phase 0 bundle+tags (incl. stashes) before any delete. ✓
- Branches retired: Phase 6. ✓
- Recurrence guard: Phase 7. ✓

## C. Owner checkpoints (the judgment gates)
1.4 (maps), 2.1 (base), 3.2 (per-conflict winner picks), 3.5 (union green), plus any residual-unique branch/stash. Everything else is mechanical + verifiable.

## D. Risk notes
- **Scale:** 122 remote + 48 stashes is large; Phase 1 is scriptable but Phase 3 per-conflict review is the real cost. Expect most branches to be `SUPERSEDED`/`DEAD` (the cherry-check will show it) — the LIVE set is likely <15.
- **`main` may already contain some recent work** (last-3-day commits): always `git cherry origin/main` before re-landing to avoid duplicate PRs.
- **Reconcile with the existing `stack/02..09`** re-slice — reuse if still equivalent to the integration diff; otherwise supersede and delete.
