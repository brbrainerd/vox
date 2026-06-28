//! Guard tests for the `vox graphify` -> `vox search` absorption rename (plan vs1).

use clap::Parser;
use vox_cli::{Cli, VoxCliRoot};

#[test]
fn search_verb_parses_and_graphify_alias_still_resolves() {
    // New canonical verb.
    let root = VoxCliRoot::try_parse_from(["vox", "search", "status"]).expect("vox search status");
    assert!(matches!(root.cmd, Cli::Search { .. }));
    // One-release deprecation alias.
    let aliased =
        VoxCliRoot::try_parse_from(["vox", "graphify", "status"]).expect("alias resolves");
    assert!(matches!(aliased.cmd, Cli::Search { .. }));
}

#[test]
fn catalog_has_no_graphify_tool_prefix() {
    let yaml = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/operations/catalog.v1.yaml",
    ))
    .expect("read catalog");
    assert!(
        !yaml.contains("name: vox_graphify_"),
        "graphify MCP tool prefix must be renamed to vox_search_ in the catalog SSOT"
    );
    for t in [
        "vox_search_status",
        "vox_search_structural",
        "vox_search_neighbors",
        "vox_search_path",
        "vox_search_compare",
        "vox_search_rebuild",
    ] {
        assert!(yaml.contains(&format!("name: {t}")), "missing renamed tool {t}");
    }
}
