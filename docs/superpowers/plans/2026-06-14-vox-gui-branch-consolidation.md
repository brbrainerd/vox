# vox-gui Design-Principles Branch Consolidation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. **This is git-surgery, not feature code** — each task's "verify" step is its test gate. Do NOT skip a verify gate.

**Goal:** Land all vox-gui Phase 0A–0D + Wave 1 work onto `main`/`origin/main` as one clean linear addition, with no duplicate or divergent branches left dangling.

**Architecture:** The complete GUI work lives as a **contiguous, GUI-only commit block** on the stale local branch `claude/vox-gui-design-principles-phase0`. That branch forked ~2567 commits behind current `main`, so it cannot be merged or rebased wholesale (its tree would delete hundreds of files main has since gained). Instead we **cherry-pick only the GUI commit block** onto a fresh worktree off current `origin/main`, graft on the 4 small CodeRabbit fixes the block predates, verify the isolated `vox-gui` build, fast-forward `main`, push, and delete the superseded branches/PRs.

**Tech Stack:** git worktrees, cherry-pick, pnpm (vox-gui/ui), Style Dictionary 5.x, Vitest, `gh` CLI.

**CI/CD:** Explicitly out of scope per the requester. We verify the `vox-gui` build locally; we do not wait on or gate against repo CI. `main` is admin-pushable (branch protection `enforce_admins=false`).

---

## Pinned constants (capture once, reuse everywhere)

These SHAs are pinned because a concurrent process is actively advancing `origin/main`. **Never** substitute a live branch ref for these in destructive steps.

| Name | SHA | Meaning |
|------|-----|---------|
| `COMP` | `299d7778d6690fe02cf29523c1ae30c6628d0505` | Comprehensive branch tip (all GUI work) |
| `GUI_BLOCK_BASE` | `0677b2ac6a` | Oldest GUI commit (the design-principles spec). Block = `GUI_BLOCK_BASE^..COMP` |
| `FOUND` | `26a1bef25c4fedfcf7db97470a0bc2268e224c12` | PR #304 foundation tip (has the 4 CodeRabbit fixes; **superseded**) |
| `OMAIN_AT_PLAN` | `b08fa2b6752b7474a8d2e0099da87c93ec661917` | `origin/main` when this plan was written (will have moved; re-capture at execution) |

**Verified before writing this plan:**
- `git rev-list GUI_BLOCK_BASE^..COMP` → every commit touches only `crates/vox-gui/`, `docs/`, or `.gitignore` (CLEAN).
- `COMP:crates/vox-gui/ui/package.json` still pins `style-dictionary ^4.4.0` and **lacks** the border-contrast fix, but **does** declare `@tanstack/react-query ^5.101.0` and `@tanstack/react-virtual ^3.14.2`.

---

### Task 0: Pre-flight snapshot & safety nets

**Files:** none (git refs only).

- [ ] **Step 1: Confirm the comprehensive branch still resolves to the pinned SHA**

Run:
```powershell
git rev-parse claude/vox-gui-design-principles-phase0
```
Expected: `299d7778d6690fe02cf29523c1ae30c6628d0505`. If it differs, STOP — someone moved the branch; re-derive `COMP` and re-validate the GUI block boundary before continuing.

- [ ] **Step 2: Create immutable backup tags for every branch we may delete**

Run:
```powershell
git tag backup/comp-20260614 299d7778d6690fe02cf29523c1ae30c6628d0505
git tag backup/found-20260614 26a1bef25c4fedfcf7db97470a0bc2268e224c12
git tag backup/phase0b-ipc-20260614 origin/claude/gui-phase0b-ipc-query
```
Expected: three tags created, no error. These are the rollback anchors — nothing in this plan deletes them.

- [ ] **Step 3: Re-verify the GUI block is still GUI-only**

Run:
```powershell
git rev-list 0677b2ac6a^..299d7778d6 | ForEach-Object { git diff-tree --no-commit-id --name-only -r $_ } |
  Where-Object { $_ -and $_ -notmatch '^crates/vox-gui/' -and $_ -notmatch '^docs/' -and $_ -ne '.gitignore' } |
  Sort-Object -Unique
```
Expected: **no output** (empty). If anything prints, a commit in the block touches a non-GUI file — STOP and inspect; the cherry-pick is no longer guaranteed safe.

