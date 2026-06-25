use super::ast::{EXTRACTOR_VERSION, ExtractedEdge, ExtractedNode, extract_ast_in_module};
use super::cache::CacheManager;
use super::cluster::{ClusterEdge, ClusterNode, cluster_nodes};
use std::fs;
use std::path::Path;

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

    for entry in walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".git" && name != "target" && name != ".vox" && name != "node_modules"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "rs" || ext == "ts" || ext == "js" || ext == "py" {
                    let content = fs::read_to_string(path)?;

                    // Cache key includes EXTRACTOR_VERSION so a scheme change invalidates
                    // stale cached graphs even when file content is unchanged.
                    let hash =
                        blake3::hash(format!("{EXTRACTOR_VERSION}\u{0}{content}").as_bytes())
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
            }
        }
    }

    let all_edges = resolve_edges(&all_nodes, &all_edges);

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
                "target": e.target
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

    let manifest_val = serde_json::json!({
        "corpus_id": meta.corpus_id,
        "built_at": meta.built_at_rfc3339,
        "git_sha": meta.git_sha,
        "scope_path": meta.scope_path,
        "node_count": node_count,
        "edge_count": edge_count,
        "graph_json_sha256": graph_digest,
        "extraction_mode": meta.extraction_mode,
    });
    let manifest_path = output_file
        .parent()
        .ok_or("output_file has no parent directory")?
        .join(".graphify_manifest.v1.json");
    fs::write(manifest_path, serde_json::to_string_pretty(&manifest_val)?)?;

    Ok(())
}

/// Module path of a qualified node id: everything before the final `::` segment. Node ids
/// are `module_id::symbol` where `module_id` is the file path relative to the source dir
/// (e.g. `a/b.rs::foo` -> `a/b.rs`). A bare id with no `::` has an empty module.
fn module_of(id: &str) -> &str {
    id.rsplit_once("::").map(|(m, _)| m).unwrap_or("")
}

/// Resolve each bare call target to a qualified definition id.
///
/// Honesty rule: never invent a cross-module edge. We only bind a call to a definition
/// that lives in the *same module* as the call site. A bare target with a single global
/// candidate in some *unrelated* module is dropped rather than collapsed (two `run` fns in
/// different files must not merge; a `len()` call must not bind to an unrelated local
/// `len`). Ambiguous, unresolved, and self-edges are also dropped.
fn resolve_edges(nodes: &[ExtractedNode], edges: &[ExtractedEdge]) -> Vec<ExtractedEdge> {
    let mut defs_by_name: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for n in nodes {
        let bare = n.id.rsplit("::").next().unwrap_or(&n.id).to_string();
        defs_by_name.entry(bare).or_default().push(n.id.clone());
    }
    edges
        .iter()
        .filter_map(|e| {
            let candidates = defs_by_name.get(&e.target)?;
            let src_mod = module_of(&e.source);
            // Only same-module definitions are honest resolutions. A single global
            // candidate in a different module is dropped, not invented.
            let same: Vec<&String> = candidates
                .iter()
                .filter(|id| module_of(id) == src_mod)
                .collect();
            let target = if same.len() == 1 {
                same[0].clone()
            } else {
                return None;
            };
            if target == e.source {
                return None; // self-edge
            }
            Some(ExtractedEdge {
                source: e.source.clone(),
                target,
            })
        })
        .collect()
}

#[cfg(test)]
mod resolve_tests {
    use super::*;

    fn node(id: &str) -> ExtractedNode {
        ExtractedNode {
            id: id.to_string(),
            label: id.rsplit("::").next().unwrap_or(id).to_string(),
            kind: "fn".to_string(),
        }
    }

    fn edge(source: &str, target: &str) -> ExtractedEdge {
        ExtractedEdge {
            source: source.to_string(),
            target: target.to_string(),
        }
    }

    #[test]
    fn same_module_single_candidate_resolves() {
        // caller `a.rs::caller` calls bare `helper`; the only definition lives in the
        // SAME module `a.rs` -> edge is emitted, qualified to that definition.
        let nodes = vec![node("a.rs::caller"), node("a.rs::helper")];
        let edges = vec![edge("a.rs::caller", "helper")];
        let resolved = resolve_edges(&nodes, &edges);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].source, "a.rs::caller");
        assert_eq!(resolved[0].target, "a.rs::helper");
    }

    #[test]
    fn cross_module_single_candidate_dropped() {
        // caller `a.rs::caller` calls bare `helper`; the ONLY definition lives in a
        // DIFFERENT module `b.rs`. The old code invented a cross-module edge; we now
        // drop it (honesty rule). No edge must be produced.
        let nodes = vec![node("a.rs::caller"), node("b.rs::helper")];
        let edges = vec![edge("a.rs::caller", "helper")];
        let resolved = resolve_edges(&nodes, &edges);
        assert!(
            resolved.is_empty(),
            "cross-module single candidate must not produce an edge, got {resolved:?}"
        );
    }
}
