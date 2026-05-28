//! Minimal parser for `docs/src/architecture/layers.toml` — only the fields
//! drift-check rules consume. We deliberately don't reach for vox-arch-check's
//! richer schema because the dependency direction would invert (arch-check is
//! a CLI; drift-check is a library/CLI pair). Keep this aligned with the
//! arch-check schema on a need-to-know basis.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Subset of a `[crates.<name>]` entry we care about.
#[derive(Debug, Default, Deserialize, Clone)]
struct CrateEntry {
    /// Other crates this one is structurally a sibling of. Used by
    /// `sweep/duplicate-body` to permit intentional code duplication across
    /// vendor splits (CUDA/Metal), extraction migrations (oratio →
    /// plugin-oratio), and similar declared clusters.
    ///
    /// Edges are treated as undirected: declaring `A.sibling_of = ["B"]`
    /// implicitly puts B and A in the same cluster.
    #[serde(default)]
    sibling_of: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawLayersFile {
    #[serde(default)]
    crates: HashMap<String, CrateEntry>,
}

/// In-memory view of layers.toml with sibling clusters resolved.
#[derive(Debug, Default, Clone)]
pub struct LayersManifest {
    /// crate_name → set of other crates in the same sibling cluster (transitive
    /// closure of declared edges). A crate not appearing here has no siblings.
    sibling_clusters: HashMap<String, HashSet<String>>,
}

impl LayersManifest {
    /// Load from `<workspace_root>/docs/src/architecture/layers.toml`. Returns
    /// an empty manifest if the file is missing or malformed (callers degrade
    /// to "no metadata available", which is the safe default).
    pub fn load(workspace_root: &Path) -> Self {
        let path = workspace_root
            .join("docs")
            .join("src")
            .join("architecture")
            .join("layers.toml");
        Self::load_from_file(&path)
    }

    pub fn load_from_file(path: &Path) -> Self {
        let Ok(body) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&body)
    }

    fn parse(body: &str) -> Self {
        let raw: RawLayersFile = match toml::from_str(body) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };
        // Build undirected adjacency: declared edge `A→B` adds both A↔B and B↔A.
        let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
        for (name, entry) in &raw.crates {
            for sib in &entry.sibling_of {
                if sib == name {
                    continue;
                }
                adj.entry(name.clone()).or_default().insert(sib.clone());
                adj.entry(sib.clone()).or_default().insert(name.clone());
            }
        }
        // BFS for connected components → each member gets the full cluster (minus self).
        let mut sibling_clusters: HashMap<String, HashSet<String>> = HashMap::new();
        let mut visited: HashSet<String> = HashSet::new();
        for start in adj.keys() {
            if visited.contains(start) {
                continue;
            }
            let mut component: HashSet<String> = HashSet::new();
            let mut stack = vec![start.clone()];
            while let Some(node) = stack.pop() {
                if !visited.insert(node.clone()) {
                    continue;
                }
                component.insert(node.clone());
                if let Some(neighbours) = adj.get(&node) {
                    for n in neighbours {
                        if !visited.contains(n) {
                            stack.push(n.clone());
                        }
                    }
                }
            }
            for member in &component {
                let mut peers = component.clone();
                peers.remove(member);
                sibling_clusters.insert(member.clone(), peers);
            }
        }
        Self { sibling_clusters }
    }

    /// True when every crate name in `crates` is in the same sibling cluster.
    /// Used by `sweep/duplicate-body` to suppress findings whose locations are
    /// confined to a declared cluster.
    pub fn all_in_one_sibling_cluster<I, S>(&self, crates: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names: Vec<String> = crates.into_iter().map(|s| s.as_ref().to_string()).collect();
        if names.len() < 2 {
            return true;
        }
        let first = &names[0];
        let Some(cluster_of_first) = self.sibling_clusters.get(first) else {
            return false;
        };
        names[1..]
            .iter()
            .all(|n| n == first || cluster_of_first.contains(n))
    }

    /// Number of declared sibling edges (post-closure, undirected) — used by
    /// tests and debug logging.
    pub fn sibling_edge_count(&self) -> usize {
        self.sibling_clusters
            .values()
            .map(|s| s.len())
            .sum::<usize>()
            / 2
    }
}

/// Stable workspace-root resolver used both by the binary and by tests that
/// pre-bake a fixture root. Defined here so layers parsing is callable from
/// anywhere in the crate without dragging in the binary's CLI handling.
#[allow(dead_code)]
pub fn default_workspace_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_file_missing() {
        let m = LayersManifest::load_from_file(Path::new("does/not/exist.toml"));
        assert_eq!(m.sibling_edge_count(), 0);
    }

    #[test]
    fn malformed_toml_yields_empty_manifest() {
        let m = LayersManifest::parse("not valid = = toml [[");
        assert_eq!(m.sibling_edge_count(), 0);
    }

    #[test]
    fn declared_edges_are_symmetric() {
        let toml = r#"
[crates.a]
sibling_of = ["b"]

[crates.b]
"#;
        let m = LayersManifest::parse(toml);
        assert!(m.all_in_one_sibling_cluster(["a", "b"]));
        assert!(m.all_in_one_sibling_cluster(["b", "a"]));
    }

    #[test]
    fn transitive_closure_unifies_clusters() {
        // a↔b, b↔c declared; a↔c should hold via closure.
        let toml = r#"
[crates.a]
sibling_of = ["b"]

[crates.b]
sibling_of = ["c"]

[crates.c]
"#;
        let m = LayersManifest::parse(toml);
        assert!(m.all_in_one_sibling_cluster(["a", "c"]));
        assert!(m.all_in_one_sibling_cluster(["a", "b", "c"]));
    }

    #[test]
    fn unrelated_crates_not_in_same_cluster() {
        let toml = r#"
[crates.a]
sibling_of = ["b"]

[crates.x]
sibling_of = ["y"]
"#;
        let m = LayersManifest::parse(toml);
        assert!(!m.all_in_one_sibling_cluster(["a", "x"]));
    }

    #[test]
    fn single_crate_trivially_passes() {
        let m = LayersManifest::default();
        assert!(m.all_in_one_sibling_cluster(["solo"]));
    }

    #[test]
    fn self_referential_edge_is_ignored() {
        let toml = r#"
[crates.a]
sibling_of = ["a", "b"]

[crates.b]
"#;
        let m = LayersManifest::parse(toml);
        assert!(m.all_in_one_sibling_cluster(["a", "b"]));
        assert_eq!(m.sibling_edge_count(), 1);
    }
}
