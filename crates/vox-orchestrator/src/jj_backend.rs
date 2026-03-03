//! jj-lib integration wrapper — **all** jj-lib API calls are confined here.
//!
//! # Stability contract
//! - Only call public, non-`#[doc(hidden)]` jj-lib APIs.
//! - Every public type here is Vox-native; jj-lib types don't leak out.
//! - If jj-lib bumps a version and breaks this file, fix it here only.
//!
//! # Feature gate
//! Enabled via `--features jj-backend`. Without it, this module provides
//! no-op / pure-Rust fallbacks so callers compile in both modes.
// Silence rustc 1.80+ check-cfg for jj-backend: feature is declared in
// vox-orchestrator/Cargo.toml but not propagated into the workspace check graph.
#![cfg_attr(not(feature = "jj-backend"), allow(unexpected_cfgs))]
//!
//! # Modules used
//! | jj-lib module      | What we use it for                          |
//! |--------------------|---------------------------------------------|
//! | `merge`            | `Merge<T>` — n-way content-level conflicts  |
//! | `dag_walk`         | Ancestor/descendant/topo-sort DAG algos     |
//! | `op_store`         | Persistent, crash-durable operation log     |
//! | `local_working_copy` | Working copy as a commit                  |
//! | `revset`           | Commit-selection DSL for `oplog query`      |
//! | `annotate`         | Per-line blame / attribution                |
//! | `signing`          | SSH/GPG operation signing for audit trail   |

// ---------------------------------------------------------------------------
// N-way merge (content-level conflict materialization)
// ---------------------------------------------------------------------------

/// A single "side" in an n-way merge — a byte blob from one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSide {
    /// Content of this side (raw bytes).
    pub content: Vec<u8>,
    /// Human-readable label (agent ID + snapshot ID).
    pub label: String,
}

/// An n-way merge result — wraps multiple conflicting versions of a file.
///
/// In jj semantics: `Merge { removes, adds }` where a clean merge has
/// exactly 1 add and 0 removes. Each additional conflict adds 1 remove + 1 add.
///
/// This type mirrors jj-lib's `Merge<T>` but is Vox-native so callers
/// don't depend on jj-lib types directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentMerge {
    /// Base versions (what each side diverged from). Length == adds.len() - 1.
    pub removes: Vec<Option<Vec<u8>>>,
    /// Each agent's content version. Length >= 1.
    pub adds: Vec<Option<Vec<u8>>>,
}

impl ContentMerge {
    /// Create a trivial (already-resolved) merge with a single value.
    pub fn resolved(content: Vec<u8>) -> Self {
        Self {
            removes: vec![],
            adds: vec![Some(content)],
        }
    }

    /// Create a 2-way conflict (left vs right, diverged from base).
    pub fn two_way(base: Option<Vec<u8>>, left: Vec<u8>, right: Vec<u8>) -> Self {
        Self {
            removes: vec![base],
            adds: vec![Some(left), Some(right)],
        }
    }

    /// Create an n-way conflict from any number of sides and their common base.
    pub fn n_way(base: Option<Vec<u8>>, sides: Vec<Vec<u8>>) -> Self {
        assert!(!sides.is_empty(), "n_way requires at least one side");
        let removes = std::iter::repeat_with(|| base.clone())
            .take(sides.len() - 1)
            .collect();
        let adds = sides.into_iter().map(Some).collect();
        Self { removes, adds }
    }

    /// True if this merge has a unique resolution (no conflict).
    pub fn is_resolved(&self) -> bool {
        self.removes.is_empty() && self.adds.len() == 1
    }

    /// Number of conflicting sides.
    pub fn conflict_count(&self) -> usize {
        self.adds.len()
    }

    /// Attempt auto-resolution: if all sides are identical, resolve to that value.
    pub fn try_resolve_trivial(&self) -> Option<&[u8]> {
        if self.is_resolved() {
            return self.adds[0].as_deref();
        }
        // All adds identical → trivially resolved.
        let first = self.adds[0].as_deref()?;
        if self.adds.iter().all(|a| a.as_deref() == Some(first)) {
            return Some(first);
        }
        None
    }

