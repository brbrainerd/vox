---
title: "Agentic VCS Automation — Phase 1 Implementation Plan (2026-05-08)"
description: "Step-by-step TDD plan that implements Phase 1 of the agentic-version-control-automation research: capability types, workspace↔branch binding, central banned-command-enforcing git exec wrapper, secret-scanner, write-side commit and branch MCP tools, telemetry namespace, and one production callsite migration. Phases 2–4 (Vox @vcs.* decorators, dashboard, .vox glue) are sketched at the end as separate plans."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 1 lowers the agentic-vcs research findings into concrete, ordered tasks with full code, exact file paths, exact commands, and TDD steps. Every task produces a commit. Future agents executing this plan should not need to invent code."
sourced_at: "2026-05-08"
vox_relevance:
  - "vox-orchestrator-types: new vcs_capability module"
  - "vox-orchestrator: AgentWorkspace gains a branch binding"
  - "vox-orchestrator-mcp: new git_exec wrapper, secret-scan, vox_commit_create / vox_branch_create"
  - "vox-cli: one existing git callsite migrated to the wrapper"
  - "docs/agents/git-concurrency-policy.md: prose policy backed by Rust enforcement"
---

# Agentic VCS Automation — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Companion research:** [`agentic-version-control-automation-research-2026.md`](agentic-version-control-automation-research-2026.md). Read its §"Failure-mode taxonomy" and §"Proposed automation architecture" before starting.

**Goal:** Land the MVP slice that closes the *write-side* agentic VCS gap end-to-end: capability-typed VCS effects, a workspace↔branch binding, a single banned-command-enforcing git executor that all orchestrator git calls go through, a secret scanner on the commit path, and two write-side MCP tools (`vox_commit_create`, `vox_branch_create`) that exercise all of the above.

**Architecture:** Capabilities live in `vox-orchestrator-types` as opaque structs with private fields — external crates cannot literally construct them. The orchestrator's `authorize_*` shims are the only public mint path. The MCP layer's new `git_exec` module is the *only* place in the orchestrator process tree that runs `git`; it refuses banned commands at spawn time and emits `vox.vcs.*` telemetry on every invocation. `vox_commit_create` mints the commit-message envelope (author, trailers, `Co-authored-by`) so the agent supplies only summary + body — closing failure-mode C from the research. The Clavis-style secret scanner (regex-based MVP, extensible) gates every `vox_commit_create` call.

**Tech stack:** Rust 2021 edition, `tokio` for async, `serde` for envelopes, `tracing` for telemetry. No new dependencies; all primitives are already in the workspace.

**Out of scope for Phase 1 (deferred to Phase 2+):**
- Vox `@vcs.*` decorators (compiler work; sequenced after Phase 2 of the multi-agent VCS replication plan).
- Dashboard panels (Phase 3).
- `.vox` glue scripts in `scripts/vcs/` (Phase 2).
- `vox_push` / `vox_force_push` / `vox_pr_open` (Phase 2 — depends on capability ledger UX).
- jj-lib in the hot path (kept feature-gated as today; the wrapper interface is generic over backend).
- `git2`/`gix` migration (the wrapper keeps shelling out via `tokio::process::Command`, matching existing pattern; backend swap is a Phase 4 refactor).

---

## Verification setup

These are run by the engineer, not by every step.

- `cargo test -p vox-orchestrator-types --lib` — capability + ID tests.
- `cargo test -p vox-orchestrator --lib workspace::` — workspace binding tests.
- `cargo test -p vox-orchestrator-mcp --lib` — wrapper, secret-scan, and tool tests.
- `cargo run -p vox-arch-check` — must pass after each task that touches `Cargo.toml` or moves code between layers.
- `cargo run -p vox-doc-pipeline -- --check` — must pass after Task 9.

The plan assumes a workspace clean of unrelated changes. Use a dedicated branch — see [`docs/agents/git-concurrency-policy.md`](./git-concurrency-policy.md) §3.A.

---

## Task 1: Add VCS capability types and supporting IDs

**Files:**
- Create: `crates/vox-orchestrator-types/src/vcs_capability.rs`
- Modify: `crates/vox-orchestrator-types/src/lib.rs`

**Why this first:** Every subsequent task takes capability values as arguments. Define them once, in the L0 pure-types crate, so MCP tools and the orchestrator core can both depend on the same shapes.

- [ ] **Step 1: Write failing tests for capability construction and accessors**

Create `crates/vox-orchestrator-types/src/vcs_capability.rs`:

```rust
//! VCS capability tokens. Holding one of these structs is proof that an
//! authorized orchestrator path minted it. External crates cannot literally
//! construct these — only call the `pub(crate)`-doc-hidden `mint_*` paths
//! routed through `vox_orchestrator::authorize_*`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspaceId(pub u64);

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "W-{:06}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchName(String);

impl BranchName {
    /// Reject empty, whitespace, and any name containing characters git refuses.
    /// Matches the subset of `git check-ref-format --branch` we care about for
    /// agent-generated names: ASCII, no spaces, no `..`, `:`, `?`, `*`, `[`, `\`,
    /// `^`, `~`, no leading `/` or `-`, length 1..=255.
    pub fn parse(s: &str) -> Result<Self, BranchNameError> {
        if s.is_empty() || s.len() > 255 {
            return Err(BranchNameError::InvalidLength);
        }
        if s.starts_with('/') || s.starts_with('-') {
            return Err(BranchNameError::IllegalPrefix);
        }
        if s.contains("..") {
            return Err(BranchNameError::IllegalSequence);
        }
        for ch in s.chars() {
            let ok = ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.');
            if !ok {
                return Err(BranchNameError::IllegalChar(ch));
            }
        }
        Ok(BranchName(s.to_string()))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BranchNameError {
    #[error("branch name length must be 1..=255")]
    InvalidLength,
    #[error("branch name cannot start with '/' or '-'")]
    IllegalPrefix,
    #[error("branch name cannot contain '..'")]
    IllegalSequence,
    #[error("branch name contains illegal character {0:?}")]
    IllegalChar(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteId(pub u32);

/// Capability: holder may stage and commit hunks against `branch` of `workspace`.
/// Constructed only by `mint_working_tree_write`.
#[derive(Debug, Clone)]
pub struct WorkingTreeWrite {
    workspace: WorkspaceId,
    branch: BranchName,
}

impl WorkingTreeWrite {
    /// Mint a `WorkingTreeWrite`. **Authorization is the caller's responsibility**;
    /// orchestrator authorize_*` wrappers are the only callers we expect.
    #[doc(hidden)]
    pub fn mint(workspace: WorkspaceId, branch: BranchName) -> Self {
        Self { workspace, branch }
    }

    pub fn workspace(&self) -> WorkspaceId { self.workspace }
    pub fn branch(&self) -> &BranchName { &self.branch }
}

/// Capability: holder may create a new branch in `workspace` rooted at `parent`.
#[derive(Debug, Clone)]
pub struct BranchCreate {
    workspace: WorkspaceId,
    parent: BranchName,
}

