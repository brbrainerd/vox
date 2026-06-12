# Policy Registry — Phase 1d: GUI Policies Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the GUI **Policies** surface (Tauri + React) over the unified policy registry + per-branch status — a **read-only, visually-organized catalog** where any rule's contents are readable. The rule **detail + contents pane is the primary/largest area**; "Needs attention" gracefully shrinks to an all-clear state when empty; a secondary **group rail** reuses the existing `Sidebar` collapse machinery; per-group **status-colored counts** and a **master-sidebar badge** colored by worst status; a **multi-branch selector** (worktrees) where every section reflects the selected branch set; **Edit ✎ / Disable ⏻** buttons present but gated read-only with a phase tooltip.

**Architecture:** `vox-gui` already depends on `vox-config` directly ([`crates/vox-gui/Cargo.toml:21`](../../../crates/vox-gui/Cargo.toml)), so the four IPC handlers (`policy_list`, `policy_show`, `policy_status`, `list_branches`) call **`vox-config`'s loader in-process** — no CLI shell-out, no daemon round-trip — except `list_branches`, which shells `git` (net-new; no git dep needed). The surface is registered in [`contracts/gui/surface-registry.v1.yaml`](../../../contracts/gui/surface-registry.v1.yaml) (rename `matrix`'s label to "Routing"; add `view_key: policies`, `nav_group: operate`) and regenerated into [`crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`](../../../crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts). React: a new `PoliciesView` surface in `components/surfaces/Policies/`, with a pure group-tree builder unit-tested under vitest.

**Tech Stack:** Rust (Tauri 2 `#[command]`, `serde`, `serde_yaml` — all present in `vox-gui`), `vox-config` (`PolicyRegistry` model + `load_policy_registry`). Frontend: React 19, TypeScript, Tailwind, vitest 2.1 (`vitest.config.ts` present; `npm test` → `vitest run`). Format Rust with `cargo fmt -p vox-gui` (never `--all` on Windows). Build with `cargo build -p vox-gui`.

**Scope note:** Phase 1d of the initiative in
[`docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`](../specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md) — read its **§4.7 (layout)** and **§10 addendum** (verified file:line evidence). Pattern + quality bar follow
[`docs/superpowers/plans/2026-06-06-policy-registry-phase-1a-catalog-foundation.md`](2026-06-06-policy-registry-phase-1a-catalog-foundation.md).

**Verified pre-state (hand-checked against real code, 2026-06-06):**
- Plan 1a **is landed**: `crates/vox-config/src/policy/registry.rs` exists; `PolicyRegistry`/`load_policy_registry` are re-exported from `vox-config`; `crates/vox-cli/src/commands/policy/mod.rs` exists; `contracts/policy/policy-registry.v1.yaml` + schema exist. This plan consumes that, it does not rebuild it.
- Plan 1c (status store) is **NOT landed** — no `PolicyRunReport` / `policy-status` reader in `vox-config/src/`, no `.vox/policy-status/` dir. **Therefore `policy_status` is designed to tolerate a missing store and degrade every rule to grey `not_run`.** When 1c lands its reader, swap the stub body (Task 4) for the real reader call; the DTO/frontend contract does not change.
- Nav collision confirmed: `view_key: matrix` owns `nav_label: Policies` ([`surface-registry.v1.yaml:46-52`](../../../contracts/gui/surface-registry.v1.yaml)) — rename to "Routing".
- Sidebar collapse already exists as `SidebarMode = 'rail' | 'default' | 'wide'` ([`Sidebar.tsx:8`](../../../crates/vox-gui/ui/src/components/layout/Sidebar.tsx)) + per-section collapse (`collapsedSections`, [`Sidebar.tsx:95-97`](../../../crates/vox-gui/ui/src/components/layout/Sidebar.tsx)). Reuse — do not build new collapse.
- GUI branch access is net-new: no branch/worktree handler exists in `crates/vox-gui/src/commands/` (verified: 20 modules, none git-related). `tauri-plugin-shell` is initialized ([`main.rs:47`](../../../crates/vox-gui/src/main.rs)) but `list_branches` uses `std::process::Command` directly — simpler and synchronous.

---

## File Structure

**Create:**
- `crates/vox-gui/src/commands/policy.rs` — 4 Tauri handlers (`policy_list`, `policy_show`, `policy_status`, `list_branches`) + DTOs.
- `crates/vox-gui/ui/src/components/surfaces/Policies/policyTree.ts` — pure group-tree builder + status roll-up (unit target).
- `crates/vox-gui/ui/src/components/surfaces/Policies/policyTree.test.ts` — vitest unit tests.
- `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx` — the surface (3-pane layout).
- `crates/vox-gui/ui/src/components/surfaces/Policies/types.ts` — shared TS types mirroring the Rust DTOs.

**Modify:**
- `contracts/gui/surface-registry.v1.yaml` — rename `matrix` label → "Routing"; add `policies` entry.
- `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` — regenerated (never hand-edited).
- `crates/vox-gui/src/commands/mod.rs` — add `pub mod policy;`.
- `crates/vox-gui/src/main.rs` — register 4 handlers in `tauri::generate_handler!`.
- `crates/vox-gui/ui/src/App.tsx` — add `'policies'` to `View` union, import + `case 'policies'` in `renderView()`, and the initial-view allow-list.
- `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` — add `policies` to the `operate` `SECTION_ORDER` + slot the master status badge.

**Defers (documented, not gaps):**
- Edit/Disable **toggling** → **Phase 2** (buttons render but are disabled with a tooltip here).
- User-authored custom rules → **Phase 4**.
- Real per-branch status numbers → **Plan 1c** (this plan ships the grey-degrading reader + the full color pipeline so 1c is a one-line swap).
- Playwright golden-route smoke is behind the existing GUI smoke flag (`test:e2e`, not in CI by default) — noted in Task 9, not wired blocking.

---

## Task 1: Surface registry — rename Matrix label, add `policies`

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml`
- Regenerate: `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`

- [ ] **Step 1: Rename the Matrix label**

In `contracts/gui/surface-registry.v1.yaml`, the `matrix` entry currently at lines 46-52 reads:

```yaml
- view_key: matrix
  cli_group: null
  representation_tier: live_backend
  nav_label: Policies
  nav_icon: scale
  nav_group: operate
  notes: policy/intention matrix
