//! Versioned YAML semantic rules + optional `cargo metadata` workspace crate injection.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

/// Bundled default (repo root `contracts/review/coderabbit-semantic-groups.v1.yaml`).
const BUNDLED_GROUPS_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/review/coderabbit-semantic-groups.v1.yaml"
));

#[derive(Debug, Clone)]
pub enum RuleMatcher {
    AnyPrefix(Vec<String>),
    RootFilesOnly,
}

#[derive(Debug, Clone)]
pub struct SemanticRule {
    pub order: u32,
    pub name: String,
    pub matcher: RuleMatcher,
}

#[derive(Debug, Clone)]
pub struct SemanticRuleSet {
    pub rules: Vec<SemanticRule>,
    pub unassigned_order: u32,
    pub unassigned_name: String,
}

impl SemanticRuleSet {
    /// First matching rule wins (stable list order).
    pub fn group_for(&self, path: &str) -> (u32, String) {
        let p = path.replace('\\', "/");
        for r in &self.rules {
            if rule_matches(&r.matcher, &p) {
                return (r.order, r.name.clone());
            }
        }
        (self.unassigned_order, self.unassigned_name.clone())
    }
}

fn rule_matches(m: &RuleMatcher, path: &str) -> bool {
    match m {
        RuleMatcher::AnyPrefix(prefixes) => prefixes.iter().any(|pre| path.starts_with(pre)),
        RuleMatcher::RootFilesOnly => !path.contains('/'),
    }
}

#[derive(Debug, Deserialize)]
struct GroupsFileV1 {
    version: u32,
    unassigned: UnassignedV1,
    workspace_crates: WorkspaceCratesV1,
    prefix_rules: Vec<RuleEntryV1>,
    suffix_rules: Vec<RuleEntryV1>,
}

#[derive(Debug, Deserialize)]
struct UnassignedV1 {
    order: u32,
    name: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceCratesV1 {
    enabled: bool,
    rule_order: u32,
    name_prefix: String,
}

#[derive(Debug, Deserialize)]
struct RuleEntryV1 {
    order: u32,
    name: String,
    any_prefix: Option<Vec<String>>,
    root_files_only: Option<bool>,
}

impl RuleEntryV1 {
    fn into_rule(self) -> Result<SemanticRule> {
        let root = self.root_files_only.unwrap_or(false);
        let has_prefix = self.any_prefix.as_ref().map_or(false, |p| !p.is_empty());
        match (root, has_prefix) {
            (true, false) => Ok(SemanticRule {
                order: self.order,
                name: self.name,
                matcher: RuleMatcher::RootFilesOnly,
            }),
            (false, true) => {
                let prefs: Vec<String> = self
                    .any_prefix
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| s.replace('\\', "/"))
                    .collect();
                Ok(SemanticRule {
                    order: self.order,
                    name: self.name,
                    matcher: RuleMatcher::AnyPrefix(prefs),
                })
            }
            (true, true) => anyhow::bail!(
                "semantic group rule `{}`: root_files_only and any_prefix are mutually exclusive",
                self.name
            ),
            (false, false) => anyhow::bail!(
                "semantic group rule `{}`: need any_prefix or root_files_only",
                self.name
            ),
        }
    }
}

fn parse_groups_yaml(text: &str) -> Result<GroupsFileV1> {
    let g: GroupsFileV1 =
        serde_yaml::from_str(text).context("parse coderabbit semantic groups YAML")?;
    if g.version != 1 {
        anyhow::bail!(
            "unsupported coderabbit semantic groups version {}",
            g.version
        );
    }
    Ok(g)
}

fn entry_rules(entries: Vec<RuleEntryV1>) -> Result<Vec<SemanticRule>> {
    entries.into_iter().map(|e| e.into_rule()).collect()
}

/// Load rules from an explicit file path (absolute or relative to `repo_root`).
pub fn load_rules_from_path(
    repo_root: &Path,
    config_path: &Path,
    inject_workspace_crates: bool,
) -> Result<SemanticRuleSet> {
    let path = if config_path.is_absolute() {
        config_path.to_path_buf()
    } else {
        repo_root.join(config_path)
    };
    let text = read_yaml_file(&path)
        .with_context(|| format!("read semantic groups {}", path.display()))?;
    build_rule_set_from_yaml_text(&text, repo_root, inject_workspace_crates)
}

