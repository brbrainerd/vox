//! `vox ci config-hygiene`: machine guardrails that keep config single-homed,
//! safe-by-default, and never silently inert. Run BEFORE the configurability plan.

use std::path::Path;

/// A single hygiene violation (file:line + message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub check: &'static str,
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Check A: forbid cwd-relative `contracts/...` paths passed to file loaders.
/// Such paths are inert in any non-repo-root binary. Use `include_str!`-embedded
/// contracts instead (see Phase 0).
pub fn check_no_cwd_relative_contract_paths(source: &str, file: &str) -> Vec<Violation> {
    let mut hits = Vec::new();
    let re = regex::Regex::new(r#""contracts/[^"]+\.(?:ya?ml|toml)""#).unwrap();
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
                message: "cwd-relative \"contracts/...\" path is inert in deployed binaries; \
                          embed the contract with include_str! (see config-guardrails Phase 0)"
                    .to_string(),
            });
        }
    }
    hits
}

/// Check B placeholder — implemented in Task 6. Keep signature stable.
pub fn check_protected_modules_have_no_env_reads(_source: &str, _file: &str) -> Vec<Violation> {
    Vec::new()
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
        if rel.contains("config_hygiene.rs") {
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

fn collect_rs_files(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
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
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with("_tests.rs") || name == "tests.rs" {
                continue;
            }
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
    fn allows_include_str_and_comments() {
        let ok = r#"const E: &str = include_str!("../../../contracts/gamify/economy.v1.yaml");"#;
        assert!(check_no_cwd_relative_contract_paths(ok, "x.rs").is_empty());
        let comment = r#"// loads contracts/gamify/economy.v1.yaml at build time"#;
        assert!(check_no_cwd_relative_contract_paths(comment, "x.rs").is_empty());
    }
}
