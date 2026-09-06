//! `vox ci node-pnpm-ssot-guard` — Node and pnpm versions in CI must match
//! `contracts/toolchain/workspace-toolchain.v1.yaml`.
//!
//! That file already declares `versions.node` and `versions.pnpm`. The Rust
//! half of the same contract is enforced by `toolchain-ssot`. Until this
//! guard existed, every workflow installed Node 24 and pnpm 11 while the
//! SSOT still said 22.0.0 / 9.1.0 — the exact shape of bug the toolchain
//! contract was written to stop, just not yet applied to the other two
//! pins.
//!
//! A violation is any `actions/setup-node` `node-version:` or
//! `pnpm/action-setup` `version:` whose *major* disagrees with the SSOT.
//! Majors are compared (`24`, `"24"`, `24.0.0` all agree) because both
//! actions accept a major-only pin and that is what every workflow writes.
//! A step that uses one of those actions with the version key missing is
//! also a violation — an unpinned install is drift waiting to happen.
//!
//! Always fails on a violation — a gate that cannot fail is the bug.

use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_yaml::Value;

const SSOT_REL: &str = "contracts/toolchain/workspace-toolchain.v1.yaml";
const SETUP_NODE_PREFIX: &str = "actions/setup-node";
const PNPM_SETUP_PREFIX: &str = "pnpm/action-setup";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SsotVersions {
    node_major: u32,
    pnpm_major: u32,
}

struct Violation {
    file: String,
    step: String,
    what: String,
    found: String,
    expected: u32,
}

/// Leading integer of a version pin (`"24"`, `24`, `24.0.0` → 24).
fn major(raw: &str) -> Option<u32> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn yaml_scalar_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_ssot(yaml: &str) -> Result<SsotVersions> {
    let doc: Value = serde_yaml::from_str(yaml).context("parse toolchain SSOT")?;
    let versions = doc
        .as_mapping()
        .and_then(|m| m.get(Value::String("versions".into())))
        .and_then(|v| v.as_mapping())
        .ok_or_else(|| anyhow!("{SSOT_REL}: missing top-level `versions:` mapping"))?;
    let node = versions
        .get(Value::String("node".into()))
        .and_then(yaml_scalar_string)
        .ok_or_else(|| anyhow!("{SSOT_REL}: missing `versions.node`"))?;
    let pnpm = versions
        .get(Value::String("pnpm".into()))
        .and_then(yaml_scalar_string)
        .ok_or_else(|| anyhow!("{SSOT_REL}: missing `versions.pnpm`"))?;
    let node_major = major(&node)
        .ok_or_else(|| anyhow!("{SSOT_REL}: versions.node={node:?} is not a version pin"))?;
    let pnpm_major = major(&pnpm)
        .ok_or_else(|| anyhow!("{SSOT_REL}: versions.pnpm={pnpm:?} is not a version pin"))?;
    Ok(SsotVersions {
        node_major,
        pnpm_major,
    })
}

fn step_name(step: &serde_yaml::Mapping) -> String {
    step.get(Value::String("name".into()))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            step.get(Value::String("uses".into()))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "<unnamed step>".to_string())
}

fn with_key<'a>(step: &'a serde_yaml::Mapping, key: &str) -> Option<&'a Value> {
    step.get(Value::String("with".into()))
        .and_then(|w| w.as_mapping())
        .and_then(|m| m.get(Value::String(key.into())))
}