/// `inject_workspace_crates`: `yaml.workspace_crates.enabled &&` operator flag from `Vox.toml`.
pub fn build_rule_set_from_yaml_text(
    text: &str,
    repo_root: &Path,
    inject_workspace_crates: bool,
) -> Result<SemanticRuleSet> {
    let g = parse_groups_yaml(text)?;
    let mut rules: Vec<SemanticRule> = Vec::new();
    rules.extend(entry_rules(g.prefix_rules)?);
    if inject_workspace_crates && g.workspace_crates.enabled {
        match discover_workspace_crate_rules(repo_root, &g.workspace_crates) {
            Ok(ws) => rules.extend(ws),
            Err(e) => {
                eprintln!(
                    "[semantic-groups] warning: workspace crate discovery failed ({}); continuing without injected crate rules.",
                    e
                );
            }
        }
    }
    rules.extend(entry_rules(g.suffix_rules)?);
    Ok(SemanticRuleSet {
        rules,
        unassigned_order: g.unassigned.order,
        unassigned_name: g.unassigned.name,
    })
}

fn read_yaml_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

/// Bundled defaults + workspace injection when enabled in YAML and `inject_workspace_crates`.
pub fn load_bundled_rule_set(
    repo_root: &Path,
    inject_workspace_crates: bool,
) -> Result<SemanticRuleSet> {
    build_rule_set_from_yaml_text(BUNDLED_GROUPS_YAML, repo_root, inject_workspace_crates)
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    id: String,
    manifest_path: String,
}

fn discover_workspace_crate_rules(
    repo_root: &Path,
    ws: &WorkspaceCratesV1,
) -> Result<Vec<SemanticRule>> {
    let cargo_exe = std::env::var_os("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"));
    let out = Command::new(&cargo_exe)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(repo_root)
        .output()
        .context("spawn cargo metadata")?;
    if !out.status.success() {
        anyhow::bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let meta: CargoMetadata =
        serde_json::from_slice(&out.stdout).context("parse cargo metadata JSON")?;
    let members: HashSet<&str> = meta.workspace_members.iter().map(|s| s.as_str()).collect();
    let repo_abs = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());

    let mut rules: Vec<SemanticRule> = Vec::new();
    for pkg in meta.packages {
        if !members.contains(pkg.id.as_str()) {
            continue;
        }
        let man = PathBuf::from(&pkg.manifest_path);
        let man_abs = man.canonicalize().unwrap_or(man);
        let Some(parent) = man_abs.parent() else {
            continue;
        };
        let Ok(rel_dir) = parent.strip_prefix(&repo_abs) else {
            continue;
        };
        let mut prefix = rel_dir
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/");
        if !prefix.is_empty() {
            prefix.push('/');
        }
        if !prefix.starts_with("crates/") {
            continue;
        }
        let safe = pkg.name.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
        let name = format!("{}{}", ws.name_prefix, safe);
        rules.push(SemanticRule {
            order: ws.rule_order,
            name,
            matcher: RuleMatcher::AnyPrefix(vec![prefix]),
        });
    }
    rules.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rules)
}

/// Optional `groups_config` replaces the bundled YAML; workspace injection obeys the loaded file's
/// `workspace_crates.enabled` and `inject_workspace_crates` from `Vox.toml`.
pub fn resolve_semantic_rule_set(
    repo_root: &Path,
    groups_config: Option<&Path>,
    inject_workspace_crates: bool,
) -> Result<SemanticRuleSet> {
    match groups_config {
        Some(p) => load_rules_from_path(repo_root, p, inject_workspace_crates),
        None => load_bundled_rule_set(repo_root, inject_workspace_crates),
    }
}