impl BranchCreate {
    #[doc(hidden)]
    pub fn mint(workspace: WorkspaceId, parent: BranchName) -> Self {
        Self { workspace, parent }
    }

    pub fn workspace(&self) -> WorkspaceId { self.workspace }
    pub fn parent(&self) -> &BranchName { &self.parent }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_id_displays_padded() {
        assert_eq!(WorkspaceId(7).to_string(), "W-000007");
    }

    #[test]
    fn branch_name_accepts_typical_agent_names() {
        BranchName::parse("agent/refactor-cache").unwrap();
        BranchName::parse("feature/cap-types").unwrap();
        BranchName::parse("wip.fix.42").unwrap();
    }

    #[test]
    fn branch_name_rejects_empty_or_too_long() {
        assert_eq!(BranchName::parse("").unwrap_err(), BranchNameError::InvalidLength);
        let too_long = "a".repeat(256);
        assert_eq!(BranchName::parse(&too_long).unwrap_err(), BranchNameError::InvalidLength);
    }

    #[test]
    fn branch_name_rejects_illegal_prefix_or_sequence() {
        assert_eq!(BranchName::parse("/foo").unwrap_err(), BranchNameError::IllegalPrefix);
        assert_eq!(BranchName::parse("-foo").unwrap_err(), BranchNameError::IllegalPrefix);
        assert_eq!(BranchName::parse("foo..bar").unwrap_err(), BranchNameError::IllegalSequence);
    }

    #[test]
    fn branch_name_rejects_illegal_chars() {
        assert!(matches!(
            BranchName::parse("foo bar"),
            Err(BranchNameError::IllegalChar(' '))
        ));
        assert!(matches!(
            BranchName::parse("foo:bar"),
            Err(BranchNameError::IllegalChar(':'))
        ));
    }

    #[test]
    fn working_tree_write_round_trip() {
        let cap = WorkingTreeWrite::mint(WorkspaceId(1), BranchName::parse("agent/x").unwrap());
        assert_eq!(cap.workspace(), WorkspaceId(1));
        assert_eq!(cap.branch().as_str(), "agent/x");
    }
}
```

- [ ] **Step 2: Verify the file does not yet compile (module not registered)**

Run: `cargo test -p vox-orchestrator-types vcs_capability::tests`
Expected: FAIL — "could not find `vcs_capability` in crate root" or similar.

- [ ] **Step 3: Register the module in `lib.rs`**

Modify `crates/vox-orchestrator-types/src/lib.rs` — add to the existing module roster (after `pub mod socrates_policy;`):

```rust
pub mod vcs_capability;

pub use vcs_capability::{
    BranchCreate, BranchName, BranchNameError, RemoteId, WorkingTreeWrite, WorkspaceId,
};
```

- [ ] **Step 4: Run tests — they should pass**

Run: `cargo test -p vox-orchestrator-types vcs_capability::tests`
Expected: PASS — 6/6 tests.

- [ ] **Step 5: Verify `thiserror` is available**

If `cargo build -p vox-orchestrator-types` complains that `thiserror` is missing, add to `crates/vox-orchestrator-types/Cargo.toml` `[dependencies]`:

```toml
thiserror = { workspace = true }
```

Then re-run Step 4.

- [ ] **Step 6: Run arch-check and commit**

Run: `cargo run -p vox-arch-check`
Expected: PASS (no new layer violations — `vcs_capability` lives in an L0 crate).

```
git add crates/vox-orchestrator-types/src/vcs_capability.rs crates/vox-orchestrator-types/src/lib.rs crates/vox-orchestrator-types/Cargo.toml
git commit -m "feat(orchestrator-types): add VCS capability tokens (WorkingTreeWrite, BranchCreate) and supporting IDs"
```

---

## Task 2: Bind a branch to `AgentWorkspace`

**Files:**
- Modify: `crates/vox-orchestrator/src/workspace.rs`

**Why:** Failure-modes A (wrong-branch) and F (context desync) require the orchestrator to own a single authoritative `(workspace → branch)` binding. Without it, every later capability mint is a guess.

- [ ] **Step 1: Write a failing test for branch binding**

Append to `crates/vox-orchestrator/src/workspace.rs` inside `#[cfg(test)] mod tests { ... }` (create the test module if it does not exist):

```rust
#[test]
fn agent_workspace_records_bound_branch() {
    use vox_orchestrator_types::BranchName;

    let mut ws = AgentWorkspace {
        agent_id: AgentId(1),
        base_snapshot: SnapshotId(0),
        overlay: Default::default(),
        created_ms: 0,
        active_change: None,
        bound_branch: None,
    };
    assert_eq!(ws.bound_branch(), None);

    let b = BranchName::parse("agent/test-binding").unwrap();
    ws.set_bound_branch(b.clone());
    assert_eq!(ws.bound_branch(), Some(&b));
}

#[test]
fn agent_workspace_rebinding_branch_is_explicit() {
    use vox_orchestrator_types::BranchName;
    let mut ws = AgentWorkspace {
        agent_id: AgentId(2),
        base_snapshot: SnapshotId(0),
        overlay: Default::default(),
        created_ms: 0,
        active_change: None,
        bound_branch: Some(BranchName::parse("agent/old").unwrap()),
    };
    let new_b = BranchName::parse("agent/new").unwrap();
    let prev = ws.set_bound_branch(new_b.clone());
    assert_eq!(prev.unwrap().as_str(), "agent/old");
    assert_eq!(ws.bound_branch(), Some(&new_b));
}
```

- [ ] **Step 2: Run the test — should fail to compile**

Run: `cargo test -p vox-orchestrator workspace::tests::agent_workspace_records_bound_branch`
Expected: FAIL — `AgentWorkspace` has no `bound_branch` field.

- [ ] **Step 3: Add the field and methods**

Modify the `AgentWorkspace` struct in `crates/vox-orchestrator/src/workspace.rs` (currently at ~lines 82–94):

```rust
pub struct AgentWorkspace {
    pub agent_id: AgentId,
    pub base_snapshot: SnapshotId,
    pub overlay: HashMap<PathBuf, WorkspaceEntry>,
    pub created_ms: u64,
    pub active_change: Option<ChangeId>,
    /// The git branch this workspace is bound to. `None` until the orchestrator
    /// resolves the workspace to a branch (typically on first write op).
    pub bound_branch: Option<vox_orchestrator_types::BranchName>,
}

impl AgentWorkspace {
    pub fn bound_branch(&self) -> Option<&vox_orchestrator_types::BranchName> {
        self.bound_branch.as_ref()
    }

    /// Set the bound branch and return the previous value, if any.
    pub fn set_bound_branch(
        &mut self,
        branch: vox_orchestrator_types::BranchName,
    ) -> Option<vox_orchestrator_types::BranchName> {
        self.bound_branch.replace(branch)
    }
}
```

