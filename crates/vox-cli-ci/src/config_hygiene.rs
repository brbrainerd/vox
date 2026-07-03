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
    /// For `env-var-not-in-registry` violations, the specific env var name.
    /// Other checks leave this as `None` and use file-level granularity.
    pub env_var: Option<String>,
}

/// Baseline key for a violation.
///
/// - `env-var-not-in-registry`: fine-grained `check|file|env_var` so that a
///   new unregistered var in a pre-existing dirty file is still caught.
/// - All other checks: coarse `check|file` (pre-existing ratchet behaviour).
pub fn baseline_key(v: &Violation) -> String {
    if v.check == "env-var-not-in-registry"
        && let Some(ref var) = v.env_var {
            return format!("{}|{}|{}", v.check, v.file.replace('\\', "/"), var);
        }
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
                env_var: None,
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
                env_var: None,
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
            env_var: None,
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

    // Load registry for Check D (env-var parity).
    let registry_path = root.join("contracts/config/registry.v1.yaml");
    let yaml_vars: std::collections::HashSet<String> = match parse_registry_file(&registry_path) {
        Ok(rows) => rows
            .into_iter()
            .filter(|r| !r.env_var.is_empty() && r.env_var != "null")
            .map(|r| r.env_var)
            .collect(),
        Err(_) => {
            // Registry file absent or unreadable — treat as empty (Check D becomes no-op).
            std::collections::HashSet::new()
        }
    };
    // Union in Clavis-managed secrets so credentials don't need manual YAML rows.
    let mut registered_env_vars = yaml_vars;
    registered_env_vars.extend(
        vox_secrets::spec::managed_secret_env_names()
            .into_iter()
            .map(|s| s.to_string()),
    );

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
        violations.extend(check_env_reads_registered(src, &rel, &registered_env_vars));
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

// ---------------------------------------------------------------------------
// Serde structs for registry.v1.yaml (Phase 1 — replaces fragile line grep)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct RegistryFile {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    knobs: Vec<KnobRow>,
}

/// A single row from the config registry YAML.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct KnobRow {
    pub env_var: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
    /// Present on rows written by `--write`; absent on hand-authored rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_crate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// Parse a registry YAML file. Returns `Err` if the file is malformed — never
/// silently returns an empty set and swallows parse errors.
pub fn parse_registry_file(path: &std::path::Path) -> Result<Vec<KnobRow>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read registry {}: {e}", path.display()))?;
    let file: RegistryFile =
        serde_yaml::from_str(&text).map_err(|e| format!("registry YAML parse error: {e}"))?;
    Ok(file.knobs)
}

/// Load registered env vars from the config registry YAML text.
///
/// Kept for unit-test compatibility (accepts raw YAML text).
/// For production use, prefer [`parse_registry_file`] which uses serde and propagates
/// parse errors instead of silently returning an empty set.
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

// ---------------------------------------------------------------------------
// --write: auto-register / prune registry rows
// ---------------------------------------------------------------------------

/// Options for [`write_registry`].
pub struct WriteRegistryOpts {
    /// Repository root (contains `crates/` and `contracts/config/registry.v1.yaml`).
    pub root: std::path::PathBuf,
}

/// `true` if the env var name looks like a secret credential.
fn infer_secret(name: &str) -> bool {
    let n = name.to_uppercase();
    n.ends_with("_KEY")
        || n.ends_with("_TOKEN")
        || n.ends_with("_SECRET")
        || n.ends_with("_PASSWORD")
        || n.ends_with("_PWD")
        || n.ends_with("_CREDENTIAL")
        || n.ends_with("API_KEY")
}

/// Infer the registry bucket from the env var name.
fn infer_bucket(name: &str) -> &'static str {
    if name.starts_with("VOX_SECRETS_") || name.starts_with("VOX_CLAVIS_") {
        "clavis-selector"
    } else if name.starts_with("VOX_") {
        "vox-knob"
    } else {
        "third-party"
    }
}

