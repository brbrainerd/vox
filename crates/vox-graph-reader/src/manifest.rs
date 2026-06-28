//! GUI content-manifest emitter.
//!
//! Emits `gui-content-manifest.json` from the structural graph + surface-registry YAML.
//! Called by `rebuild::rebuild_graph` in gui-wiring mode after the graph is written.

use std::path::Path;

/// Emit `gui-content-manifest.json` alongside the graph in `out_path`.
///
/// # Parameters
/// - `graph_json` — the already-written `graph.json` string (read it back, or pass in).
/// - `surface_registry_yaml` — contents of `contracts/gui/surface-registry.v1.yaml`.
/// - `surface_dir` — the GUI `ui/src/` directory to scan for headings in TSX files.
/// - `out_path` — destination file path (typically `<cache_dir>/gui-content-manifest.json`).
pub fn emit_content_manifest(
    _graph_json: &str,
    _surface_registry_yaml: &str,
    _surface_dir: &Path,
    _out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Implemented in Task G7.
    Err("emit_content_manifest: not yet implemented (VG-1 Task G7)".into())
}