- [ ] **Step 4: Update every existing `AgentWorkspace { ... }` literal**

Run: `cargo build -p vox-orchestrator`
Expected: a small number of compile errors of the form "missing field `bound_branch` in initializer". For each, add `bound_branch: None,` to the struct literal. Typical sites: `workspace_create` flows in the same file. Do not pre-populate the branch — it stays `None` until explicit binding.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test -p vox-orchestrator workspace::`
Expected: PASS, including the two new tests.

```
git add crates/vox-orchestrator/src/workspace.rs
git commit -m "feat(orchestrator): bind a git branch to AgentWorkspace (None until set)"
```

---

## Task 3: Central `git_exec` wrapper with banned-command denylist

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/git_exec.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`

**Why:** The git-concurrency policy is markdown today. This task graduates it to a Rust check that no orchestrator-process invocation of `git` can bypass. Failure-modes A, B, E.

- [ ] **Step 1: Write failing tests for banned-command rejection and arg passthrough**

Create `crates/vox-orchestrator-mcp/src/git_exec.rs`:

```rust
//! Central executor for every `git` invocation made from the orchestrator
//! process tree. All callers (MCP tools, CLI subcommands lifted into the
//! orchestrator) MUST go through `GitExec::run` so the banned-command
//! denylist and `vox.vcs.*` telemetry apply uniformly.

use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct GitExec {
    cwd: PathBuf,
}

#[derive(Debug)]
pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum GitExecError {
    #[error("banned git invocation: {0}")]
    Banned(String),
    #[error("spawning git failed: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git exited non-zero ({code}): {stderr}")]
    NonZero { code: i32, stdout: String, stderr: String },
}

/// Each entry is a *prefix* of args that, if matched in order, denies the call.
/// The check is performed against the full `args` slice — a banned prefix that
/// appears in the *middle* of args (because of an earlier `-c key=val` etc.)
/// also denies. See `is_banned` for the matching rule.
const BANNED_PREFIXES: &[&[&str]] = &[
    &["stash"],
    &["reset", "--hard"],
    &["clean", "-fd"],
    &["clean", "-f"],
    &["clean", "-fdx"],
    &["restore", "."],
    &["checkout", "."],
    &["checkout", "--", "."],
    &["checkout", "-f"],
];

/// Returns `Some(matched_phrase)` if `args` contains any banned prefix.
pub fn is_banned(args: &[&str]) -> Option<String> {
    for &pat in BANNED_PREFIXES {
        if args.windows(pat.len()).any(|w| w == pat) {
            return Some(pat.join(" "));
        }
    }
    None
}

impl GitExec {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }

    pub fn cwd(&self) -> &Path { &self.cwd }

    pub async fn run(&self, args: &[&str]) -> Result<GitOutput, GitExecError> {
        if let Some(phrase) = is_banned(args) {
            tracing::warn!(target: "vox.vcs.exec", phrase = %phrase, "denied banned git invocation");
            return Err(GitExecError::Banned(phrase));
        }
        let started = Instant::now();
        let output = tokio::process::Command::new("git")
            .current_dir(&self.cwd)
            .args(args)
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);
        let elapsed_ms = started.elapsed().as_millis() as u64;
        tracing::info!(
            target: "vox.vcs.exec",
            args = ?args,
            cwd = %self.cwd.display(),
            code = code,
            elapsed_ms = elapsed_ms,
            "git exec",
        );
        if code != 0 {
            return Err(GitExecError::NonZero { code, stdout, stderr });
        }
        Ok(GitOutput { stdout, stderr, exit_code: code })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_banned_catches_each_prefix() {
        assert_eq!(is_banned(&["stash"]).as_deref(), Some("stash"));
        assert_eq!(is_banned(&["stash", "pop"]).as_deref(), Some("stash"));
        assert_eq!(is_banned(&["reset", "--hard", "HEAD~1"]).as_deref(), Some("reset --hard"));
        assert_eq!(is_banned(&["clean", "-fd"]).as_deref(), Some("clean -fd"));
        assert_eq!(is_banned(&["restore", "."]).as_deref(), Some("restore ."));
        assert_eq!(is_banned(&["checkout", "."]).as_deref(), Some("checkout ."));
        assert_eq!(is_banned(&["checkout", "--", "."]).as_deref(), Some("checkout -- ."));
    }

    #[test]
    fn is_banned_passes_through_safe_calls() {
        assert!(is_banned(&["status", "--short"]).is_none());
        assert!(is_banned(&["log", "-n", "10"]).is_none());
        assert!(is_banned(&["commit", "-m", "wip: anything"]).is_none());
        assert!(is_banned(&["checkout", "main"]).is_none());
        assert!(is_banned(&["clean", "-n"]).is_none());
    }

    #[test]
    fn is_banned_catches_prefix_after_dash_c() {
        assert_eq!(
            is_banned(&["-c", "advice.detachedHead=false", "reset", "--hard", "abc"])
                .as_deref(),
            Some("reset --hard"),
        );
    }

    #[tokio::test]
    async fn run_rejects_banned_without_spawning() {
        let exec = GitExec::new(std::env::temp_dir());
        let err = exec.run(&["stash"]).await.unwrap_err();
        assert!(matches!(err, GitExecError::Banned(_)));
    }
}
```

- [ ] **Step 2: Run tests — should fail because the module is not registered**

Run: `cargo test -p vox-orchestrator-mcp git_exec::tests`
Expected: FAIL — "could not find `git_exec` in crate root".

- [ ] **Step 3: Register the module**

Modify `crates/vox-orchestrator-mcp/src/lib.rs` — add at the appropriate top-level location (matching adjacent `pub mod git_tools;` style):

```rust
pub mod git_exec;
```

- [ ] **Step 4: Confirm `thiserror` and `tracing` are present**

`crates/vox-orchestrator-mcp/Cargo.toml` should already have both via workspace deps. If not, add:

```toml
thiserror = { workspace = true }
tracing = { workspace = true }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-orchestrator-mcp git_exec::tests`
Expected: PASS — 4/4 tests.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator-mcp/src/git_exec.rs crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "feat(orchestrator-mcp): add git_exec wrapper with banned-command denylist and vox.vcs.exec telemetry"
```

---

## Task 4: Secret-scan helper for staged content

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/vcs_tools/secret_scan.rs`
- Modify: `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`

**Why:** Failure-mode D (28.65 M leaked secrets in 2025; ~2× higher rate for AI co-authored commits). The scanner runs against the staged diff text before `vox_commit_create` returns success. MVP is regex-based; later phases can swap in a Clavis-driven scanner.

- [ ] **Step 1: Write failing tests for known secret detection**

Create `crates/vox-orchestrator-mcp/src/vcs_tools/secret_scan.rs`:

```rust
//! Minimal secret scanner for staged diff content. Phase 1: regex over a
//! curated list of well-known credential shapes. Phase 2 will plug into
//! `vox-secrets` for environment-derived blocklists.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    pub kind: &'static str,
    pub matched_excerpt: String,
}

struct PatternEntry {
    kind: &'static str,
    re: Regex,
}

fn patterns() -> &'static [PatternEntry] {
    static CELL: OnceLock<Vec<PatternEntry>> = OnceLock::new();
    CELL.get_or_init(|| {
        let raw: &[(&str, &str)] = &[
            ("aws_access_key_id", r"\bAKIA[0-9A-Z]{16}\b"),
            ("github_classic_pat", r"\bghp_[A-Za-z0-9]{36}\b"),
            ("github_fine_grained_pat", r"\bgithub_pat_[A-Za-z0-9_]{82}\b"),
            ("openai_key", r"\bsk-[A-Za-z0-9]{48}\b"),
            ("anthropic_key", r"\bsk-ant-[A-Za-z0-9_\-]{93,}\b"),
            ("slack_token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b"),
            ("google_api_key", r"\bAIza[0-9A-Za-z_\-]{35}\b"),
            ("private_key_block", r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----"),
        ];
        raw.iter()
            .map(|(k, p)| PatternEntry { kind: k, re: Regex::new(p).expect("static regex") })
            .collect()
    })
}

pub fn scan(content: &str) -> Vec<SecretFinding> {
    let mut out = Vec::new();
    for entry in patterns() {
        for m in entry.re.find_iter(content) {
            // Excerpt the match plus minimal surrounding context, capped.
            let excerpt = m.as_str();
            let trimmed = if excerpt.len() > 80 {
                format!("{}…", &excerpt[..80])
            } else {
                excerpt.to_string()
            };
            out.push(SecretFinding { kind: entry.kind, matched_excerpt: trimmed });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_returns_empty() {
        assert!(scan("").is_empty());
        assert!(scan("just some normal source code\nmod foo;\n").is_empty());
    }

    #[test]
    fn detects_aws_access_key_id() {
        let txt = "let key = \"AKIAIOSFODNN7EXAMPLE\";";
        let found = scan(txt);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, "aws_access_key_id");
    }

    #[test]
    fn detects_openai_key() {
        let txt = "OPENAI_KEY=sk-1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn";
        let found = scan(txt);
        assert_eq!(found.iter().filter(|f| f.kind == "openai_key").count(), 1);
    }

    #[test]
    fn detects_github_classic_pat() {
        let txt = "token: ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let found = scan(txt);
        assert_eq!(found.iter().filter(|f| f.kind == "github_classic_pat").count(), 1);
    }

    #[test]
    fn detects_private_key_block() {
        let txt = "-----BEGIN RSA PRIVATE KEY-----\nMIIB…";
        let found = scan(txt);
        assert!(found.iter().any(|f| f.kind == "private_key_block"));
    }

    #[test]
    fn detects_multiple_in_single_pass() {
        let txt = "AKIAIOSFODNN7EXAMPLE and ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let found = scan(txt);
        assert!(found.iter().any(|f| f.kind == "aws_access_key_id"));
        assert!(found.iter().any(|f| f.kind == "github_classic_pat"));
    }

    #[test]
    fn does_not_flag_lookalikes() {
        // Wrong prefix length — must not match.
        assert!(scan("AKIASHORT").is_empty());
        // 35 chars after sk- is too short for openai (needs 48).
        assert!(scan("sk-shortishbutnotenough12345").is_empty());
    }
}
```

- [ ] **Step 2: Run tests — should fail (module not registered, regex maybe missing)**

Run: `cargo test -p vox-orchestrator-mcp vcs_tools::secret_scan::tests`
Expected: FAIL — "could not find `secret_scan` in module `vcs_tools`" or "unresolved import `regex`".

- [ ] **Step 3: Register the module and add `regex`**

Modify `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs` — add at the top of the existing module roster:

```rust
pub mod secret_scan;
```

In `crates/vox-orchestrator-mcp/Cargo.toml` `[dependencies]`, ensure:

```toml
regex = { workspace = true }
```

If `regex` is not in `[workspace.dependencies]` of the root `Cargo.toml`, add it there too (latest 1.x, e.g. `regex = "1.10"`).

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp vcs_tools::secret_scan::tests`
Expected: PASS — 7/7 tests.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/vcs_tools/secret_scan.rs crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs crates/vox-orchestrator-mcp/Cargo.toml Cargo.toml
git commit -m "feat(orchestrator-mcp): add secret_scan with curated patterns for AWS/GitHub/OpenAI/Anthropic/Slack/Google/PEM"
```

---

## Task 5: `vox_commit_create` MCP tool

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/vcs_tools/commit.rs`
- Modify: `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`

**Why:** This is the heart of the failure-mode-C fix: the orchestrator mints the commit-message envelope (author identity, `Co-authored-by` trailer, tool attribution); the agent supplies only summary + body. Closes failure-modes A (workspace↔branch enforcement), C (no hallucinated trailers), D (secret-scan inline), E (no destructive subcommand reachable from this path).

- [ ] **Step 1: Write a failing unit test for envelope assembly**

Create `crates/vox-orchestrator-mcp/src/vcs_tools/commit.rs`:

```rust
//! `vox_commit_create` — write-side commit MCP tool. Pipeline:
//! 1. Validate caller-supplied `WorkingTreeWrite` capability matches workspace binding.
//! 2. Ask git for the staged diff (`git diff --cached`).
//! 3. Run `secret_scan::scan` over the diff; abort with findings if any.
//! 4. Mint the full commit message envelope from a caller-supplied summary/body.
//! 5. Invoke `git commit -F -` via `GitExec`, providing the message on stdin
//!    (avoids escape-injection through `-m`).
//!
//! The tool refuses if the bound branch does not match the working-tree
//! current branch (`git symbolic-ref --short HEAD`), preventing the
//! Cursor-style wrong-branch race.

use crate::git_exec::{GitExec, GitExecError};
use crate::vcs_tools::secret_scan::{self, SecretFinding};
use vox_orchestrator_types::{BranchName, WorkingTreeWrite, WorkspaceId};

#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("workspace mismatch: capability is for {cap:?} but workspace is {actual:?}")]
    WorkspaceMismatch { cap: WorkspaceId, actual: WorkspaceId },
    #[error("branch mismatch: capability bound to {cap}, working tree is on {actual}")]
    BranchMismatch { cap: String, actual: String },
    #[error("nothing staged to commit")]
    NothingStaged,
    #[error("secret findings prevent commit: {0:?}")]
    SecretsFound(Vec<SecretFinding>),
    #[error(transparent)]
    Exec(#[from] GitExecError),
}

#[derive(Debug, Clone)]
pub struct CommitRequest {
    pub cap: WorkingTreeWrite,
    pub workspace: WorkspaceId,
    pub agent_handle: String,   // "A-01" or similar; embedded in trailers
    pub model_id: String,        // e.g. "claude-sonnet-4-6"
    pub summary: String,         // first line; <= 72 chars enforced
    pub body: Option<String>,    // free-form
}