/// Convert an env var name (`ALL_CAPS`) to a snake_case registry name.
fn env_var_to_name(env_var: &str) -> String {
    env_var.to_lowercase()
}

/// Infer `owner_crate` from the first source file path that contains the env var.
fn crate_from_path(path: &str) -> String {
    // path form: crates/<crate-name>/src/...
    let norm = path.replace('\\', "/");
    if let Some(after_crates) = norm.strip_prefix("crates/")
        && let Some(slash) = after_crates.find('/') {
            return after_crates[..slash].to_string();
        }
    "unknown".to_string()
}

/// Collect all env var names found in source (excluding allowlist and Clavis-managed).
fn collect_source_env_vars(root: &std::path::Path) -> std::collections::HashMap<String, String> {
    let re = regex::Regex::new(
        r#"(?:env::var(?:_os)?|env_var|env_flag|env_u32|env_i64|env_u64|env_duration|env_truthy)\s*\(\s*["']([A-Z][A-Z0-9_]{2,})["']"#,
    )
    .unwrap();
    let mut found: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    collect_rs_files(&root.join("crates"), &mut |path, src| {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        if rel.replace('\\', "/").contains("config_hygiene.rs") {
            return;
        }
        for line in src.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for cap in re.captures_iter(line) {
                let var = cap[1].to_string();
                if THIRD_PARTY_ALLOWLIST.contains(&var.as_str()) {
                    continue;
                }
                found.entry(var).or_insert_with(|| rel.clone());
            }
        }
    });
    found
}

/// Apply `--write` to `contracts/config/registry.v1.yaml`:
///
/// 1. Append stub rows for env vars found in source but absent from the registry.
/// 2. Remove rows whose `env_var` no longer appears in source, **unless** `status: deprecated`.
/// 3. Idempotent: a second call produces no changes.
pub fn write_registry(opts: WriteRegistryOpts) -> anyhow::Result<()> {
    let registry_path = opts.root.join("contracts/config/registry.v1.yaml");

    // Collect env vars visible in the Clavis-managed set (don't add rows for those).
    let clavis_names: std::collections::HashSet<String> =
        vox_secrets::spec::managed_secret_env_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

    // Read existing registry (or start empty).
    let existing_text = std::fs::read_to_string(&registry_path).unwrap_or_default();
    let mut registry: RegistryFile = if existing_text.trim().is_empty() {
        RegistryFile {
            schema_version: "1".to_string(),
            knobs: Vec::new(),
        }
    } else {
        serde_yaml::from_str(&existing_text)
            .map_err(|e| anyhow::anyhow!("registry YAML parse error: {e}"))?
    };

    // Collect env vars present in source.
    let source_vars = collect_source_env_vars(&opts.root);

    // Build set of env_vars already in registry (non-null, non-empty).
    let registered: std::collections::HashSet<String> = registry
        .knobs
        .iter()
        .filter(|r| !r.env_var.is_empty() && r.env_var != "null")
        .map(|r| r.env_var.clone())
        .collect();

    // 1. Append stub rows for unregistered source vars (skip Clavis-managed).
    for (var, file_path) in &source_vars {
        if registered.contains(var) {
            continue;
        }
        if clavis_names.contains(var.as_str()) {
            continue;
        }
        let stub = KnobRow {
            name: Some(env_var_to_name(var)),
            env_var: var.clone(),
            description: String::new(),
            status: "declared".to_string(),
            secret: Some(infer_secret(var)),
            bucket: Some(infer_bucket(var).to_string()),
            source: Some("env".to_string()),
            owner_crate: Some(crate_from_path(file_path)),
            since: Some("2026-06-15".to_string()),
        };
        registry.knobs.push(stub);
    }

    // 2. Prune orphan rows (env_var not in source, not deprecated, not null).
    registry.knobs.retain(|r| {
        if r.env_var.is_empty() || r.env_var == "null" {
            // null/empty env_var rows are config-only (no env wire) — keep them
            return true;
        }
        if r.status == "deprecated" {
            return true;
        }
        // Keep if still referenced in source OR in Clavis-managed set.
        source_vars.contains_key(&r.env_var) || clavis_names.contains(r.env_var.as_str())
    });

    // Serialize back. Use serde_yaml to produce canonical output.
    let out = serde_yaml::to_string(&registry)
        .map_err(|e| anyhow::anyhow!("registry YAML serialize error: {e}"))?;

    // Only write if changed (idempotency: avoid touching mtime when nothing changed).
    if out != existing_text {
        if let Some(parent) = registry_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&registry_path, &out)?;
        println!(
            "config-hygiene --write: registry updated ({} rows)",
            registry.knobs.len()
        );
    } else {
        println!(
            "config-hygiene --write: no changes ({} rows)",
            registry.knobs.len()
        );
    }

    Ok(())
}

