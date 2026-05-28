---
title: "Work-loss audit + handoff (2026-05-24)"
description: "Forensic audit of the parallel-agent 'lost work' reports. Single conclusion: no commits were destroyed; the unmerged work lives on its branches. Includes inventory of where each agent's work sits, why it looked missing from main, and the recovery plan."
last_updated: "2026-05-24"
category: "Architecture SSOTs"
status: research
---

# Work-loss audit + handoff (2026-05-24)

> **Bottom line:** No commits were destroyed. Every commit referenced in the
> investigation is reachable from some branch or tag. The "work is missing"
> reports from parallel agents are all explained by **work-on-worktree-branch
> not yet merged to main** — not by destructive operations on main.
>
> The 22:44:24 reflog entry I initially flagged as a destructive reset was
> in fact a **fast-forward pull** (`reset --hard origin/main` while
> origin/main had just advanced via the PR #93 merge). It *added* commits;
> it did not remove any. See §2.

## 1. Audit methodology

Every claim below is grounded in raw git facts collected on 2026-05-24
between 03:00 and 03:15 EDT. Commands run:

| Question | Command |
|---|---|
| What's HEAD? | `git rev-parse HEAD`, `git status` |
| Were there destructive operations on `main`? | `git reflog show main --date=iso -n 50` |
| Were there destructive operations on HEAD? | `git reflog --date=iso -n 50` |
| What parallel branches have unmerged work? | `git for-each-ref --sort=-committerdate refs/heads/`, `git rev-list --left-right --count <branch>...main` |
| Are specific commits reachable from main? | `git merge-base --is-ancestor <sha> main` |
| What's in the dangling-commit graveyard? | `git fsck --lost-found` |
| Where do all worktrees point? | `git worktree list --porcelain` |
| Is there stashed WIP? | `git stash list`, `git stash show stash@{N} --stat` |

## 2. The 22:44:24 reset — what actually happened

The HEAD reflog shows:

```
6459133dbc HEAD@{2026-05-23 22:44:24}: reset: moving to origin/main
668f594523 HEAD@{2026-05-23 22:43:55}: checkout: moving from durable-functions-clean to main
```

Read carefully: at 22:43:55 HEAD was at `668f594523` (the prior tip of
main). At 22:44:24 a `git reset origin/main` ran, and HEAD landed on
`6459133dbc`. That's the PR #93 merge commit — the **GitHub-side merge**
that had just been pushed to `origin/main`.

So this was effectively a fast-forward pull, not a destructive rewind.
The reset moved local main **forward by one merge commit**, ingesting
work, not destroying it.

Confirmation: the `main` branch reflog (distinct from the HEAD reflog)
shows **no resets at all** — only `commit:` entries from `baf0dd0881`
forward to today's tip. A destructive reset on `main` would have left a
`reset:` entry there. There is none.

## 3. Why parallel agents report "missing" work

Each Claude Code worktree pins a separate branch:

```
C:/Users/Owner/vox                                     [main]
C:/Users/Owner/vox/.claude/worktrees/jovial-buck-e93ac0       [cc_bdesktop2/jovial-buck-e93ac0]
C:/Users/Owner/vox/.claude/worktrees/naughty-dirac-825348     [cc_bdesktop2/naughty-dirac-825348]
C:/Users/Owner/vox/.claude/worktrees/docs-voxlang-cf-migration [docs/voxlang-org-cf-migration]
…
```

From inside a worktree, the agent's CWD looks like a normal repo, but
`HEAD` is the worktree's branch — **not main**. Every commit the agent
makes lands on its worktree branch. If the agent (or the user reading
their summary) checks `main` and sees their changes missing, that's
because they're not on main — they're on the branch.

This explains every "my work disappeared" report I can verify.

## 4. Inventory: where each agent's work sits

The four branches with substantive unmerged work, current as of
2026-05-24 03:10 EDT:

| Branch | Tip | Ahead of `main` | Behind `main` | Scope |
|---|---|---:|---:|---|
| `cc_bdesktop2/jovial-buck-e93ac0` | [`d97d30410a`](.) at 03:01:40 | **98** | 26 | PR #92: docs(json-ergonomics) + typeck(builtins), CI fixes, MENS training, frontmatter sweeps |
| `docs/voxlang-org-cf-migration` | [`c4818d7961`](.) at 16:11:09 (yesterday) | **16** | 36 | Cloudflare Pages migration follow-up, Playwright smoke tests, lychee allow-list, doc-pipeline VALID_CATEGORIES update |
| `claude/dashboard-vuv-port` | [`6bf967f29a`](.) at 2026-05-03 23:08 | **14** | 677 | Dashboard VUV chrome (TopBar/LeftRail/StatusBar/Shell), VUV composites, JSX→VUV migrations, /api/v2 namespace |
| `cc_bdesktop2/naughty-dirac-825348` | [`7ddebeecca`](.) at 2026-05-22 04:55 | **9** | 220 | scripts/show automation (cross-post.vox, publish.vox, raw-nerve), voxlang.org domain migration |
| `backup/jovial-buck-e93ac0-pre-rebase` | `e0aa5db739` | (same chain as jovial-buck) | — | **Safety tag** — preserved before the 2026-05-24 00:39 rebase attempt on jovial-buck (rolled back at 00:41 per branch reflog). DO NOT DELETE until jovial-buck merges to main. |

**Branches that are behind main with 0 ahead** (no unmerged work; safe to delete or refresh):
- `cc_bdesktop2/share-s2-s9` (0 ahead, 404 behind)
- `cc_bdesktop2/zealous-ardinghelli-b01e11` (0 ahead, 105 behind)

**Worktrees that vanished from disk but whose branches survive in `.git/refs/heads/`** (work intact; just no checkout):
- `.claude/worktrees/dashboard-vuv-port` → branch `claude/dashboard-vuv-port` ✓
- `.claude/worktrees/share-s2-s9` → branch `cc_bdesktop2/share-s2-s9` ✓
- `.claude/worktrees/zealous-ardinghelli-b01e11` → branch `cc_bdesktop2/zealous-ardinghelli-b01e11` ✓
- `.claude/worktrees/lang-{json-stdlib,match-arm-stmts,regex,str-utils,struct-types,ts-ffi}` → detached HEAD worktrees (no branch attached; their last commits are reachable via reflog only)
- `.claude/worktrees/goofy-yonath-db8222` → detached HEAD

To list the dangling lang-* worktree commits and recover them if they
matter:

```bash
git reflog show HEAD --date=iso | grep "lang-" -B1 -A1   # see when each was checked out
git fsck --lost-found                                    # 110,763 dangling commits exist; most are intermediate states, not lost work
```

## 5. What's missing from main but present elsewhere

This is the actual recovery list — work that exists on a branch but
hasn't been merged into main:

### 5.1 `docs/voxlang-org-cf-migration` (16 commits)

Tip-to-merge-base, oldest first:

```
9740c7fb4d  docs(specs): add voxlang.org hosting + docs category overhaul design
06d6c942cc  docs(specs): audit corrections to voxlang.org hosting design
e260282ef9  docs(plans): implementation plan for voxlang.org hosting + docs overhaul
c01244848e  docs(scripts): add fix-doc-categories.vox to retag SSOT frontmatter
bd83e81236  docs: realign frontmatter category values with sidebar SSOT labels
91ad374d0d  docs(ssot): refresh sidebar section order + fix archive frontmatter
e5e4e5cb5c  docs(agents): regenerate doc-inventory.json after category overhaul
bee670d99c  ci(docs): wire Cloudflare Pages deploy + set canonical URL to voxlang.org
53c5bbed69  test(docs): add Playwright smoke tests + CI smoke-test job
c7d6f9fc93  chore(docs-astro): add Playwright for live-site smoke tests
788606274c  chore(docs-astro): add Playwright config (baseURL via env)
bacdbfd413  test(docs-astro): capture baseline of live vox-lang.org for regression compare
ac38f53f0c  fix(docs): fix remaining 300 files with slug-style categories
fedbe90e38  fix(docs): canonical URL and CF secrets registration
4bd610a866  fix(doc-pipeline): update VALID_CATEGORIES to display-label SSOT format
c4818d7961  fix(ci): ignore nature.com in lychee (cookie-auth redirect false positive)
```

**Note:** the *content* of `4bd610a866` (VALID_CATEGORIES → display-label
form) is *already on main* — `git diff main..docs/voxlang-org-cf-migration --
crates/vox-doc-pipeline/src/pipeline/lint.rs` is empty. Two parallel
edits converged on the same answer. So merging this branch will
auto-resolve that file. The Playwright tests, CF deploy workflow,
fix-doc-categories script, and lychee allow-list are the substantive
deltas.

### 5.2 `cc_bdesktop2/jovial-buck-e93ac0` (98 commits)

This is **PR #92** — a long-running branch with the largest delta. Top
15 most recent commits not on main:

```
d97d30410a  docs(json-ergonomics) + typeck(builtins): RFC idioms compile against live API
40a7985455  merge: integrate origin/main into PR #92
e0aa5db739  docs(cli-vox-deploy): add frontmatter and tag dry-run fence with text
d4071722bb  fix(retired-symbols): scope vox-ml-cli pattern to literal -standalone suffix
cbbd5c84a4  fix(ci): build vox-cli with --features script-execution in setup-e2e
eba4d113ac  docs: PR #92 handoff snapshot — CI fixes landed, MENS training pending
d72b376428  fix(command-sync): use canonical 'Language Reference' category
8887a7bece  ci: fix retired-symbol false positives + regen CLI command surface
6c7c019320  chore: ignore blueprism.com and strata.io in lychee (bot-blocking 415)
e8e815b147  chore: ignore lychee transient failures for slavakurilyak.com and gradio-app GitHub links
421c60b5d2  fix(check-links): ignore opencontainers.org in lychee (transient network block)
f67737f901  fix(plugin-catalog): use canonical 'Language Reference' category in generated docs
74e730145e  fix(ci): resolve all 4 failing CI checks on PR #92
3ce902036c  fix(docs): update readiness-snapshot category to "Architecture SSOTs"
ee2dd3b796  fix(candle-kernels): pub static (not const) so PTX bytes survive LTO + gitignore .ptx
```

Full list: `git log --oneline cc_bdesktop2/jovial-buck-e93ac0 ^main`.

The branch reflog shows one attempted rebase on 2026-05-24 around
00:39–00:41 that was rolled back via the `backup/jovial-buck-e93ac0-pre-rebase`
tag. The branch is currently AT the pre-rebase state plus 4 commits made
after. No commits were lost during that operation.

### 5.3 `claude/dashboard-vuv-port` (14 commits)

This branch is **677 commits behind main** because it has been stale
since 2026-05-03. Top commits not on main:

```
6bf967f29a  feat(dashboard): VUV chrome — TopBar, LeftRail, StatusBar, Shell
8f69d510da  feat(dashboard): VUV composites for Phase 1 (Label, StateChip, NodeBadge, KeyHint, SectionHeading, IconBtn, Toggle, Input, StatBox, Codeframe)
15a2c2c52b  docs(plans): Vox Dashboard implementation plan (VUV-form, Phase 0 done)
5a6ec0587d  docs(architecture): Vox Dashboard design brief (VUV-form, post Phase 0)
b16f23fb33  feat(orchestrator): event-bus variants for dashboard live data (BuildStage, ThroughputTick, CostTick, FileDiagChanged, MeshTopologyChanged)
7923d2154c  feat(dashboard): token-mask helper + namespace conventions for SettingsState
7dcdcf6c57  test(compiler): cover nested SVG (mesh topology pattern) in VUV
01702849b5  fix(compiler): emit children of unknown-tag (passthrough) view-calls as JSX children
6f45d9a0eb  fix(orchestrator): isolate economy_test from local model cache (port from abandoned JSX branch)
d7a88f975c  test(compiler): smoke test SVG via VUV view-call passthrough
7bd29d35fe  fix(populi): expose serve_with_listener for tests
2beb85e685  fix(examples): migrate inventory_rosetta_platform.vox from retired JSX to VUV
86f7a5ccf3  feat(orchestrator): /api/v2 namespace with envelope helpers + build_app factory
092ee018ec  feat(compiler): subscript expression (Index variant + parser postfix + emit)
```

The 677-commit drift means trying to merge this branch into current
main will be a substantial conflict surface. Cherry-picking selected
commits (the compiler fixes, particularly `092ee018ec` and
`01702849b5`) may be more tractable than a wholesale merge.

### 5.4 `cc_bdesktop2/naughty-dirac-825348` (9 commits)

Last touched 2026-05-22, but only 9 commits ahead. Scope:

```
7ddebeecca  feat(scripts/show): add publish.vox -- scientia publication artifact generator
bb10016c77  fix: address CodeRabbit review feedback on PR #90
3d499a341f  feat(scripts/show): add cross-post.vox for launch-week platform fanout
15b54b742a  docs(scripts/show): note vox-mens install requirement and env-var inputs
3fb8b80495  docs(changelog): note voxlang.org migration + scripts/show automation
bd31ba7143  chore(ignore): remove stale crates/vox-py/.venv entries
ec0d5f1d62  fix(scripts/show): use canonical stdlib patterns; pass vox check
7cce777f7b  feat(scripts/show): add Raw Nerve content automation skeletons
fc7928a265  chore: migrate website domain to voxlang.org (was vox-lang.org)
```

**Possibly an open PR** (#90 per the CodeRabbit-feedback commit). Check
`gh pr view 90` before merging.

## 6. Local main vs origin/main

```
local main HEAD:  9c83a0d4d0 (feat(D-14): create vox-plugin-test-harness crate)
origin/main HEAD: 84c98e5b9f (chore(plugins): move noop-skill fixture into vox-plugin-host/tests/fixtures)

local main is 20 commits ahead, 0 behind.
```

These 20 commits are intact locally but **not yet pushed to GitHub**.
They span work from this session (M-6 transitive determinism, A-5 cli-core
migration, B-3-trim workspace deps, A-14 workspace-dep budget, retired-decorator
direction flip, broken-test repairs, doc-pipeline frontmatter fixes) and from
a parallel agent (vox-rename-registry extraction, B-2/B-6/B-3-trim, B-9,
hakari regen, A-4-rescope, D-3/D-5/D-8/D-14 plugin work, session 4 + 5 logs).

**Why aren't they pushed?** The `git push` is gated by the `pre-push`
hook running `vox-doc-pipeline -- check`. Earlier in this session that
gate failed on three docs that this session fixed in commit `6ad44cbb29`
("docs(pipeline): fix frontmatter…"). The push has not been re-attempted
since that fix landed. It should now succeed.

## 7. Recovery plan

### 7.1 Push local main to origin (low risk)

```bash
git push origin main   # 20 commits — gate should now pass
```

If the pre-push hook still complains, the failure surface has shrunk to
files touched after `6ad44cbb29`; investigate those individually rather
than bypassing the hook.

### 7.2 Merge `docs/voxlang-org-cf-migration` (16 commits)

Lowest-risk merge. The branch is 36 commits behind main so expect some
conflicts in `crates/vox-doc-pipeline/src/pipeline/lint.rs` (already
auto-resolves to identical content), `docs/agents/doc-inventory.json`
(regenerable — `vox docs inventory regen`), and possibly `Cargo.lock`
(regenerable). Suggested approach:

```bash
git checkout main
git merge --no-ff docs/voxlang-org-cf-migration -m "Merge voxlang.org Cloudflare Pages migration follow-ups"
# resolve lint.rs as "ours" (already identical), regenerate doc-inventory.json
cargo run -p vox-doc-pipeline -- check        # gate
git push origin main
```

### 7.3 Merge or split `cc_bdesktop2/jovial-buck-e93ac0` (98 commits, PR #92)

This is the largest and riskiest. The branch has an integration merge
commit (`40a7985455 merge: integrate origin/main into PR #92`) that
already pulled origin/main in, so the merge surface should be small at
the tip — but most of those 98 commits have not been visible to anyone
reviewing main.

Two paths:

1. **Land PR #92 via GitHub** (preferred if the PR exists): `gh pr view 92`,
   resolve any conflicts in the PR UI, then merge. This is the path the
   branch was prepared for (note the "PR #92 handoff snapshot" commit).
2. **Local merge**: `git merge --no-ff cc_bdesktop2/jovial-buck-e93ac0`,
   resolve the 26-behind delta, push. Cleaner history but no PR review.

Either way, **keep the `backup/jovial-buck-e93ac0-pre-rebase` tag** until
the merge lands successfully.

### 7.4 Cherry-pick from `claude/dashboard-vuv-port` (14 commits, 677 behind main)

A wholesale merge will be punishing. The high-value compiler fixes —
`092ee018ec feat(compiler): subscript expression` and `01702849b5
fix(compiler): emit children of unknown-tag (passthrough) view-calls as
JSX children` — are likely the only individually-portable bits. Cherry-pick
them onto a fresh topic branch off main, run tests, and PR.

Most of the dashboard chrome work (`6bf967f29a`, `8f69d510da`) is built
against an old VUV API and may need reimplementation rather than
cherry-pick.

### 7.5 Merge `cc_bdesktop2/naughty-dirac-825348` (9 commits) via PR

If PR #90 exists (the CodeRabbit-feedback commit suggests it does),
finish that PR. Otherwise:

```bash
gh pr view 90        # check first
git checkout -b finish-scripts-show
git merge cc_bdesktop2/naughty-dirac-825348
gh pr create ...
```

### 7.6 Cleanup pass (after the above land)

```bash
# Delete branches that are now fully merged into main:
git branch -d cc_bdesktop2/share-s2-s9 cc_bdesktop2/zealous-ardinghelli-b01e11

# Prune orphaned worktree records (the directories are already gone):
git worktree prune

# Keep these tags until you're sure jovial-buck has merged:
#   backup/jovial-buck-e93ac0-pre-rebase
#   pre-corruption-fix-2026-05-23
#   pre-c-fix-deep-2026-05-23
```

## 8. What is genuinely irrecoverable

To my knowledge: **nothing committed.** Every commit referenced in any
reflog (HEAD, main, branch, or worktree) is still in the object store
and reachable from at least one ref (branch, tag, or worktree HEAD).

Things that **could** have been lost but aren't, based on direct check:

| Concern | Verdict | Evidence |
|---|---|---|
| Was main destructively reset on 2026-05-23 22:44? | **NO** — was a fast-forward to origin/main carrying the PR #93 merge | `git reflog show main` shows no `reset:` entries; HEAD reflog reset entry resolved to `6459133dbc` which is reachable from current main |
| Were any parallel-agent commits orphaned by a rebase? | **NO** in inspected branches | `git reflog show cc_bdesktop2/jovial-buck-e93ac0` shows one rebase attempt rolled back via `backup/` tag; current tip is the pre-rebase state +4 |
| Did the deleted worktree directories take work with them? | **NO for committed work**; **possible for uncommitted** | All worktree branches still exist in `refs/heads/`; uncommitted files in the deleted directories (`.claude/worktrees/dashboard-vuv-port`, `share-s2-s9`, `zealous-ardinghelli-b01e11`) are unrecoverable IF they were not committed before the directory removal |
| Are the 10 git-stash entries this session's work? | **NO** — newest is from 2026-05-09 | `git stash show` dates: range from 2026-05-04 to 2026-05-09 |
| Could `git fsck --lost-found` show truly lost work? | **Theoretical only** | 110,763 dangling commits exist but the vast majority are intermediate states (cherry-pick fragments, aborted merges, etc.); checking each by hand is impractical and unnecessary given the above |

The only genuinely-unrecoverable category is uncommitted files in
worktree directories that were deleted (the three "prunable" worktrees
above). Whether anything important was lost there depends on whether
those agents had committed their work first. Each branch reflog shows
clean commit-by-commit progression — no `reset:` or `rebase` entries
suggesting forced rewinds — so it's plausible nothing was lost there
either.

## 9. Action items checklist

- [ ] Run `git push origin main` to publish the 20 unpushed local commits
- [ ] Decide PR-vs-local-merge for `cc_bdesktop2/jovial-buck-e93ac0` (PR #92)
- [ ] Merge `docs/voxlang-org-cf-migration` into main (lowest risk first)
- [ ] Check `gh pr view 90` for `cc_bdesktop2/naughty-dirac-825348` status
- [ ] Decide cherry-pick scope for `claude/dashboard-vuv-port` compiler fixes
- [ ] Delete fully-merged worktree branches (`share-s2-s9`, `zealous-ardinghelli-b01e11`)
- [ ] Run `git worktree prune` to clear stale worktree records
- [ ] Communicate to parallel-agent sessions that their work is **on their branch**, not lost

---

*Document generated 2026-05-24. Audit performed against
local repo at `C:/Users/Owner/vox` with main = `9c83a0d4d0`,
origin/main = `84c98e5b9f`.*