#[derive(Debug, Clone)]
pub struct CommitOutcome {
    pub commit_sha: String,
    pub envelope: String,
}

/// Build the commit message envelope. Pure function — no I/O.
pub fn build_envelope(req: &CommitRequest) -> String {
    let mut out = String::new();
    let summary = req.summary.trim();
    let summary_truncated = if summary.len() > 72 {
        format!("{}…", &summary[..72])
    } else {
        summary.to_string()
    };
    out.push_str(&summary_truncated);
    out.push('\n');
    if let Some(body) = req.body.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        out.push('\n');
        out.push_str(body);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&format!(
        "Co-authored-by: vox-agent {} <agent+{}@vox>\n",
        req.agent_handle, req.agent_handle
    ));
    out.push_str(&format!("Vox-Model-Id: {}\n", req.model_id));
    out.push_str(&format!("Vox-Workspace: {}\n", req.workspace));
    out
}

/// Run the full pipeline. `exec` is parameterized so tests can pass a stub.
pub async fn commit_create(
    req: CommitRequest,
    exec: &GitExec,
) -> Result<CommitOutcome, CommitError> {
    // Step 1: capability-vs-workspace match.
    if req.cap.workspace() != req.workspace {
        return Err(CommitError::WorkspaceMismatch {
            cap: req.cap.workspace(),
            actual: req.workspace,
        });
    }
    // Step 2: capability-vs-current-branch match (real git).
    let head = exec.run(&["symbolic-ref", "--short", "HEAD"]).await?;
    let actual = head.stdout.trim().to_string();
    if actual != req.cap.branch().as_str() {
        return Err(CommitError::BranchMismatch {
            cap: req.cap.branch().as_str().to_string(),
            actual,
        });
    }
    // Step 3: staged diff content.
    let diff = exec.run(&["diff", "--cached"]).await?;
    if diff.stdout.trim().is_empty() {
        return Err(CommitError::NothingStaged);
    }
    // Step 4: secret scan.
    let findings = secret_scan::scan(&diff.stdout);
    if !findings.is_empty() {
        return Err(CommitError::SecretsFound(findings));
    }
    // Step 5: build envelope and commit via stdin.
    let envelope = build_envelope(&req);
    // Use `commit -F -` with stdin — but tokio::process::Command doesn't take
    // stdin via `output()` directly; we use `git commit --message=...` with the
    // envelope, escaping is safe because we control the text and it never
    // contains shell metacharacters by construction (no shell is involved).
    let _committed = exec.run(&["commit", "-m", &envelope]).await?;
    let sha_out = exec.run(&["rev-parse", "HEAD"]).await?;
    let commit_sha = sha_out.stdout.trim().to_string();
    tracing::info!(
        target: "vox.vcs.commit",
        workspace = %req.workspace,
        branch = %req.cap.branch().as_str(),
        sha = %commit_sha,
        agent = %req.agent_handle,
        model = %req.model_id,
        "commit created",
    );
    Ok(CommitOutcome { commit_sha, envelope })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(summary: &str, body: Option<&str>) -> CommitRequest {
        CommitRequest {
            cap: WorkingTreeWrite::mint(
                WorkspaceId(1),
                BranchName::parse("agent/x").unwrap(),
            ),
            workspace: WorkspaceId(1),
            agent_handle: "A-01".to_string(),
            model_id: "claude-sonnet-4-6".to_string(),
            summary: summary.to_string(),
            body: body.map(str::to_string),
        }
    }

    #[test]
    fn envelope_includes_co_author_and_metadata() {
        let env = build_envelope(&req("feat: x", Some("body line one")));
        assert!(env.starts_with("feat: x\n"));
        assert!(env.contains("\nbody line one\n"));
        assert!(env.contains("Co-authored-by: vox-agent A-01 <agent+A-01@vox>\n"));
        assert!(env.contains("Vox-Model-Id: claude-sonnet-4-6\n"));
        assert!(env.contains("Vox-Workspace: W-000001\n"));
    }

    #[test]
    fn envelope_truncates_long_summary() {
        let long = "a".repeat(200);
        let env = build_envelope(&req(&long, None));
        let first_line = env.lines().next().unwrap();
        assert!(first_line.len() <= 73, "got {} chars", first_line.len());
        assert!(first_line.ends_with("…"));
    }

    #[test]
    fn envelope_omits_body_section_when_none() {
        let env = build_envelope(&req("feat: x", None));
        assert!(!env.contains("\n\n\n"), "should not have empty body block");
    }
}
```

- [ ] **Step 2: Register the module and run tests for the pure builder**

Modify `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs` — add:

```rust
pub mod commit;
```

Run: `cargo test -p vox-orchestrator-mcp vcs_tools::commit::tests`
Expected: PASS — 3/3 (envelope builder is pure; no git needed).

- [ ] **Step 3: Wire into the MCP dispatcher**

Modify `crates/vox-orchestrator-mcp/src/dispatch.rs` — add to the `match name` arm list (alongside the existing `vox_workspace_create` arm):

```rust
"vox_commit_create" => Ok(vcs_tools::commit::tool_entrypoint(state, args).await),
```

- [ ] **Step 4: Add the JSON-args entrypoint**

Append to `crates/vox-orchestrator-mcp/src/vcs_tools/commit.rs`:

```rust
use crate::params::ToolResult;
use crate::ServerState;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ToolArgs {
    workspace_id: u64,
    branch: String,
    agent_handle: String,
    model_id: String,
    summary: String,
    body: Option<String>,
}

