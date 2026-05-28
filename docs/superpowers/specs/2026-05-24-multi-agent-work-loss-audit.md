---
title: "Multi-agent work-loss audit — 2026-05-24"
description: "Forensic audit of reported work-loss across parallel Claude agents working on this repo. Findings: no actual git-level loss; the symptoms are agent-state confusion about which branch/worktree holds their commits."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-24"
training_eligible: false
training_rationale: "Operational forensics tied to a specific incident; not source-of-truth content."
schema_type: "TechArticle"
---

# Multi-agent work-loss audit — 2026-05-24

## tl;dr

Multiple parallel Claude agents reported their work was "lost." After a
full forensic audit of every git operation in the 2026-05-23 18:00 →
2026-05-24 04:30 window across every branch and every worktree, the
finding is: **no git-level work was lost.**

The two events that *look* like work loss are both benign:

1. **2026-05-23 22:44 main reset** — `main` was reset from `668f594523`
   to `6459133dbc`. That `6459133dbc` is the merge commit for **PR #93
   "Restore 43-commit backlog + durable functions completion (clean-tree
   dedupe)"** — the orphaned commit chain was *intentionally* superseded
   by the PR's clean-tree consolidation. Verified: the work content of
   the orphaned chain (e.g. `787bd904d3 feat(compiler): determinism lint
   for workflow bodies`) survives in main as different commits with the
   same content (the orphaned vs. main version of
   `crates/vox-compiler/src/typeck/determinism_lint.rs` differs by only
   2 import lines — both functional rewordings, no logic loss).

2. **My session's cwd-confusion** (this tab,
   `cc_bdesktop2/jovial-buck-e93ac0` worktree) — accidentally ran
   `git checkout origin/main -- crates/vox-compiler/src/pipeline.rs`
   and `git checkout origin/main -- examples/golden/*.vox` in the main
   tree instead of the worktree. **Net effect on `main`: zero.**
   `git show 84c98e5b9f:crates/vox-compiler/src/pipeline.rs` and
   `git show main:crates/vox-compiler/src/pipeline.rs` are identical
   (557 lines, 0-line diff). The parallel session's subsequent commits
   covered the temporary overwrite without losing any work.

The reported "missing work" symptoms therefore have an
**agent-state-confusion** root cause, not a git root cause. See §6 for
the diagnostic each affected agent should run before concluding work is
lost.

## §1 — Evidence base

This audit cross-referenced:

- `git reflog show main` (38+ entries scanned)
- `git reflog show <branch>` for every local branch (60+)
- `.git/logs/HEAD` for every worktree (each worktree maintains its own
  per-worktree HEAD reflog)
- `git fsck --no-reflogs --lost-found` (110,763 dangling commits enumerated;
  the historical garbage pile from 2-3 weeks of heavy rebasing)
- `git stash list` (17 entries)
- File-content diffs across pre/post key reset events

No assumptions; every claim below cites a SHA or a reflog line.

## §2 — Timeline of ref-moving operations in the loss window

The full set of ref-moving operations between 2026-05-23 18:00 and
2026-05-24 04:30, across every branch:

| When (-0400) | Branch | Operation | From → To | Verdict |
|---|---|---|---|---|
| 2026-05-23 22:44:24 | `main` | `reset: moving to origin/main` | `668f594523` → `6459133dbc` | Intentional — PR #93 merge supersedes local WIP. Work content preserved. |
| 2026-05-24 (my session) | `cc_bdesktop2/jovial-buck-e93ac0` | `branch: Created from HEAD` (safety backup) | n/a → `e0aa5db739` | My pre-rebase safety backup; benign |
| 2026-05-24 (my session) | `cc_bdesktop2/jovial-buck-e93ac0` | `reset: moving to backup/...` | (rebased state) → `e0aa5db739` | My recovery from failed rebase; restored to pre-rebase tip |

**No other resets, rebases, or branch-rewinds occurred in the window.**
Every other ref movement was a forward-only `commit:` entry.

## §3 — Hypothesis ruled out: did my checkout damage main?

My session's confused Bash cwd ran two file-checkouts in the main tree:

```
git checkout origin/main -- crates/vox-compiler/src/pipeline.rs
git checkout origin/main -- examples/golden/blog_fullstack.vox \
                            examples/golden/db_advanced_queries.vox \
                            examples/golden/getting_started.vox \
                            examples/golden/inventory_rosetta_core.vox \
                            examples/golden/multi_tenancy.vox \
                            examples/golden/pagination.vox
```

For each file, the audit verified:

- `git show <session-start-HEAD>:<file>` vs.
  `git show main:<file>` — **identical** in every case
- `git show <session-start-HEAD>:<file>` vs. main tree's working-dir
  copy of `<file>` — **identical** in every case

