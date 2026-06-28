//! Guard: MCP dispatch + input-schema string keys route the renamed `vox_search_*`
//! tools and no longer carry the retired `vox_graphify_*` prefix (plan vs1, T3).

#[test]
fn dispatch_routes_vox_search_keys_not_graphify() {
    let dispatch = include_str!("../src/dispatch.rs");
    let schemas = include_str!("../src/input_schemas.rs");
    for s in [dispatch, schemas] {
        assert!(!s.contains("\"vox_graphify_"), "old vox_graphify_ key still present");
    }
    for k in [
        "\"vox_search_status\"",
        "\"vox_search_structural\"",
        "\"vox_search_neighbors\"",
        "\"vox_search_path\"",
        "\"vox_search_compare\"",
        "\"vox_search_rebuild\"",
    ] {
        assert!(dispatch.contains(k), "dispatch missing {k}");
        assert!(schemas.contains(k), "schemas missing {k}");
    }
}
