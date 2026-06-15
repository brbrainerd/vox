//! `vox ci config-hygiene`: machine guardrails that keep config single-homed,
//! safe-by-default, and never silently inert. Run BEFORE the configurability plan.

use std::collections::BTreeSet;
use std::path::Path;

/// A single hygiene violation (file:line + message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub check: &'static str,
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Baseline key for a violation: `check|file` (coarse-but-robust ratchet — new
/// files/crates introducing the anti-pattern fail; pre-existing dirty files are
/// grandfathered until the backlog is burned down).
pub fn baseline_key(v: &Violation) -> String {
    format!("{}|{}", v.check, v.file.replace('\\', "/"))
}

/// Return only the violations whose key is NOT in the baseline (the NEW ones).
pub fn unbaselined<'a>(
    violations: &'a [Violation],
    baseline: &BTreeSet<String>,
) -> Vec<&'a Violation> {
    violations
        .iter()
        .filter(|v| !baseline.contains(&baseline_key(v)))
        .collect()
}

/// Path to the grandfathered-violations baseline, relative to repo root.
const BASELINE_REL_PATH: &str = "contracts/config/config-hygiene-baseline.txt";

/// Load the baseline keys from disk. Non-empty, non-`#` lines are keys. A
/// missing file yields an empty set (gate then fails on every violation).
fn load_baseline(root: &Path) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let path = root.join(BASELINE_REL_PATH);
    if let Ok(text) = std::fs::read_to_string(&path) {
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            set.insert(line.to_string());
        }
    }
    set
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

/// Crates/paths whose constants are protocol-, format-, crypto-, grammar-, or
/// calibration-fixed: configurability is an explicit NON-GOAL. Reading env here
/// is forbidden. The structural form of "unless it never needs configuring".
pub const PROTECTED_PATH_FRAGMENTS: &[&str] = &[
    "crates/vox-crypto/",
    "crates/vox-wire-format-validator/",
    "crates/vox-grammar-export/",
    "crates/vox-ast/",
    "crates/vox-populi/src/mens/tensor/memory_budget.rs",
];

/// Check B: no `std::env::var` reads inside protected never-configure modules.
pub fn check_protected_modules_have_no_env_reads(source: &str, file: &str) -> Vec<Violation> {
    let norm = file.replace('\\', "/");
    if !PROTECTED_PATH_FRAGMENTS.iter().any(|p| norm.contains(p)) {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim_start();
        if line.starts_with("//") {
            continue;
        }
        if raw.contains("std::env::var") || raw.contains("env::var(") {
            hits.push(Violation {
                check: "protected-module-no-env",
                file: file.to_string(),
                line: i + 1,
                message: "protected never-configure module must not read env; if this value truly \
                          needs configuring, move it out of the protected path and register it"
                    .to_string(),
            });
        }
    }
    hits
}

use std::collections::HashMap;

/// Check C (pure core): given config-resolver definitions and a map of how many
/// NON-test references each symbol has, flag any with zero references (dead config).
pub fn check_unwired_config(
    defined: &[(String, String, usize)],
    ref_counts: &HashMap<String, usize>,
) -> Vec<Violation> {
    defined
        .iter()
        .filter(|(sym, _, _)| ref_counts.get(sym).copied().unwrap_or(0) == 0)
        .map(|(sym, file, line)| Violation {
            check: "declared-but-unwired-config",
            file: file.clone(),
            line: *line,
            message: format!(
                "config resolver `{sym}` has no non-test caller — wire it or delete it (YAGNI)"
            ),
        })
        .collect()
}

/// Find `pub fn resolve_<x>` / `pub fn <x>_from_env` definitions: (symbol, file, line).
fn collect_resolver_defs(root: &Path) -> Vec<(String, String, usize)> {
    let re = regex::Regex::new(r"pub fn (resolve_[a-z0-9_]+|[a-z0-9_]+_from_env)\b").unwrap();
    let mut defs = Vec::new();
    collect_rs_files(&root.join("crates"), &mut |path, src| {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        if rel.contains("config_hygiene.rs") {
            return;
        }
        for (i, line) in src.lines().enumerate() {
            if let Some(c) = re.captures(line) {
                defs.push((c[1].to_string(), rel.clone(), i + 1));
            }
        }
    });
    defs
}

/// Count references to each known symbol across the tree. `collect_rs_files`
/// already skips `*_tests.rs`/`tests.rs`, so test-file references don't count.
/// Note: a `#[cfg(test)] mod tests` INSIDE a normal file WILL be counted; that
/// is an acceptable conservative approximation (a resolver referenced only by an
/// in-file test still counts as referenced).
fn count_symbol_refs(root: &Path, symbols: &[String]) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = symbols.iter().map(|s| (s.clone(), 0usize)).collect();
    collect_rs_files(&root.join("crates"), &mut |path, src| {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        if rel.contains("config_hygiene.rs") {
            return;
        }
        for sym in symbols {
            // word-boundary-ish: count occurrences of the bare symbol name
            let n = src.matches(sym.as_str()).count();
            if n > 0 {
                *counts.get_mut(sym).unwrap() += n;
            }
        }
    });
    // Subtract the definition occurrence (each def line contains the symbol once).
    counts
}

/// Run all config-hygiene checks across the workspace.
///
/// Uses a baseline ratchet: violations whose `(check, file)` key is already
/// recorded in `contracts/config/config-hygiene-baseline.txt` are grandfathered,
/// and the gate fails only on NEW (unbaselined) violations. With
/// `update_baseline`, regenerate the baseline file from the current tree.
pub fn run(update_baseline: bool) -> anyhow::Result<()> {
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

    // Check C: two-pass workspace scan — gather resolver definitions, then count
    // references, then flag any with no non-test caller beyond their own def line.
    let defs = collect_resolver_defs(&root);
    let symbols: Vec<String> = defs.iter().map(|(s, _, _)| s.clone()).collect();
    let mut ref_counts = count_symbol_refs(&root, &symbols);
    // A definition's own line counts the symbol once; require a reference BEYOND that.
    for (sym, _, _) in &defs {
        if let Some(c) = ref_counts.get_mut(sym) {
            *c = c.saturating_sub(1);
        }
    }
    violations.extend(check_unwired_config(&defs, &ref_counts));

    if update_baseline {
        let keys: BTreeSet<String> = violations.iter().map(baseline_key).collect();
        let mut out = String::from(
            "# config-hygiene baseline — grandfathered violations; burn down over time. \
             Regenerate: vox ci config-hygiene --update-baseline\n",
        );
        for key in &keys {
            out.push_str(key);
            out.push('\n');
        }
        let path = root.join(BASELINE_REL_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, out)?;
        println!(
            "config-hygiene: wrote {} grandfathered key(s) to {}",
            keys.len(),
            BASELINE_REL_PATH
        );
        return Ok(());
    }

    let baseline = load_baseline(&root);
    let news = unbaselined(&violations, &baseline);
    let grandfathered = violations.len() - news.len();
    if news.is_empty() {
        println!("config-hygiene OK: {grandfathered} grandfathered, 0 new");
        return Ok(());
    }
    for v in &news {
        eprintln!("[{}] {}:{} — {}", v.check, v.file, v.line, v.message);
    }
    anyhow::bail!(
        "config-hygiene found {} NEW violation(s) ({} grandfathered). \
         Fix them, or run `vox ci config-hygiene --update-baseline` to grandfather.",
        news.len(),
        grandfathered
    )
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
