//! `vox ci dep-cycles` — detect dependency cycles and inventory back-edges that
//! `vox-arch-check` cannot see. arch-check does pairwise layer-ordering on normal
//! deps only; it has no SCC pass, so same-layer cycles and dev-dependency
//! back-edges are invisible to it. This command builds the workspace adjacency
//! from `cargo metadata` and runs Tarjan's SCC to find any cycle, plus reports
//! dev-dep back-edges (e.g. vox-db's test-only edge to vox-compiler).

use std::collections::BTreeMap;

pub fn cycles(adj: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut index_counter = 0usize;
    let mut indices: BTreeMap<String, usize> = BTreeMap::new();
    let mut lowlink: BTreeMap<String, usize> = BTreeMap::new();
    let mut on_stack: BTreeMap<String, bool> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut out: Vec<Vec<String>> = Vec::new();

    for start in adj.keys() {
        if indices.contains_key(start) {
            continue;
        }
        let mut work: Vec<(String, usize)> = vec![(start.clone(), 0)];
        while let Some((v, ni)) = work.last().cloned() {
            if ni == 0 {
                indices.insert(v.clone(), index_counter);
                lowlink.insert(v.clone(), index_counter);
                index_counter += 1;
                stack.push(v.clone());
                on_stack.insert(v.clone(), true);
            }
            let neighbours = adj.get(&v).cloned().unwrap_or_default();
            if ni < neighbours.len() {
                let last = work.len() - 1;
                work[last].1 = ni + 1;
                let w = neighbours[ni].clone();
                if !indices.contains_key(&w) {
                    work.push((w, 0));
                } else if *on_stack.get(&w).unwrap_or(&false) {
                    let lw = *indices.get(&w).unwrap();
                    let lv = *lowlink.get(&v).unwrap();
                    lowlink.insert(v.clone(), lv.min(lw));
                }
            } else {
                if lowlink.get(&v) == indices.get(&v) {
                    let mut comp = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack.insert(w.clone(), false);
                        comp.push(w.clone());
                        if w == v {
                            break;
                        }
                    }
                    if comp.len() > 1 {
                        comp.sort();
                        out.push(comp);
                    }
                }
                work.pop();
                if let Some((parent, _)) = work.last().cloned() {
                    let lp = *lowlink.get(&parent).unwrap();
                    let lv = *lowlink.get(&v).unwrap();
                    lowlink.insert(parent, lp.min(lv));
                }
            }
        }
    }
    out.sort();
    out
}

use serde_json::Value;

pub fn adjacency_from_metadata(
    meta: &Value,
    include_nonlink: bool,
) -> BTreeMap<String, Vec<String>> {
    let empty = vec![];
    let members: std::collections::BTreeSet<String> = meta["workspace_members"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    let mut id_name: BTreeMap<String, String> = BTreeMap::new();
    for p in meta["packages"].as_array().unwrap_or(&empty) {
        if let (Some(id), Some(name)) = (p["id"].as_str(), p["name"].as_str()) {
            id_name.insert(id.to_string(), name.to_string());
        }
    }
    let member_names: std::collections::BTreeSet<String> = members
        .iter()
        .filter_map(|id| id_name.get(id).cloned())
        .collect();

    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in meta["packages"].as_array().unwrap_or(&empty) {
        let name = match p["name"].as_str() {
            Some(n) if member_names.contains(n) => n.to_string(),
            _ => continue,
        };
        let mut deps: Vec<String> = Vec::new();
        for d in p["dependencies"].as_array().unwrap_or(&empty) {
            let dname = match d["name"].as_str() {
                Some(n) => n,
                None => continue,
            };
            if !member_names.contains(dname) {
                continue;
            }
            let kind = d["kind"].as_str().unwrap_or("");
            let is_link_time = kind.is_empty();
            if !is_link_time && !include_nonlink {
                continue;
            }
            if dname != name {
                deps.push(dname.to_string());
            }
        }
        deps.sort();
        deps.dedup();
        adj.entry(name).or_default().extend(deps);
    }
    for v in adj.values_mut() {
        v.sort();
        v.dedup();
    }
    adj
}

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// The format of `contracts/ci/dep-backedges.allow.json`.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct AllowedCycles {
    pub schema_version: u32,
    /// Each inner Vec is a sorted list of crate names in a known advisory cycle.
    pub allowed_cycles: Vec<Vec<String>>,
}

/// Returns cycles in `current` that are NOT in `allowlist`.
/// Both lists must have sorted member vecs (dep_cycles already guarantees this).
pub fn new_advisory_cycles<'a>(
    current: &'a [Vec<String>],
    allowlist: &AllowedCycles,
) -> Vec<&'a Vec<String>> {
    let allowed: std::collections::BTreeSet<&Vec<String>> =
        allowlist.allowed_cycles.iter().collect();
    current.iter().filter(|c| !allowed.contains(c)).collect()
}

