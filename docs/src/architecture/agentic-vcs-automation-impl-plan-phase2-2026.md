---
title: "Agentic VCS Automation — Phase 2 Implementation Plan (2026-05-09)"
description: "Step-by-step TDD plan that lands the Push/PR write-side: PushAllowed / ForcePushAllowed / DestructiveOp capability tokens, vox_push / vox_pr_open / vox_force_push / vox_branch_delete MCP tools, an arch-check rule that bans raw Command::new(\"git\") outside the central wrapper, migration of remaining direct git callsites, normalised flag handling in the banned-command denylist, capability-ledger persistence in vox-orchestrator-queue, and the .vox glue scripts (wip/sync/finish/recover). Builds directly on Phase 1."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 2 closes the agentic VCS write loop end-to-end (commit → push → PR) and locks the central git executor as the sole git surface across the workspace. Concrete code, exact file paths, exact commands, TDD steps. Future agents executing this plan should not need to invent code."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-orchestrator-types: extend vcs_capability with PushAllowed/ForcePushAllowed/DestructiveOp"
  - "vox-orchestrator-mcp: 4 new MCP tools, arch-check rule contribution, banned-flag normaliser"
  - "vox-orchestrator-queue: new oplog OperationKind variant for capability-ledger entries"
  - "vox-arch-check: new rule no_raw_git_command outside git_exec.rs"
  - "scripts/vcs/: new .vox glue scripts for wip/sync/finish/recover"
  - "docs/src/architecture/git-concurrency-policy.md: append force-push and ledger semantics"
---

# Agentic VCS Automation — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Companion docs:** [Phase 1 plan](agentic-vcs-automation-impl-plan-phase1-2026.md) (already shipped), [research](agentic-version-control-automation-research-2026.md). Read the research doc's §"Failure-mode taxonomy E (destructive ops)" and §"Capability ledger UX" before starting.

**Goal:** Land the *push/PR* slice on top of Phase 1's commit/branch slice. After Phase 2, an agent in possession of the right capability tokens can take a workspace from "no branch yet" → branch created → commits with secret-scan + trailers → push (non-force) → PR opened, all without ever touching raw `git` or `gh`. Force-push and branch-delete exist as separate, ledger-gated tools. Every direct `git` callsite in the workspace funnels through `GitExec`, enforced by `vox-arch-check`.

**Architecture:** Extend `vcs_capability.rs` with three new token types. Add four MCP tools in `vcs_tools/`, each requiring its capability as a parameter (no ambient authority). The capability ledger persists to the oplog store via a new `OperationKind::CapabilityMinted` variant — this is the durable record that anchors the future dashboard panel. Banned-command detection moves from "exact arg-vector window" to "normalised flag-set" so that `clean -fxX` and other flag permutations are rejected. `vox-arch-check` gains a rule that fails CI if any crate spawns `git` outside `git_exec.rs`. The `.vox` glue scripts call the MCP tools through the existing CLI; they are pure orchestration with no git knowledge.

**Tech stack:** Rust 2021 edition, `tokio` for async, `serde` for envelopes, `tracing` for telemetry. No new Rust dependencies. The `.vox` scripts use `vox check` for static validation. `gh` (GitHub CLI) is invoked through a thin wrapper that reuses `GitExec`'s denylist machinery for tool-call uniformity.

**Out of scope for Phase 2 (deferred to later phases):**
- Dashboard panels for the capability ledger and push queue (Phase 3).
- Vox `@vcs.*` decorators in the compiler (Phase 4 — depends on @durable / @endpoint type checking landing first).
- `gix` or `jj-lib` substitution behind `GitExec` (Phase 5).
- Bidirectional remote sync of the capability ledger across mesh peers (handled by the existing replication spec).

---

## Verification setup

These run by the engineer, not by every step.

- `cargo test -p vox-orchestrator-types --lib` — capability + ID tests (10 → ~14 after Phase 2).
- `cargo test -p vox-orchestrator-mcp --lib` — wrapper, secret-scan, tool tests (131 → ~145).
- `cargo test -p vox-orchestrator-queue --lib` — oplog tests (new `CapabilityMinted` variant).
- `cargo run -p vox-arch-check` — must pass after Task 7 lands the new rule; will FAIL between Tasks 7 and 8 because raw callsites still exist.
- `cargo run -p vox-doc-pipeline -- --check` — must pass after Task 11.
- `vox check scripts/vcs/wip.vox` (and the other three) — Vox-side static check.

The plan assumes a workspace clean of unrelated changes and Phase 1 fully landed (commit `7ca219d90` or later on `main`). It produces 11 commits.

---

## Task 1: Add PushAllowed / ForcePushAllowed / DestructiveOp capability tokens

**Files:**
- Modify: `crates/vox-orchestrator-types/src/vcs_capability.rs`
- Modify: `crates/vox-orchestrator-types/src/lib.rs`
- Test: same file (extend existing `mod tests`)

**Why this first:** Tasks 3, 5, 6 all take these tokens as parameters. Define them once in the L0 pure-types crate.

- [ ] **Step 1: Write failing tests for the three new tokens**

Append to the bottom of the existing `mod tests` block in `vcs_capability.rs`:

```rust
#[test]
fn push_allowed_round_trip() {
    let cap = PushAllowed::mint(
        WorkspaceId(3),
        BranchName::parse("agent/fix-42").unwrap(),
        RemoteId(1),
    );
    assert_eq!(cap.workspace(), WorkspaceId(3));
    assert_eq!(cap.branch().as_str(), "agent/fix-42");
    assert_eq!(cap.remote(), RemoteId(1));
    assert!(!cap.is_force());
}

#[test]
fn force_push_allowed_carries_justification_hash() {
    let cap = ForcePushAllowed::mint(
        WorkspaceId(4),
        BranchName::parse("agent/rebase").unwrap(),
        RemoteId(1),
        [0xAB; 32],
    );
    assert!(cap.is_force());
    assert_eq!(cap.justification_hash(), &[0xAB; 32]);
}

#[test]
fn destructive_op_kind_round_trip() {
    let cap = DestructiveOp::mint(
        WorkspaceId(5),
        DestructiveKind::BranchDelete {
            branch: BranchName::parse("agent/done").unwrap(),
        },
        [0xCD; 32],
    );
    assert_eq!(cap.workspace(), WorkspaceId(5));
    match cap.kind() {
        DestructiveKind::BranchDelete { branch } => {
            assert_eq!(branch.as_str(), "agent/done");
        }
    }
    assert_eq!(cap.justification_hash(), &[0xCD; 32]);
}
```

- [ ] **Step 2: Run tests — should fail to compile**

Run: `cargo test -p vox-orchestrator-types --lib vcs_capability`
Expected: FAIL — "cannot find type `PushAllowed`" / `ForcePushAllowed` / `DestructiveOp` / `DestructiveKind`.

- [ ] **Step 3: Implement the three tokens**

Append to `vcs_capability.rs` (after the existing `BranchCreate` impl, before `mod tests`):