---

### Task 1: Create an isolated worktree off the latest `origin/main`

**Files:** new worktree at `.worktrees/gui-consolidation` (already gitignored by the GUI block; safe even before the block lands because `.worktrees/` is ignored on `main` too — verify in Step 2).

- [ ] **Step 1: Fetch and re-capture the live `origin/main` SHA**

Run:
```powershell
git fetch origin main
git rev-parse origin/main
```
Record the printed SHA as `OMAIN` (the *current* tip — may differ from `OMAIN_AT_PLAN`). Use `OMAIN` for the rest of this task.

- [ ] **Step 2: Confirm `.worktrees/` is ignored on the current `origin/main`**

Run:
```powershell
git check-ignore .worktrees
```
Expected: prints `.worktrees`. If it prints nothing, add `.worktrees/` to `.gitignore` on `main` first (a one-line commit) — do not create an un-ignored worktree dir.

- [ ] **Step 3: Create the worktree on a fresh consolidation branch rooted at `OMAIN`**

Run (substitute the `OMAIN` SHA recorded in Step 1):
```powershell
git worktree add -b claude/gui-consolidation .worktrees/gui-consolidation <OMAIN>
```
Expected: `Preparing worktree (new branch 'claude/gui-consolidation')` + `HEAD is now at <OMAIN> ...`.

- [ ] **Step 4: Verify the worktree baseline is clean and at `OMAIN`**

Run:
```powershell
cd .worktrees/gui-consolidation
git status --short
git rev-parse HEAD
```
Expected: no output from `status --short`; `HEAD` equals `OMAIN`.

---

### Task 2: Cherry-pick the GUI commit block

**Files:** all under `crates/vox-gui/`, plus `.gitignore`, `docs/src/architecture/layers.toml`, `docs/src/architecture/where-things-live.md`, and the new `docs/superpowers/plans/2026-06-14-vox-gui-phase0*.md` / `docs/src/architecture/gui-*.md` files.

- [ ] **Step 1: Cherry-pick the entire block in order**

Run (from inside `.worktrees/gui-consolidation`):
```powershell
git cherry-pick 0677b2ac6a^..299d7778d6
```
Expected (happy path): a sequence of `[claude/gui-consolidation <sha>] <subject>` lines, ending with the Wave-1 TopHud commit and no error.

- [ ] **Step 2: If cherry-pick pauses on a conflict, resolve it**

Conflicts are only expected in files **both** the GUI block and recent `main` edited — realistically `.gitignore`, `docs/src/architecture/layers.toml`, and `docs/src/architecture/where-things-live.md`. For each:

```powershell
git diff --name-only --diff-filter=U   # list conflicted files
```

Resolution rule: these are **additive** files (the GUI block *adds* a layer rule / a lookup row / an ignore line; main *added* unrelated lines). Take the **union** — keep both sides' additions, drop the conflict markers. After editing every conflicted file:

```powershell
git add <each-resolved-file>
git cherry-pick --continue
```

