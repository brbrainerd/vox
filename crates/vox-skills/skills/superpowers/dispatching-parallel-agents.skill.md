---
name: dispatching-parallel-agents
description: Use when 2+ independent tasks can run without shared state or sequential dependencies - decomposes work for concurrent subagents and integrates results safely.
---

# Dispatching Parallel Agents (Vox Adaptation)

## Overview

Run independent work concurrently across subagents (Vox Orchestrator workers, or an IDE's dynamic subagents) and integrate the results without clobbering.

**Announce at start:** "I'm using the dispatching-parallel-agents skill."

## When to use

Two or more tasks that (a) touch **disjoint file sets** and (b) have **no output dependency** on each other. If either is false, run them sequentially on one agent.

## The Golden Rule

**Never dispatch two agents that write the same file.** Subagents run in isolated context windows; they will not see each other's edits and the last writer wins. When in doubt, mark the task SEQUENTIAL.

## Procedure

1. **Tag** each task `[PARALLEL-SAFE]` or `[SEQUENTIAL]` by file-set disjointness and dependency.
2. **Group** parallel-safe tasks into waves; within a wave all file sets are disjoint.
3. **Dispatch** one subagent per task with ONLY that task's text as context (keep windows small).
4. **Barrier** — wait for all subagents in a wave to return green before integrating.
5. **Integrate** sequentially: pull each result, run the full crate test suite once, resolve any surprise overlap.
6. **Two-strike rule** — if a subagent fails its verification twice, STOP it and surface its handoff note; do not re-dispatch the same failing work.

## Isolation

For parallel tasks that mutate files and might race a shared build/target dir, give each subagent its own git worktree (see `using-git-worktrees`). For disjoint single-crate edits, worktrees are usually unnecessary.

## Anti-patterns

- Parallelizing tasks that share a file "because it's faster" — it corrupts work.
- Forwarding the whole conversation to each subagent — wastes context and invites cross-talk.
- Masking subagent failures with catch-all error handling — breaks resumability.