/// Build the full recognized-env-var set for Check D.
///
/// Unions YAML-registry rows with Clavis-managed secret env names so that
/// credentials like `GEMINI_API_KEY` and `VAULT_TOKEN` are auto-recognized
/// without needing manual rows in `contracts/config/registry.v1.yaml`.
pub fn build_recognized_env_vars(registry_yaml: &str) -> std::collections::HashSet<String> {
    let mut recognized = load_registered_env_vars(registry_yaml);
    // Fold in Clavis-managed secrets so credentials don't need manual YAML rows.
    recognized.extend(
        vox_secrets::spec::managed_secret_env_names()
            .into_iter()
            .map(|s| s.to_string()),
    );
    recognized
}

/// OS/toolchain/CI env names that don't need registry rows.
/// These are treated as "registered" for Check D purposes (bucket: third-party).
pub const THIRD_PARTY_ALLOWLIST: &[&str] = &[
    "HOME",
    "PATH",
    "RUST_LOG",
    "RUST_BACKTRACE",
    "OUT_DIR",
    "CARGO_MANIFEST_DIR",
    "CARGO_PKG_VERSION",
    "CARGO_PKG_NAME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "GITHUB_SHA",
    "GITHUB_REF",
    "GITHUB_ACTIONS",
    "TMPDIR",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "LOCALAPPDATA",
];

/// Check D: scan source for env reads (any ALL_CAPS name) that are NOT in the
/// registry or the third-party allowlist.
/// Detects `env::var`, `env::var_os`, and common wrapper helpers
/// (`env_var`, `env_flag`, `env_u32`, `env_i64`, `env_u64`, `env_duration`, `env_truthy`).
pub fn check_env_reads_registered(
    source: &str,
    file: &str,
    registered: &std::collections::HashSet<String>,
) -> Vec<Violation> {
    // Matches the env var name (ALL_CAPS, ≥3 chars) in env read calls.
    let re = regex::Regex::new(
        r#"(?:env::var(?:_os)?|env_var|env_flag|env_u32|env_i64|env_u64|env_duration|env_truthy)\s*\(\s*["']([A-Z][A-Z0-9_]{2,})["']"#,
    )
    .unwrap();
    let mut hits = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        for cap in re.captures_iter(raw) {
            let var_name = &cap[1];
            if registered.contains(var_name) {
                continue;
            }
            if THIRD_PARTY_ALLOWLIST.contains(&var_name) {
                continue;
            }
            hits.push(Violation {
                check: "env-var-not-in-registry",
                file: file.to_string(),
                line: i + 1,
                message: format!(
                    "env var `{var_name}` is not in contracts/config/registry.v1.yaml \
                     — add an entry with status: active or declared, or add to \
                     THIRD_PARTY_ALLOWLIST if it is an OS/toolchain/CI name"
                ),
                env_var: Some(var_name.to_string()),
            });
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
            env_var: None,
        };
        let fresh = Violation {
            check: "no-cwd-relative-contract-path",
            file: "crates/b.rs".into(),
            line: 9,
            message: "x".into(),
            env_var: None,
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