If a conflict is in any `crates/vox-gui/**` file, that is unexpected (main shouldn't have touched vox-gui) — inspect with `git log <OMAIN> -- crates/vox-gui/<file>` before resolving; prefer the GUI block's version.

**Never** run `git cherry-pick --abort` and restart without re-reading this task — partial progress is fine to continue from.

- [ ] **Step 3: Verify the block landed and touched only expected paths**

Run:
```powershell
git log --oneline <OMAIN>..HEAD | Measure-Object | Select-Object -ExpandProperty Count
git diff --name-only <OMAIN>..HEAD | Where-Object { $_ -notmatch '^crates/vox-gui/' -and $_ -notmatch '^docs/' -and $_ -ne '.gitignore' }
```
Expected: count is ~34 (the block size, minus any empty commits git auto-dropped); the second command prints **nothing**.

---

### Task 3: Graft the 4 CodeRabbit fixes the block predates

**Files:**
- Modify: `crates/vox-gui/ui/package.json`
- Modify: `crates/vox-gui/ui/tokens/semantic.json`
- Modify: `crates/vox-gui/ui/src/lib/theme.test.ts`
- Regenerate: `crates/vox-gui/ui/src/styles/tokens.generated.css`, `tokens.generated.ts`
- Update: `crates/vox-gui/ui/pnpm-lock.yaml`

> Rationale: the cherry-picked block is from before PR #304's review. These are the exact fixes already verified on `FOUND` (commits `c1962714b4` + `26a1bef25c`). We re-apply them rather than cherry-pick `FOUND`'s commits, because `FOUND`'s tree diverges (it lacks 0C/0D/Wave1 and the `react-virtual` dep). First **check** whether the block already contains any of them (it should not), then apply only what's missing.

- [ ] **Step 1: Confirm the fixes are absent (guards against double-apply)**

Run (from `.worktrees/gui-consolidation`):
```powershell
Select-String -Path crates/vox-gui/ui/package.json -Pattern 'style-dictionary'
Select-String -Path crates/vox-gui/ui/tokens/semantic.json -Pattern 'border'
```
Expected: `style-dictionary` shows `^4.4.0`; `border` shows `"subtle": { "value": "{color.neutral.800}" }, "strong": { "value": "{color.neutral.700}" }`. If either already shows the fixed value, skip that sub-fix below.

- [ ] **Step 2: Bump style-dictionary to 5.x**

Edit `crates/vox-gui/ui/package.json`: change `"style-dictionary": "^4.4.0"` → `"style-dictionary": "^5.4.4"`.

- [ ] **Step 3: Fix the border-contrast token pair**

Edit `crates/vox-gui/ui/tokens/semantic.json`, the `border` line:
```json
    "border": { "subtle": { "value": "{color.neutral.700}" }, "strong": { "value": "{color.neutral.400}" } },
```
(was `subtle: 800` — identical to `bg.elevated`, a 1:1 contrast WCAG failure; `strong: 400` keeps `subtle < strong`.)

- [ ] **Step 4: Add the `arcane` normalization assertion**

Edit `crates/vox-gui/ui/src/lib/theme.test.ts`, inside the `'keeps known accent themes'` test, add as the first assertion:
```ts
    expect(normalizeTheme('arcane')).toBe('arcane');
```

- [ ] **Step 5: Install (updates lockfile to SD 5.x) and regenerate tokens**

Run:
```powershell
cd crates/vox-gui/ui
pnpm install
pnpm tokens:build
```
Expected: `pnpm install` reports `+ style-dictionary 5.4.4` and updates `pnpm-lock.yaml`; `tokens:build` prints `tokens built: tokens.generated.css, tokens.contrast.generated.css, tokens.generated.ts` (8 collision warnings are non-fatal). Confirm the regenerated border value:
```powershell
Select-String -Path src/styles/tokens.generated.css -Pattern 'color-border-subtle'
```
Expected: `--color-border-subtle: #3f3f46;`

- [ ] **Step 6: Commit the grafted fixes**

Run (from `.worktrees/gui-consolidation`):
```powershell
cd ../..
git add crates/vox-gui/ui/package.json crates/vox-gui/ui/tokens/semantic.json `
        crates/vox-gui/ui/src/lib/theme.test.ts crates/vox-gui/ui/src/styles/tokens.generated.css `
        crates/vox-gui/ui/src/styles/tokens.generated.ts crates/vox-gui/ui/pnpm-lock.yaml
git commit -m @'
fix(vox-gui): graft PR #304 CodeRabbit fixes onto consolidated GUI work

- style-dictionary ^4.4.0 -> ^5.4.4 (+ regenerated pnpm-lock) — prototype-pollution CVE
- border.subtle 800->700 (was == bg.elevated, 1:1 contrast) / border.strong 700->400
- theme.test.ts: add normalizeTheme(arcane) assertion
- regenerated token artifacts

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
'@
```
Expected: one commit created.

---

### Task 4: Verify the isolated vox-gui build

**Files:** none (verification only).

- [ ] **Step 1: Typecheck**

Run:
```powershell
cd .worktrees/gui-consolidation/crates/vox-gui/ui
pnpm tsc --noEmit
```
Expected: exit 0, no output.

- [ ] **Step 2: Full test suite**

Run:
```powershell
pnpm test
```
Expected: all test files pass (≈42 files / ≈250 tests across the full 0A–0D + Wave 1 surface; the exact count is whatever the block + grafts produce — the gate is **0 failures**).

- [ ] **Step 3: Production build**

Run:
```powershell
pnpm build
```
Expected: `✓ built in …` with exit 0. (The >500 kB chunk-size warning is pre-existing and not a failure.)

- [ ] **Step 4: If any gate fails, fix in this worktree and amend/commit before proceeding**

Do not advance to Task 5 with a red gate. Typical failures and fixes:
- Missing dep after install → ensure `@tanstack/react-query` and `@tanstack/react-virtual` are in `package.json` `dependencies` (the block should already have both — verify with `Select-String -Path crates/vox-gui/ui/package.json -Pattern 'tanstack'`).
- Token build error under SD5 → the `style-dictionary.config.mjs` from the block is already v5-compatible (verified on `FOUND`); if not, compare against `git show 26a1bef25c:crates/vox-gui/ui/style-dictionary.config.mjs`.

---

### Task 5: Fast-forward `main` and push (race-aware)

**Files:** none (ref updates only).

- [ ] **Step 1: Re-fetch and check whether `origin/main` moved during the build**

Run (from the main checkout, not the worktree):
```powershell
cd C:\Users\Owner\vox
git fetch origin main
git rev-parse origin/main
```
Compare to the `OMAIN` recorded in Task 1 Step 1.

- [ ] **Step 2a: If `origin/main` is UNCHANGED — fast-forward directly**

Run:
```powershell
git rev-list --count claude/gui-consolidation..origin/main   # expect 0 (consolidation is ahead, main not ahead of it)
git branch -f main origin/main                                 # ensure local main == origin/main first
git merge --ff-only claude/gui-consolidation                   # FF main onto the consolidation (run while checked out on main)
```
To run the FF on `main`: `git switch main` (or `-C`), then `git merge --ff-only claude/gui-consolidation`. Expected: `Fast-forward`.

- [ ] **Step 2b: If `origin/main` MOVED — rebase the consolidation, then FF**

The consolidation only touches `crates/vox-gui/` + a few docs, so rebasing onto the new tip is conflict-free in practice:
```powershell
cd .worktrees/gui-consolidation
git rebase origin/main
cd C:\Users\Owner\vox
git switch main
git merge --ff-only claude/gui-consolidation
```
Resolve any doc conflicts per Task 2 Step 2's union rule, then `git rebase --continue`.

- [ ] **Step 3: Push `main`**

Run:
```powershell
git push origin main
```
Expected: `<old>..<new>  main -> main`. If rejected as non-fast-forward, `origin/main` moved again — repeat from Step 1 (the work is already committed on `claude/gui-consolidation`, so this is just a re-FF + re-push, never a re-do).

- [ ] **Step 4: Verify main and remote are synchronized**

Run:
```powershell
git fetch origin main
git rev-parse refs/heads/main; git rev-parse origin/main
```
Expected: both SHAs identical.

---

### Task 6: Retire the dangling branches and superseded PRs

**Files:** none (branch/PR lifecycle).

- [ ] **Step 1: Close PR #304 and the phase0b PR (if open) as superseded**

Run:
```powershell
gh pr comment 304 --body "Superseded by the consolidated vox-gui design-principles landing on main (Phase 0A-0D + Wave 1, including these CodeRabbit fixes). Closing in favor of the unified history."
gh pr close 304
gh pr list --state open --search "gui-phase0b OR gui-consolidation" --json number,headRefName
```
For any open phase0b PR returned, comment + close the same way. (If #304 is already merged/closed, skip — `gh pr view 304 --json state` first.)

- [ ] **Step 2: Delete the superseded REMOTE branches**

Run:
```powershell
git push origin --delete claude/gui-phase0a-foundation
git push origin --delete claude/gui-phase0b-ipc-query
```
Expected: `- [deleted]` for each. (Skip any that 404 — already gone.)

- [ ] **Step 3: Delete the superseded LOCAL branches**

Run from `C:\Users\Owner\vox` (must not be checked out on them):
```powershell
git switch main
git branch -D claude/vox-gui-design-principles-phase0 claude/gui-phase0a-foundation claude/gui-phase0b-ipc-query
```
Expected: `Deleted branch …` for each present. The `backup/*` tags from Task 0 remain as rollback anchors.

- [ ] **Step 4: Remove the consolidation worktree and branch**

Run:
```powershell
git worktree remove .worktrees/gui-consolidation
git branch -D claude/gui-consolidation
```
Expected: clean removal. (If `worktree remove` complains about modifications, the build left `node_modules`/`dist` — add `--force`; those are gitignored artifacts.)

---

### Task 7: Post-consolidation verification

**Files:** none.

- [ ] **Step 1: Confirm the GUI work is on `main`**

Run:
```powershell
git switch main
git log --oneline -5 -- crates/vox-gui/ui/src/hooks/useVoxQuery.ts
git ls-files crates/vox-gui/ui/src/components/ui/ | Select-String 'Async|Button|Dialog|Skeleton'
```
Expected: the GUI history is present; the Phase 0B/0C primitives are tracked on `main`.

- [ ] **Step 2: Confirm no dangling vox-gui design-principles branches remain**

Run:
```powershell
git branch -a | Select-String 'gui-phase0|gui-consolidation|design-principles'
```
Expected: **no output**.

- [ ] **Step 3: Confirm main == origin/main one final time**

Run:
```powershell
git fetch origin main
"local=$(git rev-parse refs/heads/main)  remote=$(git rev-parse origin/main)"
```
Expected: the two SHAs are equal.

- [ ] **Step 4: Sanity-rebuild vox-gui from `main`**

Run:
```powershell
cd crates/vox-gui/ui
pnpm install
pnpm test
```
Expected: install clean, all tests pass — confirming `main` itself produces a green vox-gui.

---

## Self-Review

**Spec coverage:**
- "all Phase 0A–0D + Wave 1 work on main" → Task 2 cherry-picks the complete GUI block (verified GUI-only); Task 4 proves it builds; Task 7 confirms presence on `main`. ✅
- "main + origin/main synchronized" → Task 5 Step 4 + Task 7 Step 3 assert SHA equality. ✅
- "no duplicate/divergent branches dangling" → Task 6 deletes comprehensive, foundation, phase0b (local + remote) and closes their PRs; Task 7 Step 2 asserts none remain. ✅
- "one clean intelligent merge" → single linear cherry-pick block + one graft commit, FF onto main (no merge commit, no stale-tree clobber). ✅
- "ignore CI/CD" → stated up front; verification is the local vox-gui build only. ✅

**Risk register (and the mitigation each):**
- Concurrent process advancing `origin/main` → pinned SHAs + worktree + race-aware Task 5 (Step 2b rebase, Step 3 retry-on-reject). The cherry-pick block touches only `crates/vox-gui/` so re-basing onto a moved main is conflict-free.
- Catastrophic wholesale revert → avoided entirely by cherry-pick (replays only what each GUI commit changed) instead of merge/reset to the stale tree.
- Data loss on delete → Task 0 backup tags (`backup/comp-20260614`, `backup/found-20260614`, `backup/phase0b-ipc-20260614`) are never deleted by this plan.
- Double-applying CodeRabbit fixes → Task 3 Step 1 guard checks current values before editing.

**Placeholder scan:** none — every step has exact commands and expected output. The only intentionally-symbolic value is `<OMAIN>` (the live `origin/main` SHA), which must be re-captured at execution time precisely *because* it moves; its capture is an explicit step (Task 1 Step 1).

**Type/name consistency:** branch names, tag names, and SHAs are used identically across tasks. `claude/gui-consolidation` is created in Task 1, used in Task 5, deleted in Task 6.
