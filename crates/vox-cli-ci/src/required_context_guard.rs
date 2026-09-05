//! `vox ci required-context-guard` — only ONE job may claim the required
//! branch-protection context on a normal pull request.
//!
//! Branch protection has a single required context, `Check, Build, and Test
//! (Rust)`. GitHub identifies it by job `name:` alone, so ANY workflow can
//! claim it — and a job that is *skipped* still posts a check-run under its
//! name, which GitHub counts as SATISFYING the requirement.
//!
//! That is not hypothetical. `ci-fallback-hosted.yml` names its `gate` job
//! with the required context on purpose, so a green hosted run can satisfy
//! the gate during a fleet outage. Its trigger list also included
//! `synchronize`, so it fired on every push to every PR, skipped (no
//! `fleet-down` label), and posted `conclusion=skipped` under the required
//! name. Measured on PR #502: started and completed at 18:34:38Z, the same
//! second, while ci.yml's `setup` was still queued and nothing had compiled.
//! The PR was `mergeable=MERGEABLE`.
//!
//! The invariant this enforces: a workflow may name a job with the required
//! context ONLY IF it cannot be triggered by an ordinary pull-request event.
//! `types: [labeled]` qualifies (a human must act); `synchronize`, `opened`,
//! `reopened` and an unfiltered `pull_request:` do not.
//!
//! `ci.yml` is the one legitimate owner and is exempt.
//!
//! Always fails on a violation — a gate that cannot fail is the bug.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

/// The sole required branch-protection context on `main`.
const REQUIRED_CONTEXT: &str = "Check, Build, and Test (Rust)";
/// The workflow that legitimately owns it.
const OWNER_WORKFLOW: &str = "ci.yml";
/// PR activity types that fire without a deliberate human act on the PR.
const ORDINARY_PR_TYPES: &[&str] = &["opened", "synchronize", "reopened", "ready_for_review"];

struct Violation {
    file: String,
    job: String,
    why: String,
}

/// The `pull_request:` activity types a workflow reacts to.
///
/// `None` means the workflow has no `pull_request:` trigger at all (safe).
/// `Some(vec![])` means `pull_request:` with no `types:` filter — which GitHub
/// expands to the default set INCLUDING `synchronize`, so it is the dangerous
/// case, not an empty one.
fn pull_request_types(doc: &serde_yaml::Value) -> Option<Vec<String>> {
    // `on:` parses as the YAML boolean `true` (YAML 1.1), so check both.
    let map = doc.as_mapping()?;
    let on = map
        .get(serde_yaml::Value::String("on".into()))
        .or_else(|| map.get(serde_yaml::Value::Bool(true)))?;
    let pr = on
        .as_mapping()?
        .get(serde_yaml::Value::String("pull_request".into()))?;
    let types = pr
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String("types".into())))
        .and_then(|t| t.as_sequence())
        .map(|s| {
            s.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(types)
}

/// True when this workflow can be triggered by an ordinary PR event.
fn fires_on_ordinary_pr(doc: &serde_yaml::Value) -> bool {
    match pull_request_types(doc) {
        None => false,
        // No `types:` filter => GitHub's default set, which includes synchronize.
        Some(t) if t.is_empty() => true,
        Some(t) => t.iter().any(|ty| ORDINARY_PR_TYPES.contains(&ty.as_str())),
    }
}

/// Every job in `doc` whose `name:` is exactly the required context.
fn jobs_claiming_context(doc: &serde_yaml::Value) -> Vec<String> {
    let Some(jobs) = doc
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String("jobs".into())))
        .and_then(|j| j.as_mapping())
    else {
        return Vec::new();
    };
    jobs.iter()
        .filter(|(_, job)| {
            job.as_mapping()
                .and_then(|j| j.get(serde_yaml::Value::String("name".into())))
                .and_then(|n| n.as_str())
                .is_some_and(|n| n.trim() == REQUIRED_CONTEXT)
        })
        .filter_map(|(k, _)| k.as_str().map(str::to_string))
        .collect()
}