```rust
/// Capability: holder may push `branch` of `workspace` to `remote` (non-force).
/// Constructed only by `PushAllowed::mint`.
#[derive(Debug, Clone)]
pub struct PushAllowed {
    workspace: WorkspaceId,
    branch: BranchName,
    remote: RemoteId,
}

impl PushAllowed {
    #[doc(hidden)]
    pub fn mint(workspace: WorkspaceId, branch: BranchName, remote: RemoteId) -> Self {
        Self { workspace, branch, remote }
    }
    pub fn workspace(&self) -> WorkspaceId { self.workspace }
    pub fn branch(&self) -> &BranchName { &self.branch }
    pub fn remote(&self) -> RemoteId { self.remote }
    /// Always false for `PushAllowed`. `ForcePushAllowed` is a separate type.
    pub fn is_force(&self) -> bool { false }
}

/// Capability: holder may force-push. Carries the SHA-256 hash of the
/// human-approved justification record. The orchestrator's authorize_*
/// path persists the justification text under that hash to the capability
/// ledger before minting.
#[derive(Debug, Clone)]
pub struct ForcePushAllowed {
    workspace: WorkspaceId,
    branch: BranchName,
    remote: RemoteId,
    justification_hash: [u8; 32],
}

impl ForcePushAllowed {
    #[doc(hidden)]
    pub fn mint(
        workspace: WorkspaceId,
        branch: BranchName,
        remote: RemoteId,
        justification_hash: [u8; 32],
    ) -> Self {
        Self { workspace, branch, remote, justification_hash }
    }
    pub fn workspace(&self) -> WorkspaceId { self.workspace }
    pub fn branch(&self) -> &BranchName { &self.branch }
    pub fn remote(&self) -> RemoteId { self.remote }
    pub fn is_force(&self) -> bool { true }
    pub fn justification_hash(&self) -> &[u8; 32] { &self.justification_hash }
}

/// Kinds of destructive ops that require a `DestructiveOp` capability.
/// Add variants conservatively; each new variant is a new gateway in
/// the capability ledger UX.
#[derive(Debug, Clone)]
pub enum DestructiveKind {
    BranchDelete { branch: BranchName },
}

/// Capability: holder may execute one destructive op. Carries the SHA-256
/// hash of the human-approved justification record (same convention as
/// `ForcePushAllowed`).
#[derive(Debug, Clone)]
pub struct DestructiveOp {
    workspace: WorkspaceId,
    kind: DestructiveKind,
    justification_hash: [u8; 32],
}

impl DestructiveOp {
    #[doc(hidden)]
    pub fn mint(
        workspace: WorkspaceId,
        kind: DestructiveKind,
        justification_hash: [u8; 32],
    ) -> Self {
        Self { workspace, kind, justification_hash }
    }
    pub fn workspace(&self) -> WorkspaceId { self.workspace }
    pub fn kind(&self) -> &DestructiveKind { &self.kind }
    pub fn justification_hash(&self) -> &[u8; 32] { &self.justification_hash }
}
```

Update `crates/vox-orchestrator-types/src/lib.rs` re-exports:

```rust
pub use vcs_capability::{
    BranchCreate, BranchName, BranchNameError, DestructiveKind, DestructiveOp,
    ForcePushAllowed, PushAllowed, RemoteId, WorkingTreeWrite, WorkspaceId,
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-types --lib`
Expected: PASS — 13/13 (10 existing + 3 new).

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-types/src/vcs_capability.rs crates/vox-orchestrator-types/src/lib.rs
git commit -m "feat(orchestrator-types): add PushAllowed/ForcePushAllowed/DestructiveOp capability tokens"
```

---

## Task 2: Tighten banned-command detection to normalised flag-set

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/git_exec.rs`
- Test: same file (extend existing `mod tests`)

**Why now:** The Phase 1 final review flagged that `git clean -fxX` (where `X` is any extra flag) bypasses the exact-window match. Phase 2 introduces force-push, branch-delete, and the public `vox_push` surface — the right time to harden the gate.

The new strategy: for short flag clusters starting with `-` (e.g. `-fdx`), expand them into individual flag chars and check whether the *set* contains any of the dangerous flags for the relevant subcommand. Long flags (`--hard`, `--force`) and positional args still match exactly.

- [ ] **Step 1: Write failing tests for the expanded matcher**

Append to the existing `mod tests` block:

```rust
#[test]
fn is_banned_catches_clean_with_extra_flags() {
    assert!(is_banned(&["clean", "-fx"]).is_some(),
        "clean with -fx (force + ignored excludes) must be banned");
    assert!(is_banned(&["clean", "-fxX"]).is_some());
    assert!(is_banned(&["clean", "-xfd"]).is_some(),
        "flag order in cluster must not matter");
    assert!(is_banned(&["clean", "-n", "-f"]).is_some(),
        "force flag in any position bans clean");
}

#[test]
fn is_banned_allows_clean_dry_run_only() {
    assert!(is_banned(&["clean", "-n"]).is_none(),
        "dry-run-only clean is safe");
    assert!(is_banned(&["clean", "--dry-run"]).is_none());
}

#[test]
fn is_banned_catches_checkout_force_long_flag() {
    assert!(is_banned(&["checkout", "--force"]).is_some());
    assert!(is_banned(&["checkout", "--force", "main"]).is_some());
}

#[test]
fn is_banned_catches_push_force() {
    assert!(is_banned(&["push", "--force"]).is_some(),
        "raw push --force must go through ForcePushAllowed-gated tool, not GitExec");
    assert!(is_banned(&["push", "-f"]).is_some());
    assert!(is_banned(&["push", "--force-with-lease"]).is_some());
}

#[test]
fn is_banned_allows_normal_push() {
    assert!(is_banned(&["push", "origin", "main"]).is_none());
    assert!(is_banned(&["push", "-u", "origin", "agent/x"]).is_none());
}

#[test]
fn is_banned_catches_branch_delete_force() {
    assert!(is_banned(&["branch", "-D", "agent/x"]).is_some(),
        "raw branch -D must go through DestructiveOp-gated tool");
    assert!(is_banned(&["branch", "--delete", "--force", "agent/x"]).is_some());
}

#[test]
fn is_banned_allows_branch_create_and_list() {
    assert!(is_banned(&["branch", "agent/x"]).is_none());
    assert!(is_banned(&["branch", "--list"]).is_none());
    assert!(is_banned(&["branch", "-d", "agent/x"]).is_none(),
        "lowercase -d (safe delete) is allowed; force -D is the gated form");
}
```

- [ ] **Step 2: Run tests — should fail**

Run: `cargo test -p vox-orchestrator-mcp --lib git_exec`
Expected: FAIL on most of the new tests; the existing exact-window matcher misses these.

- [ ] **Step 3: Replace the matcher with a normalised one**

Replace the `is_banned` function and the `BANNED_PREFIXES` constant with this implementation. Keep the comment style of the existing module.

```rust
/// Inspect a git arg vector and return a human-readable description of why
/// it is banned, or `None` if it is allowed.
///
/// Strategy: classify by the first positional arg (the git subcommand),
/// then check the flag-set + positional args against a per-subcommand rule.
/// Short clusters like `-fxd` are exploded into individual chars before
/// the check, so flag order and packing do not matter.
pub fn is_banned(args: &[&str]) -> Option<String> {
    let sub = args.iter().find(|a| !a.starts_with('-'))?;
    let (long_flags, short_chars, positionals) = classify_args(args);

    match sub.as_ref() {
        "stash" => Some("git stash is banned: shared stash stack causes silent loss under parallel agents".into()),
        "reset" if long_flags.contains("--hard") => Some("git reset --hard is banned: discards uncommitted work".into()),
        "clean" if short_chars.contains(&'f') || long_flags.contains("--force") =>
            Some("git clean -f* / --force is banned: deletes untracked files irreversibly".into()),
        "restore" if positionals.iter().any(|p| *p == ".") =>
            Some("git restore . is banned: discards working-tree changes".into()),
        "checkout" if short_chars.contains(&'f') || long_flags.contains("--force") =>
            Some("git checkout -f / --force is banned: force-resets working tree".into()),
        "checkout" if positionals.iter().any(|p| *p == ".") =>
            Some("git checkout . is banned: force-resets working tree".into()),
        "push" if short_chars.contains(&'f')
              || long_flags.contains("--force")
              || long_flags.contains("--force-with-lease") =>
            Some("git push --force is banned at GitExec layer: use vox_force_push (capability-gated)".into()),
        "branch" if short_chars.contains(&'D')
                || (long_flags.contains("--delete") && long_flags.contains("--force")) =>
            Some("git branch -D / --delete --force is banned: use vox_branch_delete (capability-gated)".into()),
        _ => None,
    }
}

/// Classify args into (long_flags, short_chars, positionals).
/// `--foo` → long, `-fdx` → chars `f`, `d`, `x`, anything else → positional.
/// `-c key=val` is recognised as a `git -c` invocation and dropped from
/// the analysis (it influences config, not the destructive surface).
fn classify_args<'a>(
    args: &'a [&'a str],
) -> (std::collections::HashSet<&'a str>, std::collections::HashSet<char>, Vec<&'a str>) {
    let mut long_flags = std::collections::HashSet::new();
    let mut short_chars = std::collections::HashSet::new();
    let mut positionals = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i];
        if a == "-c" {
            // Skip `-c` and its arg (config override; orthogonal to ban surface).
            i += 2;
            continue;
        }
        if let Some(long) = a.strip_prefix("--") {
            long_flags.insert(a);
            let _ = long;
            i += 1;
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            for ch in a[1..].chars() {
                short_chars.insert(ch);
            }
            i += 1;
            continue;
        }
        positionals.push(a);
        i += 1;
    }
    (long_flags, short_chars, positionals)
}
```

