use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const SENTINEL_EXACT: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".config/hakari.toml",
];
pub const SENTINEL_PREFIX: &[&str] = &[".cargo/", "crates/workspace-hack/"];

pub fn is_sentinel(path: &str) -> bool {
    SENTINEL_EXACT.contains(&path) || SENTINEL_PREFIX.iter().any(|p| path.starts_with(p))
}

pub fn file_to_crate(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("crates/")?;
    let name = rest.split('/').next()?;
    if name.is_empty() { None } else { Some(name) }
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
}