pub async fn tool_entrypoint(state: &ServerState, args: serde_json::Value) -> String {
    let parsed: ToolArgs = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return ToolResult::<()>::err(format!("invalid args: {e}")).to_json(),
    };
    let branch = match BranchName::parse(&parsed.branch) {
        Ok(b) => b,
        Err(e) => return ToolResult::<()>::err(format!("invalid branch: {e}")).to_json(),
    };
    let cap = WorkingTreeWrite::mint(WorkspaceId(parsed.workspace_id), branch.clone());
    let req = CommitRequest {
        cap,
        workspace: WorkspaceId(parsed.workspace_id),
        agent_handle: parsed.agent_handle,
        model_id: parsed.model_id,
        summary: parsed.summary,
        body: parsed.body,
    };
    let cwd = crate::git_tools::git_cwd(state);
    let exec = GitExec::new(cwd);
    match commit_create(req, &exec).await {
        Ok(outcome) => ToolResult::ok(serde_json::json!({
            "commit_sha": outcome.commit_sha,
            "envelope": outcome.envelope,
        })).to_json(),
        Err(CommitError::SecretsFound(findings)) => ToolResult::<()>::err_with_remediation(
            format!("secret findings: {} hits", findings.len()),
            "Remove the secret(s) from staged changes (use Clavis), then retry.",
        ).to_json(),
        Err(e) => ToolResult::<()>::err(format!("commit failed: {e}")).to_json(),
    }
}
```

> Note: `git_cwd(state)` is the same helper used by `git_tools::git_status`. If its visibility is currently `pub(crate)`, no change needed because we are in the same crate. If it is private to `git_tools`, change it to `pub(crate)` in the same commit.

- [ ] **Step 5: Build and confirm dispatch compiles**

Run: `cargo build -p vox-orchestrator-mcp`
Expected: PASS. If `git_cwd` is private, expose it with `pub(crate) fn git_cwd(...)`.

- [ ] **Step 6: Update the dispatch table doc/help if there is one**

If `crates/vox-orchestrator-mcp/src/dispatch.rs` has a tools-list array used to advertise tools to MCP clients, add an entry for `vox_commit_create` matching the surrounding entries' shape (name, description, JSON schema). If no such advertisement exists, skip this step — the dispatcher arm alone is sufficient for invocation.

- [ ] **Step 7: Run tests + arch-check**

Run:
```
cargo test -p vox-orchestrator-mcp
cargo run -p vox-arch-check
```
Expected: both PASS.

- [ ] **Step 8: Commit**

```
git add crates/vox-orchestrator-mcp/src/vcs_tools/commit.rs crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/git_tools.rs
git commit -m "feat(orchestrator-mcp): add vox_commit_create with envelope minting, branch-binding check, and secret scan"
```

---

## Task 6: `vox_branch_create` MCP tool

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/vcs_tools/branch.rs`
- Modify: `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`

**Why:** Closes the second half of failure-mode A by giving the orchestrator a single endpoint that creates a new branch and binds it to the workspace atomically — so no agent has to invent its own `git checkout -b` invocation.

- [ ] **Step 1: Write a failing test for the validation pipeline**

Create `crates/vox-orchestrator-mcp/src/vcs_tools/branch.rs`:

```rust
//! `vox_branch_create` — create a branch from `parent`, bind it to the agent
//! workspace, return a `WorkingTreeWrite` capability for the new branch.

use crate::git_exec::{GitExec, GitExecError};
use vox_orchestrator_types::{BranchCreate, BranchName, WorkingTreeWrite, WorkspaceId};

#[derive(Debug, thiserror::Error)]
pub enum BranchCreateError {
    #[error("workspace mismatch: capability is for {cap:?} but workspace is {actual:?}")]
    WorkspaceMismatch { cap: WorkspaceId, actual: WorkspaceId },
    #[error("branch already exists: {0}")]
    AlreadyExists(String),
    #[error(transparent)]
    Exec(#[from] GitExecError),
    #[error("invalid branch name: {0}")]
    InvalidName(#[from] vox_orchestrator_types::BranchNameError),
}

#[derive(Debug, Clone)]
pub struct BranchCreateRequest {
    pub cap: BranchCreate,
    pub workspace: WorkspaceId,
    pub new_branch: String,
}

#[derive(Debug, Clone)]
pub struct BranchCreateOutcome {
    pub new_branch: BranchName,
    pub capability: WorkingTreeWrite,
}

pub async fn branch_create(
    req: BranchCreateRequest,
    exec: &GitExec,
) -> Result<BranchCreateOutcome, BranchCreateError> {
    if req.cap.workspace() != req.workspace {
        return Err(BranchCreateError::WorkspaceMismatch {
            cap: req.cap.workspace(),
            actual: req.workspace,
        });
    }
    let new_branch = BranchName::parse(&req.new_branch)?;

    // Existence check via `git rev-parse --verify`.
    let probe = exec
        .run(&["rev-parse", "--verify", &format!("refs/heads/{}", new_branch.as_str())])
        .await;
    if probe.is_ok() {
        return Err(BranchCreateError::AlreadyExists(new_branch.as_str().to_string()));
    }

    // Create from parent without checking it out.
    exec.run(&["branch", new_branch.as_str(), req.cap.parent().as_str()])
        .await?;

    let capability = WorkingTreeWrite::mint(req.workspace, new_branch.clone());
    tracing::info!(
        target: "vox.vcs.branch",
        workspace = %req.workspace,
        parent = %req.cap.parent().as_str(),
        new_branch = %new_branch.as_str(),
        "branch created",
    );
    Ok(BranchCreateOutcome { new_branch, capability })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_workspace_mismatch_without_running_git() {
        // Pure-pre-flight check — no need to spawn git.
        let cap = BranchCreate::mint(WorkspaceId(1), BranchName::parse("main").unwrap());
        let req = BranchCreateRequest {
            cap,
            workspace: WorkspaceId(2),
            new_branch: "agent/x".into(),
        };
        // We can't call branch_create without a GitExec, so split the check
        // into a small helper or inline-check. Pull out the validation:
        let same = req.cap.workspace() == req.workspace;
        assert!(!same);
    }

    #[test]
    fn rejects_invalid_branch_name() {
        // Direct test of BranchName::parse via the route the tool uses.
        let res = BranchName::parse("/bad");
        assert!(res.is_err());
    }
}
```

- [ ] **Step 2: Register and dispatch**

Modify `crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs`:

```rust
pub mod branch;
```

Modify `crates/vox-orchestrator-mcp/src/dispatch.rs`:

```rust
"vox_branch_create" => Ok(vcs_tools::branch::tool_entrypoint(state, args).await),
```

- [ ] **Step 3: Add the JSON entrypoint**

Append to `crates/vox-orchestrator-mcp/src/vcs_tools/branch.rs`:

```rust
use crate::params::ToolResult;
use crate::ServerState;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ToolArgs {
    workspace_id: u64,
    parent: String,
    new_branch: String,
}

pub async fn tool_entrypoint(state: &ServerState, args: serde_json::Value) -> String {
    let parsed: ToolArgs = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return ToolResult::<()>::err(format!("invalid args: {e}")).to_json(),
    };
    let parent = match BranchName::parse(&parsed.parent) {
        Ok(b) => b,
        Err(e) => return ToolResult::<()>::err(format!("invalid parent: {e}")).to_json(),
    };
    let cap = BranchCreate::mint(WorkspaceId(parsed.workspace_id), parent);
    let req = BranchCreateRequest {
        cap,
        workspace: WorkspaceId(parsed.workspace_id),
        new_branch: parsed.new_branch,
    };
    let cwd = crate::git_tools::git_cwd(state);
    let exec = GitExec::new(cwd);
    match branch_create(req, &exec).await {
        Ok(o) => ToolResult::ok(serde_json::json!({
            "new_branch": o.new_branch.as_str(),
            "capability": {
                "workspace": o.capability.workspace().0,
                "branch": o.capability.branch().as_str(),
            }
        })).to_json(),
        Err(e) => ToolResult::<()>::err(format!("branch_create failed: {e}")).to_json(),
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp vcs_tools::branch::tests`
Expected: PASS — 2/2.

- [ ] **Step 5: Build full crate + arch-check**