/// Pack oversized groups: prefix clusters (`crates/name` or first path segment), greedy bins,
/// subdivide huge clusters by 3rd segment then alphabetical chunks.
pub fn pack_oversized_files(
    files: Vec<String>,
    max_files: usize,
    legacy_alphabetical: bool,
) -> Vec<Vec<String>> {
    if legacy_alphabetical {
        let mut v = files;
        v.sort();
        return v.chunks(max_files.max(1)).map(|c| c.to_vec()).collect();
    }
    prefix_cluster_pack(files, max_files.max(1))
}

fn cluster_key_two(path: &str) -> String {
    let p = path.replace('\\', "/");
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 2 && parts[0] == "crates" {
        format!("{}/{}", parts[0], parts[1])
    } else if let Some(first) = parts.first() {
        (*first).to_string()
    } else {
        ".".to_string()
    }
}

fn cluster_key_three(path: &str) -> String {
    let p = path.replace('\\', "/");
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    parts.iter().take(3).copied().collect::<Vec<_>>().join("/")
}

fn subdivide_large_cluster(files: Vec<String>, max: usize) -> Vec<Vec<String>> {
    let mut sub: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in files {
        let k = cluster_key_three(&f);
        sub.entry(k).or_default().push(f);
    }
    let mut out: Vec<Vec<String>> = Vec::new();
    for mut v in sub.into_values() {
        v.sort();
        if v.len() <= max {
            out.push(v);
        } else {
            for chunk in v.chunks(max) {
                out.push(chunk.to_vec());
            }
        }
    }
    out
}

fn prefix_cluster_pack(files: Vec<String>, max: usize) -> Vec<Vec<String>> {
    if files.is_empty() {
        return vec![];
    }
    if files.len() <= max {
        return vec![files];
    }
    let mut clusters: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in files {
        let k = cluster_key_two(&f);
        clusters.entry(k).or_default().push(f);
    }
    let mut cluster_groups: Vec<Vec<String>> = Vec::new();
    for mut cfiles in clusters.into_values() {
        cfiles.sort();
        if cfiles.len() > max {
            cluster_groups.extend(subdivide_large_cluster(cfiles, max));
        } else {
            cluster_groups.push(cfiles);
        }
    }
    cluster_groups.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then_with(|| a.first().cmp(&b.first()))
    });
    let mut bins: Vec<Vec<String>> = Vec::new();
    for chunk in cluster_groups {
        if chunk.is_empty() {
            continue;
        }
        if bins.is_empty() {
            bins.push(chunk);
            continue;
        }
        let last_len = bins.last().map(|b| b.len()).unwrap_or(0);
        if last_len + chunk.len() <= max {
            bins.last_mut().unwrap().extend(chunk);
        } else {
            bins.push(chunk);
        }
    }
    bins
}

/// Top-level path prefixes for unassigned paths (first segment or `crates/name`).
pub fn unassigned_prefix_histogram(
    paths: &[String],
    rule_set: &SemanticRuleSet,
) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for p in paths {
        if rule_set.group_for(p).1 != rule_set.unassigned_name {
            continue;
        }
        let k = cluster_key_two(p);
        *counts.entry(k).or_insert(0) += 1;
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_yaml_parses() {
        let dir = tempfile::tempdir().unwrap();
        let set = load_bundled_rule_set(dir.path(), false).expect("bundled");
        assert!(!set.rules.is_empty());
        assert_eq!(set.unassigned_name, "99_unassigned");
    }

    #[test]
    fn group_non_crate_matches_prefix_rules() {
        let dir = tempfile::tempdir().unwrap();
        let set = load_bundled_rule_set(dir.path(), false).expect("bundled");
        let (_, n) = set.group_for("contracts/foo.json");
        assert_eq!(n, "05_contracts");
        let (_, n2) = set.group_for(".github/workflows/ci.yml");
        assert_eq!(n2, "02_github_agents");
    }

    #[test]
    fn pack_prefers_crate_locality() {
        let files = vec![
            "crates/a/x.rs".to_string(),
            "crates/a/y.rs".to_string(),
            "crates/b/z.rs".to_string(),
        ];
        let bins = pack_oversized_files(files, 2, false);
        assert!(
            bins.iter()
                .any(|b| b.len() == 2 && b[0].starts_with("crates/a"))
        );
    }
}
