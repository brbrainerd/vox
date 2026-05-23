---
title: Git Concurrency Policy
description: Rules for safe git use by agentic workers in the Vox orchestrator
date: 2026-05-09
category: "Architecture SSOTs"
---

# Git Concurrency Policy

Agentic workers in Vox interact with git through `GitExec` in
`crates/vox-orchestrator-mcp/src/git_exec.rs`. This wrapper enforces
a static denylist of commands that are unsafe under concurrent agent
operation.

## Banned commands

The following git commands are rejected at the `GitExec` level and will
never reach the shell. A `tracing::warn!` event is emitted at target
`vox.vcs.exec` when a banned invocation is attempted.

| Command pattern | Why banned |
|---|---|
| `git stash` | Shared stash stack causes silent data loss under parallel agents |
| `git reset --hard` | Discards uncommitted work without recoverability |
| `git clean -f`, `-fd`, `-fdx` | Deletes untracked files; irreversible |
| `git restore .` | Discards working-tree changes |
| `git checkout .` / `-- .` / `-f` | Force-resets working tree |

The ban uses **contiguous arg-vector window matching**, not prefix
matching. `-f` does NOT implicitly ban `-fd`; each pattern is listed
separately.

## VCS capability tokens

Before any write operation (commit or branch create), callers must hold
a `WorkingTreeWrite` capability token (defined in
`crates/vox-orchestrator-types/src/vcs_capability.rs`). Tokens are
minted by `vox_branch_create` and are not constructable by arbitrary
code (soft-private via `#[doc(hidden)] pub fn mint`).

## Commit trailers

Every commit created via `commit_create` in
`crates/vox-orchestrator-mcp/src/vcs_tools/commit_tools.rs` receives
three trailers appended to the commit message:

```
Co-authored-by: <author_name> <author_email>
Vox-Model-Id: <model_id>
Vox-Workspace: W-<workspace_id zero-padded to 6 digits>
```

These trailers make every agentic commit auditable: you can identify
which LLM model and workspace produced any commit in `git log`.

## Secret scan gate

`commit_create` runs `scan_for_secrets` (from
`crates/vox-orchestrator-mcp/src/vcs_tools/secret_scan.rs`) over the
staged diff before committing. If any known secret pattern is detected,
the commit is aborted with `CommitError::SecretsDetected`. Patterns
currently detected: AWS access keys, AWS secret keys, GitHub tokens,
OpenAI keys, Anthropic keys, Slack tokens, Google API keys, PEM private
key headers.

`commit_create` passes `--no-verify` to `git commit`, which means any
`.git/hooks/pre-commit` or `.git/hooks/commit-msg` hooks in the target
repository are **skipped**. The orchestrator's own secret scan replaces
the need for a hook-level check for the patterns above, but operators
should be aware that repo-local hooks will not run for agentic commits.

## Telemetry

All VCS operations emit `tracing` events under the `vox.vcs.*`
namespace:

| Event target | Emitted when |
|---|---|
| `vox.vcs.exec` (debug) | A git command completes successfully via `GitExec` |
| `vox.vcs.exec` (warn) | A banned command is attempted |
| `vox.vcs.commit` (info) | A commit is created by `commit_create` |
| `vox.vcs.branch` (info) | A branch is created by `branch_create` |
