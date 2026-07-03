//! `vox ci workflow-concurrency-guard` — require a top-level `concurrency:` mapping
//! with `cancel-in-progress: true` on every workflow triggered by `push` or
//! `pull_request`, so superseded runs are cancelled at the source (flood
//! prevention for the local runner fleet). A bare group string, or a mapping
//! without `cancel-in-progress: true`, serializes without cancelling and
//! provides zero flood protection — that counts as a violation.
//!
//! Advisory by default; `--strict` fails. Exceptions: backticked filenames in
//! `docs/src/ci/concurrency-exceptions.md` (pattern mirrors runner_policy_check.rs).

use std::path::Path;

use anyhow::{Context, Result, anyhow};

const EXCEPTIONS_DOC: &str = "docs/src/ci/concurrency-exceptions.md";

/// True when the workflow's triggers include `push` or `pull_request`.
/// serde_yaml (YAML 1.1) parses the bare `on:` key as `Bool(true)`.
fn needs_concurrency(doc: &serde_yaml::Value) -> bool {
    let Some(map) = doc.as_mapping() else {
        return false;
    };
    let triggers = map
        .get(serde_yaml::Value::String("on".into()))
        .or_else(|| map.get(serde_yaml::Value::Bool(true)));
    let Some(triggers) = triggers else {
        return false;
    };
    let hit = |s: &str| s == "push" || s == "pull_request";
    match triggers {
        serde_yaml::Value::String(s) => hit(s),
        serde_yaml::Value::Sequence(seq) => seq.iter().any(|v| v.as_str().is_some_and(hit)),
        serde_yaml::Value::Mapping(m) => m.keys().any(|k| k.as_str().is_some_and(hit)),
        _ => false,
    }
}

/// True only when the top-level `concurrency:` is a mapping containing
/// `cancel-in-progress: true`. A scalar group string or a mapping without
/// that key merely serializes runs — no flood protection — so it fails.
fn has_cancelling_concurrency(doc: &serde_yaml::Value) -> bool {
    doc.as_mapping()
        .and_then(|m| m.get(serde_yaml::Value::String("concurrency".into())))
        .and_then(|c| c.as_mapping())
        .and_then(|c| c.get(serde_yaml::Value::String("cancel-in-progress".into())))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Scoped to markdown list-item lines (`- \`file.yml\` — reason`), not the
/// whole doc — mirrors `runner_policy_check::parse_exceptions_doc` scoping to
/// table rows, so a filename mentioned in unrelated prose (e.g. contrastive
/// documentation) can never silently exempt a workflow.
fn is_excepted(exceptions_text: &str, file_name: &str) -> bool {
    let marker = format!("`{file_name}`");
    exceptions_text
        .lines()
        .any(|line| line.trim_start().starts_with('-') && line.contains(&marker))
}

pub fn run(repo_root: &Path, strict: bool) -> Result<()> {
    let exceptions_path = repo_root.join(EXCEPTIONS_DOC);
    let exceptions_text = std::fs::read_to_string(&exceptions_path)
        .with_context(|| format!("read {}", exceptions_path.display()))?;
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
        if needs_concurrency(&doc)
            && !has_cancelling_concurrency(&doc)
            && !is_excepted(&exceptions_text, &name)
        {
            violations.push(name);
        }
    }
    if violations.is_empty() {
        println!("workflow-concurrency-guard OK ({EXCEPTIONS_DOC} consulted)");
        return Ok(());
    }
    let msg = format!(
        "workflow-concurrency-guard: {} workflow(s) with push/pull_request triggers lack a \
         top-level `concurrency:` mapping with `cancel-in-progress: true` and are not \
         registered in {EXCEPTIONS_DOC}:\n  {}\n\
         Fix: add\n\
         concurrency:\n  \
           group: ${{{{ github.workflow }}}}-${{{{ github.ref }}}}\n  \
           cancel-in-progress: true\n\
         or register an exception with a reason.",
        violations.len(),
        violations.join("\n  ")
    );
    if strict {
        Err(anyhow!(msg))
    } else {
        eprintln!("WARN {msg}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_detection_handles_yaml_11_on_key() {
        // serde_yaml (YAML 1.1) parses a bare `on:` key as Bool(true).
        let pr: serde_yaml::Value =
            serde_yaml::from_str("on:\n  pull_request:\n    paths: ['x']\njobs: {}").unwrap();
        assert!(needs_concurrency(&pr));
        let push: serde_yaml::Value =
            serde_yaml::from_str("on:\n  push:\n    tags: ['v*']\njobs: {}").unwrap();
        assert!(needs_concurrency(&push)); // tag-push still "needs" — exceptions doc carries it
        let sched: serde_yaml::Value =
            serde_yaml::from_str("on:\n  schedule:\n    - cron: '0 0 * * *'\njobs: {}").unwrap();
        assert!(!needs_concurrency(&sched));
        let wf_run: serde_yaml::Value =
            serde_yaml::from_str("on:\n  workflow_run:\n    types: [completed]\njobs: {}").unwrap();
        assert!(!needs_concurrency(&wf_run));
        let scalar: serde_yaml::Value = serde_yaml::from_str("on: push\njobs: {}").unwrap();
        assert!(needs_concurrency(&scalar));
        let seq: serde_yaml::Value =
            serde_yaml::from_str("on: [push, workflow_dispatch]\njobs: {}").unwrap();
        assert!(needs_concurrency(&seq));
    }

    #[test]
    fn cancelling_concurrency_detection() {
        // Mapping with cancel-in-progress: true — the only passing shape.
        let with: serde_yaml::Value = serde_yaml::from_str(
            "on: push\nconcurrency:\n  group: g\n  cancel-in-progress: true\njobs: {}",
        )
        .unwrap();
        assert!(has_cancelling_concurrency(&with));
        // No concurrency at all.
        let without: serde_yaml::Value = serde_yaml::from_str("on: push\njobs: {}").unwrap();
        assert!(!has_cancelling_concurrency(&without));
        // Mapping without cancel-in-progress: serializes but never cancels — fails.
        let no_cancel: serde_yaml::Value =
            serde_yaml::from_str("on: push\nconcurrency:\n  group: g\njobs: {}").unwrap();
        assert!(!has_cancelling_concurrency(&no_cancel));
        // Scalar group string: same — fails.
        let scalar: serde_yaml::Value =
            serde_yaml::from_str("on: push\nconcurrency: my-group\njobs: {}").unwrap();
        assert!(!has_cancelling_concurrency(&scalar));
        // Explicit cancel-in-progress: false — fails.
        let explicit_false: serde_yaml::Value = serde_yaml::from_str(
            "on: push\nconcurrency:\n  group: g\n  cancel-in-progress: false\njobs: {}",
        )
        .unwrap();
        assert!(!has_cancelling_concurrency(&explicit_false));
    }

    #[test]
    fn exception_matching_is_backticked_filename() {
        let doc = "- `release-binaries.yml` — tag-push only.";
        assert!(is_excepted(doc, "release-binaries.yml"));
        assert!(!is_excepted(doc, "ci.yml"));
    }

    #[test]
    fn exception_matching_ignores_prose_mentions() {
        // A filename mentioned in non-list-item prose must NOT silently exempt it —
        // only backticked mentions inside `- ` list items count.
        let doc = "Unlike `ci.yml`, this workflow needs no concurrency group.\n\n\
                   - `release-binaries.yml` — tag-push only.";
        assert!(!is_excepted(doc, "ci.yml"));
        assert!(is_excepted(doc, "release-binaries.yml"));
    }
}
