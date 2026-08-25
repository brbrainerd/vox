//! Full inventory JSON generation.

use std::fs;
use std::path::Path;

use anyhow::Result;
use chrono::{SecondsFormat, Utc};

use crate::constants::{INVENTORY_DESCRIPTION, SYMBOL_HINT_PATHS};
use crate::file_entry::build_file_entry;
use crate::hints::rust_symbol_hints;
use crate::types::{DocInventory, SymbolHintGroup};
use crate::walk::iter_repo_files;

/// Generate inventory JSON at `out_path` (default: `docs/agents/doc-inventory.json`).
pub fn generate(root: &Path, out_path: &Path) -> Result<DocInventory> {
    let paths = iter_repo_files(root)?;
    let mut files: Vec<crate::types::FileEntry> = paths
        .iter()
        .map(|p| {
            let rel = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
            build_file_entry(root, &rel)
        })
        .collect::<Result<Vec<_>>>()?;
    files.sort_by(|a, b| {
        let ta = a.hotspot_tier;
        let tb = b.hotspot_tier;
        let da = a.lines_triple_slash + a.lines_inner_doc;
        let db = b.lines_triple_slash + b.lines_inner_doc;
        tb.cmp(&ta)
            .then_with(|| (db as i64).cmp(&(da as i64)))
            .then_with(|| (b.lines_total as i64).cmp(&(a.lines_total as i64)))
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut symbol_hints: Vec<SymbolHintGroup> = Vec::new();
    for rel in SYMBOL_HINT_PATHS {
        let p = root.join(rel);
        if !p.is_file() {
            continue;
        }
        let text = vox_bounded_fs::read_utf8_path_capped(&p).unwrap_or_else(|_| String::new());
        let hints = rust_symbol_hints(&text);
        if !hints.is_empty() {
            symbol_hints.push(SymbolHintGroup {
                path: (*rel).to_string(),
                hints,
            });
        }
    }

    let inv = DocInventory {
        schema_version: 3,
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        description: INVENTORY_DESCRIPTION.to_string(),
        first_read_for_agents: vec![
            "AGENTS.md".into(),
            "docs/agents/orchestrator.md".into(),
            "docs/src/reference/cli.md".into(),
            // vox-mcp was renamed vox-orchestrator-mcp and its `tools/`
            // subdirectory flattened; lib.rs is now the tool registry and
            // dispatcher (see its module doc). The old path was hardcoded
            // here, so regenerating the inventory reproduced the dead entry.
            "crates/vox-orchestrator-mcp/src/lib.rs".into(),
            "docs/agents/doc-inventory.json".into(),
        ],
        files,
        symbol_hints,
    };

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&inv)? + "\n";
    fs::write(out_path, json)?;
    Ok(inv)
}