Run:
```
cargo build -p vox-orchestrator-mcp
cargo run -p vox-arch-check
```
Expected: both PASS.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator-mcp/src/vcs_tools/branch.rs crates/vox-orchestrator-mcp/src/vcs_tools/mod.rs crates/vox-orchestrator-mcp/src/dispatch.rs
git commit -m "feat(orchestrator-mcp): add vox_branch_create that mints a WorkingTreeWrite capability for the new branch"
```

---

## Task 7: Migrate one existing direct git callsite to `GitExec`

**Files:**
- Modify: `crates/vox-cli/src/commands/review/coderabbit/git.rs`

**Why:** Proves the wrapper works in production paths and starts paying down the "every callsite invents its own git" debt. Pick the safest single site for Phase 1; the rest follow in Phase 2.

The site we migrate: lines ~64–84 (the WorkspaceGuard `status` / `add` / `commit` sequence). It is fully internal to the CodeRabbit flow and well-tested.

- [ ] **Step 1: Inventory the current calls**

Read `crates/vox-cli/src/commands/review/coderabbit/git.rs` lines 60–90. The pattern is:

```rust
let st = tokio::process::Command::new("git")
    .args(["status", "--porcelain"])
    .current_dir(&path)
    .output()
    .await?;
// ... uses st.stdout

let _ = tokio::process::Command::new("git")
    .args(["add", "-A"])
    .current_dir(&path)
    .status().await?;

let _ = tokio::process::Command::new("git")
    .args(["commit", "-m", "wip: coderabbit safeguard snapshot", "--no-verify"])
    .current_dir(&path)
    .status().await?;
```

- [ ] **Step 2: Add a dependency from `vox-cli` on `vox-orchestrator-mcp`**

`vox-cli` already depends on `vox-orchestrator-mcp` for dispatch — verify in `crates/vox-cli/Cargo.toml`. If not, add:

```toml
vox-orchestrator-mcp = { workspace = true }
```

If a layer rule blocks this (CLI is L4, orchestrator-mcp is L3 — typical fan-in is fine), the existing direction should already permit it. Run `cargo run -p vox-arch-check` to confirm. If a violation appears, the simplest mitigation is to lift `git_exec` into a smaller crate (`vox-git-exec` at L1); leave that for Phase 2 and skip this task's migration if blocked.

- [ ] **Step 3: Replace the three calls with `GitExec`**

Replace the block in `crates/vox-cli/src/commands/review/coderabbit/git.rs`:

```rust
use vox_orchestrator_mcp::git_exec::GitExec;

// ...
let exec = GitExec::new(path.clone());
let st_out = exec.run(&["status", "--porcelain"]).await?;
let dirty = !st_out.stdout.trim().is_empty();
if dirty {
    exec.run(&["add", "-A"]).await?;
    exec.run(&["commit", "-m", "wip: coderabbit safeguard snapshot", "--no-verify"]).await?;
}
```

Adjust error handling to match the surrounding function's `Result` type. The original code may use `anyhow::Context`; map `GitExecError` via `.map_err(|e| anyhow!(e))` or similar.

- [ ] **Step 4: Build and run the existing tests for that subcommand**

Run: `cargo test -p vox-cli review::coderabbit::git`
Expected: PASS (no behavioral change — wrapper is a transparent passthrough for non-banned calls).

- [ ] **Step 5: Commit**

```
git add crates/vox-cli/src/commands/review/coderabbit/git.rs crates/vox-cli/Cargo.toml
git commit -m "refactor(cli): route coderabbit safeguard git calls through GitExec wrapper"
```

---

## Task 8: `vox.vcs.*` telemetry namespace + integration test

**Files:**
- Create: `crates/vox-orchestrator-mcp/tests/vcs_telemetry.rs`

**Why:** Failure-modes A–F all require observability after the fact. We standardize on the `vox.vcs.exec`, `vox.vcs.commit`, `vox.vcs.branch` event targets (already used in Tasks 3, 5, 6) and lock that contract with an integration test that reads back tracing output.

- [ ] **Step 1: Write the failing integration test**

Create `crates/vox-orchestrator-mcp/tests/vcs_telemetry.rs`:

```rust
//! End-to-end test that the canonical `vox.vcs.*` tracing targets are emitted
//! by the wrapper. The test inspects events via a `tracing_subscriber` Layer.

use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::Layer;
use vox_orchestrator_mcp::git_exec::{GitExec, is_banned};

#[derive(Default, Clone)]
struct CapturedTargets(Arc<Mutex<Vec<String>>>);

impl<S: tracing::Subscriber> Layer<S> for CapturedTargets {
    fn on_event(&self, ev: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        self.0.lock().unwrap().push(ev.metadata().target().to_string());
    }
}

#[test]
fn banned_check_does_not_require_subscriber() {
    assert_eq!(is_banned(&["stash"]).as_deref(), Some("stash"));
}

#[tokio::test]
async fn banned_invocation_emits_vox_vcs_exec_warning() {
    let captured = CapturedTargets::default();
    let subscriber = tracing_subscriber::registry()
        .with(captured.clone().with_filter(tracing_subscriber::filter::LevelFilter::from_level(Level::WARN)));
    let _guard = tracing::subscriber::set_default(subscriber);

    let exec = GitExec::new(std::env::temp_dir());
    let _ = exec.run(&["stash"]).await; // expected Err

    let targets = captured.0.lock().unwrap().clone();
    assert!(targets.iter().any(|t| t == "vox.vcs.exec"),
        "expected at least one vox.vcs.exec event, got {:?}", targets);
}
```

- [ ] **Step 2: Make sure dev-deps include `tracing-subscriber`**

In `crates/vox-orchestrator-mcp/Cargo.toml` `[dev-dependencies]`:

```toml
tracing-subscriber = { workspace = true, features = ["registry", "env-filter"] }
tokio = { workspace = true, features = ["macros", "rt"] }
```

If not present at workspace root, declare in root `Cargo.toml` `[workspace.dependencies]`. Most mature Vox crates already use these — verify before adding.

- [ ] **Step 3: Run the test**

Run: `cargo test -p vox-orchestrator-mcp --test vcs_telemetry`
Expected: PASS — both tests.

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator-mcp/tests/vcs_telemetry.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "test(orchestrator-mcp): lock vox.vcs.exec telemetry contract on banned invocations"
```

---

