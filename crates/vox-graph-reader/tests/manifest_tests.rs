// Tests for gui-content-manifest emission.
// G7 golden tests + the G2 signature smoke.

use std::path::Path;
use tempfile::TempDir;
use vox_graph_reader::manifest::emit_content_manifest;

/// Fixture A: graph JSON using the PRODUCTION edge key `"links"` (rebuild.rs emits this).
/// Also includes a surface→module edge (surface:approvals → module:ApprovalsView) so the
/// heading join can key off the graph edge, not a filename guess.
const FIXTURE_GRAPH_LINKS: &str = r#"{
  "nodes": [
    { "id": "surface:approvals", "label": "approvals", "kind": "surface" },
    { "id": "cmd:vox_resolve_approval", "label": "vox_resolve_approval", "kind": "command" },
    { "id": "module:components/surfaces/Approvals/ApprovalsView.tsx", "label": "ApprovalsView", "kind": "module" }
  ],
  "links": [
    { "source": "surface:approvals", "target": "cmd:vox_resolve_approval", "confidence": "declared" },
    { "source": "surface:approvals", "target": "module:components/surfaces/Approvals/ApprovalsView.tsx", "confidence": "declared" }
  ]
}"#;

/// Fixture B: the SAME graph but using the legacy `"edges"` key — the emitter must read
/// `links` with an `edges` fallback (mirrors coverage.rs / lens.rs). Both shapes
/// must yield identical commands.
const FIXTURE_GRAPH_EDGES: &str = r#"{
  "nodes": [
    { "id": "surface:approvals", "label": "approvals", "kind": "surface" },
    { "id": "cmd:vox_resolve_approval", "label": "vox_resolve_approval", "kind": "command" }
  ],
  "edges": [
    { "source": "surface:approvals", "target": "cmd:vox_resolve_approval", "confidence": "declared" }
  ]
}"#;

/// Fixture C: a MULTI-WORD kebab view key (`sub-agents`) whose component is PascalCase
/// (`SubAgentsView.tsx`) — exercises the X5 join (must NOT mis-key via filename heuristic).
const FIXTURE_GRAPH_MULTIWORD: &str = r#"{
  "nodes": [
    { "id": "surface:sub-agents", "label": "sub-agents", "kind": "surface" },
    { "id": "module:components/surfaces/SubAgents/SubAgentsView.tsx", "label": "SubAgentsView", "kind": "module" }
  ],
  "links": [
    { "source": "surface:sub-agents", "target": "module:components/surfaces/SubAgents/SubAgentsView.tsx", "confidence": "declared" }
  ]
}"#;

/// Fixture: a minimal surface-registry YAML with one surface (approvals).
const FIXTURE_REGISTRY_YAML: &str = r#"x_vox_version: 2
schema_version: 1
surfaces:
- view_key: approvals
  cli_group: null
  representation_tier: live_backend
  nav_label: Approvals
  nav_icon: shield
  nav_group: operate
  parent_surface: runs
  notes: Operator approval queue for doubt_task feedback
"#;

/// Helper: emit + parse the manifest, return the parsed manifest value.
fn emit_and_parse(graph: &str, yaml: &str, surface_dir: &Path) -> serde_json::Value {
    let tmp = TempDir::new().unwrap();
    let out_path = tmp.path().join("gui-content-manifest.json");
    emit_content_manifest(graph, yaml, surface_dir, &out_path)
        .expect("emit_content_manifest must not error on valid fixture");
    let raw = std::fs::read_to_string(&out_path).expect("manifest file must be written");
    serde_json::from_str(&raw).expect("manifest must be valid JSON")
}

