use crate::commands::ci::cmd_enums::GuardOpts;
use anyhow::{Context, Result};
use glob;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct GuardReport {
    pub violations: Vec<String>,
    pub files_scanned: u32,
}

impl GuardReport {
    pub fn empty() -> Self {
        Self {
            violations: Vec::new(),
            files_scanned: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DataStoragePolicy {
    pub version: u32,
    #[serde(rename = "x-vox-version")]
    pub x_vox_version: u32,
}

pub fn load_policy(path: &Path) -> Result<DataStoragePolicy> {
    let yaml = std::fs::read_to_string(path).context("read policy yaml")?;
    let val: serde_json::Value = serde_yaml::from_str(&yaml).context("parse yaml")?;

    let schema_path = path
        .parent()
        .unwrap()
        .join("data-storage-policy.v1.schema.json");
    let schema_str = std::fs::read_to_string(&schema_path).context("read policy schema")?;
    let schema_val: serde_json::Value =
        serde_json::from_str(&schema_str).context("parse schema")?;

    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())?;
    vox_jsonschema_util::validate(&val, &validator, "data storage policy")?;

    serde_json::from_value(val).context("deserialize policy")
}

/// `env-parity` scans source with ripgrep; documentation and string-building often use **prefix**
/// tokens like `VOX_CLI_` or `VOX_MCP_HTTP_` that are not real environment variable names.
/// Require SSOT entries only for tokens that look like complete env vars.
fn env_var_token_requires_registry_entry(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    if name.ends_with('_') {
        return false;
    }
    for prefix in ["VOX_", "TURSO_", "XDG_"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            if rest.is_empty() {
                return false;
            }
            // e.g. `VOX_A` / `VOX_B` from overlapping matches — not actionable registry keys.
            if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_uppercase()) {
                return false;
            }
            return true;
        }
    }
    true
}