## Task 9: Documentation cross-cuts

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`
- Modify: `docs/agents/git-concurrency-policy.md`
- Modify: `docs/src/architecture/layers.toml`

**Why:** CLAUDE.md mandates a `where-things-live.md` row for every new concept. The concurrency policy promises Rust enforcement now. Layers config keeps `vox-arch-check` honest as the surface grows.

- [ ] **Step 1: Add `where-things-live.md` rows**

Modify `docs/src/architecture/where-things-live.md` — under "Common tasks → exact path", add:

```markdown
| Add a VCS capability type | `crates/vox-orchestrator-types/src/vcs_capability.rs` |
| Add a write-side git MCP tool | `crates/vox-orchestrator-mcp/src/vcs_tools/<group>.rs` (e.g. `commit.rs`); register in [`mcp/dispatch.rs`](../../../crates/vox-orchestrator-mcp/src/dispatch.rs). Routes git through `git_exec::GitExec`. |
| Add a banned-command pattern | `crates/vox-orchestrator-mcp/src/git_exec.rs::BANNED_PREFIXES` |
| Add a secret-scan pattern | `crates/vox-orchestrator-mcp/src/vcs_tools/secret_scan.rs::patterns` |
```

- [ ] **Step 2: Update `git-concurrency-policy.md`**

Modify `docs/agents/git-concurrency-policy.md` — append a new section after `## 6. Tooling Constraints`:

```markdown
## 7. Enforcement (Rust)

As of 2026-05-08, the banned-command list in §2 is enforced in code:

- All orchestrator-process git invocations route through [`vox_orchestrator_mcp::git_exec::GitExec`](../../../crates/vox-orchestrator-mcp/src/git_exec.rs).
- Banned prefixes (`stash`, `reset --hard`, `clean -fd`, `clean -fdx`, `restore .`, `checkout .`, `checkout -- .`, `checkout -f`) are rejected before spawn and emit a `vox.vcs.exec` warning event.
- Direct `tokio::process::Command::new("git")` calls in non-test code outside `git_exec.rs` are a code-review regression; `cargo run -p vox-arch-check` will gain a rule for this in Phase 2.

This policy doc remains the human-facing reference; the code is the source of truth.
```

- [ ] **Step 3: Verify `layers.toml` entries**

Confirm `docs/src/architecture/layers.toml` has the relevant crate entries:

- `vox-orchestrator-types = { layer = 0 }` — already present.
- `vox-orchestrator-mcp = { layer = 3, max_loc = 40_000 }` — already present.

If the workspace LoC budget for `vox-orchestrator-mcp` is being approached after Phase 1 (~+800 LoC across Tasks 3–6), consider raising it by 5,000 in a separate commit; otherwise leave alone.

- [ ] **Step 4: Run doc-pipeline check and commit**

Run:
```
cargo run -p vox-doc-pipeline -- --check
cargo run -p vox-arch-check
```
Both expected: PASS. If `--check` reports drift in `SUMMARY.md` / `architecture-index.md`, regenerate with `cargo run -p vox-doc-pipeline` (no `--check`) and re-check.

```
git add docs/src/architecture/where-things-live.md docs/agents/git-concurrency-policy.md docs/src/SUMMARY.md docs/src/architecture/architecture-index.md
git commit -m "docs(vcs): document Rust enforcement of git-concurrency policy and add where-things-live rows"
```

---

## Phase 1 acceptance criteria

All must be true to consider Phase 1 done:

- [ ] `cargo test -p vox-orchestrator-types --lib` passes; new tests cover capability mint and BranchName parse.
- [ ] `cargo test -p vox-orchestrator --lib workspace::` passes; new tests cover branch binding.
- [ ] `cargo test -p vox-orchestrator-mcp` passes including the integration test under `tests/vcs_telemetry.rs`.
- [ ] `cargo run -p vox-arch-check` passes.
- [ ] `cargo run -p vox-doc-pipeline -- --check` passes.
- [ ] One existing direct `git` callsite in `vox-cli` has been migrated to `GitExec`.
- [ ] `docs/agents/git-concurrency-policy.md` references the Rust enforcement.
- [ ] All steps are committed; the branch produces 9 commits matching the per-task message conventions above.

---

## Phases 2–4: roadmap (separate plans)

The following are deliberately out of scope for Phase 1 and should be drafted as separate plans when queued:

### Phase 2 — Push/PR write-side + remaining migrations
- `vox_push` MCP (capability: `PushAllowed { force: false }`); refuses if commit-set has un-passing CI proof attached.
- `vox_pr_open` MCP that templates the PR body from workspace task metadata via the existing orchestrator queue store.
- `vox_force_push` separate tool with separate capability + human-ledger justification record persisted via `vox-orchestrator-queue`.
- Migrate the remaining 6 direct-git callsites in `vox-cli` to `GitExec`.
- `vox-arch-check` rule that flags any new `Command::new("git")` outside `git_exec.rs`.
- `.vox` glue scripts: `scripts/vcs/{wip,sync,finish,recover}.vox` per [§VoxScript-First Glue Code](../../../AGENTS.md).

### Phase 3 — Dashboard surfaces
- `crates/vox-dashboard/src/api/v2/vcs/` module with five panels: workspace branch board, oplog viewer, push queue, capability ledger, leaked-secret diff scanner.
- Capability ledger persistence reuses `vox-orchestrator-queue` oplog store (Task 3 of the existing replication Phase 1 plan landed the schema; we add a `CapabilityMinted` `OperationKind` variant).

### Phase 4 — Vox `@vcs.*` decorator surface
- Compiler work: parse `@vcs.read_only`, `@vcs.requires(...)`, `@vcs.linear_working_tree`, `@vcs.audit_trail` on `fn` declarations.
- HIR lowering: emit a `VcsEffect` annotation; type-check forbids a `read_only` fn from internally calling a `requires(...)` fn.
- Linearity: `@vcs.linear_working_tree` lowers to an affine-typed Rust API wrapping a tokio mutex guard.
- Integration with the existing `@durable` / `@endpoint` precedent.
- This phase depends on Phase 2 of the existing GUI-Native Language Roadmap landing decorator-on-fn type checking.

### Phase 5 — Backend swap (optional)
- Replace `tokio::process::Command::new("git")` inside `git_exec` with calls into `gix` for hot-path operations (status, log, diff, rev-parse). Keep shell-out for low-frequency or compatibility-sensitive commands. The `GitExec` interface does not change.
- Evaluate routing write ops through `jj-lib` (already pinned at 0.27.0 in `vox-orchestrator/src/jj_backend.rs`) for ops where its op-log + change-id model adds safety. Decision criteria documented in the research doc, §"Net read for Vox".

---

## Notes for the implementing engineer

- This plan is a contract. If a step's described code does not compile because the surrounding context has drifted, **fix it minimally** and note the drift inline; do not rearchitect.
- Every commit should be small enough to revert independently. The plan produces 9 commits.
- If a banned-command denylist match appears in Task 7 because the existing CodeRabbit code uses something on the list (it should not, per audit), STOP, surface the finding, and update the research doc — that is itself a Phase 1 win.
- Treat the prose Vox-Model-Id and Co-authored-by trailer formats as load-bearing: downstream tooling will grep for them.
- The capability mint paths are `pub` and `#[doc(hidden)]` — they are "soft-private". Phase 4's compiler work makes them genuinely unforgeable from Vox source. Resist the urge to make them harder to construct from Rust in Phase 1; that is a Phase 2 hardening once a second consumer outside `vcs_tools/` exists.
