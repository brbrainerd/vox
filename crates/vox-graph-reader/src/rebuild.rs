use super::ast::{
    EXTRACTOR_VERSION, ExtractedEdge, extract_ast_in_module, extract_ast_in_module_with_wrappers,
};
use super::cache::CacheManager;
use super::cluster::{ClusterEdge, ClusterNode, cluster_nodes};
use super::registry::{
    RegistryNode, mcp_tool_nodes, surface_nodes, tauri_command_nodes, transport_wrapper_map,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Parse the `generate_handler![...]` invocation in `main.rs`, returning the final path
/// segment of each registered entry (e.g. `commands::x::do_it` → `do_it`). This is the set
/// of Tauri commands actually wired into the app; commands absent from it are dead.
fn parse_registered_handlers(main_rs: &str) -> Vec<String> {
    let Some(start) = main_rs.find("generate_handler!") else {
        return Vec::new();
    };
    let after = &main_rs[start + "generate_handler!".len()..];
    // The macro takes a `[...]` (or `(...)`) list; capture up to the matching close.
    let (open, close) = match after.trim_start().chars().next() {
        Some('[') => ('[', ']'),
        Some('(') => ('(', ')'),
        _ => return Vec::new(),
    };
    let Some(o) = after.find(open) else {
        return Vec::new();
    };
    let Some(c) = after[o..].find(close) else {
        return Vec::new();
    };
    let list = &after[o + 1..o + c];
    list.split(',')
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .filter_map(|e| e.rsplit("::").next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Collect the source files that the extractor understands, skipping vendored/VCS dirs.
pub(crate) fn walk_source_files(source_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != ".git"
                && n != "target"
                && n != ".vox"
                && n != "node_modules"
                // Build outputs and agent work dirs pollute the corpus with
                // minified-bundle "functions" (816 nodes measured 2026-07-02).
                && n != "dist"
                && n != "web-dist"
                && n != ".claude"
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

/// Resolve each bare call target to a qualified definition id. Preference:
/// same-module definition; else the unique same-crate definition (a bare name
/// defined once in the caller's own crate beats any cross-crate homonym);
/// else the unique global definition. Ambiguous, unresolved, and self-edges
/// are dropped (honesty rule: never invent an edge).
fn resolve_edges(
    nodes: &[crate::ast::ExtractedNode],
    edges: &[crate::ast::ExtractedEdge],
) -> Vec<crate::ast::ExtractedEdge> {
    use std::collections::HashMap;
    fn module_of(id: &str) -> &str {
        id.rsplit_once("::").map(|(m, _)| m).unwrap_or("")
    }
    /// "crates/<name>/…::sym" -> "crates/<name>"; anything else -> "" (no scoping).
    fn crate_scope(id: &str) -> &str {
        let path = id.split("::").next().unwrap_or("");
        let mut it = path.splitn(3, '/');
        match (it.next(), it.next()) {
            (Some("crates"), Some(name)) if !name.is_empty() => {
                &path[.."crates/".len() + name.len()]
            }
            _ => "",
        }
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
            let src_crate = crate_scope(&e.source);
            let same_crate: Vec<&String> = if src_crate.is_empty() {
                Vec::new()
            } else {
                candidates
                    .iter()
                    .filter(|id| crate_scope(id) == src_crate)
                    .collect()
            };
            let target = if same.len() == 1 {
                same[0].clone()
            } else if same_crate.len() == 1 {
                // A bare name defined once in the caller's own crate is far more
                // likely the intended callee than any cross-crate homonym.
                same_crate[0].clone()
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
    /// Serialized clap `CommandCatalog` JSON (gated-corrected). When present in
    /// gui-wiring mode, the CLI tree is folded in as `cli:` nodes with declared
    /// join edges to same-named `cmd:`/`tool:` impls.
    pub cli_catalog_json: Option<String>,
    /// Path to the GUI `ui/src/` directory for heading scans (content-manifest emit;
    /// only used in gui-wiring mode). When `None`, no content manifest is emitted.
    pub gui_source_dir: Option<std::path::PathBuf>,
    /// Contents of `contracts/gui/surface-registry.v1.yaml` (content-manifest emit).
    /// When `None`, no content manifest is emitted.
    pub surface_registry_yaml: Option<String>,
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

    let gui_wiring = meta.extraction_mode.as_deref() == Some("gui-wiring");

    // In gui-wiring mode, build the registered-command set and the transport-wrapper map
    // ONCE, up front, then reuse them across every file in the single walk loop below.
    let registered: Vec<String> = if gui_wiring {
        fs::read_to_string(source_dir.join("src/main.rs"))
            .map(|s| parse_registered_handlers(&s))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let registered_refs: Vec<&str> = registered.iter().map(String::as_str).collect();
    let wrappers: HashMap<String, String> = if gui_wiring {
        fs::read_to_string(source_dir.join("ui/src/transport.ts"))
            .map(|s| transport_wrapper_map(&s))
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    // Accumulated registry nodes (commands/tools/surfaces) carrying the unregistered flag.
    let mut reg: Vec<RegistryNode> = Vec::new();

    for path in walk_source_files(source_dir) {
        let path = path.as_path();
        let content = fs::read_to_string(path)?;
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
        let is_ts = matches!(ext, "ts" | "tsx" | "js" | "jsx");

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

        // Wrapper-aware boundary extraction is only meaningful for TS/TSX in gui-wiring
        // mode; everything else (and all non-gui-wiring builds) uses the cached path.
        let graph = if gui_wiring && is_ts {
            extract_ast_in_module_with_wrappers(path, &content, &module_id, &wrappers)
        } else if manager.get_cached_hash(path).as_deref() == Some(&hash) {
            manager
                .load_cache(path)
                .unwrap_or_else(|| extract_ast_in_module(path, &content, &module_id))
        } else {
            let g = extract_ast_in_module(path, &content, &module_id);
            manager.write_cache(path, &hash, &g);
            g
        };

        // Run the registry adapters on this same file's content (no second walk).
        if gui_wiring {
            if ext == "rs" {
                reg.extend(tauri_command_nodes(&content, &registered_refs));
                reg.extend(mcp_tool_nodes(&content));
            } else if module_id.ends_with("surfaceRegistry.generated.ts") {
                reg.extend(surface_nodes(&content));
            }
        }

        all_nodes.extend(graph.nodes);
        all_edges.extend(graph.edges);
    }

    // Fold registry nodes (commands/tools/surfaces) into the node set. A registry node may
    // appear in multiple files (e.g. a command and an mcp tool name collision) — dedup by id,
    // keeping any `unregistered` flag if set.
    let mut reg_by_id: std::collections::BTreeMap<String, RegistryNode> =
        std::collections::BTreeMap::new();
    for n in reg {
        reg_by_id
            .entry(n.id.clone())
            .and_modify(|e| e.unregistered |= n.unregistered)
            .or_insert(n);
    }
    let reg_nodes: Vec<RegistryNode> = reg_by_id.into_values().collect();
    for n in &reg_nodes {
        all_nodes.push(crate::ast::ExtractedNode {
            id: n.id.clone(),
            label: n.label.clone(),
            kind: n.kind.clone(),
        });
    }

    // Fold the clap CLI tree (gui-wiring only) once, after the registry fold so the
    // join edges can resolve against the `cmd:`/`tool:` impl nodes already present.
    // Each `cli:<group>:<command>` leaf gets a `declared`-confidence join edge to a
    // same-named `cmd:<command>` impl (name-match candidate — never a proven call;
    // coverage treats an unmatched `cli:` node as CliOnly).
    if gui_wiring {
        if let Some(cat) = meta.cli_catalog_json.as_deref() {
            for cn in crate::registry::cli_command_nodes(cat) {
                if let Some(cmd) = cn.id.rsplit(':').next() {
                    // Only leaf command nodes (cli:<group>:<command>) get a join edge;
                    // group nodes (cli:<group>) are surfaced as nodes only.
                    if cn.kind == "cli-command" {
                        all_edges.push(crate::ast::ExtractedEdge {
                            source: cn.id.clone(),
                            target: format!("cmd:{cmd}"),
                            confidence: "declared".to_string(),
                        });
                    }
                    all_nodes.push(crate::ast::ExtractedNode {
                        id: cn.id.clone(),
                        label: cn.label.clone(),
                        kind: cn.kind.clone(),
                    });
                }
            }
        }
    }

    // Resolve each bare call target to a qualified definition id. Preference: same-module
    // definition; else the unique global definition. Ambiguous, unresolved, and self-edges
    // are dropped (honesty rule: never invent an edge).
    let all_edges: Vec<ExtractedEdge> = resolve_edges(&all_nodes, &all_edges);

    // For every dangling boundary edge whose target node is absent, synthesize a
    // `missing`-flagged node so coverage can see the dead-end. The node id is the
    // boundary target (e.g. `cmd:gone`); its kind is derived from the prefix.
    let mut missing_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    {
        use std::collections::HashSet;
        let existing: HashSet<&str> = all_nodes.iter().map(|n| n.id.as_str()).collect();
        for e in &all_edges {
            if e.confidence == "dangling" && !existing.contains(e.target.as_str()) {
                missing_ids.insert(e.target.clone());
            }
        }
        for id in &missing_ids {
            let (prefix, name) = id.split_once(':').unwrap_or(("", id.as_str()));
            let kind = match prefix {
                "cmd" => "command",
                "tool" => "tool",
                "surface" => "surface",
                _ => "unknown",
            };
            all_nodes.push(crate::ast::ExtractedNode {
                id: id.clone(),
                label: name.to_string(),
                kind: kind.to_string(),
            });
        }
    }

    // Index the per-node honesty flags so they can be serialized onto the output nodes.
    let unregistered_ids: std::collections::HashSet<&str> = reg_nodes
        .iter()
        .filter(|n| n.unregistered)
        .map(|n| n.id.as_str())
        .collect();

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
            let mut node = serde_json::json!({
                "id": n.id,
                "label": n.label,
                "kind": n.kind,
                "community": comm
            });
            if missing_ids.contains(&n.id) {
                node["missing"] = serde_json::json!(true);
            }
            if unregistered_ids.contains(n.id.as_str()) {
                node["unregistered"] = serde_json::json!(true);
            }
            node
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

    // Emit the GUI content manifest alongside the graph (gui-wiring mode only — gated on the
    // caller supplying both the surface-registry YAML and the GUI source dir). The emitter is
    // fed the FULL `final_graph` value, which keys edges under "links" (NOT just nodes_val);
    // passing nodes alone would leave every surface's `commands` array empty.
    if let (Some(registry_yaml), Some(surface_dir)) =
        (&meta.surface_registry_yaml, &meta.gui_source_dir)
    {
        let graph_str = serde_json::to_string(&final_graph)?;
        let content_manifest_out = output_file
            .parent()
            .ok_or("output_file has no parent directory")?
            .join("gui-content-manifest.json");
        if let Err(e) = crate::manifest::emit_content_manifest(
            &graph_str,
            registry_yaml,
            surface_dir.as_path(),
            &content_manifest_out,
        ) {
            // Non-fatal: the graph is still written; the manifest is optional for the Omnibar.
            eprintln!("[vox-graph] WARN: content manifest emit failed: {e}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod walker_tests {
    use super::*;

    #[test]
    fn walker_excludes_build_outputs_and_agent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let mk = |rel: &str| {
            let p = tmp.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "// x").unwrap();
        };
        mk("src/a.rs");
        mk("dist/bundle.js");
        mk("web-dist/assets/index-XYZ.js");
        mk(".claude/worktrees/w1/src/b.rs");
        mk("node_modules/pkg/i.js");
        mk("target/debug/gen.rs");
        let files = walk_source_files(tmp.path());
        let names: Vec<String> = files
            .iter()
            .map(|p| {
                p.strip_prefix(tmp.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(names, vec!["src/a.rs".to_string()]);
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::ast::{ExtractedEdge, ExtractedNode};

    fn n(id: &str) -> ExtractedNode {
        ExtractedNode {
            id: id.to_string(),
            label: id.rsplit("::").next().unwrap_or(id).to_string(),
            kind: "fn".to_string(),
        }
    }

    fn e(source: &str, target: &str) -> ExtractedEdge {
        ExtractedEdge {
            source: source.to_string(),
            target: target.to_string(),
            confidence: "resolved".to_string(),
        }
    }

    #[test]
    fn resolver_recovers_same_crate_ambiguous_target() {
        // `run` is defined in the caller's crate AND another crate (ambiguous
        // globally). Old behavior: dropped. New: resolves to the same-crate def.
        let nodes = vec![
            n("crates/aaa/src/x.rs::caller"),
            n("crates/aaa/src/y.rs::run"),
            n("crates/bbb/src/z.rs::run"),
        ];
        let edges = vec![e("crates/aaa/src/x.rs::caller", "run")];
        let resolved = resolve_edges(&nodes, &edges);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].target, "crates/aaa/src/y.rs::run");
    }

    #[test]
    fn resolver_same_module_still_wins_over_same_crate() {
        let nodes = vec![
            n("crates/aaa/src/x.rs::caller"),
            n("crates/aaa/src/x.rs::run"),
            n("crates/aaa/src/y.rs::run"),
        ];
        let edges = vec![e("crates/aaa/src/x.rs::caller", "run")];
        let resolved = resolve_edges(&nodes, &edges);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].target, "crates/aaa/src/x.rs::run");
    }

    #[test]
    fn resolver_drops_ambiguous_within_same_crate() {
        let nodes = vec![
            n("crates/aaa/src/x.rs::caller"),
            n("crates/aaa/src/y.rs::run"),
            n("crates/aaa/src/z.rs::run"),
        ];
        let edges = vec![e("crates/aaa/src/x.rs::caller", "run")];
        assert!(resolve_edges(&nodes, &edges).is_empty());
    }

    #[test]
    fn resolver_unique_global_cross_crate_still_resolves() {
        let nodes = vec![
            n("crates/aaa/src/x.rs::caller"),
            n("crates/bbb/src/z.rs::unique_fn"),
        ];
        let edges = vec![e("crates/aaa/src/x.rs::caller", "unique_fn")];
        let resolved = resolve_edges(&nodes, &edges);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].target, "crates/bbb/src/z.rs::unique_fn");
    }

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