```

Change **only** `nav_label: Policies` → `nav_label: Routing` (the orchestrator routing-policy matrix; freeing the "Policies" label). Leave `nav_icon: scale` and `notes` as-is.

- [ ] **Step 2: Add the new `policies` surface**

Insert a new entry (keep the file's existing alphabetical-by-`view_key` ordering among the `view_key`-bearing block; place it after `oratio`/before `populi`, matching how the generator sorts — exact position is cosmetic, the generator re-sorts). Use `nav_icon: scale` (the freed icon; `scale` exists in `Icons.tsx:124`):

```yaml
- view_key: policies
  cli_group: null
  representation_tier: live_backend
  nav_label: Policies
  nav_icon: scale
  nav_group: operate
  notes: unified policy registry catalog + per-branch status (read-only Phase 1)
```

- [ ] **Step 3: Regenerate the TypeScript registry**

Run (this is the generator that writes the `.generated.ts` — **never hand-edit the generated file**):

```bash
cargo run -p vox-cli -- ci gui-surface-registry --write
```

Expected: `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` now contains
`{ viewKey: 'policies', ... navLabel: 'Policies', navIcon: 'scale', navGroup: 'operate' }`
and the `matrix` row now reads `navLabel: 'Routing'`.

- [ ] **Step 4: Verify the registry self-check passes**

```bash
cargo run -p vox-cli -- ci gui-surface-registry
```

Expected: drift gate green (committed YAML ↔ generated TS in sync).

- [ ] **Step 5: Commit**

```bash
git add contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
git commit -m "feat(gui): register Policies surface; rename Matrix nav label to Routing"
```

---

## Task 2: Backend DTOs + `policy_list` / `policy_show` handlers (in-process)

**Files:**
- Create: `crates/vox-gui/src/commands/policy.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`

> **Strategy (decided):** call `vox_config::load_policy_registry(repo_root)` **in-process**. `vox-gui` already depends on `vox-config` ([`Cargo.toml:21`](../../../crates/vox-gui/Cargo.toml)) which re-exports `PolicyRegistry`/`PolicyEntry`/`load_policy_registry`/`REGISTRY_REL_PATH` (Plan 1a). No CLI shell-out, no daemon RPC. `vox-config` does not self-discover the repo root, so the handler resolves it from `std::env::current_dir()` walking up to the dir containing `contracts/policy/policy-registry.v1.yaml`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/src/commands/policy.rs` with the DTOs + a repo-root finder + a tests module first:

```rust
//! Tauri IPC for the read-only "Policies" GUI surface (Phase 1d).
//!
//! In-process over `vox_config::load_policy_registry`; no CLI shell-out. Status
//! reads tolerate a missing Plan-1c store (every rule degrades to grey `not_run`).
//! NOTHING here toggles a rule — Edit/Disable are Phase 2.

use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::command;
use vox_config::{PolicyEntry, PolicyRegistry};

/// One catalog row for the GUI list/group rail. Non-sensitive metadata only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRowDto {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub group: String,
    pub severity: Option<String>,
    pub blocking: bool,
    pub protected: bool,
}

/// Full detail incl. the rule *contents* (the edit target shown in the primary pane).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDetailDto {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub group: String,
    pub description: String,
    pub severity: Option<String>,
    pub blocking: bool,
    pub runs_on: Vec<String>,
    pub protected: bool,
    pub origin: String,
    pub docs: Option<String>,
    pub source_kind: String,
    pub source_ref: String,
    pub source_detail: Option<String>,
}

fn domain_str(e: &PolicyEntry) -> String {
    serde_yaml::to_string(&e.domain)
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_string()
}

fn sev_str(e: &PolicyEntry) -> Option<String> {
    e.severity.map(|s| format!("{s:?}").to_lowercase())
}

impl From<&PolicyEntry> for PolicyRowDto {
    fn from(e: &PolicyEntry) -> Self {
        Self {
            id: e.id.clone(),
            domain: domain_str(e),
            title: e.title.clone(),
            group: e.group.clone(),
            severity: sev_str(e),
            blocking: e.blocking,
            protected: e.protected,
        }
    }
}

impl From<&PolicyEntry> for PolicyDetailDto {
    fn from(e: &PolicyEntry) -> Self {
        Self {
            id: e.id.clone(),
            domain: domain_str(e),
            title: e.title.clone(),
            group: e.group.clone(),
            description: e.description.clone(),
            severity: sev_str(e),
            blocking: e.blocking,
            runs_on: e.runs_on.clone(),
            protected: e.protected,
            origin: e.origin.clone(),
            docs: e.docs.clone(),
            source_kind: format!("{:?}", e.source.kind).to_lowercase(),
            source_ref: e.source.reference.clone(),
            source_detail: e.source.detail.clone(),
        }
    }
}

/// Walk up from `start` to the dir holding `contracts/policy/policy-registry.v1.yaml`.
/// Falls back to `start` so the loader produces a clean error if not found.
fn find_repo_root(start: &Path) -> PathBuf {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join(vox_config::REGISTRY_REL_PATH).is_file() {
            return dir.to_path_buf();
        }
        cur = dir.parent();
    }
    start.to_path_buf()
}

fn load_registry() -> Result<PolicyRegistry, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = find_repo_root(&cwd);
    vox_config::load_policy_registry(&root).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_repo_root_walks_up_to_contract() {
        let dir = tempfile::tempdir().unwrap();
        let contracts = dir.path().join("contracts/policy");
        std::fs::create_dir_all(&contracts).unwrap();
        std::fs::write(contracts.join("policy-registry.v1.yaml"), "schema_version: 1\npolicies: []\n").unwrap();
        let nested = dir.path().join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_repo_root(&nested), dir.path());
    }

    #[test]
    fn detail_dto_carries_rule_contents() {
        use vox_config::{PolicyDomain, PolicyEntry, PolicySource, PolicySourceKind};
        let e = PolicyEntry {
            id: "code-audit/stub/todo".into(),
            domain: PolicyDomain::CodeAuditRule,
            title: "TODO stub".into(),
            group: "Language rules / Stubs (TOESTUB)".into(),
            description: "Flags stub placeholders.".into(),
            severity: Some(vox_config::PolicySeverity::Error),
            blocking: true,
            runs_on: vec!["ci".into()],
            source: PolicySource {
                kind: PolicySourceKind::Pattern,
                reference: "contracts/code-audit/rules.v1.yaml#stub/todo".into(),
                detail: Some("todo!()|unimplemented!()".into()),
            },
            docs: None,
            default_enabled: true,
            protected: true,
            origin: "builtin".into(),
        };
        let dto = PolicyDetailDto::from(&e);
        assert_eq!(dto.domain, "code-audit-rule");
        assert_eq!(dto.severity.as_deref(), Some("error"));
        assert_eq!(dto.source_detail.as_deref(), Some("todo!()|unimplemented!()"));
        assert!(dto.protected);
    }
}
```

