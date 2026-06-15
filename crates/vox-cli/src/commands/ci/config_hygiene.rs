//! `vox ci config-hygiene`: machine guardrails that keep config single-homed,
//! safe-by-default, and never silently inert.

/// A single hygiene violation (file:line + message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub check: &'static str,
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Check A: forbid cwd-relative `contracts/...` path literals in Rust source.
/// Such paths are inert in any non-repo-root binary. Use include_str!-embedded contracts.
pub fn check_no_cwd_relative_contract_paths(source: &str, file: &str) -> Vec<Violation> {
    let re = regex::Regex::new(r#""contracts/[^"]+\.(?:ya?ml|toml)""#).unwrap();
    let mut hits = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim_start();
        if line.starts_with("//") || line.starts_with("//!") {
            continue;
        }
        if re.is_match(raw) {
            hits.push(Violation {
                check: "no-cwd-relative-contract-path",
                file: file.to_string(),
                line: i + 1,
                message: "cwd-relative \"contracts/...\" path is inert in deployed binaries;                           embed the contract with include_str! (see config-guardrails Phase 0)"
                    .to_string(),
            });
        }
    }
    hits
}

/// Check B: files in `vox-config` crate must not call std::env::var directly —
/// they must receive values as parameters to stay testable without env mutation.
pub fn check_protected_modules_have_no_env_reads(source: &str, file: &str) -> Vec<Violation> {
    // Only flag files inside vox-config crate (the pure-config layer).
    let is_protected = file.contains("vox-config") && !file.contains("tests");
    if !is_protected {
        return vec![];
    }
    let mut hits = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        if raw.contains("std::env::var") || raw.contains("env::var(") {
            hits.push(Violation {
                check: "no-env-read-in-pure-config",
                file: file.to_string(),
                line: i + 1,
                message: "vox-config modules must not call env::var directly;                           accept env values as function parameters instead"
                    .to_string(),
            });
        }
    }
    hits
}

/// Run all config-hygiene checks across the workspace.
pub fn run() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let mut violations = Vec::new();
    collect_rs_files(&root.join("crates"), &mut |path, src| {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        if rel.contains("config_hygiene") {
            return;
        }
        violations.extend(check_no_cwd_relative_contract_paths(src, &rel));
        violations.extend(check_protected_modules_have_no_env_reads(src, &rel));
    });
    if violations.is_empty() {
        println!("config-hygiene OK: no violations");
        return Ok(());
    }
    for v in &violations {
        eprintln!("[{}] {}:{} — {}", v.check, v.file, v.line, v.message);
    }
    anyhow::bail!("config-hygiene found {} violation(s)", violations.len())
}

fn collect_rs_files(dir: &std::path::Path, f: &mut impl FnMut(&std::path::Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_rs_files(&path, f);
        } else if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(src) = std::fs::read_to_string(&path) {
                f(&path, &src);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_cwd_relative_contract_literal() {
        let src = r#"let p = Path::new("contracts/orchestration/circuit-breaker.v1.yaml");"#;
        let v = check_no_cwd_relative_contract_paths(src, "x.rs");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].check, "no-cwd-relative-contract-path");
    }

    #[test]
    fn check_b_flags_env_read_in_vox_config() {
        let src = "let v = std::env::var(\"FOO\").ok();";
        let v = check_protected_modules_have_no_env_reads(src, "crates/vox-config/src/lib.rs");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].check, "no-env-read-in-pure-config");
    }

    #[test]
    fn check_b_ignores_non_vox_config_files() {
        let src = "let v = std::env::var(\"FOO\").ok();";
        assert!(
            check_protected_modules_have_no_env_reads(src, "crates/vox-actor-runtime/src/lib.rs")
                .is_empty()
        );
    }

    #[test]
    fn check_b_ignores_test_files_in_vox_config() {
        let src = "let v = std::env::var(\"FOO\").ok();";
        assert!(
            check_protected_modules_have_no_env_reads(src, "crates/vox-config/src/tests/mod.rs")
                .is_empty()
        );
    }

    #[test]
    fn allows_include_str_and_comments() {
        let ok = r#"const E: &str = include_str!("../../../contracts/gamify/economy.v1.yaml");"#;
        assert!(check_no_cwd_relative_contract_paths(ok, "x.rs").is_empty());
        let comment = r#"// loads contracts/gamify/economy.v1.yaml at build time"#;
        assert!(check_no_cwd_relative_contract_paths(comment, "x.rs").is_empty());
    }
}
