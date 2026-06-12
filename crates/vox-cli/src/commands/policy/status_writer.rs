//! Writer for the per-branch policy run-status store (Phase 1c).
//!
//! MERGES results by id so successive gate runs accumulate into one report.
//! Timestamps are passed in by the caller (determinism: no `now()` here).
//!
//! DEVIATION FROM PLAN: branch/commit resolution shells `git` directly rather
//! than via `vox_git::read_only`. `vox-git` is an *optional* dependency of
//! `vox-cli` (only enabled by the `coderabbit` feature), so it is not available
//! in the default build. The writer already shells `git` for worktree
//! enumeration, so this keeps the capture path dependency-free and consistent.

use std::path::Path;
use vox_config::{PolicyResult, PolicyRunReport};

/// Pure MERGE: replace any prior result with the same id, append the rest,
/// and stamp fresh provenance. `ran_at`/`commit` are passed in (no `now()`).
pub fn merge_results(
    mut report: PolicyRunReport,
    fresh: Vec<PolicyResult>,
    branch: &str,
    commit: &str,
    ran_at: &str,
) -> PolicyRunReport {
    for new in fresh {
        if let Some(slot) = report.results.iter_mut().find(|r| r.id == new.id) {
            *slot = new;
        } else {
            report.results.push(new);
        }
    }
    report.results.sort_by(|a, b| a.id.cmp(&b.id));
    report.branch = branch.to_string();
    report.commit = commit.to_string();
    report.ran_at = ran_at.to_string();
    report
}

