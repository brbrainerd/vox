---
name: using-git-worktrees
description: Use when starting feature work that needs isolation, or before running parallel file-mutating agents - creates isolated git worktrees so concurrent work does not collide.
---

# Using Git Worktrees (Vox Adaptation)

## Overview

Isolate a unit of work (or a parallel subagent) in its own checkout so edits and builds do not collide with the main workspace or with sibling agents.

**Announce at start:** "I'm using the using-git-worktrees skill."

## When to use

- Feature work that should not disturb the current workspace.
- A `[PARALLEL-SAFE]` wave where multiple agents mutate files and might race a shared `target/` or step on each other.
- Executing an implementation plan in isolation before integrating.

For disjoint single-crate edits run sequentially, worktrees are optional.

## Procedure

```bash
# create
git worktree add ../wt-<task> -b <branch>     # or an existing branch without -b
# ... work, test, commit inside ../wt-<task> ...
# integrate (from main checkout): merge/cherry-pick the branch, run full tests once
# clean up
git worktree remove ../wt-<task>
git worktree prune
```

## Safety

- One agent per worktree; never share a worktree across concurrent agents.
- Commit (or stash) before removing a worktree.
- **Windows:** if `worktree remove` fails with "directory not empty", `git worktree prune` then retry; a running `vox.exe`/`target` lock can block removal — stop the process first.
- Prune stale worktrees before deleting branches (the pre-push hook walks worktrees).

## Cleanup discipline

Stale `.claude/worktrees/` or `../wt-*` trees accumulate and consume disk. Remove a worktree as soon as its work is integrated.
