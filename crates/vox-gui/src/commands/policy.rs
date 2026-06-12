//! Tauri IPC for the read-only "Policies" GUI surface (Phase 1d).
//!
//! In-process over `vox_config::load_policy_registry`; no CLI shell-out. Status
//! reads use the Plan-1c per-branch store (`vox_config::load_status_for_branches`)
//! and tolerate a MISSING store — every rule degrades to grey `not_run`.
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

/// `policy_list` — full catalog as lightweight rows, sorted by id. Optional
/// case-insensitive substring filters on domain/group (mirrors `vox policy list`).
#[command]
pub fn policy_list(
    domain: Option<String>,
    group: Option<String>,
) -> Result<Vec<PolicyRowDto>, String> {
    let reg = load_registry()?;
    let dom = domain.map(|d| d.to_lowercase());
    let grp = group.map(|g| g.to_lowercase());
    let mut rows: Vec<PolicyRowDto> = reg
        .policies
        .iter()
        .filter(|e| {
            dom.as_deref()
                .map(|d| domain_str(e).contains(d))
                .unwrap_or(true)
                && grp
                    .as_deref()
                    .map(|g| e.group.to_lowercase().contains(g))
                    .unwrap_or(true)
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

    let flush = |branches: &mut Vec<BranchDto>,
                 path: &str,
                 head: &str,
                 branch: &Option<String>,
                 detached: bool| {
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
        branches.push(BranchDto {
            branch: name,
            path: path.to_string(),
            is_current,
        });
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
    if let Ok(o) = wt
        && o.status.success()
    {
        let parsed = parse_worktree_porcelain(&String::from_utf8_lossy(&o.stdout));
        if !parsed.is_empty() {
            return Ok(parsed);
        }
    }

    // Fallback: plain branch list (no worktree paths).
    let br = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git branch failed: {e}"))?;
    if !br.status.success() {
        return Err(format!(
            "git branch: {}",
            String::from_utf8_lossy(&br.stderr)
        ));
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

/// Map the Plan-1c `RunStatus` enum to the frontend's status string. The honest
/// "not run" grey is `not_run` on the wire (the enum's `Unknown` variant).
fn run_status_str(s: vox_config::RunStatus) -> String {
    match s {
        vox_config::RunStatus::Pass => "pass",
        vox_config::RunStatus::Fail => "fail",
        vox_config::RunStatus::Warn => "warn",
        vox_config::RunStatus::Unknown => "not_run",
    }
    .to_string()
}

/// Read the Plan-1c status store for one branch into an `id -> (status, hits)`
/// map. Returns `None` when no run has happened (file absent) — the truthful
/// "not run" default, which the join below degrades every id to grey `not_run`.
fn read_branch_status(
    repo_root: &Path,
    branch: &str,
) -> Option<std::collections::HashMap<String, (String, u32)>> {
    match vox_config::load_status(repo_root, branch) {
        Ok(Some(report)) => Some(
            report
                .results
                .into_iter()
                .map(|r| (r.id, (run_status_str(r.status), r.hits.len() as u32)))
                .collect(),
        ),
        // No file (None) OR a read/parse error both degrade to grey not_run.
        _ => None,
    }
}

/// Join the requested policy ids to whatever status the store has for `branch`,
/// defaulting unknowns to grey `not_run`.
pub(crate) fn build_status_for_branch(
    repo_root: &Path,
    branch: &str,
    ids: &[String],
) -> Vec<PolicyStatusDto> {
    let store = read_branch_status(repo_root, branch);
    ids.iter()
        .map(|id| {
            let (status, hits) = store
                .as_ref()
                .and_then(|m| m.get(id).cloned())
                .unwrap_or_else(|| ("not_run".to_string(), 0));
            PolicyStatusDto {
                branch: branch.to_string(),
                id: id.clone(),
                status,
                hits,
            }
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
    let branches = if branches.is_empty() {
        vec!["HEAD".to_string()]
    } else {
        branches
    };
    for b in branches {
        out.extend(build_status_for_branch(&root, &b, &ids));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_repo_root_walks_up_to_contract() {
        let dir = tempfile::tempdir().unwrap();
        let contracts = dir.path().join("contracts/policy");
        std::fs::create_dir_all(&contracts).unwrap();
        std::fs::write(
            contracts.join("policy-registry.v1.yaml"),
            "schema_version: 1\npolicies: []\n",
        )
        .unwrap();
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
        assert_eq!(
            dto.source_detail.as_deref(),
            Some("todo!()|unimplemented!()")
        );
        assert!(dto.protected);
    }

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

    #[test]
    fn status_defaults_to_not_run_when_store_missing() {
        // No .vox/policy-status — every requested id must come back grey/not_run.
        let dir = tempfile::tempdir().unwrap();
        let ids = vec![
            "code-audit/stub/todo".to_string(),
            "ci/policy-registry-parity".to_string(),
        ];
        let rows = build_status_for_branch(dir.path(), "main", &ids);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.status == "not_run"));
        assert_eq!(rows[0].branch, "main");
        assert_eq!(rows[0].id, "code-audit/stub/todo");
    }

    #[test]
    fn status_reads_real_plan_1c_store_when_present() {
        use vox_config::STATUS_DIR_REL;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(STATUS_DIR_REL)).unwrap();
        std::fs::write(
            vox_config::status_path(root, "main"),
            r#"{"branch":"main","commit":"a","ran_at":"t","results":[
                {"id":"code-audit/stub/todo","status":"fail","duration_ms":1,
                 "hits":[{"file":"a.rs","line":7,"note":"todo!()"}]}]}"#,
        )
        .unwrap();
        let ids = vec!["code-audit/stub/todo".to_string(), "ci/parity".to_string()];
        let rows = build_status_for_branch(root, "main", &ids);
        assert_eq!(rows[0].status, "fail");
        assert_eq!(rows[0].hits, 1);
        // An id with no recorded result still degrades to grey.
        assert_eq!(rows[1].status, "not_run");
    }
}
