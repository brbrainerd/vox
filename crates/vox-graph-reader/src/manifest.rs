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

/// Strip inner JSX/HTML tags from a heading's inner content, collapsing
/// whitespace. `<span>Foo</span> Bar` → `Foo Bar`.
fn strip_inner_tags(inner: &str) -> String {
    let mut text = String::with_capacity(inner.len());
    let mut depth = 0usize;
    for ch in inner.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => text.push(ch),
            _ => {}
        }
    }
    // Collapse runs of whitespace to single spaces.
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract heading text from a TSX source string.
/// Matches `<h1>…</h1>` through `<h6>…</h6>` (stripping any nested elements so
/// `<h2><span>Foo</span> Bar</h2>` yields "Foo Bar") and simple
/// `aria-label="…"` attributes (terminated at the closing quote).
fn extract_headings(src: &str) -> Vec<String> {
    let mut out = Vec::new();

    // ── <hN>…</hN> headings ────────────────────────────────────────────────
    let bytes = src.as_bytes();
    let mut search = src;
    while let Some(rel) = search.find("<h") {
        let abs_after = &search[rel..];
        // Advance the search cursor past this "<h" for the next iteration.
        let next_search_offset = rel + 2;

        // Next byte must be a heading level digit 1-6 (avoid <header>, <hr>, …).
        let level = abs_after.as_bytes().get(2).copied();
        if !matches!(level, Some(b'1'..=b'6')) {
            search = &search[next_search_offset..];
            continue;
        }
        let level_digit = level.unwrap();
        let close_tag = format!("</h{}>", level_digit as char);

        // Find the end of the opening tag's `>`.
        if let Some(open_gt) = abs_after.find('>') {
            let inner_start = open_gt + 1;
            // Find the MATCHING `</hN>` close, not the first `<`.
            if let Some(close_rel) = abs_after[inner_start..].find(&close_tag) {
                let inner = &abs_after[inner_start..inner_start + close_rel];
                let text = strip_inner_tags(inner);
                if !text.is_empty() && !text.starts_with('{') && text.chars().count() <= 120 {
                    out.push(text);
                }
            }
        }
        search = &search[next_search_offset..];
    }

    // ── aria-label="…" attributes ──────────────────────────────────────────
    const ARIA: &str = "aria-label=";
    let mut idx = 0usize;
    while let Some(rel) = src[idx..].find(ARIA) {
        let attr_start = idx + rel + ARIA.len();
        idx = attr_start;
        // Expect an opening quote (single or double); terminate at the MATCHING quote.
        let Some(&quote) = bytes.get(attr_start) else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        let value_start = attr_start + 1;
        if let Some(end_rel) = src[value_start..].find(quote as char) {
            let text = src[value_start..value_start + end_rel].trim().to_string();
            idx = value_start + end_rel + 1;
            if !text.is_empty() && !text.starts_with('{') && text.chars().count() <= 120 {
                out.push(text);
            }
        }
    }
    out
}

#[cfg(test)]
mod extract_headings_tests {
    use super::extract_headings;

    #[test]
    fn nested_element_heading_is_flattened() {
        let src = "return <h2><span>Foo</span> Bar</h2>;";
        let h = extract_headings(src);
        assert!(
            h.iter().any(|s| s == "Foo Bar"),
            "nested-element heading should flatten to 'Foo Bar'; got {h:?}"
        );
    }

    #[test]
    fn non_ascii_heading_is_kept() {
        // em-dash, accented letter, emoji — previously dropped by an is_ascii() gate.
        let src = "<h1>Café — Météo 🌦</h1>";
        let h = extract_headings(src);
        assert!(
            h.iter().any(|s| s == "Café — Météo 🌦"),
            "non-ASCII heading must be retained; got {h:?}"
        );
    }

    #[test]
    fn matches_closing_tag_not_first_angle_bracket() {
        // The inner `<strong>` must not prematurely terminate the heading.
        let src = "<h3>Alpha <strong>Beta</strong> Gamma</h3>";
        let h = extract_headings(src);
        assert!(
            h.iter().any(|s| s == "Alpha Beta Gamma"),
            "should match </h3>, stripping inner tags; got {h:?}"
        );
    }

    #[test]
    fn aria_label_terminates_at_matching_quote() {
        // A newline inside other attrs must NOT bleed into the captured value.
        let src = "<button aria-label=\"Close dialog\" onClick={fn}>\n  x\n</button>";
        let h = extract_headings(src);
        assert!(
            h.iter().any(|s| s == "Close dialog"),
            "aria-label must terminate at the closing quote; got {h:?}"
        );
        assert!(
            !h.iter().any(|s| s.contains("onClick")),
            "aria-label capture must not run past the closing quote; got {h:?}"
        );
    }

    #[test]
    fn single_quoted_aria_label_supported() {
        let src = "<div aria-label='Settings panel' />";
        let h = extract_headings(src);
        assert!(h.iter().any(|s| s == "Settings panel"), "got {h:?}");
    }
}