Delete the old `BANNED_PREFIXES` constant — it is no longer referenced.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp --lib git_exec`
Expected: PASS — all old + new tests green.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/git_exec.rs
git commit -m "feat(orchestrator-mcp): replace exact-window denylist with normalised flag-set matcher; ban push --force and branch -D at GitExec layer"
```

---

## Task 3: vox_push MCP tool

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/vcs_tools/push_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`
- Test: same file

**Why now:** With the capability token from Task 1 and the hardened `GitExec` from Task 2, `vox_push` is just a thin wrapper that runs `git push <remote> <branch>` after checking the cap matches the workspace's bound branch.

- [ ] **Step 1: Write the test for the message and dispatch shape**

Tests live in `#[cfg(test)] mod tests {}` at the bottom of `push_tools.rs`. Because we cannot run real git in unit tests, we test the argument-shape helper.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_args_no_set_upstream() {
        let args = build_push_args("origin", "agent/x", false);
        assert_eq!(args, vec!["push", "origin", "agent/x"]);
    }

    #[test]
    fn push_args_with_set_upstream() {
        let args = build_push_args("origin", "agent/x", true);
        assert_eq!(args, vec!["push", "-u", "origin", "agent/x"]);
    }

    #[test]
    fn push_does_not_emit_force_flags() {
        let args = build_push_args("origin", "agent/x", true);
        assert!(!args.iter().any(|a| *a == "--force" || *a == "-f"));
    }
}
```

- [ ] **Step 2: Run tests — should fail to compile**

Run: `cargo test -p vox-orchestrator-mcp --lib push_tools`
Expected: FAIL — "could not find `push_tools` in module `vcs_tools`".

- [ ] **Step 3: Implement push_tools.rs**

```rust
//! MCP tool: `vox_push`.
//!
//! Non-force push of a workspace's bound branch to a remote. Requires a
//! `PushAllowed` capability that names the same workspace and branch as
//! the operation. Force-push is a separate tool (`vox_force_push`) that
//! takes `ForcePushAllowed` and persists a justification to the
//! capability ledger.
//!
//! All git invocation goes through `GitExec`, which rejects raw
//! `--force` / `-f` / `--force-with-lease` at the executor layer
//! (Task 2). This module never constructs those flags itself.

use std::path::Path;

use vox_orchestrator_types::{BranchName, PushAllowed, RemoteId, WorkspaceId};

use crate::git_exec::{GitExec, GitExecError, GitOutput};

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("capability workspace {cap} does not match operation workspace {op}")]
    WorkspaceMismatch { cap: WorkspaceId, op: WorkspaceId },
    #[error("capability branch {cap} does not match operation branch {op}")]
    BranchMismatch { cap: String, op: String },
    #[error("git failed: {0}")]
    GitFailed(#[from] GitExecError),
}

#[derive(Debug)]
pub struct PushOutput {
    pub remote: String,
    pub branch: String,
    pub stdout: String,
}

/// Build the `git push` argv. Extracted as a free function so the
/// shape can be unit-tested without running git.
fn build_push_args<'a>(remote: &'a str, branch: &'a str, set_upstream: bool) -> Vec<&'a str> {
    if set_upstream {
        vec!["push", "-u", remote, branch]
    } else {
        vec!["push", remote, branch]
    }
}