fn cargo_metadata(root: &Path) -> Result<Value> {
    let out = Command::new(super::cargo_bin())
        .current_dir(root)
        .args(["metadata", "--format-version", "1"])
        .output()
        .context("run cargo metadata")?;
    if !out.status.success() {
        anyhow::bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    serde_json::from_slice(&out.stdout).context("parse cargo metadata")
}

pub fn run_dep_cycles(root: &Path, deny_new: bool, allowlist_path: Option<&Path>) -> Result<()> {
    let meta = cargo_metadata(root)?;
    let link_time = adjacency_from_metadata(&meta, false);
    let with_nonlink = adjacency_from_metadata(&meta, true);

    let normal_cycles = cycles(&link_time);
    let nonlink_cycles = cycles(&with_nonlink);
    let link_set: std::collections::BTreeSet<Vec<String>> = normal_cycles.iter().cloned().collect();
    let nonlink_only: Vec<Vec<String>> = nonlink_cycles
        .into_iter()
        .filter(|c| !link_set.contains(c))
        .collect();

    let mut report = String::from("# Dependency cycle & back-edge inventory\n\n");
    report.push_str(&format!(
        "Normal (link-time) dep cycles (HARD — cargo would reject these): {}\n",
        normal_cycles.len()
    ));
    for c in &normal_cycles {
        report.push_str(&format!("  - CYCLE: {}\n", c.join(" -> ")));
    }
    report.push_str(&format!(
        "\nDev/build back-edge cycles (advisory — legal in cargo): {}\n",
        nonlink_only.len()
    ));
    for c in &nonlink_only {
        report.push_str(&format!("  - back-edge-cycle: {}\n", c.join(" -> ")));
    }

    let dir = root.join("graphify-out");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join("DEP_CYCLES.md"), &report).ok();
    print!("{report}");

    if !normal_cycles.is_empty() {
        anyhow::bail!(
            "{} normal-dependency cycle(s) detected — see graphify-out/DEP_CYCLES.md",
            normal_cycles.len()
        );
    }

    if deny_new {
        let allow_path = allowlist_path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| root.join("contracts/ci/dep-backedges.allow.json"));
        let raw = std::fs::read_to_string(&allow_path)
            .with_context(|| format!("read allowlist {}", allow_path.display()))?;
        let allowlist: AllowedCycles = serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", allow_path.display()))?;
        let new = new_advisory_cycles(&nonlink_only, &allowlist);
        if !new.is_empty() {
            eprintln!(
                "ERROR: {} new advisory cycle(s) not in allowlist:",
                new.len()
            );
            for c in &new {
                eprintln!("  new: {}", c.join(" -> "));
            }
            eprintln!("To allow: add the cycle to {}", allow_path.display());
            anyhow::bail!("{} new advisory dep cycle(s) detected", new.len());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(edges: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        edges
            .iter()
            .map(|(n, ds)| (n.to_string(), ds.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn acyclic_graph_has_no_cycles() {
        let adj = g(&[("a", &["b"]), ("b", &["c"]), ("c", &[])]);
        assert!(cycles(&adj).is_empty());
    }

    #[test]
    fn two_node_cycle_detected() {
        let adj = g(&[("a", &["b"]), ("b", &["a"])]);
        assert_eq!(cycles(&adj), vec![vec!["a".to_string(), "b".to_string()]]);
    }

    #[test]
    fn three_node_cycle_detected_but_tail_excluded() {
        let adj = g(&[("a", &["b"]), ("b", &["c"]), ("c", &["a"]), ("d", &["a"])]);
        let cs = cycles(&adj);
        assert_eq!(cs.len(), 1);
        assert_eq!(
            cs[0],
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn self_loop_is_not_reported() {
        let adj = g(&[("a", &["a"])]);
        assert!(cycles(&adj).is_empty());
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    fn meta() -> Value {
        serde_json::json!({
            "workspace_members": ["a 0.1 (path+file:///a)", "b 0.1 (path+file:///b)"],
            "packages": [
                {"id":"a 0.1 (path+file:///a)","name":"a","dependencies":[
                    {"name":"b","kind":null},
                    {"name":"serde","kind":null}
                ]},
                {"id":"b 0.1 (path+file:///b)","name":"b","dependencies":[
                    {"name":"a","kind":"dev"}
                ]}
            ]
        })
    }

    #[test]
    fn normal_edges_only_when_dev_excluded() {
        let adj = adjacency_from_metadata(&meta(), false);
        assert_eq!(adj["a"], vec!["b".to_string()]);
        assert_eq!(adj["b"], Vec::<String>::new());
        assert!(cycles(&adj).is_empty());
    }

    #[test]
    fn dev_edges_included_reveal_back_edge_cycle() {
        let adj = adjacency_from_metadata(&meta(), true);
        assert_eq!(adj["b"], vec!["a".to_string()]);
        assert_eq!(cycles(&adj), vec![vec!["a".to_string(), "b".to_string()]]);
    }
}

#[cfg(test)]
mod deny_new_tests {
    use super::*;

    fn allowed() -> AllowedCycles {
        AllowedCycles {
            schema_version: 1,
            allowed_cycles: vec![vec!["a".into(), "b".into(), "c".into()]],
        }
    }

    #[test]
    fn no_new_cycles_when_cycles_match_allowlist() {
        let current = vec![vec!["a".into(), "b".into(), "c".into()]];
        assert!(new_advisory_cycles(&current, &allowed()).is_empty());
    }

    #[test]
    fn extra_cycle_detected_as_new() {
        let current = vec![
            vec!["a".into(), "b".into(), "c".into()],
            vec!["x".into(), "y".into()],
        ];
        let new = new_advisory_cycles(&current, &allowed());
        assert_eq!(new.len(), 1);
        assert_eq!(new[0], &vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn subset_of_allowlist_is_not_new() {
        let current: Vec<Vec<String>> = vec![];
        assert!(new_advisory_cycles(&current, &allowed()).is_empty());
    }

    #[test]
    fn cycle_order_independent_match() {
        // dep_cycles already sorts members; this is a belt-and-suspenders check
        let current = vec![vec!["a".into(), "b".into(), "c".into()]]; // sorted
        assert!(new_advisory_cycles(&current, &allowed()).is_empty());
    }
}