> `tempfile` is already a dev-dependency in the workspace; if `cargo test -p vox-gui` reports it missing, add `tempfile = { workspace = true }` under `[dev-dependencies]` in `crates/vox-gui/Cargo.toml`.

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-gui commands::policy::tests
```
Expected: FAIL — module `policy` not declared (and/or unresolved until Step 3 declares it).

- [ ] **Step 3: Add the two read handlers + declare the module**

Append to `crates/vox-gui/src/commands/policy.rs` (above the `tests` module):

```rust
/// `policy_list` — full catalog as lightweight rows, sorted by id. Optional
/// case-insensitive substring filters on domain/group (mirrors `vox policy list`).
#[command]
pub fn policy_list(domain: Option<String>, group: Option<String>) -> Result<Vec<PolicyRowDto>, String> {
    let reg = load_registry()?;
    let dom = domain.map(|d| d.to_lowercase());
    let grp = group.map(|g| g.to_lowercase());
    let mut rows: Vec<PolicyRowDto> = reg
        .policies
        .iter()
        .filter(|e| {
            dom.as_deref().map(|d| domain_str(e).contains(d)).unwrap_or(true)
                && grp.as_deref().map(|g| e.group.to_lowercase().contains(g)).unwrap_or(true)
        })
        .map(PolicyRowDto::from)
        .collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

/// `policy_show` — full detail (incl. rule contents) for one id, or an error.
#[command]
pub fn policy_show(id: String) -> Result<PolicyDetailDto, String> {
    let reg = load_registry()?;
    reg.policies
        .iter()
        .find(|e| e.id == id)
        .map(PolicyDetailDto::from)
        .ok_or_else(|| format!("no policy with id `{id}`"))
}
```

Add to `crates/vox-gui/src/commands/mod.rs` (keep alphabetical with neighbors `orchestrator`/`preferences`):

```rust
pub mod policy;
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p vox-gui commands::policy::tests
```
Expected: PASS (both tests).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/policy.rs crates/vox-gui/src/commands/mod.rs
git commit -m "feat(vox-gui): policy_list/policy_show IPC over vox-config (in-process)"
```

---

## Task 3: `list_branches` handler (net-new git access)

**Files:**
- Modify: `crates/vox-gui/src/commands/policy.rs`

> **Why shell git, not git2:** `vox-gui` has no git crate dep and the spec (§10.5) prescribes shelling `git worktree list` / `git branch`. `std::process::Command` is synchronous and dependency-free. We list **worktrees** (the multi-branch selector targets worktrees) and fall back to the local branch list.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `policy.rs`:

```rust
#[test]
fn parses_worktree_porcelain() {
    let sample = "\
worktree /home/u/vox
HEAD abc123
branch refs/heads/main

worktree /home/u/vox/.claude/worktrees/foo
HEAD def456
branch refs/heads/cc/foo-bar
";
    let branches = parse_worktree_porcelain(sample);
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].branch, "main");
    assert!(branches[0].is_current); // first entry = main worktree
    assert_eq!(branches[1].branch, "cc/foo-bar");
    assert_eq!(branches[1].path, "/home/u/vox/.claude/worktrees/foo");
}

#[test]
fn detached_head_worktree_is_skipped_branch_name() {
    let sample = "worktree /tmp/x\nHEAD abc\ndetached\n";
    let branches = parse_worktree_porcelain(sample);
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch, "(detached abc)");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-gui commands::policy::tests::parses_worktree_porcelain
```
Expected: FAIL — `parse_worktree_porcelain` not found.

- [ ] **Step 3: Implement the parser + handler**

Append to `policy.rs` (above tests):

```rust
/// One selectable branch/worktree in the multi-branch selector.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDto {
    /// Sanitized branch name (matches the `.vox/policy-status/<branch>.json` key).
    pub branch: String,
    /// Absolute worktree path.
    pub path: String,
    /// True for the first (primary) worktree — the default-selected branch.
    pub is_current: bool,
}

/// Parse `git worktree list --porcelain`. The first record is the primary worktree.
pub(crate) fn parse_worktree_porcelain(out: &str) -> Vec<BranchDto> {
    let mut branches = Vec::new();
    let mut path = String::new();
    let mut head = String::new();
    let mut branch: Option<String> = None;
    let mut detached = false;

    let flush = |branches: &mut Vec<BranchDto>, path: &str, head: &str, branch: &Option<String>, detached: bool| {
        if path.is_empty() {
            return;
        }
        let name = if detached {
            format!("(detached {})", head.get(..7).unwrap_or(head))
        } else {
            branch
                .as_deref()
                .map(|b| b.trim_start_matches("refs/heads/").to_string())
                .unwrap_or_else(|| "(unknown)".to_string())
        };
        let is_current = branches.is_empty();
        branches.push(BranchDto { branch: name, path: path.to_string(), is_current });
    };

    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            // New record — flush the previous one.
            flush(&mut branches, &path, &head, &branch, detached);
            path = p.to_string();
            head.clear();
            branch = None;
            detached = false;
        } else if let Some(h) = line.strip_prefix("HEAD ") {
            head = h.to_string();
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.to_string());
        } else if line.trim() == "detached" {
            detached = true;
        }
    }
    flush(&mut branches, &path, &head, &branch, detached);
    branches
}

/// `list_branches` — worktrees first; falls back to local branches if `git
/// worktree` is unavailable. Always returns at least one row when in a repo.
#[command]
pub fn list_branches() -> Result<Vec<BranchDto>, String> {
    use std::process::Command;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = find_repo_root(&cwd);

    let wt = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&root)
        .output();
    if let Ok(o) = wt {
        if o.status.success() {
            let parsed = parse_worktree_porcelain(&String::from_utf8_lossy(&o.stdout));
            if !parsed.is_empty() {
                return Ok(parsed);
            }
        }
    }

    // Fallback: plain branch list (no worktree paths).
    let br = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git branch failed: {e}"))?;
    if !br.status.success() {
        return Err(format!("git branch: {}", String::from_utf8_lossy(&br.stderr)));
    }
    let cur = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&root)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    Ok(String::from_utf8_lossy(&br.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|name| BranchDto {
            branch: name.trim().to_string(),
            path: root.display().to_string(),
            is_current: name.trim() == cur,
        })
        .collect())
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p vox-gui commands::policy::tests
```
Expected: PASS (4 tests now).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/policy.rs
git commit -m "feat(vox-gui): list_branches IPC (git worktree, net-new)"
```