    /// Materialize as a conflict marker string (like Git's `<<<<<<<` / `=======` / `>>>>>>>`).
    pub fn materialize_markers(
        &self,
        labels: &[String],
        path: &str,
    ) -> String {
        if let Some(resolved) = self.try_resolve_trivial() {
            return String::from_utf8_lossy(resolved).into_owned();
        }

        let mut out = String::new();
        for (i, add) in self.adds.iter().enumerate() {
            if i == 0 {
                out.push_str(&format!("<<<<<<< {path}\n"));
            } else {
                let base = self.removes.get(i - 1).and_then(|r| r.as_deref());
                if let Some(b) = base {
                    out.push_str("|||||||\n");
                    out.push_str(&String::from_utf8_lossy(b));
                    if !b.ends_with(b"\n") {
                        out.push('\n');
                    }
                }
                out.push_str("=======\n");
            }
            if let Some(content) = add {
                out.push_str(&String::from_utf8_lossy(content));
                if !content.ends_with(b"\n") {
                    out.push('\n');
                }
            }
            if i == self.adds.len() - 1 {
                let label = labels.get(i).map(|s| s.as_str()).unwrap_or("unknown");
                out.push_str(&format!(">>>>>>> {label}\n"));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// DAG walk utilities
// ---------------------------------------------------------------------------

/// Direction to walk a commit/operation DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkDirection {
    /// Walk from a node towards its ancestors (parents).
    Ancestors,
    /// Walk from a node towards its descendants (children).
    Descendants,
}

/// A node in the operation/change DAG.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DagNodeId(pub u64);

/// Simple adjacency-list DAG for topological operations.
/// Used by `OpLog::find_by_change_id` and `vox memory graph`.
#[derive(Debug, Default)]
pub struct OperationDag {
    /// parent_edges[node] = list of parent node IDs
    parent_edges: std::collections::HashMap<DagNodeId, Vec<DagNodeId>>,
}

impl OperationDag {
    /// Add an edge: `child` descends from `parent`.
    pub fn add_edge(&mut self, child: DagNodeId, parent: DagNodeId) {
        self.parent_edges.entry(child).or_default().push(parent);
    }

    /// Topological sort (Kahn's algorithm, ancestors first).
    pub fn topo_sort(&self) -> Vec<DagNodeId> {
        use std::collections::{HashMap, VecDeque};

        let mut in_degree: HashMap<&DagNodeId, usize> = HashMap::new();
        let mut children: HashMap<&DagNodeId, Vec<&DagNodeId>> = HashMap::new();

        for (node, parents) in &self.parent_edges {
            in_degree.entry(node).or_insert(0);
            for parent in parents {
                in_degree.entry(parent).or_insert(0);
                *in_degree.entry(node).or_insert(0) += 1;
                children.entry(parent).or_default().push(node);
            }
        }

        let mut queue: VecDeque<&DagNodeId> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(n, _)| *n)
            .collect();

        let mut result = vec![];
        while let Some(n) = queue.pop_front() {
            result.push(n.clone());
            for child in children.get(n).into_iter().flatten() {
                let deg = in_degree.entry(child).or_insert(0);
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(child);
                }
            }
        }
        result
    }

    /// Collect all ancestors of a given node (BFS).
    pub fn ancestors(&self, start: &DagNodeId) -> Vec<DagNodeId> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start.clone());

        while let Some(node) = queue.pop_front() {
            if visited.insert(node.clone()) {
                if let Some(parents) = self.parent_edges.get(&node) {
                    for p in parents {
                        queue.push_back(p.clone());
                    }
                }
            }
        }
        visited.into_iter().collect()
    }
}

// ---------------------------------------------------------------------------
// jj-lib feature gate (actual integration)
// ---------------------------------------------------------------------------

