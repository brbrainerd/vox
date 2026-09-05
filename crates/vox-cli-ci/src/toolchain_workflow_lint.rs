//! `vox ci toolchain-workflow-lint` — forbid any workflow step from installing
//! Rust directly instead of through `./.github/actions/setup-rust`.
//!
//! `.github/actions/setup-rust/action.yml` is the single place a Rust
//! toolchain version is chosen for CI: it reads
//! `contracts/toolchain/workspace-toolchain.v1.yaml` and deliberately accepts
//! no `toolchain:` input, so no call site can hand-pin a version that drifts
//! from the SSOT. Before this lint, 53 workflow steps across 27 files
//! hand-rolled `dtolnay/rust-toolchain@stable` / `@master` (10 of the latter
//! hard-coding `toolchain: "1.96.0"`), and nothing stopped a new one from
//! creeping back in on the next PR.
//!
//! A violation is any workflow step whose `uses:` starts with
//! `dtolnay/rust-toolchain` — any tag (`@stable`, `@nightly`, a pinned
//! version or commit) counts, since the whole point is that no workflow step
//! should reference that action directly at all. The composite action itself
//! (`.github/actions/setup-rust/action.yml`) is not scanned by this lint —
//! it lives outside `.github/workflows/` and is the one place a
//! commit-pinned `dtolnay/rust-toolchain@<sha>` is the correct, trusted
//! implementation this lint exists to funnel every workflow through.
//!
//! Unlike the advisory guards in this crate, this one takes no `--strict`
//! flag: it always fails on a violation, mirroring `release_draft_guard`.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

const FORBIDDEN_ACTION_PREFIX: &str = "dtolnay/rust-toolchain";
const CANONICAL_ACTION: &str = "./.github/actions/setup-rust";

/// One violation: a step in `file` named `step` whose `uses:` installs Rust
/// directly instead of through the SSOT-reading composite action.
struct Violation {
    file: String,
    step: String,
    uses: String,
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

/// Collects every step in `doc` whose `uses:` starts with
/// `dtolnay/rust-toolchain`, appending `file`-scoped violations to `out`.
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
            let uses = step_map
                .get(serde_yaml::Value::String("uses".into()))
                .and_then(|v| v.as_str());
            if let Some(uses) = uses
                && uses.starts_with(FORBIDDEN_ACTION_PREFIX)
            {
                out.push(Violation {
                    file: file.to_string(),
                    step: step_name(step_map),
                    uses: uses.to_string(),
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
        println!("toolchain-workflow-lint OK (no direct dtolnay/rust-toolchain step)");
        return Ok(());
    }

    let lines: Vec<String> = violations
        .iter()
        .map(|v| format!("{}: step \"{}\" (uses: {})", v.file, v.step, v.uses))
        .collect();
    Err(anyhow!(
        "toolchain-workflow-lint: {} workflow step(s) install Rust directly instead of through \
         {CANONICAL_ACTION}:\n  {}\n\
         Fix: replace the step's `uses:` with `{CANONICAL_ACTION}` (it reads the pinned version \
         from contracts/toolchain/workspace-toolchain.v1.yaml; it accepts no `toolchain:` input, \
         so drop that key if the step had one).",
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
    fn setup_rust_action_passes() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Install Rust toolchain\n        \
                        uses: ./.github/actions/setup-rust\n        with:\n          components: rustfmt, clippy\n";
        let violations = violations_for(yaml, "ok.yml");
        assert!(
            violations.is_empty(),
            "the canonical setup-rust action must never trip this lint"
        );
    }

    #[test]
    fn dtolnay_stable_fails() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Install Rust toolchain\n        \
                        uses: dtolnay/rust-toolchain@stable\n";
        let violations = violations_for(yaml, "stable.yml");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].file, "stable.yml");
        assert_eq!(violations[0].step, "Install Rust toolchain");
    }

    #[test]
    fn dtolnay_master_with_pinned_toolchain_fails() {
        let yaml = "jobs:\n  \
                      release:\n    steps:\n      - name: Install Rust toolchain\n        \
                        uses: dtolnay/rust-toolchain@master\n        with:\n          toolchain: \"1.96.0\"\n";
        let violations = violations_for(yaml, "master.yml");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn dtolnay_nightly_fails() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - uses: dtolnay/rust-toolchain@nightly\n";
        let violations = violations_for(yaml, "nightly.yml");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn dtolnay_pinned_sha_still_fails() {
        // Any tag counts, including a pinned commit -- only the composite
        // action wrapping it is allowed to reference dtolnay directly.
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - uses: dtolnay/rust-toolchain@d1031067263f94b142dd6c0ce24c5eb9d02d52a0\n";
        let violations = violations_for(yaml, "pinned.yml");
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn unrelated_step_passes() {
        let yaml = "jobs:\n  \
                      build:\n    steps:\n      - name: Checkout\n        uses: actions/checkout@v7\n";
        let violations = violations_for(yaml, "unrelated.yml");
        assert!(violations.is_empty());
    }

    #[test]
    fn multiple_violations_in_one_file_are_all_reported() {
        let yaml = "jobs:\n  \
                      a:\n    steps:\n      - uses: dtolnay/rust-toolchain@stable\n  \
                      b:\n    steps:\n      - uses: dtolnay/rust-toolchain@stable\n";
        let violations = violations_for(yaml, "two.yml");
        assert_eq!(violations.len(), 2);
    }
}