fn check_doc(doc: &Value, file: &str, ssot: SsotVersions, out: &mut Vec<Violation>) {
    let Some(jobs) = doc
        .as_mapping()
        .and_then(|m| m.get(Value::String("jobs".into())))
        .and_then(|j| j.as_mapping())
    else {
        return;
    };
    for (_job_name, job) in jobs {
        let Some(steps) = job
            .as_mapping()
            .and_then(|j| j.get(Value::String("steps".into())))
            .and_then(|s| s.as_sequence())
        else {
            continue;
        };
        for step in steps {
            let Some(step_map) = step.as_mapping() else {
                continue;
            };
            let Some(uses) = step_map
                .get(Value::String("uses".into()))
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            if uses.starts_with(SETUP_NODE_PREFIX) {
                match with_key(step_map, "node-version").and_then(yaml_scalar_string) {
                    None => out.push(Violation {
                        file: file.to_string(),
                        step: step_name(step_map),
                        what: "node-version".into(),
                        found: "<missing>".into(),
                        expected: ssot.node_major,
                    }),
                    Some(raw) => match major(&raw) {
                        Some(m) if m == ssot.node_major => {}
                        Some(_) | None => out.push(Violation {
                            file: file.to_string(),
                            step: step_name(step_map),
                            what: "node-version".into(),
                            found: raw,
                            expected: ssot.node_major,
                        }),
                    },
                }
            }
            if uses.starts_with(PNPM_SETUP_PREFIX) {
                match with_key(step_map, "version").and_then(yaml_scalar_string) {
                    None => out.push(Violation {
                        file: file.to_string(),
                        step: step_name(step_map),
                        what: "pnpm version".into(),
                        found: "<missing>".into(),
                        expected: ssot.pnpm_major,
                    }),
                    Some(raw) => match major(&raw) {
                        Some(m) if m == ssot.pnpm_major => {}
                        Some(_) | None => out.push(Violation {
                            file: file.to_string(),
                            step: step_name(step_map),
                            what: "pnpm version".into(),
                            found: raw,
                            expected: ssot.pnpm_major,
                        }),
                    },
                }
            }
        }
    }
}

fn scan_workflows(repo_root: &Path, ssot: SsotVersions) -> Result<Vec<Violation>> {
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
        let doc: Value =
            serde_yaml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        check_doc(&doc, &name, ssot, &mut violations);
    }
    Ok(violations)
}