pub fn run(opts: &GuardOpts) -> Result<GuardReport> {
    let root = std::env::current_dir()?;
    let policy_path = root.join("contracts/db/data-storage-policy.v1.yaml");

    if opts.check_policy_only {
        let _ = load_policy(&policy_path)?;
        if !opts.json {
            println!("Policy valid.");
        }
        return Ok(GuardReport::empty());
    }

    let mut report = GuardReport::empty();

    let run_all = opts.only.is_empty();

    // Check for stray root files
    if run_all || opts.only.contains(&"repo-root-strays-absent".to_string()) {
        let schemas_path = root.join("schemas");
        if schemas_path.exists() {
            report.violations.push(
                "schemas-dir-absent: schemas directory exists but is forbidden by policy."
                    .to_string(),
            );
        }

        let strays = [
            "build_errors.txt",
            "test_lexer.rs",
            "error.vox",
            "prototype_vox_tokenizer.json",
        ];
        for stray in strays.iter() {
            if root.join(stray).exists() {
                report.violations.push(format!(
                    "repo-root-strays-absent: {} is forbidden by policy.",
                    stray
                ));
            }
        }

        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("codex-cutover-") && name.ends_with(".sidecar.json") {
                        report.violations.push(format!(
                            "repo-root-strays-absent: {} is forbidden by policy.",
                            name
                        ));
                    }
                }
            }
        }
    }

    // Parity between ignore files and git tracking
    if run_all || opts.only.contains(&"ignore-tracked-parity".to_string()) {
        let ignore_files = [".voxignore", ".aiignore", ".cursorignore", ".aiexclude"];
        for ignore_file in ignore_files.iter() {
            if let Ok(content) = std::fs::read_to_string(root.join(ignore_file)) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty()
                        || line.starts_with('#')
                        || line.contains('*')
                        || line.ends_with('/')
                    {
                        continue;
                    }

                    if let Ok(output) = std::process::// vox-arch-check: allow git-exec
        Command::new("git")
                        .arg("ls-files")
                        .arg(line)
                        .current_dir(&root)
                        .output()
                    {
                        if !output.stdout.is_empty() {
                            report.violations.push(format!(
                                "ignore-tracked-parity: {} is in {} but is tracked by git.",
                                line, ignore_file
                            ));
                        }
                    }
                }
            }
        }
    }

    // Scratch cleanliness
    if run_all || opts.only.contains(&"scratch-clean".to_string()) {
        if let Ok(output) = std::process::// vox-arch-check: allow git-exec
        Command::new("git")
            .arg("ls-files")
            .arg("scratch/")
            .current_dir(&root)
            .output()
        {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                let tracked_files: Vec<&str> = stdout
                    .lines()
                    .filter(|l| !l.ends_with(".gitkeep"))
                    .collect();
                if !tracked_files.is_empty() {
                    report.violations.push(format!(
                        "scratch-clean: scratch/ contains tracked files other than .gitkeep: {:?}",
                        tracked_files
                    ));
                }
            }
        }
    }

    // Codegen drift check (M-11)
    if run_all || opts.only.contains(&"schema-codegen-drift".to_string()) {
        // Run emit_agent_harness --verify
        let output = std::process::Command::new("cargo")
            .args([
                "run",
                "-p",
                "vox-jsonschema-util",
                "--example",
                "emit_agent_harness",
                "--",
                "--verify",
            ])
            .current_dir(&root)
            .output();

        match output {
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                report
                    .violations
                    .push(format!("schema-codegen-drift: {}", stderr.trim()));
            }
            Err(e) => {
                report.violations.push(format!(
                    "schema-codegen-drift: failed to run generator: {}",
                    e
                ));
            }
            _ => {}
        }
    }

    // Version Header Parity (M-12)
    if run_all || opts.only.contains(&"version-header-parity".to_string()) {
        let pattern = root.join("contracts/**/*.v*.*");
        if let Ok(entries) = glob::glob(&pattern.to_string_lossy()) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.is_file() {
                    report.files_scanned += 1;
                    let path_str = entry.to_string_lossy();

                    // Extract version N from .vN. in filename
                    let mut version = None;
                    if let Some(idx) = path_str.find(".v") {
                        let rest = &path_str[idx + 2..];
                        if let Some(end_idx) = rest.find('.') {
                            let v_str = &rest[..end_idx];
                            if v_str.chars().all(|c| c.is_ascii_digit()) && !v_str.is_empty() {
                                version = Some(v_str);
                            }
                        }
                    }

                    if let Some(v) = version {
                        if let Ok(content) = std::fs::read_to_string(&entry) {
                            let has_header = content.contains(&format!("x-vox-version: {v}"))
                                || content.contains(&format!("\"x-vox-version\": {v}"));
                            if !has_header {
                                report.violations.push(format!(
                                    "version-header-parity: file {} is missing mandatory header 'x-vox-version: {}'",
                                    entry.strip_prefix(&root).unwrap_or(&entry).display(),
                                    v
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Env Parity (M-14)
    if run_all || opts.only.contains(&"env-parity".to_string()) {
        let env_yaml_path = root.join("contracts/config/env-vars.v1.yaml");
        if env_yaml_path.exists() {
            if let Ok(yaml_str) = std::fs::read_to_string(&env_yaml_path) {
                if let Ok(yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&yaml_str) {
                    let mut allowed_vars = std::collections::HashSet::new();
                    if let Some(vars) = yaml_val.get("variables").and_then(|v| v.as_sequence()) {
                        for v in vars {
                            if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                                allowed_vars.insert(name.to_string());
                            }
                        }
                    }

                    if let Ok(output) = std::process::Command::new("rg")
                        .args([
                            "-o",
                            "--no-heading",
                            "--no-line-number",
                            "(VOX|TURSO|XDG)_[A-Z0-9_]+",
                            "crates/",
                            "apps/",
                            "scripts/",
                            "docs/src/",
                        ])
                        .current_dir(&root)
                        .output()
                    {
                        if output.status.success() {
                            let matches = String::from_utf8_lossy(&output.stdout);
                            let mut found_vars = std::collections::HashSet::new();
                            for line in matches.lines() {
                                let line = line.trim();
                                if !line.is_empty() {
                                    // Extract just the matched part if rg returned "file:match" or similar
                                    let var_name = if let Some(idx) = line.rfind(':') {
                                        &line[idx + 1..]
                                    } else {
                                        line
                                    };
                                    found_vars.insert(var_name.to_string());
                                }
                            }

                            for var in found_vars {
                                if !env_var_token_requires_registry_entry(&var) {
                                    continue;
                                }
                                if !allowed_vars.contains(&var) {
                                    report.violations.push(format!(
                                        "env-parity: undocumented environment variable '{}' found in source code. Must be added to contracts/config/env-vars.v1.yaml",
                                        var
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        } else {
            report
                .violations
                .push("env-parity: contracts/config/env-vars.v1.yaml missing".to_string());
        }
    }

    if !opts.json {
        if report.violations.is_empty() {
            println!("DataStorageGuard check passed.");
        } else {
            println!(
                "DataStorageGuard check failed with violations: {:#?}",
                report.violations
            );
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_token_filters_prefix_fragments() {
        assert!(!env_var_token_requires_registry_entry("VOX_CLI_"));
        assert!(!env_var_token_requires_registry_entry("VOX_ROUTE_"));
        assert!(env_var_token_requires_registry_entry("VOX_CLI_FEATURES"));
    }

    #[test]
    fn env_var_token_filters_single_letter_suffix_noise() {
        assert!(!env_var_token_requires_registry_entry("VOX_A"));
        assert!(env_var_token_requires_registry_entry("VOX_ANDROID_KEYSTORE"));
    }

    #[test]
    fn test_bad_policy_fails() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let bad_policy_path = manifest.join("../../tests/fixtures/bad-data-storage-policy.yaml");
        let res = load_policy(&bad_policy_path);
        assert!(res.is_err(), "Bad policy should fail schema validation");
    }
}