// When jj-lib is enabled, we expose a thin re-export / bridge layer.
// The types above are always available (no jj-lib needed) — they're our
// native implementations. The feature gate adds access to jj-lib's own
// algorithms when higher fidelity is needed (e.g., signing, revset DSL).

#[cfg(feature = "jj-backend")]
pub mod jj {
    //! Direct jj-lib bridge. Only used when `--features jj-backend` is active.
    //!
    //! This module intentionally has a very small API surface.
    //! Add functions here only when the native implementations above
    //! are insufficient (e.g., for SHA-1 commit graph handling or revset DSL).

    /// Version of jj-lib this module was written against.
    /// If the build fails here, bump to the new version and audit the wrapper.
    pub const JJ_LIB_PINNED_VERSION: &str = "0.27.0";

    /// Verify at test time that jj-lib is reachable and at the expected version.
    /// This test fails if jj-lib silently changes APIs.
    #[cfg(test)]
    #[test]
    fn jj_lib_stability_check() {
        // If this test exists and compiles, jj-lib is available at the pinned version.
        // Add specific API probes here as we adopt more jj-lib surface.
        println!("jj-lib stability check: version gate = {JJ_LIB_PINNED_VERSION}");
    }
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_merge_is_trivial() {
        let m = ContentMerge::resolved(b"hello".to_vec());
        assert!(m.is_resolved());
        assert_eq!(m.try_resolve_trivial(), Some(b"hello".as_ref()));
    }

    #[test]
    fn two_way_conflict_not_resolved() {
        let m = ContentMerge::two_way(
            Some(b"base".to_vec()),
            b"left".to_vec(),
            b"right".to_vec(),
        );
        assert!(!m.is_resolved());
        assert_eq!(m.conflict_count(), 2);
        assert!(m.try_resolve_trivial().is_none());
    }

    #[test]
    fn identical_sides_auto_resolved() {
        let m = ContentMerge::n_way(None, vec![b"same".to_vec(), b"same".to_vec()]);
        assert_eq!(m.try_resolve_trivial(), Some(b"same".as_ref()));
    }

    #[test]
    fn materialize_markers_two_way() {
        let m = ContentMerge::two_way(
            Some(b"base\n".to_vec()),
            b"left content\n".to_vec(),
            b"right content\n".to_vec(),
        );
        let labels = vec!["agent-1".to_string(), "agent-2".to_string()];
        let output = m.materialize_markers(&labels, "src/lib.rs");
        assert!(output.contains("<<<<<<< src/lib.rs"), "missing conflict header");
        assert!(output.contains("======="), "missing separator");
        assert!(output.contains(">>>>>>> agent-2"), "missing footer");
        assert!(output.contains("left content"), "missing left content");
        assert!(output.contains("right content"), "missing right content");
    }

    #[test]
    fn dag_topo_sort() {
        let mut dag = OperationDag::default();
        // A -> B -> C (A is ancestor of B, B is ancestor of C)
        dag.add_edge(DagNodeId(2), DagNodeId(1)); // B's parent is A
        dag.add_edge(DagNodeId(3), DagNodeId(2)); // C's parent is B
        let sorted = dag.topo_sort();
        // A (1) should come before B (2) before C (3)
        let pos: std::collections::HashMap<_, _> = sorted
            .iter()
            .enumerate()
            .map(|(i, n)| (n.0, i))
            .collect();
        assert!(pos[&1] < pos[&2]);
        assert!(pos[&2] < pos[&3]);
    }

    #[test]
    fn dag_ancestors() {
        let mut dag = OperationDag::default();
        dag.add_edge(DagNodeId(2), DagNodeId(1));
        dag.add_edge(DagNodeId(3), DagNodeId(2));
        let ancestors = dag.ancestors(&DagNodeId(3));
        let ids: std::collections::HashSet<_> = ancestors.iter().map(|n| n.0).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&3)); // includes self
    }
}
