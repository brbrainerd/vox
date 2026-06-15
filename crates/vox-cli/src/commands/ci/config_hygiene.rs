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

/// Check C (advisory): detect pub fn from_env_values / from_env / from_contract_file
/// that appear likely to be declared-but-unwired. Advisory: prints warnings, does not fail.
pub fn check_declared_but_unwired(source: &str, file: &str) -> Vec<Violation> {
    let patterns = [
        "pub fn from_env_values",
        "pub fn from_env(",
        "pub fn from_contract_file",
    ];
    let mut hits = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        for pat in &patterns {
            if raw.contains(pat) {
                hits.push(Violation {
                    check: "declared-but-unwired-advisory",
                    file: file.to_string(),
                    line: i + 1,
                    message: format!(
                        "pub env-resolver '{}' — verify it has a non-test caller; \
                         if not, remove it or wire it up (config-guardrails §3.3)",
                        pat.trim()
                    ),
                });
            }
        }
    }
    hits
}

/// Run all config-hygiene checks across the workspace.
pub fn run() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;

    // Load registry for Check D (env-var parity).
    let registry_path = root.join("contracts/config/registry.v1.yaml");
    let registry_yaml = std::fs::read_to_string(&registry_path).unwrap_or_default();
    let registered_env_vars = load_registered_env_vars(&registry_yaml);

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
        violations.extend(check_env_reads_registered(src, &rel, &registered_env_vars));
    });
    if violations.is_empty() {
        println!("config-hygiene OK: no violations");
    } else {
        for v in &violations {
            eprintln!("[{}] {}:{} — {}", v.check, v.file, v.line, v.message);
        }
    }

    // Advisory check C: declared-but-unwired (non-blocking)
    let mut advisories = Vec::new();
    collect_rs_files(&root.join("crates"), &mut |path, src| {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        if rel.contains("config_hygiene") {
            return;
        }
        advisories.extend(check_declared_but_unwired(src, &rel));
    });
    if !advisories.is_empty() {
        eprintln!("config-hygiene advisory (non-blocking):");
        for v in &advisories {
            eprintln!("  [{}] {}:{} — {}", v.check, v.file, v.line, v.message);
        }
    }

    if !violations.is_empty() {
        anyhow::bail!("config-hygiene found {} violation(s)", violations.len());
    }
    Ok(())
}

/// Load registered env vars from the config registry YAML.
/// Returns a set of env_var names that are registered (non-null).
pub fn load_registered_env_vars(registry_yaml: &str) -> std::collections::HashSet<String> {
    let mut vars = std::collections::HashSet::new();
    for line in registry_yaml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("env_var: ") {
            let v = rest.trim().trim_matches('"');
            if v != "null" && !v.is_empty() {
                vars.insert(v.to_string());
            }
        }
    }
    vars
}

/// Check D: scan source for VOX_* env reads that are NOT in the registry.
/// Only flags lines containing `env::var` or `env_var` calls (not consts/docs).
pub fn check_env_reads_registered(
    source: &str,
    file: &str,
    registered: &std::collections::HashSet<String>,
) -> Vec<Violation> {
    let re = regex::Regex::new(r#"["']?(VOX_[A-Z0-9_]+)["']?"#).unwrap();
    let mut hits = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        if !raw.contains("env::var") && !raw.contains("env_var") {
            continue;
        }
        for cap in re.captures_iter(raw) {
            let var_name = &cap[1];
            if !registered.contains(var_name) {
                hits.push(Violation {
                    check: "env-var-not-in-registry",
                    file: file.to_string(),
                    line: i + 1,
                    message: format!(
                        "VOX_* env var `{var_name}` is not in contracts/config/registry.v1.yaml \
                         — add an entry with status: active or declared"
                    ),
                });
            }
        }
    }
    hits
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
    fn check_c_flags_declared_but_unwired_resolver() {
        let src = "    pub fn from_env_values(daily: Option<&str>) -> Self { todo!() }";
        let v = check_declared_but_unwired(src, "crates/vox-scaling-policy/src/cost_defense.rs");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].check, "declared-but-unwired-advisory");
    }

    #[test]
    fn allows_include_str_and_comments() {
        let ok = r#"const E: &str = include_str!("../../../contracts/gamify/economy.v1.yaml");"#;
        assert!(check_no_cwd_relative_contract_paths(ok, "x.rs").is_empty());
        let comment = r#"// loads contracts/gamify/economy.v1.yaml at build time"#;
        assert!(check_no_cwd_relative_contract_paths(comment, "x.rs").is_empty());
    }

    #[test]
    fn baseline_suppresses_grandfathered_only() {
        let grand = Violation {
            check: "no-cwd-relative-contract-path",
            file: "crates/a.rs".into(),
            line: 5,
            message: "x".into(),
        };
        let fresh = Violation {
            check: "no-cwd-relative-contract-path",
            file: "crates/b.rs".into(),
            line: 9,
            message: "x".into(),
        };
        let mut base = std::collections::BTreeSet::new();
        base.insert(baseline_key(&grand));
        let all = [grand.clone(), fresh.clone()];
        let news = unbaselined(&all, &base);
        assert_eq!(news.len(), 1);
        assert_eq!(news[0].file, "crates/b.rs");
    }

    #[test]
    fn flags_env_read_in_protected_module() {
        let src = "let n = std::env::var(\"VOX_NONCE_LEN\").unwrap();";
        let v = check_protected_modules_have_no_env_reads(src, "crates/vox-crypto/src/aead.rs");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].check, "protected-module-no-env");
    }
    #[test]
    fn allows_env_read_in_normal_module() {
        let src = "let n = std::env::var(\"VOX_RAG_CHUNK\").unwrap();";
        assert!(
            check_protected_modules_have_no_env_reads(src, "crates/vox-search/src/ingest.rs")
                .is_empty()
        );
    }

    #[test]
    fn registered_env_vars_parsed_from_yaml() {
        let yaml = "env_var: VOX_WASM_SKILL_FUEL\nenv_var: null\nenv_var: VOX_MENS_DEFAULT_MODEL";
        let vars = load_registered_env_vars(yaml);
        assert!(vars.contains("VOX_WASM_SKILL_FUEL"));
        assert!(vars.contains("VOX_MENS_DEFAULT_MODEL"));
        assert!(!vars.contains("null"));
    }

    #[test]
    fn unregistered_env_read_is_flagged() {
        let registered = std::collections::HashSet::new(); // empty
        let src = r#"let v = std::env::var("VOX_UNREGISTERED_KNOB").ok();"#;
        let hits = check_env_reads_registered(src, "x.rs", &registered);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].check, "env-var-not-in-registry");
    }

    #[test]
    fn registered_env_read_is_not_flagged() {
        let mut registered = std::collections::HashSet::new();
        registered.insert("VOX_WASM_SKILL_FUEL".to_string());
        let src = r#"let v = std::env::var("VOX_WASM_SKILL_FUEL").ok();"#;
        let hits = check_env_reads_registered(src, "x.rs", &registered);
        assert!(hits.is_empty());
    }

    #[test]
    fn flags_resolver_with_no_caller() {
        let mut refs = std::collections::HashMap::new();
        refs.insert("resolve_orphan".to_string(), 0usize);
        refs.insert("resolve_wired".to_string(), 3usize);
        let defined = vec![
            (
                "resolve_orphan".to_string(),
                "crates/x/src/a.rs".to_string(),
                10usize,
            ),
            (
                "resolve_wired".to_string(),
                "crates/x/src/b.rs".to_string(),
                20usize,
            ),
        ];
        let v = check_unwired_config(&defined, &refs);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("resolve_orphan"));
    }
}