fn check_doc(doc: &serde_yaml::Value, file: &str, out: &mut Vec<Violation>) {
    if file == OWNER_WORKFLOW {
        return;
    }
    let claimants = jobs_claiming_context(doc);
    if claimants.is_empty() || !fires_on_ordinary_pr(doc) {
        return;
    }
    for job in claimants {
        out.push(Violation {
            file: file.to_string(),
            job,
            why: format!(
                "names a job {REQUIRED_CONTEXT:?} in a workflow that fires on ordinary \
                 pull-request events. When its `if` is false the job is SKIPPED, and a \
                 skipped check-run satisfies the required context with nothing verified."
            ),
        });
    }
}

pub fn run(repo_root: &Path) -> Result<()> {
    let wf_dir = repo_root.join(".github").join("workflows");
    let mut entries: Vec<_> = std::fs::read_dir(&wf_dir)
        .with_context(|| format!("read {}", wf_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
        .collect();
    entries.sort();

    let mut violations = Vec::new();
    for path in entries {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        check_doc(&doc, &name, &mut violations);
    }

    if violations.is_empty() {
        println!(
            "required-context-guard OK (only {OWNER_WORKFLOW} claims {REQUIRED_CONTEXT:?} on ordinary PRs)"
        );
        return Ok(());
    }

    let lines: Vec<String> = violations
        .iter()
        .map(|v| format!("{}: job `{}` {}", v.file, v.job, v.why))
        .collect();
    Err(anyhow!(
        "required-context-guard: {} job(s) can satisfy the required gate without running:\n  {}\n\
         Fix: restrict the workflow's `pull_request:` trigger to deliberate types only \
         (e.g. `types: [labeled]`), or rename the job so it no longer claims the \
         branch-protection context.",
        violations.len(),
        lines.join("\n  ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violations_for(yaml: &str, file: &str) -> Vec<Violation> {
        let doc: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let mut out = Vec::new();
        check_doc(&doc, file, &mut out);
        out
    }

    /// The exact shape that shipped: required name + `synchronize`.
    #[test]
    fn synchronize_plus_required_name_is_a_violation() {
        let yaml = "on:\n  pull_request:\n    types: [labeled, unlabeled, synchronize, reopened]\n\
                    jobs:\n  gate:\n    name: Check, Build, and Test (Rust)\n";
        let v = violations_for(yaml, "ci-fallback-hosted.yml");
        assert_eq!(v.len(), 1, "synchronize must be caught");
        assert_eq!(v[0].job, "gate");
    }

    /// The fix: a human must act, so the job cannot silently claim the context.
    #[test]
    fn labeled_only_is_allowed() {
        let yaml = "on:\n  pull_request:\n    types: [labeled]\n\
                    jobs:\n  gate:\n    name: Check, Build, and Test (Rust)\n";
        assert!(violations_for(yaml, "ci-fallback-hosted.yml").is_empty());
    }

    /// `pull_request:` with no `types:` is GitHub's DEFAULT set, which includes
    /// synchronize — the empty list must read as dangerous, not as harmless.
    #[test]
    fn unfiltered_pull_request_is_a_violation() {
        let yaml = "on:\n  pull_request:\n\
                    jobs:\n  gate:\n    name: Check, Build, and Test (Rust)\n";
        assert_eq!(violations_for(yaml, "other.yml").len(), 1);
    }

    #[test]
    fn ci_yml_is_the_legitimate_owner() {
        let yaml = "on:\n  pull_request:\n\
                    jobs:\n  ci-summary:\n    name: Check, Build, and Test (Rust)\n";
        assert!(violations_for(yaml, "ci.yml").is_empty());
    }

    #[test]
    fn a_different_job_name_is_fine() {
        let yaml = "on:\n  pull_request:\n\
                    jobs:\n  gate:\n    name: Some Other Job\n";
        assert!(violations_for(yaml, "other.yml").is_empty());
    }

    /// No `pull_request:` trigger at all (schedule/dispatch only) is safe.
    #[test]
    fn no_pull_request_trigger_is_safe() {
        let yaml = "on:\n  schedule:\n    - cron: '0 6 * * *'\n\
                    jobs:\n  gate:\n    name: Check, Build, and Test (Rust)\n";
        assert!(violations_for(yaml, "other.yml").is_empty());
    }
}
