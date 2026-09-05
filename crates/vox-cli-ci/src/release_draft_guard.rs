//! `vox ci release-draft-guard` — require every workflow step whose `uses:`
//! starts with `softprops/action-gh-release` to set `with.draft` to the
//! boolean `true`. `action-gh-release` defaults to a published, public,
//! `latest` release when `draft:` is absent — this repo has repeatedly been
//! told not to publish a genuinely public release, so a missing or falsy
//! `draft` is a hard failure, not an advisory warning.
//!
//! Unlike the advisory guards in this crate, this one takes no `--strict`
//! flag: it always fails on a violation.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

const RELEASE_ACTION_PREFIX: &str = "softprops/action-gh-release";

/// One violation: a `uses: softprops/action-gh-release*` step in `file` named
/// `step` that does not set `with.draft: true`.
struct Violation {
    file: String,
    step: String,
}

/// True only when `with.draft` is present and is the YAML boolean `true`
/// (not the string `"true"`, not absent, not `false`). serde_yaml (YAML 1.1)
/// distinguishes `Value::Bool` from `Value::String` — mirrors the same care
/// `workflow_concurrency_guard::has_cancelling_concurrency` takes for
/// `cancel-in-progress: true`.
fn step_has_draft_true(step: &serde_yaml::Mapping) -> bool {
    step.get(serde_yaml::Value::String("with".into()))
        .and_then(|w| w.as_mapping())
        .and_then(|w| w.get(serde_yaml::Value::String("draft".into())))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Step display name: prefers the `name:` field, falls back to `uses:`.
fn step_name(step: &serde_yaml::Mapping) -> String {
    step.get(serde_yaml::Value::String("name".into()))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            step.get(serde_yaml::Value::String("uses".into()))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "<unnamed step>".to_string())
}

/// Collects every `softprops/action-gh-release` step in `doc` that lacks
/// `draft: true`, appending `file`-scoped violations to `out`.
fn check_doc(doc: &serde_yaml::Value, file: &str, out: &mut Vec<Violation>) {
    let Some(jobs) = doc
        .as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String("jobs".into())))
        .and_then(|j| j.as_mapping())
    else {
        return;
    };
    for (_job_name, job) in jobs {
        let Some(steps) = job
            .as_mapping()
            .and_then(|j| j.get(serde_yaml::Value::String("steps".into())))
            .and_then(|s| s.as_sequence())
        else {
            continue;
        };
        for step in steps {
            let Some(step_map) = step.as_mapping() else {
                continue;
            };
            let is_release_step = step_map
                .get(serde_yaml::Value::String("uses".into()))
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.starts_with(RELEASE_ACTION_PREFIX));
            if is_release_step && !step_has_draft_true(step_map) {
                out.push(Violation {
                    file: file.to_string(),
                    step: step_name(step_map),
                });
            }
        }
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
            "release-draft-guard OK (no softprops/action-gh-release step without draft: true)"
        );
        return Ok(());
    }

    let lines: Vec<String> = violations
        .iter()
        .map(|v| format!("{}: step \"{}\"", v.file, v.step))
        .collect();
    Err(anyhow!(
        "release-draft-guard: {} softprops/action-gh-release step(s) without `draft: true`:\n  {}\n\
         Fix: add `draft: true` (and `prerelease: true`) under that step's `with:`. \
         This repo never publishes a genuinely public GitHub release automatically; \
         drafts are promoted by hand.",
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

    #[test]
    fn draft_true_passes() {
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          draft: true\n";
        let violations = violations_for(yaml, "ok.yml");
        assert!(violations.is_empty(), "draft: true must pass");
    }

    #[test]
    fn missing_draft_key_fails() {
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          files: x\n";
        let violations = violations_for(yaml, "missing.yml");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].file, "missing.yml");
        assert_eq!(violations[0].step, "Create release");
    }

    #[test]
    fn draft_false_fails() {
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          draft: false\n";
        let violations = violations_for(yaml, "false.yml");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn no_release_action_passes() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Build\n        run: echo hi\n";
        let violations = violations_for(yaml, "none.yml");
        assert!(violations.is_empty());
    }

    #[test]
    fn draft_string_true_fails() {
        // "true" as a YAML string (quoted) is not the boolean true.
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Create release\n        \
                        uses: softprops/action-gh-release@v3\n        with:\n          draft: \"true\"\n";
        let violations = violations_for(yaml, "string.yml");
        assert_eq!(
            violations.len(),
            1,
            "quoted \"true\" string must not satisfy the bool check"
        );
    }
}