---

## Task 4: `policy_status` handler — Plan-1c-tolerant (degrades to grey)

**Files:**
- Modify: `crates/vox-gui/src/commands/policy.rs`

> **Plan 1c is not landed.** This handler is the **integration seam**: it returns one status per `(branch, policy-id)`, defaulting every rule to `not_run` (grey). When 1c lands `vox_config::load_policy_status(repo_root, &branches)` (the `.vox/policy-status/<branch>.json` reader), replace the `read_branch_status` stub body — the DTO and the frontend never change.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn status_defaults_to_not_run_when_store_missing() {
    // No .vox/policy-status — every requested id must come back grey/not_run.
    let dir = tempfile::tempdir().unwrap();
    let ids = vec!["code-audit/stub/todo".to_string(), "ci/policy-registry-parity".to_string()];
    let rows = build_status_for_branch(dir.path(), "main", &ids);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.status == "not_run"));
    assert_eq!(rows[0].branch, "main");
    assert_eq!(rows[0].id, "code-audit/stub/todo");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p vox-gui commands::policy::tests::status_defaults_to_not_run_when_store_missing
```
Expected: FAIL — `build_status_for_branch` not found.

- [ ] **Step 3: Implement the status handler (grey-degrading)**

Append to `policy.rs` (above tests):

```rust
/// Per-rule status for one branch. `status` ∈ pass|fail|warn|not_run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyStatusDto {
    pub branch: String,
    pub id: String,
    pub status: String,
    /// Number of findings/hits (0 when not_run or pass).
    pub hits: u32,
}

/// Read the Plan-1c status store for one branch. STUB until Plan 1c lands its
/// reader: returns `None` (→ every id degrades to `not_run`). When 1c lands,
/// replace the body with `vox_config::load_policy_status(repo_root, branch)`.
fn read_branch_status(_repo_root: &Path, _branch: &str) -> Option<std::collections::HashMap<String, (String, u32)>> {
    // Plan 1c writes `.vox/policy-status/<sanitized-branch>.json`. Until then the
    // store does not exist; returning None is the truthful "not run" default.
    None
}

/// Join the requested policy ids to whatever status the store has for `branch`,
/// defaulting unknowns to grey `not_run`.
pub(crate) fn build_status_for_branch(repo_root: &Path, branch: &str, ids: &[String]) -> Vec<PolicyStatusDto> {
    let store = read_branch_status(repo_root, branch);
    ids.iter()
        .map(|id| {
            let (status, hits) = store
                .as_ref()
                .and_then(|m| m.get(id).cloned())
                .unwrap_or_else(|| ("not_run".to_string(), 0));
            PolicyStatusDto { branch: branch.to_string(), id: id.clone(), status, hits }
        })
        .collect()
}

/// `policy_status` — per-rule status across the selected branch set. Every
/// catalog id is reported for every branch (grey `not_run` when no result).
#[command]
pub fn policy_status(branches: Vec<String>) -> Result<Vec<PolicyStatusDto>, String> {
    let reg = load_registry()?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = find_repo_root(&cwd);
    let ids: Vec<String> = reg.policies.iter().map(|e| e.id.clone()).collect();
    let mut out = Vec::new();
    let branches = if branches.is_empty() { vec!["HEAD".to_string()] } else { branches };
    for b in branches {
        out.extend(build_status_for_branch(&root, &b, &ids));
    }
    Ok(out)
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p vox-gui commands::policy::tests
```
Expected: PASS (5 tests).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/policy.rs
git commit -m "feat(vox-gui): policy_status IPC (Plan-1c-tolerant; grey not_run default)"
```

---

## Task 5: Register the 4 handlers + build the backend

**Files:**
- Modify: `crates/vox-gui/src/main.rs`

- [ ] **Step 1: Add to `tauri::generate_handler!`**

In `crates/vox-gui/src/main.rs`, inside the `.invoke_handler(tauri::generate_handler![ … ])` list (it ends at `commands::search::open_locator,` on line 128), add before the closing `])`:

```rust
            commands::policy::policy_list,
            commands::policy::policy_show,
            commands::policy::policy_status,
            commands::policy::list_branches,
```

- [ ] **Step 2: Build the backend crate**

```bash
cargo build -p vox-gui
```
Expected: clean build; the 4 commands compile into the generated context. No warnings about unregistered commands.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): register policy IPC handlers"
```

---

## Task 6: Frontend types + pure group-tree builder (vitest)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Policies/types.ts`
- Create: `crates/vox-gui/ui/src/components/surfaces/Policies/policyTree.ts`
- Create: `crates/vox-gui/ui/src/components/surfaces/Policies/policyTree.test.ts`

- [ ] **Step 1: Write the TS types (mirror the Rust DTOs, camelCase)**

Create `types.ts`:

```ts
// DTOs mirror crates/vox-gui/src/commands/policy.rs (#[serde(rename_all = "camelCase")]).
export interface PolicyRow {
  id: string;
  domain: string;
  title: string;
  group: string;
  severity: string | null;
  blocking: boolean;
  protected: boolean;
}

export interface PolicyDetail extends PolicyRow {
  description: string;
  runsOn: string[];
  origin: string;
  docs: string | null;
  sourceKind: string;
  sourceRef: string;
  sourceDetail: string | null;
}

export interface BranchInfo {
  branch: string;
  path: string;
  isCurrent: boolean;
}

export type RunStatus = 'pass' | 'fail' | 'warn' | 'not_run';

export interface PolicyStatus {
  branch: string;
  id: string;
  status: RunStatus;
  hits: number;
}

/** Worst-first status precedence for roll-ups + the master badge. */
export const STATUS_RANK: Record<RunStatus, number> = { fail: 3, warn: 2, pass: 1, not_run: 0 };
```

- [ ] **Step 2: Write the failing test**