/// Read-or-default the report for `branch`, MERGE `fresh`, write it back.
/// Atomic-ish: write to a temp file then rename.
pub fn write_results(
    repo_root: &Path,
    branch: &str,
    commit: &str,
    ran_at: &str,
    fresh: Vec<PolicyResult>,
) -> std::io::Result<()> {
    let prior = vox_config::load_status(repo_root, branch)
        .ok()
        .flatten()
        .unwrap_or_else(|| PolicyRunReport {
            branch: branch.to_string(),
            commit: commit.to_string(),
            ran_at: ran_at.to_string(),
            results: Vec::new(),
        });
    let merged = merge_results(prior, fresh, branch, commit, ran_at);
    let path = vox_config::status_path(repo_root, branch);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(&merged)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Run a read-only `git` command in `repo_root`, returning trimmed stdout on
/// success (exit 0). Best-effort: any failure → `None`.
fn git_read(repo_root: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Current branch via `git rev-parse --abbrev-ref HEAD`.
/// Falls back to `"DETACHED"` when not on a branch / git unavailable.
pub fn current_branch(repo_root: &Path) -> String {
    git_read(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .filter(|s| !s.is_empty() && s != "HEAD")
        .unwrap_or_else(|| "DETACHED".to_string())
}

/// Short HEAD commit for provenance (best-effort).
pub fn head_commit(repo_root: &Path) -> String {
    git_read(repo_root, &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string())
}

/// Enumerate active worktrees' branches via `git worktree list --porcelain`.
pub fn worktree_branches(repo_root: &Path) -> Vec<String> {
    let Some(text) = git_read(repo_root, &["worktree", "list", "--porcelain"]) else {
        return vec![current_branch(repo_root)];
    };
    let mut branches: Vec<String> = text
        .lines()
        .filter_map(|l| l.strip_prefix("branch refs/heads/"))
        .map(|b| b.to_string())
        .collect();
    if branches.is_empty() {
        branches.push(current_branch(repo_root));
    }
    branches.sort();
    branches.dedup();
    branches
}

/// Project a code-audit run into per-rule status results.
///
/// `ran_rule_ids` are the rules that were actually evaluated (so a clean rule
/// records `pass`, not `unknown`). `findings` are the issues that fired.
/// A rule with any error/critical finding → `fail`; any warn/info finding but
/// no error → `warn`; a rule that ran with no finding → `pass`.
#[cfg(feature = "completion-toestub")]
pub fn code_audit_results(
    ran_rule_ids: &[String],
    findings: &[vox_code_audit::rules::Finding],
) -> Vec<PolicyResult> {
    use std::collections::BTreeMap;
    use vox_code_audit::rules::Severity;
    use vox_config::{Hit, RunStatus};

    // Bucket findings by raw rule_id.
    let mut by_rule: BTreeMap<&str, Vec<&vox_code_audit::rules::Finding>> = BTreeMap::new();
    for f in findings {
        by_rule.entry(f.rule_id.as_str()).or_default().push(f);
    }

    ran_rule_ids
        .iter()
        .map(|raw| {
            let id = format!("code-audit/{raw}");
            let hits_for = by_rule.get(raw.as_str());
            let status = match hits_for {
                None => RunStatus::Pass,
                Some(fs) => {
                    if fs
                        .iter()
                        .any(|f| matches!(f.severity, Severity::Error | Severity::Critical))
                    {
                        RunStatus::Fail
                    } else {
                        RunStatus::Warn
                    }
                }
            };
            let hits = hits_for
                .map(|fs| {
                    fs.iter()
                        .map(|f| Hit {
                            file: f.file.display().to_string(),
                            line: f.line as u32,
                            note: f.message.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            PolicyResult {
                id,
                status,
                hits,
                duration_ms: 0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_config::{RunStatus, load_status, status_path};

    fn res(id: &str, status: RunStatus) -> PolicyResult {
        PolicyResult {
            id: id.into(),
            status,
            hits: vec![],
            duration_ms: 1,
        }
    }

    #[test]
    fn merge_replaces_by_id_and_keeps_others() {
        let prior = PolicyRunReport {
            branch: "main".into(),
            commit: "a".into(),
            ran_at: "t0".into(),
            results: vec![
                res("ci-gate/ci.manifest", RunStatus::Pass),
                res("arch-rule/fan_in", RunStatus::Warn),
            ],
        };
        let merged = merge_results(
            prior,
            vec![
                res("ci-gate/ci.manifest", RunStatus::Fail),
                res("code-audit/stub/todo", RunStatus::Pass),
            ],
            "main",
            "b",
            "t1",
        );
        // ci.manifest replaced; fan_in untouched; stub/todo added.
        assert_eq!(merged.commit, "b");
        assert_eq!(merged.ran_at, "t1");
        let by = |id: &str| merged.results.iter().find(|r| r.id == id).map(|r| r.status);
        assert_eq!(by("ci-gate/ci.manifest"), Some(RunStatus::Fail));
        assert_eq!(by("arch-rule/fan_in"), Some(RunStatus::Warn));
        assert_eq!(by("code-audit/stub/todo"), Some(RunStatus::Pass));
        assert_eq!(merged.results.len(), 3);
    }

    #[test]
    fn write_then_read_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        write_results(
            dir.path(),
            "feature/x",
            "deadbee",
            "2026-06-06T00:00:00Z",
            vec![res("ci-gate/ci.manifest", RunStatus::Pass)],
        )
        .unwrap();
        // Sanitized filename was used.
        assert!(status_path(dir.path(), "feature/x").exists());
        let r = load_status(dir.path(), "feature/x").unwrap().unwrap();
        assert_eq!(r.branch, "feature/x");
        assert_eq!(r.results[0].id, "ci-gate/ci.manifest");
    }

    #[cfg(feature = "completion-toestub")]
    #[test]
    fn findings_project_to_per_rule_results() {
        use std::path::PathBuf;
        use vox_code_audit::rules::{Finding, Severity};
        let ran_rule_ids = vec![
            "stub/todo".to_string(),
            "stub/unimplemented".to_string(),
            "victory-claim".to_string(),
        ];
        let findings = vec![Finding {
            rule_id: "stub/todo".into(),
            diagnostic_id: None,
            rule_name: "TODO stub".into(),
            severity: Severity::Error,
            file: PathBuf::from("src/a.rs"),
            line: 7,
            column: 0,
            message: "todo!()".into(),
            suggestion: None,
            alternatives: vec![],
            rationale: None,
            context: String::new(),
            confidence: None,
            evidence: None,
        }];
        let results = code_audit_results(&ran_rule_ids, &findings);
        let by = |id: &str| results.iter().find(|r| r.id == id).map(|r| r.status);
        assert_eq!(by("code-audit/stub/todo"), Some(RunStatus::Fail)); // had a finding
        assert_eq!(by("code-audit/stub/unimplemented"), Some(RunStatus::Pass)); // ran, clean
        assert_eq!(by("code-audit/victory-claim"), Some(RunStatus::Pass));
        let hit = &results
            .iter()
            .find(|r| r.id == "code-audit/stub/todo")
            .unwrap()
            .hits[0];
        assert_eq!(hit.line, 7);
        assert_eq!(hit.file, "src/a.rs");
    }
}
