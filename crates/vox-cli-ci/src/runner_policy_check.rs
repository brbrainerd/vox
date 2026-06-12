//! `vox ci runner-policy-check` — advisory lint for GitHub-hosted `runs-on` labels.
//!
//! Default: warn and exit 0. Pass `--strict` to fail CI/pre-push when a workflow job
//! uses a GitHub-hosted runner without a row in `docs/src/ci/github-hosted-exceptions.md`.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

const EXCEPTIONS_DOC: &str = "docs/src/ci/github-hosted-exceptions.md";

/// GitHub-hosted runner label substrings (lowercase).
const HOSTED_MARKERS: &[&str] = &[
    "ubuntu-latest",
    "windows-latest",
    "macos-latest",
    "macos-13",
    "macos-14",
    "macos-15",
];

/// Run the runner-policy check against workflow YAML under `.github/workflows/`.
pub fn run(repo_root: &Path, strict: bool) -> Result<()> {
    let exceptions_path = repo_root.join(EXCEPTIONS_DOC);
    let exceptions_text = std::fs::read_to_string(&exceptions_path)
        .with_context(|| format!("read {}", exceptions_path.display()))?;
    let exceptions = parse_exceptions_doc(&exceptions_text)?;
    let wf_dir = repo_root.join(".github/workflows");
    let mut violations = Vec::new();

    for entry in std::fs::read_dir(&wf_dir).with_context(|| format!("read {}", wf_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let ext = path.extension().and_then(|x| x.to_str());
        if ext != Some("yml") && ext != Some("yaml") {
            continue;
        }
        let workflow_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if exceptions.contains(&workflow_name) {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        if workflow_uses_hosted_runner(&text) {
            violations.push(format!(
                "{}: uses GitHub-hosted runner(s) but is not listed in {}",
                workflow_name, EXCEPTIONS_DOC
            ));
        }
    }

    if violations.is_empty() {
        println!(
            "runner-policy-check OK ({} exception workflow(s) registered)",
            exceptions.len()
        );
        return Ok(());
    }

    for v in &violations {
        eprintln!("runner-policy-check: {v}");
    }
    if strict {
        return Err(anyhow!(
            "runner-policy-check: {} workflow(s) use GitHub-hosted runners without a registered exception — migrate to self-hosted or add a row to {}",
            violations.len(),
            EXCEPTIONS_DOC
        ));
    }
    eprintln!(
        "runner-policy-check: {} warning(s) (advisory — re-run with --strict to fail)",
        violations.len()
    );
    Ok(())
}

/// Parse workflow basenames from the exceptions doc markdown table (`workflow.yml` in backticks).
pub fn parse_exceptions_doc(text: &str) -> Result<std::collections::HashSet<String>> {
    let mut out = std::collections::HashSet::new();
    let workflow_re = regex::Regex::new(r"`([^`]+\.ya?ml)`").expect("valid regex");
    for line in text.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        for cap in workflow_re.captures_iter(line) {
            out.insert(cap[1].to_string());
        }
    }
    Ok(out)
}

/// True when the workflow text assigns a GitHub-hosted label to any job.
pub fn workflow_uses_hosted_runner(text: &str) -> bool {
    let matrix_hosted = text.contains("ubuntu-latest")
        || text.contains("windows-latest")
        || text.contains("macos-latest")
        || text.contains("macos-13");
    if !matrix_hosted {
        return false;
    }
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("runs-on:") {
            continue;
        }
        let value = trimmed.strip_prefix("runs-on:").unwrap_or("").trim();
        let lower = value.to_lowercase();
        if HOSTED_MARKERS.iter().any(|m| lower.contains(m)) {
            return true;
        }
        if lower.contains("${{ matrix") && matrix_hosted {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exceptions_doc_extracts_workflow_names() {
        let md = r#"
| Workflow | Runner | Reason |
|----------|--------|--------|
| `docs-deploy.yml` | `ubuntu-latest` | Pages |
| `release-binaries.yml` | `windows-latest` | Binaries |
"#;
        let set = parse_exceptions_doc(md).unwrap();
        assert!(set.contains("docs-deploy.yml"));
        assert!(set.contains("release-binaries.yml"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn workflow_uses_hosted_runner_detects_ubuntu_latest() {
        let yml = r#"
jobs:
  test:
    runs-on: ubuntu-latest
"#;
        assert!(workflow_uses_hosted_runner(yml));
    }

    #[test]
    fn workflow_uses_hosted_runner_self_hosted_ok() {
        let yml = r#"
jobs:
  test:
    runs-on: [self-hosted, linux, x64]
"#;
        assert!(!workflow_uses_hosted_runner(yml));
    }

    #[test]
    fn workflow_uses_hosted_runner_matrix_os() {
        let yml = r#"
jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
"#;
        assert!(workflow_uses_hosted_runner(yml));
    }

    #[test]
    fn hosted_markers_include_macos_variants() {
        assert!(HOSTED_MARKERS.contains(&"macos-13"));
    }
}