pub fn run(repo_root: &Path) -> Result<()> {
    let ssot_path = repo_root.join(SSOT_REL);
    let ssot_text = std::fs::read_to_string(&ssot_path)
        .with_context(|| format!("read {}", ssot_path.display()))?;
    let ssot = parse_ssot(&ssot_text)?;
    let violations = scan_workflows(repo_root, ssot)?;

    if violations.is_empty() {
        println!(
            "node-pnpm-ssot-guard OK (workflows pin node {} / pnpm {} per {SSOT_REL})",
            ssot.node_major, ssot.pnpm_major
        );
        return Ok(());
    }

    let lines: Vec<String> = violations
        .iter()
        .map(|v| {
            format!(
                "{}: step \"{}\" {}={:?} (SSOT major is {})",
                v.file, v.step, v.what, v.found, v.expected
            )
        })
        .collect();
    bail!(
        "node-pnpm-ssot-guard: {} workflow step(s) disagree with {SSOT_REL}:\n  {}\n\
         Fix: set the pin to the SSOT major, or update {SSOT_REL} if the contract itself is stale.",
        violations.len(),
        lines.join("\n  ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const SSOT: SsotVersions = SsotVersions {
        node_major: 24,
        pnpm_major: 11,
    };

    fn violations_for(yaml: &str) -> Vec<Violation> {
        let doc: Value = serde_yaml::from_str(yaml).unwrap();
        let mut out = Vec::new();
        check_doc(&doc, "ci.yml", SSOT, &mut out);
        out
    }

    #[test]
    fn major_accepts_quoted_unquoted_and_semver() {
        assert_eq!(major("24"), Some(24));
        assert_eq!(major("\"24\""), Some(24));
        assert_eq!(major("'20'"), Some(20));
        assert_eq!(major("24.0.0"), Some(24));
        assert_eq!(major("11"), Some(11));
        assert_eq!(major("${{ env.NODE }}"), None);
    }

    #[test]
    fn parse_ssot_reads_quoted_and_numeric() {
        let quoted = parse_ssot("versions:\n  node: \"24\"\n  pnpm: \"11\"\n").unwrap();
        assert_eq!(quoted.node_major, 24);
        assert_eq!(quoted.pnpm_major, 11);
        let numeric =
            parse_ssot("versions:\n  rust: \"1.98.1\"\n  node: 24\n  pnpm: 11\n").unwrap();
        assert_eq!(numeric, quoted);
        let semver = parse_ssot("versions:\n  node: \"24.0.0\"\n  pnpm: \"11.0.0\"\n").unwrap();
        assert_eq!(semver, quoted);
    }

    #[test]
    fn parse_ssot_fails_when_node_or_pnpm_missing() {
        assert!(parse_ssot("versions:\n  rust: \"1.98.1\"\n").is_err());
        assert!(parse_ssot("versions:\n  node: \"24\"\n").is_err());
    }

    /// Matching majors — the shape every current workflow should have.
    #[test]
    fn matching_node_24_and_pnpm_11_pass() {
        let yaml = r#"
jobs:
  build:
    steps:
      - uses: actions/setup-node@v7
        with:
          node-version: 24
      - uses: pnpm/action-setup@v6
        with:
          version: 11
"#;
        assert!(violations_for(yaml).is_empty());
    }

    #[test]
    fn quoted_pins_that_share_the_ssot_major_pass() {
        let yaml = r#"
jobs:
  build:
    steps:
      - uses: actions/setup-node@v7
        with:
          node-version: '24'
      - uses: pnpm/action-setup@v6
        with:
          version: "11"
"#;
        assert!(violations_for(yaml).is_empty());
    }

    /// The exact drift this guard exists to catch: a leftover Node 20 pin.
    #[test]
    fn node_20_against_ssot_24_is_a_violation() {
        let yaml = r#"
jobs:
  build:
    steps:
      - name: Install Node.js
        uses: actions/setup-node@v7
        with:
          node-version: '20'
"#;
        let v = violations_for(yaml);
        assert_eq!(v.len(), 1, "node 20 must be caught");
        assert_eq!(v[0].what, "node-version");
        assert_eq!(v[0].expected, 24);
        assert_eq!(v[0].found, "20");
    }

    #[test]
    fn pnpm_9_against_ssot_11_is_a_violation() {
        let yaml = r#"
jobs:
  build:
    steps:
      - uses: pnpm/action-setup@v6
        with:
          version: 9
"#;
        let v = violations_for(yaml);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].what, "pnpm version");
        assert_eq!(v[0].expected, 11);
    }

    #[test]
    fn setup_node_without_node_version_is_a_violation() {
        let yaml = "jobs:\n  build:\n    steps:\n      - uses: actions/setup-node@v7\n";
        let v = violations_for(yaml);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].found, "<missing>");
    }

    #[test]
    fn pnpm_setup_without_version_is_a_violation() {
        let yaml = "jobs:\n  build:\n    steps:\n      - uses: pnpm/action-setup@v6\n";
        let v = violations_for(yaml);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].found, "<missing>");
    }

    #[test]
    fn unrelated_version_keys_are_ignored() {
        let yaml = r#"
jobs:
  build:
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: taiki-e/install-action@v2
        with:
          tool: tauri-cli
"#;
        assert!(violations_for(yaml).is_empty());
    }

    /// Prove `run()` itself can go red, not just the helper. A temp tree
    /// whose SSOT says 24/11 and whose one workflow pins Node 20 must fail.
    #[test]
    fn run_fails_on_a_mismatched_workflow_and_passes_when_aligned() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("contracts/toolchain")).unwrap();
        std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
        std::fs::write(
            root.join(SSOT_REL),
            "schema: vox.workspace.toolchain.v1\nversions:\n  rust: \"1.98.1\"\n  node: \"24\"\n  pnpm: \"11\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join(".github/workflows/ci.yml"),
            r#"
jobs:
  build:
    steps:
      - uses: actions/setup-node@v7
        with:
          node-version: 20
"#,
        )
        .unwrap();
        let err = run(root).expect_err("node 20 vs SSOT 24 must fail");
        let msg = err.to_string();
        assert!(msg.contains("node-pnpm-ssot-guard"), "{msg}");
        assert!(msg.contains("node-version"), "{msg}");

        std::fs::write(
            root.join(".github/workflows/ci.yml"),
            r#"
jobs:
  build:
    steps:
      - uses: actions/setup-node@v7
        with:
          node-version: 24
      - uses: pnpm/action-setup@v6
        with:
          version: 11
"#,
        )
        .unwrap();
        run(root).expect("aligned pins must pass");
    }

    /// The real checkout must pass after the SSOT and leftover Node 20 pins
    /// were brought into agreement. A failure here means a workflow drifted.
    #[test]
    fn live_repo_workflows_match_the_ssot() {
        let root = crate::repo_root();
        run(&root).expect("live .github/workflows must match workspace-toolchain.v1.yaml");
    }
}