#[test]
fn manifest_golden_surface_approvals() {
    // surface_dir: empty (no TSX files) — headings will be []; commands come from graph edges.
    let surface_dir = TempDir::new().unwrap();
    let manifest = emit_and_parse(
        FIXTURE_GRAPH_LINKS,
        FIXTURE_REGISTRY_YAML,
        surface_dir.path(),
    );

    let surfaces = manifest["surfaces"]
        .as_array()
        .expect("must have a 'surfaces' array");
    let entry = surfaces
        .iter()
        .find(|s| s["view_key"].as_str() == Some("approvals"))
        .expect("approvals must appear in the manifest");

    // nav_label extracted from YAML
    assert_eq!(entry["nav_label"].as_str(), Some("Approvals"));
    // route derived as #view=<view_key>
    assert_eq!(entry["route"].as_str(), Some("#view=approvals"));
    // nav_group extracted from YAML
    assert_eq!(entry["nav_group"].as_str(), Some("operate"));
    // X11: docs field present (empty for VG-1) so VG-2's ContentManifestEntry.docs matches.
    assert!(
        entry["docs"].is_array(),
        "docs field must be present (even if empty)"
    );
    // headings present (empty array — no TSX file on disk for the module edge here)
    assert!(
        entry["headings"].is_array(),
        "headings field must be present (even if empty)"
    );
    // commands: must include vox_resolve_approval (from the cmd: edge under the "links" key)
    let commands = entry["commands"]
        .as_array()
        .expect("must have commands array");
    assert!(
        commands
            .iter()
            .any(|c| c.as_str() == Some("vox_resolve_approval")),
        "commands must include vox_resolve_approval (from graph 'links' edge); got: {:?}",
        commands
    );
}

/// X1 (Critical): the legacy `"edges"` key must produce the SAME commands as `"links"`.
/// Guards against the production-empty-commands regression.
#[test]
fn manifest_reads_edges_key_as_fallback() {
    let empty_dir = TempDir::new().unwrap();
    let manifest = emit_and_parse(FIXTURE_GRAPH_EDGES, FIXTURE_REGISTRY_YAML, empty_dir.path());
    let surfaces = manifest["surfaces"].as_array().unwrap();
    let entry = surfaces
        .iter()
        .find(|s| s["view_key"].as_str() == Some("approvals"))
        .expect("approvals must appear (edges-key fixture)");
    let commands = entry["commands"].as_array().expect("commands array");
    assert!(
        commands
            .iter()
            .any(|c| c.as_str() == Some("vox_resolve_approval")),
        "commands must be non-empty when edges live under the legacy 'edges' key; got: {:?}",
        commands
    );
}

/// X5: a multi-word kebab view key (`sub-agents`) with a PascalCase component
/// (`SubAgentsView.tsx`) must join headings via the graph surface→module edge — NOT a
/// filename heuristic (which would mis-key `subagents` ≠ `sub-agents`).
#[test]
fn manifest_headings_multiword_view_key() {
    // Lay out a real-ish component file under a surface_dir, reachable via the graph edge.
    let dir = TempDir::new().unwrap();
    let comp = dir
        .path()
        .join("components/surfaces/SubAgents/SubAgentsView.tsx");
    std::fs::create_dir_all(comp.parent().unwrap()).unwrap();
    std::fs::write(
        &comp,
        "export function SubAgentsView() {\n  return <section><h2>Sub-Agent Roster</h2></section>;\n}\n",
    )
    .unwrap();

    const YAML: &str = "x_vox_version: 2\nschema_version: 1\nsurfaces:\n- view_key: sub-agents\n  nav_label: Sub-Agents\n  nav_group: operate\n";

    let manifest = emit_and_parse(FIXTURE_GRAPH_MULTIWORD, YAML, dir.path());
    let surfaces = manifest["surfaces"].as_array().unwrap();
    let entry = surfaces
        .iter()
        .find(|s| s["view_key"].as_str() == Some("sub-agents"))
        .expect("sub-agents (multi-word kebab) must appear in the manifest");
    let headings = entry["headings"].as_array().expect("headings array");
    assert!(
        headings
            .iter()
            .any(|h| h.as_str() == Some("Sub-Agent Roster")),
        "heading must join via the graph surface→module edge for a multi-word key; got: {:?}",
        headings
    );
}

#[test]
fn manifest_module_exists() {
    let _ = emit_content_manifest
        as fn(&str, &str, &Path, &Path) -> Result<(), Box<dyn std::error::Error>>;
}