Create `policyTree.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { buildGroupTree, worstStatus, statusForRow, needsAttention } from './policyTree';
import type { PolicyRow, PolicyStatus } from './types';

const rows: PolicyRow[] = [
  { id: 'code-audit/stub/todo', domain: 'code-audit-rule', title: 'TODO stub', group: 'Language rules / Stubs', severity: 'error', blocking: true, protected: true },
  { id: 'code-audit/stub/unimpl', domain: 'code-audit-rule', title: 'unimplemented', group: 'Language rules / Stubs', severity: 'error', blocking: true, protected: true },
  { id: 'ci/parity', domain: 'ci-gate', title: 'parity', group: 'CI Gates', severity: null, blocking: true, protected: false },
];

describe('buildGroupTree', () => {
  it('groups rows by group label and counts status colors per group', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'code-audit/stub/todo', status: 'fail', hits: 2 },
      { branch: 'main', id: 'code-audit/stub/unimpl', status: 'pass', hits: 0 },
      { branch: 'main', id: 'ci/parity', status: 'not_run', hits: 0 },
    ];
    const tree = buildGroupTree(rows, status, ['main']);
    const stubs = tree.find(g => g.group === 'Language rules / Stubs')!;
    expect(stubs.rows.length).toBe(2);
    expect(stubs.counts.fail).toBe(1);
    expect(stubs.counts.pass).toBe(1);
    expect(stubs.worst).toBe('fail'); // group badge color
    const ci = tree.find(g => g.group === 'CI Gates')!;
    expect(ci.counts.not_run).toBe(1);
    expect(ci.worst).toBe('not_run');
  });

  it('grey not_run is the default when status is missing for a branch', () => {
    const tree = buildGroupTree(rows, [], ['main']);
    expect(tree.every(g => g.worst === 'not_run')).toBe(true);
  });

  it('multi-branch: a row is worst-of across selected branches', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'ci/parity', status: 'pass', hits: 0 },
      { branch: 'feat', id: 'ci/parity', status: 'fail', hits: 1 },
    ];
    expect(statusForRow('ci/parity', status, ['main', 'feat'])).toBe('fail');
    expect(statusForRow('ci/parity', status, ['main'])).toBe('pass');
  });
});

describe('needsAttention', () => {
  it('is empty (all-clear) when nothing fails/warns', () => {
    const status: PolicyStatus[] = rows.map(r => ({ branch: 'main', id: r.id, status: 'pass' as const, hits: 0 }));
    expect(needsAttention(rows, status, ['main'])).toEqual([]);
  });
  it('collects only failing/warning rows', () => {
    const status: PolicyStatus[] = [
      { branch: 'main', id: 'code-audit/stub/todo', status: 'fail', hits: 1 },
      { branch: 'main', id: 'ci/parity', status: 'warn', hits: 1 },
    ];
    const na = needsAttention(rows, status, ['main']);
    expect(na.map(r => r.id).sort()).toEqual(['ci/parity', 'code-audit/stub/todo']);
  });
});

describe('worstStatus', () => {
  it('ranks fail > warn > pass > not_run', () => {
    expect(worstStatus(['pass', 'not_run', 'warn'])).toBe('warn');
    expect(worstStatus(['not_run'])).toBe('not_run');
    expect(worstStatus([])).toBe('not_run');
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cd crates/vox-gui/ui && npm test -- policyTree
```
(Or `npx vitest run src/components/surfaces/Policies/policyTree.test.ts`.)
Expected: FAIL — `policyTree` module not found.

- [ ] **Step 4: Implement the pure builder**

Create `policyTree.ts`:

```ts
import type { PolicyRow, PolicyStatus, RunStatus } from './types';
import { STATUS_RANK } from './types';

export interface GroupNode {
  group: string;
  rows: PolicyRow[];
  counts: Record<RunStatus, number>;
  worst: RunStatus;
}

/** Worst (highest-rank) of a list; empty → not_run (grey). */
export function worstStatus(statuses: RunStatus[]): RunStatus {
  return statuses.reduce<RunStatus>(
    (acc, s) => (STATUS_RANK[s] > STATUS_RANK[acc] ? s : acc),
    'not_run',
  );
}

/** A rule's effective status = worst across the selected branch set. */
export function statusForRow(id: string, status: PolicyStatus[], branches: string[]): RunStatus {
  const sel = new Set(branches);
  const hits = status.filter(s => s.id === id && sel.has(s.branch)).map(s => s.status);
  return worstStatus(hits);
}

function emptyCounts(): Record<RunStatus, number> {
  return { fail: 0, warn: 0, pass: 0, not_run: 0 };
}

/** Group rows by their `group` label; roll up per-group status counts + worst. */
export function buildGroupTree(rows: PolicyRow[], status: PolicyStatus[], branches: string[]): GroupNode[] {
  const byGroup = new Map<string, PolicyRow[]>();
  for (const r of rows) {
    const arr = byGroup.get(r.group) ?? [];
    arr.push(r);
    byGroup.set(r.group, arr);
  }
  const nodes: GroupNode[] = [];
  for (const [group, grpRows] of byGroup) {
    const counts = emptyCounts();
    const perRow: RunStatus[] = [];
    for (const r of grpRows) {
      const s = statusForRow(r.id, status, branches);
      counts[s] += 1;
      perRow.push(s);
    }
    nodes.push({ group, rows: grpRows, counts, worst: worstStatus(perRow) });
  }
  // Stable display order: worst groups first, then alphabetical.
  nodes.sort((a, b) => STATUS_RANK[b.worst] - STATUS_RANK[a.worst] || a.group.localeCompare(b.group));
  return nodes;
}

/** Rows that are failing or warning on any selected branch (the shrinking group). */
export function needsAttention(rows: PolicyRow[], status: PolicyStatus[], branches: string[]): PolicyRow[] {
  return rows
    .filter(r => {
      const s = statusForRow(r.id, status, branches);
      return s === 'fail' || s === 'warn';
    })
    .sort((a, b) => a.id.localeCompare(b.id));
}

/** Master-sidebar badge: worst status across the whole catalog for the selection. */
export function overallWorst(rows: PolicyRow[], status: PolicyStatus[], branches: string[]): RunStatus {
  return worstStatus(rows.map(r => statusForRow(r.id, status, branches)));
}
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd crates/vox-gui/ui && npm test -- policyTree
```
Expected: PASS (all describe blocks green).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Policies/types.ts crates/vox-gui/ui/src/components/surfaces/Policies/policyTree.ts crates/vox-gui/ui/src/components/surfaces/Policies/policyTree.test.ts
git commit -m "feat(gui): policy group-tree builder + status roll-up (vitest)"
```

---

## Task 7: `PoliciesView` surface — detail-dominant 3-pane layout

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx`

> **Layout (spec §4.7):** three columns — **(A)** the group rail (secondary, collapsible, reusing the same rail width pattern as `Sidebar`), and **(B)** the **rule detail + contents pane as the PRIMARY/largest area** (`flex-1`). "Needs attention" lives at the top of the rail and **renders an all-clear strip when empty** (it must not dominate). Branch chips support **multi-select**. Edit/Disable buttons render in the detail header **disabled** with a Phase-2 tooltip.