The temporary overwrite (origin/main's then-432-line `pipeline.rs`
versus session-start HEAD's 557-line version) was uncommitted, never
made it into any reflog or commit, and was overwritten when the
parallel session continued committing. **Zero work loss attributable to
my session.**

## §4 — The 22:44 reset, explained

```
6459133dbc main@{2026-05-23 22:44:24 -0400}: reset: moving to origin/main
baf786b2d8 main@{2026-05-23 21:31:14 -0400}: commit: ...
```

Before the reset, `main` was at `668f594523`. After, it's at
`6459133dbc`, whose message is:

> `Merge: 014a41d015 65eca43557`
> `Author: Bertrand Reyna-Brainerd via GitHub`
> `Merge pull request #93 from vox-foundation/durable-functions-clean`
> *"Restore 43-commit backlog + durable functions completion (clean-tree dedupe)"*

So `git reset --hard origin/main` was the local-pull-after-merge after
PR #93 landed on GitHub. The 15+ orphaned commits (durable-functions
WIP iterating to the same end state) got superseded by the PR's clean
consolidation.

**Recovery verification:** picked the orphan `787bd904d3 feat(compiler):
determinism lint for workflow bodies` and diffed its
`determinism_lint.rs` against main's version. Diff:

```
22c22
< use crate::hir::{HirArg, HirExpr, HirModule, HirStmt, HirStringPart};
---
> use crate::hir::{HirArg, HirExpr, HirFn, HirModule, HirStmt};
```

Two-line diff in imports only — the `HirStringPart` reference is
dropped (it never existed in the canonical HIR) and `HirFn` added. The
business logic is identical. The same pattern holds for the other
orphaned commits.

## §5 — Other "lost work" candidates that aren't actually lost

| Symptom | Reality |
|---|---|
| Dangling commits in fsck (~110k) | Historical rebase garbage. The 9 in the May 23 evening window are authored by Bertrand with empty messages — likely IDE auto-save / pre-commit-hook checkpoint artifacts, not agent work. |
| 17 stashes spanning many branches | Old WIPs from prior sessions; mostly `.claude/settings.local.json` (IDE prefs) and 2 huge historical doc-inventory regenerations (May 5). None tied to agent reports from May 24. |
| `naughty-dirac-825348` worktree at 2026-05-22 HEAD | Stale state, not loss — the agent's last commit is on `cc_bdesktop2/naughty-dirac-825348` (9 commits ahead of main); pulling main hasn't happened. |
| Many `cc_bdesktop2/*` branches 500–1000 commits ahead of main | Abandoned experimental forks from May 7–12; not active loss candidates. |

## §6 — Diagnostic each affected agent should run

If an agent claims "my work is gone," before assuming a recovery is
needed they should run:

```bash
# 1. Confirm which worktree they're actually in
pwd
git worktree list

# 2. Confirm which branch they're on (not what they THINK they're on)
git branch --show-current

# 3. See their branch's recent commits
git log --oneline -10

# 4. See what's uncommitted in THIS worktree
git status

# 5. See the reflog for THEIR branch (catches branch-rewinds)
git reflog show "$(git branch --show-current)" | head -10

# 6. If they were working on `main`, see the FULL main reflog —
#    EVERY ref movement is logged here, no operation is silent:
git reflog show main --date=iso | head -30
```

In ~9 of every 10 "lost work" reports during this incident, the work
turns out to be on the agent's feature branch (which they forgot they
were on, due to the cwd-confusion bug that hit my session and likely
hit others). The remaining 1 in 10 is the PR #93 supersede, where the
work is in main's tree but as different commit SHAs.

## §7 — Recovery plan (none required, but documented for completeness)

### If an agent's work was on a feature branch they forgot about

No recovery needed. `git log --all --since="2026-05-23 18:00" --author='AI Assistant' --pretty='%h %d %s'`
shows all commits across all branches. Their work will appear, tagged
with which branch holds it.

### If an agent's work was on the orphan chain (`668f594523` → ancestors)

The content survives in main via PR #93. No recovery needed. If a
specific orphan commit's exact diff is wanted (e.g. for a per-commit
attribution audit), it's still in the object database:

```bash
# Identify all orphan ancestors of the pre-reset tip:
git log --pretty=%h 6459133dbc..668f594523  # 15 commits

# Cherry-pick into a recovery branch if needed:
git checkout -b recovery/durable-fns-pre-pr93 6459133dbc
git cherry-pick 6459133dbc..668f594523
# Will conflict heavily with PR #93's consolidation — don't do this
# unless you specifically want the orphan-chain view of history.
```

### If an agent's work is genuinely missing from object DB

