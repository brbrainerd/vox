//! GUI content-manifest emitter.
//!
//! Emits `gui-content-manifest.json` from the structural graph + surface-registry YAML.
//! Called by `rebuild::rebuild_graph` in gui-wiring mode after the graph is written.
//!
//! Output format:
//! ```json
//! {
//!   "schema_version": 1,
//!   "surfaces": [
//!     {
//!       "view_key": "approvals",
//!       "nav_label": "Approvals",
//!       "nav_group": "operate",
//!       "route": "#view=approvals",
//!       "headings": [],
//!       "commands": ["vox_resolve_approval"],
//!       "notes": "Operator approval queue for doubt_task feedback",
//!       "docs": []
//!     }
//!   ]
//! }
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

/// Emit `gui-content-manifest.json` to `out_path`.
///
/// # Parameters
/// - `graph_json` — the `graph.json` content string.
/// - `surface_registry_yaml` — contents of `contracts/gui/surface-registry.v1.yaml`.
/// - `surface_dir` — the GUI `ui/src/` directory to scan for TSX heading text.
/// - `out_path` — destination file path.
pub fn emit_content_manifest(
    graph_json: &str,
    surface_registry_yaml: &str,
    surface_dir: &Path,
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let graph: Value = serde_json::from_str(graph_json)?;

    // 1. Collect surface node ids (view_key strings) from the graph.
    let surface_ids: HashSet<String> = graph["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| n["kind"].as_str() == Some("surface"))
        .filter_map(|n| {
            let id = n["id"].as_str()?;
            let view_key = id.strip_prefix("surface:")?;
            Some(view_key.to_string())
        })
        .collect();

    // 2. Build edge maps from the graph.
    //    X1: the PRODUCTION writer emits the edge array under "links" (rebuild.rs);
    //    read "links" first, fall back to "edges" (mirrors coverage.rs / lens.rs).
    //    Keying only on "edges" would make every surface's `commands` silently empty in prod.
    let edges = graph["links"]
        .as_array()
        .or_else(|| graph["edges"].as_array());

    // 2a. surface view_key → set of cmd: neighbor names.
    let mut cmd_neighbors: HashMap<String, Vec<String>> = HashMap::new();
    // 2b. surface view_key → component module path (for the X5 heading join — key off the
    //     graph surface→module edge, NOT a filename heuristic).
    let mut module_by_surface: HashMap<String, String> = HashMap::new();
    if let Some(edges) = edges {
        for edge in edges {
            let src = edge["source"].as_str().unwrap_or("");
            let tgt = edge["target"].as_str().unwrap_or("");
            if let Some(view_key) = src.strip_prefix("surface:") {
                if let Some(cmd_name) = tgt.strip_prefix("cmd:") {
                    cmd_neighbors
                        .entry(view_key.to_string())
                        .or_default()
                        .push(cmd_name.to_string());
                } else if let Some(module_path) = tgt.strip_prefix("module:") {
                    // Deterministic: pick the lexically smallest module path per surface.
                    module_by_surface
                        .entry(view_key.to_string())
                        .and_modify(|existing| {
                            if module_path < existing.as_str() {
                                *existing = module_path.to_string();
                            }
                        })
                        .or_insert_with(|| module_path.to_string());
                }
            }
        }
    }
    // Dedup and sort commands for determinism.
    for v in cmd_neighbors.values_mut() {
        v.sort();
        v.dedup();
    }

    // 3. Parse surface-registry YAML for label/group/notes per surface.
    //    Targeted line scan (no serde_yaml dep in this crate).
    let registry_meta = parse_surface_registry_yaml(surface_registry_yaml);

    // 4. Scan headings per surface by resolving the graph's surface→module edge to a file
    //    under `surface_dir` (X5 — no filename guessing).
    let headings_by_surface = scan_surface_headings(surface_dir, &module_by_surface);

    // 5. Build manifest entries for every surface node in the graph.
    let mut surfaces_out: Vec<Value> = surface_ids
        .iter()
        .map(|view_key| {
            let meta = registry_meta.get(view_key);
            let nav_label = meta
                .and_then(|m| m.get("nav_label"))
                .cloned()
                .unwrap_or_else(|| view_key.clone());
            let nav_group = meta
                .and_then(|m| m.get("nav_group"))
                .cloned()
                .unwrap_or_default();
            let notes = meta
                .and_then(|m| m.get("notes"))
                .cloned()
                .unwrap_or_default();
            let route = format!("#view={view_key}");
            let commands = cmd_neighbors.get(view_key).cloned().unwrap_or_default();
            let headings = headings_by_surface
                .get(view_key)
                .cloned()
                .unwrap_or_default();
            serde_json::json!({
                "view_key": view_key,
                "nav_label": nav_label,
                "nav_group": nav_group,
                "route": route,
                "headings": headings,
                "commands": commands,
                "notes": notes,
                // X11: empty for VG-1; present so VG-2's ContentManifestEntry.docs type matches.
                "docs": Vec::<String>::new(),
            })
        })
        .collect();

    // Sort by view_key for determinism.
    surfaces_out.sort_by(|a, b| a["view_key"].as_str().cmp(&b["view_key"].as_str()));

    let manifest = serde_json::json!({
        "schema_version": 1,
        "surfaces": surfaces_out,
    });

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

/// Targeted YAML scanner for the surface-registry.
/// Returns a map of view_key → { nav_label, nav_group, notes }.
/// Uses line-by-line parsing (no serde_yaml dep) since the YAML format is regular.
fn parse_surface_registry_yaml(yaml: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current_view_key: Option<String> = None;

    for line in yaml.lines() {
        let trimmed = line.trim();

        if let Some(val) = trimmed.strip_prefix("- view_key:") {
            let vk = val.trim().trim_matches('\'').trim_matches('"').to_string();
            if !vk.is_empty() && vk != "null" {
                current_view_key = Some(vk.clone());
                out.entry(vk).or_default();
            } else {
                current_view_key = None;
            }
            continue;
        }

        let Some(ref vk) = current_view_key else {
            continue;
        };

        for field in &["nav_label", "nav_group", "notes"] {
            let prefix = format!("{field}:");
            if let Some(val) = trimmed.strip_prefix(prefix.as_str()) {
                let v = val.trim().trim_matches('\'').trim_matches('"').to_string();
                if v != "null" && !v.is_empty() {
                    out.entry(vk.clone())
                        .or_default()
                        .insert(field.to_string(), v);
                }
            }
        }
    }
    out
}

/// Scan surface component files for heading text — keyed by the graph's surface→module edge.
///
/// X5: `module_by_surface` maps `view_key → component module path` (taken from the graph's
/// `surface:<vk> → module:<path>` edge). We resolve each module path to a real file under
/// `surface_dir` and extract headings from it. This avoids the filename heuristic that
/// mis-keys multi-word kebab view keys (`SubAgentsView.tsx → "subagents" ≠ "sub-agents"`).
///
/// A surface with no module edge (or whose module file is missing) gets NO headings entry —
/// the caller emits `headings: []` (honest empty, not a wrong best-effort match).
///
/// Returns a map of view_key → sorted deduplicated heading strings.
fn scan_surface_headings(
    surface_dir: &Path,
    module_by_surface: &HashMap<String, String>,
) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();

    for (view_key, module_path) in module_by_surface {
        // The module id is a repo-relative-ish path (e.g.
        // "components/surfaces/SubAgents/SubAgentsView.tsx"). Resolve against surface_dir;
        // also try matching by basename in case the graph stores a different path root.
        let candidate = surface_dir.join(module_path);
        let resolved = if candidate.is_file() {
            Some(candidate)
        } else {
            let base = std::path::Path::new(module_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if base.is_empty() {
                None
            } else {
                walkdir::WalkDir::new(surface_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .map(|e| e.path().to_path_buf())
                    .filter(|p| p.file_name().and_then(|s| s.to_str()) == Some(base))
                    .min()
            }
        };

        let Some(path) = resolved else { continue };
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let headings = extract_headings(&src);
        if !headings.is_empty() {
            out.entry(view_key.clone()).or_default().extend(headings);
        }
    }

    // Sort + dedup each surface's headings for determinism.
    for v in out.values_mut() {
        v.sort();
        v.dedup();
    }
    out
}

/// Extract heading text from a TSX source string.
/// Matches `<h1>…</h1>` through `<h6>…</h6>` and simple `aria-label="…"` attributes.
fn extract_headings(src: &str) -> Vec<String> {
    let mut out = Vec::new();

    for tag_open in &["<h", "aria-label="] {
        let tag_open = *tag_open;
        let mut rest = src;
        while let Some(start) = rest.find(tag_open) {
            let after = &rest[start..];
            // For h-tags, ensure the next byte is a digit 1-6 (avoid matching e.g. <header>).
            if tag_open == "<h" {
                let next = after.as_bytes().get(2).copied();
                if !matches!(next, Some(b'1'..=b'6')) {
                    rest = &rest[start + tag_open.len()..];
                    continue;
                }
            }
            // Find the closing > for the opening tag.
            if let Some(close_bracket) = after.find('>') {
                let after_open = &after[close_bracket + 1..];
                // End of content: < for h-tags (closing tag), newline for aria-label.
                let end_marker = if tag_open == "<h" { "<" } else { "\n" };
                if let Some(end) = after_open.find(end_marker) {
                    let text = after_open[..end].trim().to_string();
                    if !text.is_empty()
                        && !text.starts_with('{')
                        && text.len() <= 120
                        && text.is_ascii()
                    {
                        out.push(text);
                    }
                }
            }
            rest = &rest[start + tag_open.len()..];
        }
    }
    out
}