- [ ] **Step 1: Write the view**

Create `PoliciesView.tsx`:

```tsx
import React, { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { useLocalStorage } from '../../../hooks/useLocalStorage';
import { buildGroupTree, needsAttention, overallWorst, statusForRow } from './policyTree';
import type { PolicyRow, PolicyDetail, PolicyStatus, BranchInfo, RunStatus } from './types';

const STATUS_DOT: Record<RunStatus, string> = {
  fail: 'bg-red-500',
  warn: 'bg-amber-400',
  pass: 'bg-emerald-400',
  not_run: 'bg-zinc-600',
};
const STATUS_GLYPH: Record<RunStatus, string> = { fail: '●', warn: '▲', pass: '✓', not_run: '—' };

function StatusCount({ counts }: { counts: Record<RunStatus, number> }) {
  return (
    <span className="flex items-center gap-1.5 font-mono text-[10px]">
      {(['fail', 'warn', 'pass', 'not_run'] as RunStatus[])
        .filter(s => counts[s] > 0)
        .map(s => (
          <span key={s} className={`flex items-center gap-0.5 ${s === 'fail' ? 'text-red-400' : s === 'warn' ? 'text-amber-300' : s === 'pass' ? 'text-emerald-300' : 'text-zinc-500'}`}>
            {STATUS_GLYPH[s]}{counts[s]}
          </span>
        ))}
    </span>
  );
}

export function PoliciesView({ pushToast }: { pushToast: (t: any) => void }) {
  const [rows, setRows] = useState<PolicyRow[]>([]);
  const [status, setStatus] = useState<PolicyStatus[]>([]);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [selectedBranches, setSelectedBranches] = useState<string[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<PolicyDetail | null>(null);
  const [railCollapsed, setRailCollapsed] = useLocalStorage<boolean>('vox_policy_rail_collapsed', false);
  const [collapsedGroups, setCollapsedGroups] = useLocalStorage<Record<string, boolean>>('vox_policy_groups', {});

  // Load catalog + branches once.
  useEffect(() => {
    invoke<PolicyRow[]>('policy_list', { domain: null, group: null })
      .then(r => { setRows(r); if (r.length && !selectedId) setSelectedId(r[0].id); })
      .catch(err => pushToast({ tone: 'warn', title: 'Policy catalog failed', body: String(err) }));
    invoke<BranchInfo[]>('list_branches')
      .then(b => { setBranches(b); setSelectedBranches(b.filter(x => x.isCurrent).map(x => x.branch)); })
      .catch(() => setBranches([]));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Reload status when the branch selection changes.
  useEffect(() => {
    if (selectedBranches.length === 0) { setStatus([]); return; }
    invoke<PolicyStatus[]>('policy_status', { branches: selectedBranches })
      .then(setStatus)
      .catch(() => setStatus([]));
  }, [selectedBranches]);

  // Load detail when the selected rule changes.
  useEffect(() => {
    if (!selectedId) { setDetail(null); return; }
    invoke<PolicyDetail>('policy_show', { id: selectedId })
      .then(setDetail)
      .catch(err => pushToast({ tone: 'warn', title: 'Detail failed', body: String(err) }));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const tree = useMemo(() => buildGroupTree(rows, status, selectedBranches), [rows, status, selectedBranches]);
  const attention = useMemo(() => needsAttention(rows, status, selectedBranches), [rows, status, selectedBranches]);
  const worst = useMemo(() => overallWorst(rows, status, selectedBranches), [rows, status, selectedBranches]);

  const toggleBranch = (b: string) =>
    setSelectedBranches(prev => prev.includes(b) ? prev.filter(x => x !== b) : [...prev, b]);
  const toggleGroup = (g: string) =>
    setCollapsedGroups(prev => ({ ...prev, [g]: !prev[g] }));

  return (
    <div className="flex gap-4 h-full min-h-0">
      {/* ── SECONDARY: group rail (collapsible, mirrors Sidebar rail widths) ── */}
      <div className="shrink-0 transition-[width] duration-200" style={{ width: railCollapsed ? 56 : 300 }}>
        <Glass className="flex h-full flex-col p-3 gap-3 overflow-hidden">
          <div className="flex items-center justify-between">
            {!railCollapsed && (
              <span className="font-display text-[10px] uppercase tracking-[0.28em] text-zinc-400">
                Policies <span className={`ml-1 inline-block size-1.5 rounded-full align-middle ${STATUS_DOT[worst]}`} />
              </span>
            )}
            <button onClick={() => setRailCollapsed(c => !c)} title={railCollapsed ? 'Expand' : 'Collapse'}
              className="flex size-6 items-center justify-center rounded-md border border-white/5 text-zinc-400 hover:bg-white/5 hover:text-zinc-100">
              <Icon.chevL className={`size-3 transition-transform ${railCollapsed ? 'rotate-180' : ''}`} />
            </button>
          </div>

          {!railCollapsed && (
            <>
              {/* Multi-branch selector (worktrees) */}
              <div className="flex flex-wrap gap-1">
                {branches.map(b => (
                  <button key={b.branch} onClick={() => toggleBranch(b.branch)}
                    className={`rounded-full px-2 py-0.5 font-mono text-[9px] border ${selectedBranches.includes(b.branch) ? 'border-brass/40 bg-brass/10 text-brass' : 'border-white/5 text-zinc-500 hover:text-zinc-300'}`}>
                    {b.branch}{b.isCurrent ? ' ◆' : ''}
                  </button>
                ))}
                {branches.length === 0 && <span className="font-mono text-[9px] text-zinc-600">no git worktrees</span>}
              </div>

              {/* ⚠ Needs attention — gracefully shrinks to an all-clear strip */}
              <div className="rounded-lg border border-white/5 bg-white/[0.02] p-2">
                {attention.length === 0 ? (
                  <div className="flex items-center gap-1.5 font-mono text-[10px] text-emerald-300/80">
                    <Icon.check className="size-3" /> all clear
                  </div>
                ) : (
                  <div className="flex flex-col gap-0.5">
                    <span className="font-display text-[9px] uppercase tracking-[0.2em] text-red-300/90">⚠ Needs attention ({attention.length})</span>
                    {attention.map(r => (
                      <button key={r.id} onClick={() => setSelectedId(r.id)}
                        className="text-left font-mono text-[10px] text-zinc-300 hover:text-red-200 truncate">{r.id}</button>
                    ))}
                  </div>
                )}
              </div>

              {/* Group tree with status-colored counts */}
              <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-1">
                {tree.map(node => {
                  const open = !collapsedGroups[node.group];
                  return (
                    <div key={node.group} className="flex flex-col">
                      <button onClick={() => toggleGroup(node.group)}
                        className="flex items-center justify-between gap-2 rounded-md px-1.5 py-1 hover:bg-white/[0.03]">
                        <span className="flex items-center gap-1.5 min-w-0">
                          <span className={`size-1.5 rounded-full shrink-0 ${STATUS_DOT[node.worst]}`} />
                          <span className="font-display text-[10px] text-zinc-300 truncate">{node.group}</span>
                        </span>
                        <span className="flex items-center gap-1.5 shrink-0">
                          <StatusCount counts={node.counts} />
                          <Icon.chevronDown className={`size-3 text-zinc-600 transition-transform ${open ? '' : '-rotate-90'}`} />
                        </span>
                      </button>
                      {open && node.rows.map(r => {
                        const s = statusForRow(r.id, status, selectedBranches);
                        return (
                          <button key={r.id} onClick={() => setSelectedId(r.id)}
                            className={`flex items-center gap-1.5 rounded-md pl-5 pr-1.5 py-1 text-left ${selectedId === r.id ? 'bg-white/[0.05] text-zinc-100' : 'text-zinc-500 hover:bg-white/[0.02] hover:text-zinc-300'}`}>
                            <span className={`size-1 rounded-full shrink-0 ${STATUS_DOT[s]}`} />
                            <span className="font-mono text-[10px] truncate">{r.title}</span>
                          </button>
                        );
                      })}
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </Glass>
      </div>

      {/* ── PRIMARY: rule detail + contents (largest pane) ── */}
      <div className="flex-1 min-w-0">
        <Glass className="flex h-full flex-col p-5 gap-4 overflow-y-auto custom-scrollbar">
          {!detail ? (
            <div className="m-auto font-mono text-xs text-zinc-600">select a policy</div>
          ) : (
            <>
              <header className="flex items-start justify-between gap-4 border-b border-white/5 pb-3">
                <div className="min-w-0">
                  <div className="font-mono text-sm text-zinc-100 truncate">{detail.id}</div>
                  <div className="font-display text-[11px] text-zinc-400">{detail.title}</div>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <button disabled title="Editing arrives in Phase 3 (read-only now)"
                    className="flex items-center gap-1 rounded-md border border-white/5 px-2 py-1 font-mono text-[10px] text-zinc-600 opacity-50 cursor-not-allowed">
                    ✎ Edit
                  </button>
                  <button disabled title="Enable/disable arrives in Phase 2 (read-only now)"
                    className="flex items-center gap-1 rounded-md border border-white/5 px-2 py-1 font-mono text-[10px] text-zinc-600 opacity-50 cursor-not-allowed">
                    ⏻ Disable
                  </button>
                </div>
              </header>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-zinc-500">What it does</div>
                <p className="text-xs text-zinc-300 leading-relaxed">{detail.description}</p>
                <div className="flex flex-wrap gap-3 pt-1 font-mono text-[10px] text-zinc-500">
                  <span>domain: <span className="text-zinc-300">{detail.domain}</span></span>
                  <span>severity: <span className="text-zinc-300">{detail.severity ?? '—'}</span></span>
                  <span>{detail.blocking ? 'blocking' : 'non-blocking'}</span>
                  {detail.protected && <span className="text-amber-300/80">protected</span>}
                  <span>runs on: <span className="text-zinc-300">{detail.runsOn.join(', ') || '—'}</span></span>
                  <span>origin: <span className="text-zinc-300">{detail.origin}</span></span>
                </div>
              </section>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-zinc-500">Contents (edit target)</div>
                <div className="rounded-lg border border-white/5 bg-black/30 p-3 font-mono text-[11px] text-zinc-300">
                  <div className="text-zinc-500">kind: <span className="text-zinc-300">{detail.sourceKind}</span></div>
                  <div className="text-zinc-500">source: <span className="text-zinc-300 break-all">{detail.sourceRef}</span></div>
                  {detail.sourceDetail && (
                    <pre className="mt-2 whitespace-pre-wrap break-all text-emerald-200/80">{detail.sourceDetail}</pre>
                  )}
                </div>
                {detail.docs && <a className="font-mono text-[10px] text-brass/80 hover:underline">{detail.docs}</a>}
              </section>

              <section className="flex flex-col gap-1.5">
                <div className="font-display text-[9px] uppercase tracking-[0.28em] text-zinc-500">Last run (per branch)</div>
                <div className="flex flex-wrap gap-2">
                  {selectedBranches.length === 0 && <span className="font-mono text-[10px] text-zinc-600">no branch selected</span>}
                  {selectedBranches.map(b => {
                    const s = statusForRow(detail.id, status, [b]);
                    return (
                      <span key={b} className="flex items-center gap-1.5 rounded-full border border-white/5 px-2 py-0.5 font-mono text-[10px]">
                        <span className={`size-1.5 rounded-full ${STATUS_DOT[s]}`} />
                        <span className="text-zinc-300">{b}</span>
                        <span className="text-zinc-500">{s}</span>
                      </span>
                    );
                  })}
                </div>
              </section>
            </>
          )}
        </Glass>
      </div>
    </div>
  );
}
```

