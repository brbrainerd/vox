use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const CRATE_GRAPH_SENTINEL: &str = "contracts/ci/crate-graph.v1.json";

pub const SENTINEL_EXACT: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".config/hakari.toml",
    CRATE_GRAPH_SENTINEL,
];
pub const SENTINEL_PREFIX: &[&str] = &[".cargo/", "crates/workspace-hack/"];

pub const COMPILER_CRATES: &[&str] = &["vox-compiler", "vox-codegen", "vox-integration-tests"];

pub fn is_sentinel(path: &str) -> bool {
    SENTINEL_EXACT.contains(&path) || SENTINEL_PREFIX.iter().any(|p| path.starts_with(p))
}

/// Non-graph `contracts/**` edits can change SSOT surfaces workspace-wide; force a full PR gate.
pub fn contracts_outside_graph_force_full(changed_files: &[String]) -> bool {
    changed_files
        .iter()
        .any(|f| f.starts_with("contracts/") && f != CRATE_GRAPH_SENTINEL)
}

/// CI workflow edits can change gate behavior workspace-wide; force a full PR gate.
pub fn ci_workflow_force_full(changed_files: &[String]) -> bool {
    changed_files
        .iter()
        .any(|f| f.starts_with(".github/workflows/"))
}

pub fn file_to_crate(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("crates/")?;
    let name = rest.split('/').next()?;
    if name.is_empty() { None } else { Some(name) }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathFlags {
    pub affects_golden: bool,
    pub affects_contracts: bool,
    pub affects_scripts: bool,
    pub affects_gui: bool,
    pub affects_web: bool,
    pub affects_plugins: bool,
}

pub fn compute_path_flags(changed_files: &[String]) -> PathFlags {
    let mut flags = PathFlags::default();
    for f in changed_files {
        if f.starts_with("examples/golden/") {
            flags.affects_golden = true;
        }
        if f.starts_with("contracts/") {
            flags.affects_contracts = true;
        }
        if f.starts_with("scripts/") {
            flags.affects_scripts = true;
        }
        if f.starts_with("crates/vox-gui/") || f.starts_with("apps/editor/vox-vscode/") {
            flags.affects_gui = true;
        }
        if f.starts_with("crates/vox-integration-tests/")
            || f.starts_with("crates/vox-compiler/src/codegen_ts/")
            || f.starts_with("examples/golden-ts/")
            || f.starts_with("apps/experimental/visualizer/")
        {
            flags.affects_web = true;
        }
        if f == "examples/mesh-compose.yml" {
            flags.affects_scripts = true;
        }
        if f.starts_with("crates/vox-plugin-") {
            flags.affects_plugins = true;
        }
    }
    flags
}

pub fn invert(graph: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, BTreeSet<String>> {
    let mut rev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (krate, deps) in graph {
        rev.entry(krate.clone()).or_default();
        for dep in deps {
            rev.entry(dep.clone()).or_default().insert(krate.clone());
        }
    }
    rev
}

pub fn reverse_closure(
    graph: &BTreeMap<String, Vec<String>>,
    seeds: &BTreeSet<String>,
) -> BTreeSet<String> {
    let rev = invert(graph);
    let mut out = BTreeSet::new();
    let mut q: VecDeque<String> = seeds.iter().cloned().collect();
    while let Some(c) = q.pop_front() {
        if !out.insert(c.clone()) {
            continue;
        }
        if let Some(deps) = rev.get(&c) {
            for d in deps {
                if !out.contains(d) {
                    q.push_back(d.clone());
                }
            }
        }
    }
    out
}

pub fn set_includes_compiler(set: &BTreeSet<String>) -> bool {
    set.iter().any(|c| COMPILER_CRATES.contains(&c.as_str()))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Affected {
    Full,
    None,
    Crates(BTreeSet<String>),
}

pub fn compute_affected(
    changed_files: &[String],
    graph: &BTreeMap<String, Vec<String>>,
) -> Affected {
    if changed_files.iter().any(|f| is_sentinel(f)) {
        return Affected::Full;
    }
    if contracts_outside_graph_force_full(changed_files) {
        return Affected::Full;
    }
    if ci_workflow_force_full(changed_files) {
        return Affected::Full;
    }
    let seeds: BTreeSet<String> = changed_files
        .iter()
        .filter_map(|f| file_to_crate(f))
        .map(String::from)
        .collect();
    if seeds.is_empty() {
        return Affected::None;
    }
    Affected::Crates(reverse_closure(graph, &seeds))
}

/// Parse nextest JUnit for failing tests; return crate names not in `affected`.
pub fn shadow_misses(junit_xml: &str, affected: &BTreeSet<String>) -> Vec<String> {
    if affected.is_empty() {
        return Vec::new();
    }
    let mut failing_crates: BTreeSet<String> = BTreeSet::new();
    for line in junit_xml.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("<failure") && !trimmed.contains("<error") {
            continue;
        }
        if let Some(crate_name) = extract_crate_from_junit_line(trimmed) {
            failing_crates.insert(crate_name);
        }
    }
    // Also scan testcase lines with failures in multi-line elements
    for cap in junit_xml.split("<testcase ").skip(1) {
        let has_fail = cap.contains("<failure") || cap.contains("<error");
        if !has_fail {
            continue;
        }
        if let Some(name) = cap
            .split('"')
            .nth(1)
            .and_then(|classname| crate_from_junit_classname(classname))
        {
            failing_crates.insert(name);
        }
    }
    failing_crates
        .into_iter()
        .filter(|c| !affected.contains(c))
        .collect()
}

fn extract_crate_from_junit_line(line: &str) -> Option<String> {
    if let Some(idx) = line.find("classname=\"") {
        let rest = &line[idx + "classname=\"".len()..];
        let classname = rest.split('"').next()?;
        return crate_from_junit_classname(classname);
    }
    None
}

fn crate_from_junit_classname(classname: &str) -> Option<String> {
    // nextest: `crate_name::module::test` or `crate_name::test`
    let head = classname.split("::").next()?;
    if head.is_empty() || head.contains('.') {
        return None;
    }
    Some(head.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_toml_sentinel() {
        assert!(is_sentinel("Cargo.toml"));
    }

    #[test]
    fn crate_toml_not_sentinel() {
        assert!(!is_sentinel("crates/vox-gui/Cargo.toml"));
    }

    #[test]
    fn cargo_config_sentinel() {
        assert!(is_sentinel(".cargo/config.toml"));
    }

    #[test]
    fn lockfile_sentinel() {
        assert!(is_sentinel("Cargo.lock"));
    }

    #[test]
    fn workspace_hack_sentinel() {
        assert!(is_sentinel("crates/workspace-hack/Cargo.toml"));
    }

    #[test]
    fn crate_graph_file_sentinel() {
        assert!(is_sentinel(CRATE_GRAPH_SENTINEL));
    }

    #[test]
    fn normal_file_not_sentinel() {
        assert!(!is_sentinel("crates/vox-gui/src/App.tsx"));
    }

    #[test]
    fn maps_crate_src() {
        assert_eq!(file_to_crate("crates/vox-db/src/lib.rs"), Some("vox-db"));
    }

    #[test]
    fn non_crate_none() {
        assert_eq!(file_to_crate("docs/foo.md"), Option::None);
    }

    #[test]
    fn golden_path_sets_flag() {
        let flags = compute_path_flags(&["examples/golden/foo.vox".into()]);
        assert!(flags.affects_golden);
    }

    #[test]
    fn contracts_outside_graph_force_full_flag() {
        assert!(super::contracts_outside_graph_force_full(&[
            "contracts/db/foo.v1.yaml".into()
        ]));
        assert!(!super::contracts_outside_graph_force_full(&[
            CRATE_GRAPH_SENTINEL.into()
        ]));
    }

    #[test]
    fn contracts_outside_graph_in_compute_affected() {
        assert_eq!(
            compute_affected(
                &["contracts/config/env-vars.v1.yaml".into()],
                &BTreeMap::new()
            ),
            Affected::Full
        );
    }

    #[test]
    fn ci_workflow_force_full_flag() {
        assert!(ci_workflow_force_full(&[".github/workflows/ci.yml".into()]));
        assert!(!ci_workflow_force_full(&[
            "docs/ci/runner-contract.md".into()
        ]));
    }

    #[test]
    fn ci_workflow_in_compute_affected() {
        assert_eq!(
            compute_affected(&[".github/workflows/ci.yml".into()], &BTreeMap::new()),
            Affected::Full
        );
    }

    #[test]
    fn golden_only_none_affected() {
        assert_eq!(
            compute_affected(&["examples/golden/foo.vox".into()], &BTreeMap::new()),
            Affected::None
        );
    }

    #[test]
    fn set_includes_compiler_membership() {
        let mut set = BTreeSet::from(["vox-db".into()]);
        assert!(!set_includes_compiler(&set));
        set.insert("vox-compiler".into());
        assert!(set_includes_compiler(&set));
    }

    #[test]
    fn unknown_seed_still_in_closure() {
        let g = BTreeMap::from([("vox-new".into(), vec![])]);
        let aff = compute_affected(&["crates/vox-new/src/lib.rs".into()], &g);
        assert_eq!(aff, Affected::Crates(BTreeSet::from(["vox-new".into()])));
    }

    fn g() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([
            ("a".into(), vec!["b".into()]),
            ("b".into(), vec!["c".into()]),
            ("c".into(), vec![]),
            ("d".into(), vec!["b".into()]),
        ])
    }

    #[test]
    fn leaf_only_itself() {
        assert_eq!(
            reverse_closure(&g(), &BTreeSet::from(["a".to_string()])),
            BTreeSet::from(["a".to_string()])
        );
    }

    #[test]
    fn base_pulls_all() {
        assert_eq!(
            reverse_closure(&g(), &BTreeSet::from(["c".to_string()])),
            BTreeSet::from(["a".into(), "b".into(), "c".into(), "d".into()])
        );
    }

    #[test]
    fn sentinel_forces_full() {
        assert_eq!(
            compute_affected(&["Cargo.lock".into()], &g()),
            Affected::Full
        );
    }

    #[test]
    fn docs_only_none() {
        assert_eq!(
            compute_affected(&["docs/x.md".into()], &BTreeMap::new()),
            Affected::None
        );
    }

    #[test]
    fn shadow_miss_detects_failing_crate_outside_affected() {
        let junit = r#"
        <testcase classname="vox-compiler::tests::foo" name="bar">
          <failure message="boom"/>
        </testcase>
        "#;
        let affected = BTreeSet::from(["vox-db".into()]);
        let misses = shadow_misses(junit, &affected);
        assert_eq!(misses, vec!["vox-compiler"]);
    }
}
