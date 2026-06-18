use std::path::{Path, PathBuf};
use std::fs;
use super::ast::{extract_ast, ExtractedGraph, ExtractedNode, ExtractedEdge};
use super::cache::CacheManager;
use super::cluster::{cluster_nodes, ClusterNode, ClusterEdge};

pub fn rebuild_graph(
    _repo_root: &Path,
    source_dir: &Path,
    output_file: &Path,
    cache_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let manager = CacheManager::new(cache_dir.to_path_buf());
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();

    for entry in walkdir::WalkDir::new(source_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "rs" || ext == "ts" || ext == "js" {
                    let content = fs::read_to_string(path)?;
                    let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
                    
                    let graph = if manager.get_cached_hash(path).as_deref() == Some(&hash) {
                        manager.load_cache(path).unwrap_or_else(|| extract_ast(path, &content))
                    } else {
                        let g = extract_ast(path, &content);
                        manager.write_cache(path, &hash, &g);
                        g
                    };

                    all_nodes.extend(graph.nodes);
                    all_edges.extend(graph.edges);
                }
            }
        }
    }

    // Convert nodes to ClusterNode format for Leiden clustering
    let cluster_nodes_input: Vec<ClusterNode> = all_nodes.iter().map(|n| ClusterNode {
        id: n.id.clone(),
        label: n.label.clone(),
    }).collect();

    let cluster_edges_input: Vec<ClusterEdge> = all_edges.iter().map(|e| ClusterEdge {
        source: e.source.clone(),
        target: e.target.clone(),
    }).collect();

    let communities = cluster_nodes(&cluster_nodes_input, &cluster_edges_input);

    // Build standard NetworkX JSON export format
    let nodes_val: Vec<serde_json::Value> = all_nodes.iter().map(|n| {
        let comm = communities.get(&n.id).cloned().unwrap_or_else(|| "c_0".to_string());
        serde_json::json!({
            "id": n.id,
            "label": n.label,
            "kind": n.kind,
            "community": comm
        })
    }).collect();

    let links_val: Vec<serde_json::Value> = all_edges.iter().map(|e| {
        serde_json::json!({
            "source": e.source,
            "target": e.target
        })
    }).collect();

    let final_graph = serde_json::json!({
        "nodes": nodes_val,
        "links": links_val
    });

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_file, serde_json::to_string_pretty(&final_graph)?)?;

    // Create manifest file
    let git_sha = "dev-sha"; // Fallback or retrieve from env
    let manifest_val = serde_json::json!({
        "git_sha256": git_sha,
        "node_count": nodes_val.len(),
        "edge_count": links_val.len(),
    });
    let manifest_path = output_file.parent().unwrap().join(".graphify_manifest.v1.json");
    fs::write(manifest_path, serde_json::to_string_pretty(&manifest_val)?)?;

    Ok(())
}