> **Icon availability:** `chevL`, `chevronDown`, `check` are all referenced in `Sidebar.tsx`/`Icons.tsx`. If `Icon.chevL` is undefined at runtime, substitute `Icon.chevronDown` with a rotation (the rail toggle is cosmetic).

- [ ] **Step 2: Type-check the surface**

```bash
cd crates/vox-gui/ui && npm run typecheck
```
Expected: no TS errors (the import paths + DTO field names resolve).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx
git commit -m "feat(gui): PoliciesView detail-dominant surface (read-only)"
```

---

## Task 8: Wire `policies` into App routing + sidebar order + master badge

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx`, `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Add `'policies'` to the `View` union**

In `crates/vox-gui/ui/src/App.tsx`, the `type View` union ends at `| 'search';` (line 58). Add a member (place after `'approvals'`, line 53, to sit near its nav group):

```ts
  | 'policies'
```

- [ ] **Step 2: Import + render the surface**

After the existing surface imports (e.g. after `ApprovalsView` import, line 26), add:

```ts
import { PoliciesView } from './components/surfaces/Policies/PoliciesView';
```

In `renderView()` (the `switch (activeView)` starting at line 528), add a case alongside `approvals`/`skills` (before `default:`):

```tsx
      case 'policies':
        return <PoliciesView pushToast={pushToast} />;