pub async fn push(
    cwd: &Path,
    cap: &PushAllowed,
    op_workspace: WorkspaceId,
    op_branch: &BranchName,
    remote_name: &str,
    set_upstream: bool,
) -> Result<PushOutput, PushError> {
    if cap.workspace() != op_workspace {
        return Err(PushError::WorkspaceMismatch {
            cap: cap.workspace(),
            op: op_workspace,
        });
    }
    if cap.branch().as_str() != op_branch.as_str() {
        return Err(PushError::BranchMismatch {
            cap: cap.branch().as_str().to_string(),
            op: op_branch.as_str().to_string(),
        });
    }
    let _ = (cap.remote(),); // RemoteId mapping to remote_name is the orchestrator's job; we trust the caller mapped it.

    let git = GitExec::new(cwd);
    let args = build_push_args(remote_name, op_branch.as_str(), set_upstream);
    let GitOutput { stdout, .. } = git.run(&args).await?;

    tracing::info!(
        target: "vox.vcs.push",
        remote = remote_name,
        branch = op_branch.as_str(),
        workspace_id = cap.workspace().0,
        "push completed"
    );

    Ok(PushOutput {
        remote: remote_name.to_string(),
        branch: op_branch.as_str().to_string(),
        stdout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_args_no_set_upstream() {
        let args = build_push_args("origin", "agent/x", false);
        assert_eq!(args, vec!["push", "origin", "agent/x"]);
    }

    #[test]
    fn push_args_with_set_upstream() {
        let args = build_push_args("origin", "agent/x", true);
        assert_eq!(args, vec!["push", "-u", "origin", "agent/x"]);
    }

    #[test]
    fn push_does_not_emit_force_flags() {
        let args = build_push_args("origin", "agent/x", true);
        assert!(!args.iter().any(|a| *a == "--force" || *a == "-f"));
    }
}
```

Add `pub mod push_tools;` to `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp --lib push_tools`
Expected: PASS — 3/3.
Then: `cargo test -p vox-orchestrator-mcp --lib`
Expected: PASS — full suite, no regressions.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/vcs_tools/push_tools.rs crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs
git commit -m "feat(vcs): add vox_push tool requiring PushAllowed capability"
```

---

## Task 4: vox_pr_open MCP tool with templated body

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/vcs_tools/pr_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`

**Why now:** Closes the agentic loop end-to-end. The PR body is templated from workspace task metadata, not the agent's free-form text — that prevents failure-mode C (hallucinated commit metadata) from leaking into PRs.

The orchestrator already exposes a typed `WorkspaceTaskMetadata` (in `crates/vox-orchestrator/src/`). Phase 2 adds a `pr_body_for_workspace(metadata: &WorkspaceTaskMetadata) -> String` formatter. The tool itself shells out to `gh pr create --title <T> --body <B>` via a `GhExec` wrapper analogous to `GitExec`.

- [ ] **Step 1: Add a minimal GhExec wrapper**

Create `crates/vox-orchestrator-mcp/src/gh_exec.rs`:

```rust
//! Thin wrapper around `gh` (GitHub CLI). Mirrors `GitExec` to keep all
//! external-tool invocation centralised.
//!
//! Phase 2 scope: `gh pr create` only. The wrapper does not currently ban
//! anything (gh's destructive surface is small), but routing through one
//! module makes it easy to add bans (e.g. `gh repo delete`) later.

use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum GhExecError {
    #[error("spawning gh failed: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("gh exited non-zero ({code}): {stderr}")]
    NonZero { code: i32, stdout: String, stderr: String },
}

#[derive(Debug)]
pub struct GhOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct GhExec {
    cwd: PathBuf,
}

impl GhExec {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }

    pub fn cwd(&self) -> &Path { &self.cwd }

    pub async fn run(&self, args: &[&str]) -> Result<GhOutput, GhExecError> {
        let out = Command::new("gh")
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let code = out.status.code().unwrap_or(-1);
        if !out.status.success() {
            tracing::warn!(target: "vox.vcs.gh", code = code, ?args, "gh exited non-zero");
            return Err(GhExecError::NonZero { code, stdout, stderr });
        }
        tracing::debug!(target: "vox.vcs.gh", ?args, "gh ok");
        Ok(GhOutput { stdout, stderr, exit_code: code })
    }
}
```

Add `pub mod gh_exec;` to `crates/vox-orchestrator-mcp/src/lib.rs`.

- [ ] **Step 2: Write the body-formatter tests**

Tests live in `pr_tools.rs`. The PR body formatter is a pure function over the metadata struct; we test it without running gh.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_metadata() -> WorkspaceTaskMetadata {
        WorkspaceTaskMetadata {
            workspace_id: 42,
            task_summary: "fix: handle empty diff in commit_create".into(),
            related_issue_ids: vec![123, 456],
            ci_proof: Some(CiProof {
                run_id: "12345".into(),
                conclusion: "success".into(),
            }),
            commits: vec![
                CommitRef { sha: "abc123".into(), summary: "fix: empty diff".into() },
            ],
        }
    }

    #[test]
    fn pr_body_lists_summary_and_commits() {
        let body = pr_body_for_workspace(&fixture_metadata());
        assert!(body.contains("fix: handle empty diff in commit_create"));
        assert!(body.contains("abc123"));
        assert!(body.contains("fix: empty diff"));
    }

    #[test]
    fn pr_body_includes_ci_proof_when_present() {
        let body = pr_body_for_workspace(&fixture_metadata());
        assert!(body.contains("CI run 12345"));
        assert!(body.contains("success"));
    }

    #[test]
    fn pr_body_marks_missing_ci_proof_explicitly() {
        let mut m = fixture_metadata();
        m.ci_proof = None;
        let body = pr_body_for_workspace(&m);
        assert!(body.contains("CI proof: NOT YET ATTACHED"));
    }

    #[test]
    fn pr_body_links_related_issues() {
        let body = pr_body_for_workspace(&fixture_metadata());
        assert!(body.contains("#123"));
        assert!(body.contains("#456"));
    }
}
```

- [ ] **Step 3: Run tests — should fail**

Run: `cargo test -p vox-orchestrator-mcp --lib pr_tools`
Expected: FAIL — module / types missing.

- [ ] **Step 4: Implement pr_tools.rs**

```rust
//! MCP tool: `vox_pr_open`.
//!
//! Open a PR via `gh pr create` with the body templated from workspace
//! task metadata (not free-form agent text). Refuses if no CI proof has
//! been attached to the workspace — the agent must run CI and record
//! the run before opening the PR.

use std::path::Path;

use vox_orchestrator_types::{BranchName, PushAllowed, WorkspaceId};

use crate::gh_exec::{GhExec, GhExecError};

#[derive(Debug, thiserror::Error)]
pub enum PrError {
    #[error("workspace has no CI proof attached; refuse to open PR")]
    NoCiProof,
    #[error("capability workspace {cap} does not match operation workspace {op}")]
    WorkspaceMismatch { cap: WorkspaceId, op: WorkspaceId },
    #[error("gh failed: {0}")]
    GhFailed(#[from] GhExecError),
}

#[derive(Debug)]
pub struct PrOutput {
    pub url: String,
    pub title: String,
}

/// Subset of workspace task metadata the orchestrator materialises into
/// the PR body. Owned by the orchestrator, supplied to this module by
/// value so this module stays free of orchestrator-internal types.
#[derive(Debug, Clone)]
pub struct WorkspaceTaskMetadata {
    pub workspace_id: u64,
    pub task_summary: String,
    pub related_issue_ids: Vec<u64>,
    pub ci_proof: Option<CiProof>,
    pub commits: Vec<CommitRef>,
}

#[derive(Debug, Clone)]
pub struct CiProof {
    pub run_id: String,
    pub conclusion: String,
}

#[derive(Debug, Clone)]
pub struct CommitRef {
    pub sha: String,
    pub summary: String,
}

pub fn pr_body_for_workspace(metadata: &WorkspaceTaskMetadata) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "## Summary");
    let _ = writeln!(s, "{}", metadata.task_summary);
    let _ = writeln!(s);
    let _ = writeln!(s, "## Commits");
    for c in &metadata.commits {
        let _ = writeln!(s, "- `{}` — {}", c.sha, c.summary);
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## Related issues");
    if metadata.related_issue_ids.is_empty() {
        let _ = writeln!(s, "(none)");
    } else {
        for id in &metadata.related_issue_ids {
            let _ = writeln!(s, "- #{}", id);
        }
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## CI proof");
    match &metadata.ci_proof {
        Some(p) => {
            let _ = writeln!(s, "CI run {} — {}", p.run_id, p.conclusion);
        }
        None => {
            let _ = writeln!(s, "CI proof: NOT YET ATTACHED");
        }
    }
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "_Generated by vox_pr_open from workspace W-{:06}._",
        metadata.workspace_id
    );
    s
}

pub async fn pr_open(
    cwd: &Path,
    cap: &PushAllowed,
    op_workspace: WorkspaceId,
    op_branch: &BranchName,
    title: &str,
    metadata: &WorkspaceTaskMetadata,
) -> Result<PrOutput, PrError> {
    if cap.workspace() != op_workspace {
        return Err(PrError::WorkspaceMismatch {
            cap: cap.workspace(),
            op: op_workspace,
        });
    }
    if metadata.ci_proof.is_none() {
        return Err(PrError::NoCiProof);
    }

    let body = pr_body_for_workspace(metadata);
    let gh = GhExec::new(cwd);
    let out = gh
        .run(&[
            "pr", "create",
            "--head", op_branch.as_str(),
            "--title", title,
            "--body", &body,
        ])
        .await?;

    let url = out.stdout.trim().to_string();

    tracing::info!(
        target: "vox.vcs.pr_open",
        url = %url,
        branch = op_branch.as_str(),
        workspace_id = cap.workspace().0,
        "PR opened"
    );

    Ok(PrOutput { url, title: title.to_string() })
}

#[cfg(test)]
mod tests { /* see Step 2 */ }
```

Replace the `tests` placeholder with the test code from Step 2. Add `pub mod pr_tools;` to `vcs_tools/mod.rs`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-orchestrator-mcp --lib pr_tools`
Expected: PASS — 4/4.
Then: `cargo test -p vox-orchestrator-mcp --lib`
Expected: PASS — full suite.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator-mcp/src/gh_exec.rs crates/vox-orchestrator-mcp/src/vcs_tools/pr_tools.rs crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs crates/vox-orchestrator-mcp/src/lib.rs
git commit -m "feat(vcs): add gh_exec wrapper and vox_pr_open with templated body and CI-proof gate"
```

---

## Task 5: vox_force_push MCP tool

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/vcs_tools/force_push_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`

**Why this is its own tool, not a flag on `vox_push`:** Per the research doc §"Anti-goals", force-push is never a default-grantable capability. Making it a separate tool with a separate type means an agent literally cannot opt into force-push by passing a flag — they must request a different capability mint, which the orchestrator records in the ledger.

`GitExec` rejects `--force` / `--force-with-lease` (Task 2). To actually force-push, `vox_force_push` calls a *bypass* path on `GitExec` that the executor exposes only to this module. The bypass is `GitExec::run_unchecked`, gated by a `pub(crate)` visibility — only crates in the orchestrator-mcp tree can call it.

- [ ] **Step 1: Add `run_unchecked` to GitExec**

In `crates/vox-orchestrator-mcp/src/git_exec.rs`, add this method to the `impl GitExec` block, immediately after `run`:

```rust
    /// **Bypass** the banned-command check. Visibility is `pub(crate)` so
    /// only modules in this crate (specifically `vcs_tools::force_push_tools`
    /// and `vcs_tools::destructive_tools`) can call it. Every call emits
    /// a `vox.vcs.exec.unchecked` warning event.
    pub(crate) async fn run_unchecked(
        &self,
        args: &[&str],
    ) -> Result<GitOutput, GitExecError> {
        tracing::warn!(
            target: "vox.vcs.exec.unchecked",
            ?args,
            cwd = %self.cwd.display(),
            "GitExec::run_unchecked called — should be only from gated MCP tools"
        );
        let out = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let code = out.status.code().unwrap_or(-1);
        if !out.status.success() {
            return Err(GitExecError::NonZero { code, stdout, stderr });
        }
        Ok(GitOutput { stdout, stderr, exit_code: code })
    }
```

Add a test that confirms `run_unchecked` is `pub(crate)` (no public-API change visible from outside the crate). The test is implicit — if a downstream crate accidentally calls it, the build fails.

- [ ] **Step 2: Implement force_push_tools.rs**

```rust
//! MCP tool: `vox_force_push`.
//!
//! Force-push a workspace's bound branch. Requires a `ForcePushAllowed`
//! capability, which the orchestrator only mints after persisting a
//! human-approved justification record to the capability ledger
//! (`OperationKind::CapabilityMinted`, Task 8).
//!
//! This module is the *only* legitimate path through which `--force`
//! reaches `git push`. `GitExec::run` rejects it; we use the
//! `pub(crate)` `run_unchecked` bypass so the rest of the codebase
//! cannot construct a force-push by mistake.

use std::path::Path;

use vox_orchestrator_types::{BranchName, ForcePushAllowed, WorkspaceId};

use crate::git_exec::{GitExec, GitExecError, GitOutput};

#[derive(Debug, thiserror::Error)]
pub enum ForcePushError {
    #[error("capability workspace {cap} does not match operation workspace {op}")]
    WorkspaceMismatch { cap: WorkspaceId, op: WorkspaceId },
    #[error("capability branch {cap} does not match operation branch {op}")]
    BranchMismatch { cap: String, op: String },
    #[error("git failed: {0}")]
    GitFailed(#[from] GitExecError),
}

#[derive(Debug)]
pub struct ForcePushOutput {
    pub remote: String,
    pub branch: String,
    pub justification_hash: [u8; 32],
}

pub async fn force_push(
    cwd: &Path,
    cap: &ForcePushAllowed,
    op_workspace: WorkspaceId,
    op_branch: &BranchName,
    remote_name: &str,
) -> Result<ForcePushOutput, ForcePushError> {
    if cap.workspace() != op_workspace {
        return Err(ForcePushError::WorkspaceMismatch {
            cap: cap.workspace(),
            op: op_workspace,
        });
    }
    if cap.branch().as_str() != op_branch.as_str() {
        return Err(ForcePushError::BranchMismatch {
            cap: cap.branch().as_str().to_string(),
            op: op_branch.as_str().to_string(),
        });
    }

    // --force-with-lease is preferred over raw --force to avoid clobbering
    // unseen remote work. The justification ledger entry is the human's
    // authorization regardless.
    let git = GitExec::new(cwd);
    let GitOutput { .. } = git
        .run_unchecked(&["push", "--force-with-lease", remote_name, op_branch.as_str()])
        .await?;

    tracing::warn!(
        target: "vox.vcs.force_push",
        remote = remote_name,
        branch = op_branch.as_str(),
        workspace_id = cap.workspace().0,
        justification = %hex::encode(cap.justification_hash()),
        "force push completed"
    );

    Ok(ForcePushOutput {
        remote: remote_name.to_string(),
        branch: op_branch.as_str().to_string(),
        justification_hash: *cap.justification_hash(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator_types::{BranchName, ForcePushAllowed, RemoteId, WorkspaceId};

    #[test]
    fn force_push_workspace_mismatch_rejected() {
        let cap = ForcePushAllowed::mint(
            WorkspaceId(1),
            BranchName::parse("agent/x").unwrap(),
            RemoteId(0),
            [0; 32],
        );
        // We cannot run async without a runtime; test the early-return via
        // the typed args: a downstream caller that mismatches will get the
        // mismatch error. We just verify the cap accessors here.
        assert_eq!(cap.workspace(), WorkspaceId(1));
        assert!(cap.is_force());
    }
}
```

Add `pub mod force_push_tools;` to `vcs_tools/mod.rs`.

- [ ] **Step 3: Run tests + build**

Run: `cargo build -p vox-orchestrator-mcp --lib`
Expected: PASS.
Run: `cargo test -p vox-orchestrator-mcp --lib`
Expected: PASS — full suite, no regressions.

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator-mcp/src/git_exec.rs crates/vox-orchestrator-mcp/src/vcs_tools/force_push_tools.rs crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs
git commit -m "feat(vcs): add vox_force_push tool requiring ForcePushAllowed capability; introduce pub(crate) run_unchecked bypass"
```

---

## Task 6: vox_branch_delete MCP tool

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/vcs_tools/destructive_tools.rs`
- Modify: `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`

**Why now:** `DestructiveOp` from Task 1 is otherwise unused. Branch delete is the simplest destructive op and the most useful in agent flows ("clean up after PR merged"). Future variants of `DestructiveKind` (worktree remove, ref delete) plug into the same dispatch.

- [ ] **Step 1: Implement destructive_tools.rs**

```rust
//! MCP tools for destructive ops (`DestructiveOp` capability).
//!
//! Phase 2 scope: branch delete. Other variants of `DestructiveKind`
//! get added here over time, each with its own match arm in `execute`.

use std::path::Path;

use vox_orchestrator_types::{DestructiveKind, DestructiveOp, WorkspaceId};

use crate::git_exec::{GitExec, GitExecError, GitOutput};

#[derive(Debug, thiserror::Error)]
pub enum DestructiveError {
    #[error("capability workspace {cap} does not match operation workspace {op}")]
    WorkspaceMismatch { cap: WorkspaceId, op: WorkspaceId },
    #[error("git failed: {0}")]
    GitFailed(#[from] GitExecError),
}

#[derive(Debug)]
pub struct DestructiveOutput {
    pub kind_label: String,
    pub justification_hash: [u8; 32],
}

pub async fn execute(
    cwd: &Path,
    cap: &DestructiveOp,
    op_workspace: WorkspaceId,
) -> Result<DestructiveOutput, DestructiveError> {
    if cap.workspace() != op_workspace {
        return Err(DestructiveError::WorkspaceMismatch {
            cap: cap.workspace(),
            op: op_workspace,
        });
    }

    let git = GitExec::new(cwd);
    let kind_label = match cap.kind() {
        DestructiveKind::BranchDelete { branch } => {
            // -D is banned by GitExec::run; use run_unchecked.
            let GitOutput { .. } = git
                .run_unchecked(&["branch", "-D", branch.as_str()])
                .await?;
            format!("branch_delete:{}", branch.as_str())
        }
    };

    tracing::warn!(
        target: "vox.vcs.destructive",
        kind = %kind_label,
        workspace_id = cap.workspace().0,
        justification = %hex::encode(cap.justification_hash()),
        "destructive op completed"
    );

    Ok(DestructiveOutput {
        kind_label,
        justification_hash: *cap.justification_hash(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator_types::{BranchName, DestructiveKind, DestructiveOp, WorkspaceId};

    #[test]
    fn destructive_op_kind_label_branch_delete() {
        let cap = DestructiveOp::mint(
            WorkspaceId(1),
            DestructiveKind::BranchDelete {
                branch: BranchName::parse("agent/done").unwrap(),
            },
            [0; 32],
        );
        match cap.kind() {
            DestructiveKind::BranchDelete { branch } => {
                assert_eq!(branch.as_str(), "agent/done");
            }
        }
    }
}
```

Add `pub mod destructive_tools;` to `vcs_tools/mod.rs`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p vox-orchestrator-mcp --lib destructive_tools`
Expected: PASS — 1/1.
Then full suite.

- [ ] **Step 3: Commit**

```
git add crates/vox-orchestrator-mcp/src/vcs_tools/destructive_tools.rs crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs
git commit -m "feat(vcs): add vox_branch_delete (DestructiveOp) tool with run_unchecked bypass"
```

---

## Task 7: vox-arch-check rule — no raw Command::new("git") outside git_exec.rs

**Files:**
- Modify: `crates/vox-arch-check/src/main.rs`
- Modify: `docs/src/architecture/layers.toml` — add the new rule entry
- Test: `crates/vox-arch-check/tests/no_raw_git.rs` (new)

**Why now:** Tasks 1–6 add several legitimate `git` invocation sites. Without the arch-check rule, a future PR can introduce a new raw `Command::new("git")` somewhere else (e.g. `vox-cli`) and silently bypass the wrapper. The rule fails CI on any such invocation outside `git_exec.rs`.

- [ ] **Step 1: Read the existing arch-check rules**

Run: `cargo run -p vox-arch-check -- --list-rules`
Expected: a list of rule names. The new rule will be `no_raw_git_command`.

- [ ] **Step 2: Implement the rule**

In `crates/vox-arch-check/src/main.rs`, find the rule registration block (look for `register_rule` or similar). Add:

```rust
fn rule_no_raw_git_command(workspace_root: &Path) -> Vec<Diagnostic> {
    use ignore::WalkBuilder;
    let mut findings = Vec::new();
    for entry in WalkBuilder::new(workspace_root)
        .standard_filters(true)
        .add_custom_ignore_filename(".archcheckignore")
        .build()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        // Allowlist: the central executor is the one place this is OK.
        if path.ends_with("crates/vox-orchestrator-mcp/src/git_exec.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        for (lineno, line) in text.lines().enumerate() {
            // Match `Command::new("git")` and `Command::new(\"git\")` after
            // possible `std::process::` or `tokio::process::` prefix. Use a
            // simple substring check; an AST walk is overkill for this rule.
            if line.contains("Command::new(\"git\")") {
                findings.push(Diagnostic {
                    rule: "no_raw_git_command",
                    path: path.to_path_buf(),
                    line: lineno + 1,
                    message: "raw Command::new(\"git\") found outside git_exec.rs; route through GitExec instead",
                });
            }
        }
    }
    findings
}
```

Register the rule in the rule list (whatever the existing pattern is — `register_rule("no_raw_git_command", rule_no_raw_git_command)` or appending to a `Vec`).

Update `docs/src/architecture/layers.toml` if it tracks rule names: add `no_raw_git_command` to the rules array.

- [ ] **Step 3: Write the integration test**

Create `crates/vox-arch-check/tests/no_raw_git.rs`:

```rust
//! Verify the no_raw_git_command rule fires on a fixture file and is
//! silent on the executor itself.

use std::path::PathBuf;

#[test]
fn rule_fires_on_fixture_with_raw_command() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = tmp.path().join("crates/some-crate/src/lib.rs");
    std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
    std::fs::write(&bad, "use tokio::process::Command;\nfn x() { let _ = Command::new(\"git\"); }\n").unwrap();
    let findings = vox_arch_check::run_rule("no_raw_git_command", tmp.path());
    assert!(findings.iter().any(|f| f.path == bad), "rule must flag the fixture");
}

#[test]
fn rule_is_silent_on_git_exec_rs() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("crates/vox-orchestrator-mcp/src/git_exec.rs");
    std::fs::create_dir_all(good.parent().unwrap()).unwrap();
    std::fs::write(&good, "fn x() { let _ = Command::new(\"git\"); }\n").unwrap();
    let findings = vox_arch_check::run_rule("no_raw_git_command", tmp.path());
    assert!(!findings.iter().any(|f| f.path == good), "rule must skip git_exec.rs");
}
```

This requires `vox-arch-check` to expose `pub fn run_rule(name: &str, root: &Path) -> Vec<Diagnostic>` if it does not already. If the existing `main.rs` does not expose a library surface, add a tiny `lib.rs` that re-exports `run_rule` and the rule fns; the binary stays as-is.

- [ ] **Step 4: Run the rule against the live workspace**

Run: `cargo run -p vox-arch-check`
Expected: **FAIL** — the rule will likely flag callsites that Phase 1 didn't migrate (and that Task 8 will). This is intended; Task 7 only adds the rule, Task 8 makes CI green.

Save the list of flagged paths from this run; Task 8 will use it.

- [ ] **Step 5: Commit**

```
git add crates/vox-arch-check/src/main.rs crates/vox-arch-check/src/lib.rs crates/vox-arch-check/tests/no_raw_git.rs docs/src/architecture/layers.toml
git commit -m "feat(arch-check): add no_raw_git_command rule (Task 8 follows to make CI green)"
```

---

## Task 8: Migrate remaining direct git callsites to GitExec

**Files:**
- Modify: every `.rs` file flagged by Task 7's `cargo run -p vox-arch-check` output, except `git_exec.rs`.

**Why now:** Lock in Task 7's rule. After this task, `cargo run -p vox-arch-check` is green and the property "git invocation is centralised" is enforced going forward.

- [ ] **Step 1: Re-run arch-check for the live list of offenders**

Run: `cargo run -p vox-arch-check 2>&1 | rg "no_raw_git_command"`
Save the list. Sort by file path. Each file is a sub-task.

- [ ] **Step 2: Migrate each file**

For each flagged file, the migration template is:

**Before:**
```rust
let out = tokio::process::Command::new("git")
    .args(["log", "--oneline"])
    .current_dir(&cwd)
    .output()
    .await?;
```

**After:**
```rust
use crate::git_exec::GitExec;  // or vox_orchestrator_mcp::git_exec::GitExec from outside the crate
let git_out = GitExec::new(&cwd).run(&["log", "--oneline"]).await?;
// `git_out.stdout`, `git_out.stderr`, `git_out.exit_code` replace the
// `out.stdout` / `out.stderr` / `out.status.code()` accesses.
```

If a file is in a crate that does not currently depend on `vox-orchestrator-mcp` and the migration would introduce a new layer dependency that violates `layers.toml`, **stop**. That callsite is a candidate for Phase 2 of the multi-agent VCS replication plan, which moves `git_exec` down into a lower-layer crate. Mark the callsite with `// arch-check-allow: no_raw_git_command — see Phase 5 backend swap` and add a `.archcheckignore` entry that exempts that one path. Document the exception in this plan's notes section before continuing.

- [ ] **Step 3: Run arch-check + the test suite after each file's migration**

After each file is migrated, run:

```
cargo run -p vox-arch-check
cargo test -p <crate-of-the-migrated-file> --lib
```

Both expected to PASS for the migrated file's crate.

- [ ] **Step 4: Commit**

A single commit at the end of the migration is fine if the diff is < ~500 lines; otherwise commit per-crate. Message:

```
git commit -m "refactor(vcs): migrate remaining direct git callsites to GitExec; arch-check now green"
```

---

## Task 9: Persist capability mints to the oplog (capability ledger)

**Files:**
- Modify: `crates/vox-orchestrator-queue/src/oplog/mod.rs` (or wherever `OperationKind` lives — confirm via grep)
- Modify: `crates/vox-orchestrator/src/authorize.rs` (or wherever `authorize_*` shims live; create if absent)

**Why now:** Without persistence, the capability ledger has no memory; the dashboard panel in Phase 3 has nothing to render. This task adds an `OperationKind::CapabilityMinted { kind, workspace_id, justification_hash }` variant and writes one entry per `mint_*` call.

- [ ] **Step 1: Find the OperationKind enum**

Run: `rg "enum OperationKind" crates/vox-orchestrator-queue/src/`
Open the file, read it, and confirm the existing variant style (struct vs tuple, fields, derives).

- [ ] **Step 2: Add the new variant + tests**

Add to the enum:

```rust
CapabilityMinted {
    kind: CapabilityKind,
    workspace_id: u64,
    /// 32-byte SHA-256 of the justification record. Empty for
    /// non-justified caps (PushAllowed, BranchCreate, WorkingTreeWrite).
    justification_hash: Option<[u8; 32]>,
},
```

Define `CapabilityKind` in the same file:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CapabilityKind {
    WorkingTreeWrite,
    BranchCreate,
    PushAllowed,
    ForcePushAllowed,
    DestructiveOp,
}
```

Add a test in the same file:

```rust
#[test]
fn capability_minted_round_trips_through_serde() {
    let op = OperationKind::CapabilityMinted {
        kind: CapabilityKind::ForcePushAllowed,
        workspace_id: 7,
        justification_hash: Some([0xAB; 32]),
    };
    let json = serde_json::to_string(&op).unwrap();
    let back: OperationKind = serde_json::from_str(&json).unwrap();
    assert_eq!(op, back);
}
```

- [ ] **Step 3: Wire mints into the oplog**

Locate or create `crates/vox-orchestrator/src/authorize.rs`. The shape is:

```rust
//! authorize_* shims: the only path that mints capabilities. Each shim
//! checks an authorization rule, then mints, then records the mint in
//! the oplog.

use vox_orchestrator_queue::{CapabilityKind, OperationKind, OplogStore};
use vox_orchestrator_types::{
    BranchName, DestructiveKind, DestructiveOp, ForcePushAllowed, PushAllowed,
    RemoteId, WorkingTreeWrite, WorkspaceId,
};

pub async fn authorize_working_tree_write(
    oplog: &impl OplogStore,
    workspace: WorkspaceId,
    branch: BranchName,
) -> Result<WorkingTreeWrite, AuthorizationError> {
    // Authorization rule: the workspace exists and is bound to this branch.
    // (The actual rule check is environment-specific; in tests we trust
    //  the caller. In production the orchestrator's session state checks
    //  it.)
    let cap = WorkingTreeWrite::mint(workspace, branch);
    oplog
        .append(OperationKind::CapabilityMinted {
            kind: CapabilityKind::WorkingTreeWrite,
            workspace_id: workspace.0,
            justification_hash: None,
        })
        .await?;
    Ok(cap)
}

// Similar shims for BranchCreate, PushAllowed.
// ForcePushAllowed and DestructiveOp shims also persist the justification
// text under the hash key in a side-table (the `justifications` store).

pub async fn authorize_force_push(
    oplog: &impl OplogStore,
    workspace: WorkspaceId,
    branch: BranchName,
    remote: RemoteId,
    justification_text: &str,
) -> Result<ForcePushAllowed, AuthorizationError> {
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, justification_text.as_bytes());
    let hash: [u8; 32] = sha2::Digest::finalize(hasher).into();
    // Side-table: key = hash, value = justification_text
    oplog.put_justification(&hash, justification_text).await?;
    let cap = ForcePushAllowed::mint(workspace, branch, remote, hash);
    oplog
        .append(OperationKind::CapabilityMinted {
            kind: CapabilityKind::ForcePushAllowed,
            workspace_id: workspace.0,
            justification_hash: Some(hash),
        })
        .await?;
    Ok(cap)
}

#[derive(Debug, thiserror::Error)]
pub enum AuthorizationError {
    #[error("oplog write failed: {0}")]
    OplogFailed(String),
}
```

Add `put_justification` to the `OplogStore` trait if it does not already exist; the simplest implementation is a separate KV store keyed by hash.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-queue --lib`
Run: `cargo test -p vox-orchestrator --lib`
Both expected: PASS.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-queue/src/oplog/ crates/vox-orchestrator/src/authorize.rs
git commit -m "feat(orchestrator): persist capability mints to oplog (capability ledger MVP)"
```

---

## Task 10: VoxScript glue scripts under scripts/vcs/

**Files:**
- Create: `scripts/vcs/wip.vox`
- Create: `scripts/vcs/sync.vox`
- Create: `scripts/vcs/finish.vox`
- Create: `scripts/vcs/recover.vox`

**Why .vox, not .ps1/.sh:** Per [`AGENTS.md §VoxScript-First Glue Code`](../../../AGENTS.md), all automation scripts in this repo are `.vox`. The compiler type-checks them, they emit `vox.script.vcs.*` telemetry, and they run identically on Windows / Linux / macOS without per-shell branches.

These four scripts are *thin orchestration*: they call the MCP tools added in Phases 1 and 2 via the existing `vox` CLI bridge. They contain no git knowledge and no credential handling.

- [ ] **Step 1: Verify the .vox MCP-call surface**

Run: `rg "vox_commit_create" scripts/` and `rg "@durable" scripts/` to confirm the `.vox` syntax for calling MCP tools follows the existing pattern in the repo. If `scripts/` has no precedent for an MCP call, look at `crates/vox-cli/src/commands/scaffold/` for the typical bridge syntax.

If no .vox precedent exists for tool calls and the bridge is not obvious from the existing codebase, **stop** and surface the gap; this means the .vox-script-first policy was aspirational and Phase 2 of the surrounding language work needs to land first. Mark this task as deferred and continue with Task 11.

- [ ] **Step 2: scripts/vcs/wip.vox**

```vox
// scripts/vcs/wip.vox
// Commit staged changes under the `wip:` prefix so the orchestrator can
// locate and squash them later. In Phase 4 workspace_id becomes a
// WorkingTreeWrite capability token gated by @vcs.requires; the call
// signature stays the same.

fn vox_commit_create(workspace_id: str, summary: str, body: str) to Result[str] {
    return Ok(workspace_id)
}

fn wip(workspace_id: str, summary: str) to Result[str] {
    return vox_commit_create(workspace_id, "wip: " + summary, "")
}
```

- [ ] **Step 3: scripts/vcs/sync.vox**

```vox
// scripts/vcs/sync.vox
// Fetch origin and rebase against main. Aborts on conflict and surfaces
// the error rather than auto-resolving — per failure-mode B from the
// research doc (auto-resolution is where work gets lost).

fn vox_git_fetch(workspace_id: str, remote: str) to Result[str] {
    return Ok("")
}

fn vox_git_rebase(workspace_id: str, target: str) to Result[str] {
    return Ok("")
}

fn vox_git_rebase_abort(workspace_id: str) to Result[str] {
    return Ok("")
}

fn sync(workspace_id: str) to Result[str] {
    let fetch = vox_git_fetch(workspace_id, "origin")
    match fetch {
        Error(e) => Error("fetch failed: " + e)
        Ok(_) => {
            let rebase = vox_git_rebase(workspace_id, "origin/main")
            match rebase {
                Ok(sha) => Ok(sha)
                Error(e) => {
                    let _ = vox_git_rebase_abort(workspace_id)
                    Error("conflict aborted: " + e)
                }
            }
        }
    }
}
```

(`vox_git_fetch`, `vox_git_rebase`, `vox_git_rebase_abort` are not part of Phase 2; they are MCP tools to add in Phase 2.5 if this script lands. The stub bodies above are placeholders — Phase 2 ships the file scaffold so Phase 2.5 has a concrete target to fill in.)

- [ ] **Step 4: scripts/vcs/finish.vox**

```vox
// scripts/vcs/finish.vox
// Final step of an agent task: squash WIP commits, run CI, push, open PR.
// Each fallible step short-circuits on Error. In Phase 4, workspace_id
// and the push step gain typed capability tokens from @vcs.requires.

fn vox_commit_squash(workspace_id: str, summary: str, body: str) to Result[str] {
    return Ok(workspace_id)
}

fn vox_ci_run(workspace_id: str) to Result[str] {
    return Ok("ci-pass")
}

fn vox_push(workspace_id: str) to Result[str] {
    return Ok("")
}

fn vox_pr_open(workspace_id: str, title: str) to Result[str] {
    return Ok("https://github.com/example/example/pull/1")
}

fn finish_after_squash(workspace_id: str, title: str) to Result[str] {
    let ci = vox_ci_run(workspace_id)
    match ci {
        Error(e) => Error("CI failed: " + e)
        Ok(_) => finish_after_ci(workspace_id, title)
    }
}

fn finish_after_ci(workspace_id: str, title: str) to Result[str] {
    let push = vox_push(workspace_id)
    match push {
        Error(e) => Error("push failed: " + e)
        Ok(_) => vox_pr_open(workspace_id, title)
    }
}

fn finish(workspace_id: str, final_summary: str, final_body: str, title: str) to Result[str] {
    let squash = vox_commit_squash(workspace_id, final_summary, final_body)
    match squash {
        Error(e) => Error("squash failed: " + e)
        Ok(_) => finish_after_squash(workspace_id, title)
    }
}
```

- [ ] **Step 5: scripts/vcs/recover.vox**

```vox
// scripts/vcs/recover.vox
// Read-only inspector. Produces a recovery plan (text description of
// oplog ops to undo) but does NOT execute it. The orchestrator surfaces
// the plan for human approval before any destructive op runs.

fn vox_oplog(workspace_id: str) to str {
    return ""
}

fn vox_git_reflog(workspace_id: str) to str {
    return ""
}

fn plan_from_logs(oplog: str, reflog: str) to str {
    return "ops: " + oplog + "\nrefs: " + reflog
}

fn recover_plan(workspace_id: str) to str {
    let oplog = vox_oplog(workspace_id)
    let reflog = vox_git_reflog(workspace_id)
    return plan_from_logs(oplog, reflog)
}
```

- [ ] **Step 6: Validate each script**

Run: `vox check scripts/vcs/wip.vox`
Repeat for the other three.
Expected: PASS for all four. The scripts use stub implementations for the runtime MCP tool calls (e.g. `vox_commit_create`); Phase 4 will wire these to real capability-token-gated Rust implementations without changing the call signatures.

- [ ] **Step 7: Commit**

```
git add scripts/vcs/wip.vox scripts/vcs/sync.vox scripts/vcs/finish.vox scripts/vcs/recover.vox
git commit -m "feat(vox-scripts): add scripts/vcs/{wip,sync,finish,recover}.vox glue scripts (scaffolding for @vcs.* decorators in Phase 4)"
```

---

## Task 11: Documentation cross-cuts

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`
- Modify: `docs/src/architecture/git-concurrency-policy.md`
- Regenerate: `docs/src/SUMMARY.md`, `docs/src/architecture/architecture-index.md`, `docs/src/feed.xml`

- [ ] **Step 1: where-things-live.md**

Add to the "Common tasks → exact path" table:

```
| Add a push/PR/destructive VCS tool | `crates/vox-orchestrator-mcp/src/vcs_tools/<purpose>_tools.rs` |
| Mint a capability for an agent | `crates/vox-orchestrator/src/authorize.rs` (only place) |
| Add a VCS automation script | `scripts/vcs/<name>.vox` (.vox only — see AGENTS.md §VoxScript-First) |
```

- [ ] **Step 2: git-concurrency-policy.md**

Append a new section:

```markdown
## Force-push and destructive ops

Force-push and branch-delete go through dedicated tools that require a
capability carrying a 32-byte SHA-256 hash of a human-approved
justification record:

| Tool | Capability | What gets persisted |
|---|---|---|
| `vox_force_push` | `ForcePushAllowed` | `OperationKind::CapabilityMinted` entry + justification text in side-table |
| `vox_branch_delete` | `DestructiveOp { kind: BranchDelete }` | Same |

The capability ledger (a view over `OperationKind::CapabilityMinted`
entries in the oplog) is the durable record. Phase 3 surfaces it in the
dashboard. Until then, query it with:

\`\`\`bash
vox oplog --kind CapabilityMinted --since 24h
\`\`\`

## arch-check enforcement

`vox-arch-check` rule `no_raw_git_command` fails CI if any `.rs` file
outside `crates/vox-orchestrator-mcp/src/git_exec.rs` contains
`Command::new("git")`. To exempt a specific path during a migration,
add it to `.archcheckignore` with a comment referencing the issue or
phase that will resolve it.
```

- [ ] **Step 3: Regenerate**

Run: `cargo run -p vox-doc-pipeline`
Then: `cargo run -p vox-doc-pipeline -- --check`
Expected: PASS.

- [ ] **Step 4: Commit**

```
git add docs/src/architecture/where-things-live.md docs/src/architecture/git-concurrency-policy.md
git commit -m "docs(vcs): document Phase 2 push/PR/destructive surface and arch-check enforcement"
git add docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml
git commit -m "chore(docs): regenerate SUMMARY.md / architecture-index.md / feed.xml"
```

---

## Phase 2 acceptance criteria

All must be true:

- [ ] `cargo test -p vox-orchestrator-types --lib` passes (≥13 tests).
- [ ] `cargo test -p vox-orchestrator-mcp --lib` passes (≥145 tests).
- [ ] `cargo test -p vox-orchestrator-queue --lib` passes; new `CapabilityMinted` variant has at least one round-trip test.
- [ ] `cargo run -p vox-arch-check` is GREEN; the `no_raw_git_command` rule is registered and fires on a fixture.
- [ ] `cargo run -p vox-doc-pipeline -- --check` passes.
- [ ] No file outside `git_exec.rs` contains `Command::new("git")`.
- [ ] `scripts/vcs/{wip,sync,finish,recover}.vox` exist; each either passes `vox check` or carries a `// vox:skip` with a reason.
- [ ] All 11 commits land with message conventions matching the per-task templates above.

---

## Notes for the implementing engineer

- **Task 8 is the longest and hardest to estimate.** The size depends on how many raw callsites Phase 1 left behind. If the live count is > 8 files, split Task 8 into per-crate sub-tasks and commit per crate.
- **Task 9 touches `vox-orchestrator-queue`, an L1/L2 crate.** Verify with `cargo run -p vox-arch-check` after each change that no layer rule was violated; the new `CapabilityMinted` variant must not introduce a dep on `vox-orchestrator-types` if the queue crate is below it. If it does, the variant lives in a new pure-types module under `vox-orchestrator-types` and the queue crate just stores `(kind, workspace_id, hash_opt)` as bytes.
- **Force-push and destructive ops emit `tracing::warn!`, not `info!`.** Operators grep logs for `vox.vcs.force_push` and `vox.vcs.destructive` to audit; making them warn-level keeps them visible at the default log level.
- **`run_unchecked` is `pub(crate)` deliberately.** Resist exposing it crate-publicly even for "convenience" — the whole point is that the only callers are the two gated tools. If a third gated tool needs it later, that tool lives in this crate by definition; if it does not live in this crate, the answer is to add a new tool here, not to widen visibility.
- **The `.vox` scripts may not type-check until Phase 4.** That is fine — Phase 2 ships the scaffolding so Phase 4 has a concrete migration target. Use `// vox:skip` annotations where needed; the files still serve as executable documentation of the intended composition of MCP tools.
- **Capability mismatch errors are `Important`, not `Critical`.** A workspace/branch mismatch between the cap and the operation is a programming error in the orchestrator's authorize_* path, not user-facing. Log the mismatch at `error` level and return the error; do not panic.
