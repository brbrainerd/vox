use super::ast::{EXTRACTOR_VERSION, ExtractedEdge, extract_ast_in_module};
use super::cache::CacheManager;
use super::cluster::{ClusterEdge, ClusterNode, cluster_nodes};
use std::fs;
use std::path::Path;

/// Collect the source files that the extractor understands, skipping vendored/VCS dirs.
pub(crate) fn walk_source_files(source_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != ".git" && n != "target" && n != ".vox" && n != "node_modules"
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| {
            matches!(
                e.path().extension().and_then(|x| x.to_str()),
                Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py")
            )
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// Resolve each bare call target to a qualified definition id. Preference: same-module
/// definition; else the unique global definition. Ambiguous, unresolved, and self-edges
/// are dropped (honesty rule: never invent an edge). Pure refactor — behavior identical
/// to the former inline closure in `rebuild_graph`.
fn resolve_edges(
    nodes: &[crate::ast::ExtractedNode],
    edges: &[crate::ast::ExtractedEdge],
) -> Vec<crate::ast::ExtractedEdge> {
    use std::collections::HashMap;
    fn module_of(id: &str) -> &str {
        id.rsplit_once("::").map(|(m, _)| m).unwrap_or("")
    }
    let mut defs_by_name: HashMap<String, Vec<String>> = HashMap::new();
    for n in nodes {
        let bare = n.id.rsplit("::").next().unwrap_or(&n.id).to_string();
        defs_by_name.entry(bare).or_default().push(n.id.clone());
    }
    use std::collections::HashSet;
    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    edges
        .iter()
        .filter_map(|e| {
            // Boundary targets (cmd:/tool:/surface:) are dead-end-preserving: keep the
            // edge whether or not the target node exists. If it is missing, downgrade the
            // confidence to "dangling" so coverage can find the dead-end.
            if e.target.starts_with("cmd:")
                || e.target.starts_with("tool:")
                || e.target.starts_with("surface:")
            {
                let confidence = if node_ids.contains(e.target.as_str()) {
                    e.confidence.clone()
                } else {
                    "dangling".to_string()
                };
                return Some(ExtractedEdge {
                    source: e.source.clone(),
                    target: e.target.clone(),
                    confidence,
                });
            }
            let candidates = defs_by_name.get(&e.target)?;
            let src_mod = module_of(&e.source);
            let same: Vec<&String> = candidates
                .iter()
                .filter(|id| module_of(id) == src_mod)
                .collect();
            let target = if same.len() == 1 {
                same[0].clone()
            } else if candidates.len() == 1 {
                candidates[0].clone()
            } else {
                return None;
            };
            if target == e.source {
                return None; // self-edge
            }
            Some(ExtractedEdge {
                source: e.source.clone(),
                target,
                confidence: e.confidence.clone(),
            })
        })
        .collect()
}

/// Caller-supplied metadata so the manifest is freshness-correct. Field names of the
/// written manifest match `vox_config::graphify::GraphifyManifest`.
#[derive(Debug, Clone, Default)]
pub struct RebuildMeta {
    pub corpus_id: String,
    pub git_sha: Option<String>,
    pub scope_path: String,
    pub extraction_mode: Option<String>,
    pub built_at_rfc3339: String,
}

pub fn rebuild_graph(
    _repo_root: &Path,
    source_dir: &Path,
    output_file: &Path,
    cache_dir: &Path,
    meta: &RebuildMeta,
) -> Result<(), Box<dyn std::error::Error>> {
    let manager = CacheManager::new(cache_dir.to_path_buf());
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();

    for path in walk_source_files(source_dir) {
        let path = path.as_path();
        let content = fs::read_to_string(path)?;

        // Cache key includes EXTRACTOR_VERSION so a scheme change invalidates
        // stale cached graphs even when file content is unchanged.
        let hash = blake3::hash(format!("{EXTRACTOR_VERSION}\u{0}{content}").as_bytes())
            .to_hex()
            .to_string();
        let module_id = path
            .strip_prefix(source_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let graph = if manager.get_cached_hash(path).as_deref() == Some(&hash) {
            manager
                .load_cache(path)
                .unwrap_or_else(|| extract_ast_in_module(path, &content, &module_id))
        } else {
            let g = extract_ast_in_module(path, &content, &module_id);
            manager.write_cache(path, &hash, &g);
            g
        };

        all_nodes.extend(graph.nodes);
        all_edges.extend(graph.edges);
    }

    // Resolve each bare call target to a qualified definition id. Preference: same-module
    // definition; else the unique global definition. Ambiguous, unresolved, and self-edges
    // are dropped (honesty rule: never invent an edge).
    let all_edges: Vec<ExtractedEdge> = resolve_edges(&all_nodes, &all_edges);

    // Convert nodes to ClusterNode format for Leiden clustering
    let cluster_nodes_input: Vec<ClusterNode> = all_nodes
        .iter()
        .map(|n| ClusterNode {
            id: n.id.clone(),
            label: n.label.clone(),
        })
        .collect();

    let cluster_edges_input: Vec<ClusterEdge> = all_edges
        .iter()
        .map(|e| ClusterEdge {
            source: e.source.clone(),
            target: e.target.clone(),
        })
        .collect();

    let communities = cluster_nodes(&cluster_nodes_input, &cluster_edges_input);

    // Build standard NetworkX JSON export format
    let nodes_val: Vec<serde_json::Value> = all_nodes
        .iter()
        .map(|n| {
            let comm = communities
                .get(&n.id)
                .cloned()
                .unwrap_or_else(|| "c_0".to_string());
            serde_json::json!({
                "id": n.id,
                "label": n.label,
                "kind": n.kind,
                "community": comm
            })
        })
        .collect();

    let links_val: Vec<serde_json::Value> = all_edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "source": e.source,
                "target": e.target,
                "confidence": e.confidence
            })
        })
        .collect();

    let structural_graph = serde_json::json!({
        "nodes": nodes_val,
        "links": links_val
    });
    let final_graph = if meta.extraction_mode.as_deref() == Some("modules") {
        super::lens::collapse_to_modules(&structural_graph)
    } else {
        structural_graph
    };
    let node_count = final_graph["nodes"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let edge_count = final_graph["links"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let graph_bytes = serde_json::to_string_pretty(&final_graph)?;
    fs::write(output_file, &graph_bytes)?;

    // Content digest of the exact bytes written. Despite the legacy field name
    // `graph_json_sha256`, the digest is BLAKE3 (already a dep); the ingest path MUST
    // use the same algorithm so `lexical_lag` comparisons are valid.
    let graph_digest = crate::graph_digest(graph_bytes.as_bytes());

    // Tally edge confidence so freshness/coverage consumers can see how many edges are
    // resolved vs declared vs dangling without reparsing graph.json.
    let mut confidence_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for e in &all_edges {
        *confidence_counts.entry(e.confidence.clone()).or_insert(0) += 1;
    }

    let manifest_val = serde_json::json!({
        "corpus_id": meta.corpus_id,
        "built_at": meta.built_at_rfc3339,
        "git_sha": meta.git_sha,
        "scope_path": meta.scope_path,
        "node_count": node_count,
        "edge_count": edge_count,
        "graph_json_sha256": graph_digest,
        "extraction_mode": meta.extraction_mode,
        "confidence_counts": confidence_counts,
    });
    let manifest_path = output_file
        .parent()
        .ok_or("output_file has no parent directory")?
        .join(".graphify_manifest.v1.json");
    fs::write(manifest_path, serde_json::to_string_pretty(&manifest_val)?)?;

    Ok(())
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::ast::{ExtractedEdge, ExtractedNode};
    #[test]
    fn same_module_unique_resolves_and_drops_ambiguous() {
        let nodes = vec![
            ExtractedNode {
                id: "m.rs::a".into(),
                label: "a".into(),
                kind: "fn".into(),
            },
            ExtractedNode {
                id: "m.rs::b".into(),
                label: "b".into(),
                kind: "fn".into(),
            },
        ];
        let edges = vec![ExtractedEdge {
            source: "m.rs::a".into(),
            target: "b".into(),
            confidence: "resolved".into(),
        }];
        let out = resolve_edges(&nodes, &edges);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, "m.rs::b");
    }

    #[test]
    fn prefixed_target_resolves_or_dangles() {
        use crate::ast::{ExtractedEdge, ExtractedNode};
        let nodes = vec![ExtractedNode {
            id: "cmd:real".into(),
            label: "real".into(),
            kind: "command".into(),
        }];
        let edges = vec![
            ExtractedEdge {
                source: "S.tsx::a".into(),
                target: "cmd:real".into(),
                confidence: "declared".into(),
            },
            ExtractedEdge {
                source: "S.tsx::b".into(),
                target: "cmd:gone".into(),
                confidence: "declared".into(),
            },
        ];
        let out = resolve_edges(&nodes, &edges);
        assert!(
            out.iter()
                .any(|e| e.target == "cmd:real" && e.confidence == "declared")
        );
        let dangling = out
            .iter()
            .find(|e| e.target == "cmd:gone")
            .expect("dead-end edge must survive");
        assert_eq!(dangling.confidence, "dangling");
    }
}