```

- [ ] **Step 3: Allow `policies` as an initial view**

In the `get_initial_view` allow-list array (line 234), add `'policies'` to the `includes(...)` list so deep-link `--command policies` works:

```ts
// …, 'approvals', 'policies', 'skills', 'settings', …
```

- [ ] **Step 4: Add `policies` to the sidebar `operate` order**

In `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`, the `SECTION_ORDER.operate` array (line 49) currently is `['dashboard', 'flow', 'approvals', 'runs', 'matrix']`. Add `'policies'` (place after `'runs'`; `matrix` now renders as "Routing"):

```ts
  operate: ['dashboard', 'flow', 'approvals', 'runs', 'policies', 'matrix'],
```

> The nav label + icon come from the generated registry (Task 1) — the Sidebar renders any registered surface automatically (`navigableSurfaces()`); `SECTION_ORDER` only fixes display order, so this line is optional polish, not required for the item to appear.

- [ ] **Step 5: Master-status badge on the Policies nav item (optional, drift-safe)**

The `NavItem` already accepts a `badge` prop ([`Sidebar.tsx:123`](../../../crates/vox-gui/ui/src/components/layout/Sidebar.tsx) passes `agentsCount` to `flow`). To color the Policies badge by worst status the Sidebar would need the worst-status value threaded from `App`. **Phase-1d minimal:** skip the live colored badge in the sidebar (the rail header in `PoliciesView` already shows the worst-status dot). Record this as a deliberate scope cut in Self-Review rather than threading new global state through `Sidebar`. *(If desired later: add an optional `policyWorst?: RunStatus` prop to `SidebarProps`, computed in `App` from a lightweight `policy_status` poll, and render it in `renderItem` when `e.viewKey === 'policies'`.)*

- [ ] **Step 6: Type-check + build the full UI**

```bash
cd crates/vox-gui/ui && npm run typecheck && npm test
```
Expected: typecheck clean; all vitest suites (incl. `policyTree`) green.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/Sidebar.tsx
git commit -m "feat(gui): route Policies surface + sidebar order"
```

---

## Task 9: Final integration build + gates

**Files:** none (verification only)

- [ ] **Step 1: Backend build + tests**

```bash
cargo build -p vox-gui
cargo test -p vox-gui commands::policy
```
Expected: clean build; all 5 Rust unit tests pass.

- [ ] **Step 2: Frontend tests + typecheck**

```bash
cd crates/vox-gui/ui && npm run typecheck && npm test
```
Expected: typecheck clean; vitest green.

- [ ] **Step 3: Surface-registry drift gate**

```bash
cargo run -p vox-cli -- ci gui-surface-registry
```
Expected: green (YAML ↔ generated TS in sync; Matrix=Routing, Policies present).

- [ ] **Step 4: (Optional, behind the GUI smoke flag) Playwright golden route**

The repo's Playwright e2e (`npm run test:e2e`) is **not in CI by default** (behind the existing GUI smoke flag). If running locally with the app up, assert the `policies` route renders the fixture catalog and the all-clear "Needs attention" strip. Not wired as a blocking gate here.

- [ ] **Step 5: Commit any formatting**

```bash
cargo fmt -p vox-gui
git add -A
git commit -m "chore(gui): policy surface integration verification" --allow-empty
```

---

## Self-Review

**Spec coverage (Phase 1d slice):**
- Rule detail + contents = PRIMARY/largest pane (`flex-1`) → Task 7. ✓
- "Needs attention" gracefully shrinks to an all-clear strip when empty → `needsAttention()` (Task 6) + the all-clear branch in the rail (Task 7). ✓
- Secondary group rail reusing existing collapse machinery (no new collapse engine: rail width + per-group `collapsedGroups` mirror `Sidebar`'s `SidebarMode`/`collapsedSections`) → Tasks 7. ✓
- Per-group status-colored counts (red/yellow/green/grey) + worst-status group dot → `buildGroupTree` counts + `StatusCount`/`STATUS_DOT` (Tasks 6, 7). ✓
- Master-sidebar badge colored by worst status → rail-header worst dot (Task 7); sidebar-item colored badge **deliberately deferred** (Task 8 Step 5) to avoid threading new global state — recorded, not a silent gap. ◑
- Multi-branch selector (worktrees), every section reflects the selection → `list_branches` (Task 3) + `selectedBranches` + `statusForRow` worst-of-branches (Tasks 6, 7). ✓
- Edit ✎ / Disable ⏻ present but gated read-only with phase tooltip → detail header (Task 7). ✓
- Nav collision resolved (Matrix→Routing; Policies claimed) → Task 1. ✓
- IPC strategy: in-process `vox-config` for catalog/status (no shell-out), `git` shell only for branches → Tasks 2-4. ✓
- Plan-1c-tolerant status (grey `not_run` when store missing; one-line swap when 1c lands) → Task 4 `read_branch_status` stub. ✓

**Chosen handler strategy:** **In-process** `vox_config::load_policy_registry` for `policy_list`/`policy_show`/`policy_status` (vox-gui already depends on vox-config — [`Cargo.toml:21`](../../../crates/vox-gui/Cargo.toml) — so no CLI/daemon seam needed); `list_branches` uses `std::process::Command` git (net-new, no git crate).

**Placeholder scan:** No "TBD"/hollow bodies. `read_branch_status` returning `None` is a **real, truthful** behavior (grey "not run"), not a stub-to-fill — it is the documented Plan-1c integration seam with an explicit swap instruction. The two `>` notes (tempfile dev-dep, icon fallback) are verification reminders with concrete fallbacks.

**Type consistency:** Rust DTO field names (`serde camelCase`) match the TS `types.ts` exactly (`sourceKind`/`sourceRef`/`sourceDetail`/`runsOn`/`isCurrent`). `RunStatus` strings (`pass|fail|warn|not_run`) are identical across the Rust `policy_status` output and the TS `STATUS_RANK`/`STATUS_DOT` maps. `statusForRow`/`buildGroupTree`/`needsAttention`/`overallWorst` signatures are consistent between `policyTree.ts`, its test, and `PoliciesView.tsx`.

**Defers (documented):** Edit/Disable toggling → Phase 2; custom rules → Phase 4; real status numbers → Plan 1c; colored sidebar-item badge → follow-on (Task 8 Step 5); Playwright golden route remains behind the existing GUI smoke flag.