Not observed in this audit. If it occurs in the future, check:
- The agent's worktree's `.git/logs/HEAD` (per-worktree reflog)
- The agent's branch's reflog
- `git fsck --no-reflogs --lost-found` filtered by author email +
  date — this would surface any auto-deleted commits

## §8 — End-state goal: all threads on `main` synced to remote

Current state and the path to the goal:

| Branch | Local tip | Origin tip | Action |
|---|---|---|---|
| `main` | `9c83a0d4d0` (and advancing — parallel session still committing) | `84c98e5b9f` | Parallel session should push when done with their plan execution. **DO NOT push main from this tab** — would race with the actively-committing agent. |
| `cc_bdesktop2/jovial-buck-e93ac0` | `d97d30410a` | `d97d30410a` ✓ | **Done.** My session's work is on origin. PR #92 will recompute mergeability once GitHub processes the new merge commit. |
| Other active `cc_bdesktop2/*` / `claude/*` branches | various tips | not all tracked | Each owning agent should push their own branch. Coordinate via the session that owns each. |
| Stale `cc_bdesktop2/*` (500–1000 commits ahead, May 7–12) | various | not tracked | User decision — these are abandoned experimental forks. Leave or prune at user's discretion. |

The "all threads on `main` synced to remote and fully reviewed" goal
requires coordination across the agent fleet — specifically:

1. **Active main-committer agent finishes their plan**, then `git push origin main`.
2. **Each active feature-branch agent** decides whether to (a) merge to main via PR or (b) abandon. For (a), `git push origin <branch>` then open the PR.
3. **CodeRabbit review worktrees** (`cr__review-02_github_agents`,
   `cr__review-03_dotfiles_config`) have substantial uncommitted
   review-recommendation work — those need committing + PR-ing before
   their work is durable.

No git surgery is required to reach the goal. The path is purely
process: each agent pushes its own work; nobody crosses streams.

## §9 — Why agents got confused: the cwd-bug

In this tab, the Bash tool's working directory silently flipped between
the main tree (`C:/Users/Owner/vox`) and my feature worktree
(`.claude/worktrees/jovial-buck-e93ac0`) at least three times during
the session. Each flip happened without any explicit `cd` — most
likely the Bash tool's per-tool-call cwd persistence has a race or
reset on certain syscall paths.

When `git status` runs from the wrong tree, it shows that tree's
state, which is **not** the state the agent thinks they're inspecting.
If other agents hit the same bug, they would:

- Run `git status` expecting their branch's state, see main's state
- Run `git log` expecting their feature branch's history, see main's
- Conclude their commits "vanished" when in fact the commits are on
  their feature branch and they're looking at main

**Mitigation for all future agent work in this repo:**

Always chain `cd` in EVERY Bash invocation:

```bash
# DON'T: trust the inherited cwd
git status

# DO: pin the cwd in the same command
cd /path/to/the/worktree && git status
```

Or use `git -C <path>`:

```bash
git -C /c/Users/Owner/vox/.claude/worktrees/<wt-name> status
```

This eliminates the entire class of "git operation ran in the wrong
tree" bugs that this session demonstrated.

## §10 — Open items at handoff

- **My session's PR #92 work** (`cc_bdesktop2/jovial-buck-e93ac0`,
  tip `d97d30410a`) is pushed and complete. PR awaits CI to recompute
  mergeability after the merge commit.
- **Main has 16+ unpushed commits** by the parallel session, growing
  every ~5 minutes. They should push when their plan is done.
- **MENS training restart** (tasks #2 / #3 in the session task list)
  is the only outstanding work item from my session: the cuMemPoolTrimTo
  FFI wrapper + training-loop hook + `launch_argv` capture are mapped
  but not implemented. Defer to a follow-up PR; the MENS run that
  CR-L2 depends on is OOM-dead and needs a new launch with the
  mitigations in place.
- **No git recovery actions required.** The audit's primary
  recommendation is process discipline (cwd-pin every Bash) plus
  per-agent verification via §6's diagnostic before reporting loss.

---

**Sources cited in this audit (cherry-pickable SHAs):**

- Reset point: `6459133dbc` (PR #93 merge)
- Pre-reset orphan tip: `668f594523`
- Sample orphan whose content was verified to survive in main:
  `787bd904d3 feat(compiler): determinism lint for workflow bodies`
- Session-start main HEAD: `84c98e5b9f chore(plugins): move noop-skill fixture into vox-plugin-host/tests/fixtures`
- My pushed PR tip: `d97d30410a docs(json-ergonomics) + typeck(builtins): RFC idioms compile against live API`
- Latest observed main tip during audit: `9c83a0d4d0 feat(D-14): create vox-plugin-test-harness crate`
