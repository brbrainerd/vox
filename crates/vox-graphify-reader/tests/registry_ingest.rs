use vox_graphify_reader::registry::{tauri_command_nodes, RegistryNode};

#[test]
fn extracts_tauri_commands_and_flags_unregistered() {
    let src = "#[tauri::command]\npub async fn do_it(x:u64)->Result<(),String>{Ok(())}\n#[tauri::command]\nfn hidden(){}\nfn helper(){}";
    let registered = ["do_it"]; // generate_handler! lists do_it but not hidden
    let nodes = tauri_command_nodes(src, &registered);
    let d = nodes.iter().find(|n| n.id == "cmd:do_it").unwrap();
    assert_eq!(d.kind, "command");
    assert!(!d.unregistered);
    let h = nodes.iter().find(|n| n.id == "cmd:hidden").unwrap();
    assert!(h.unregistered, "hidden should be flagged dead");
    assert!(!nodes.iter().any(|n| n.label == "helper"));
}

#[test]
fn registry_node_fields_are_public() {
    let n = RegistryNode {
        id: "cmd:x".into(),
        label: "x".into(),
        kind: "command".into(),
        unregistered: false,
    };
    assert_eq!(n.label, "x");
}

#[test]
fn extracts_tools_and_surfaces_viewkey() {
    use vox_graphify_reader::registry::{mcp_tool_nodes, surface_nodes};
    let dispatch = "  \"vox_resolve_feedback\" => f::r(a),\n  \"vox_skill_info\" => s::i(a),";
    assert!(mcp_tool_nodes(dispatch)
        .iter()
        .any(|n| n.id == "tool:vox_resolve_feedback" && n.kind == "tool"));
    let reg = "{ viewKey: 'chat', cliGroup: null, tier: 'live_backend' },\n  { viewKey: null, cliGroup: 'add', tier: 'none' },";
    let s = surface_nodes(reg);
    assert!(s.iter().any(|n| n.id == "surface:chat"));
    assert!(!s.iter().any(|n| n.label == "null"), "null viewKey must be skipped");
}

#[test]
fn maps_wrapper_methods_to_commands() {
    use vox_graphify_reader::registry::transport_wrapper_map;
    let ts = "doubtTask(taskId: number){ return invoke('doubt_orchestrator_task', { taskId }); }\n  getCatalog(){ return invoke('get_command_catalog'); }";
    let m = transport_wrapper_map(ts);
    assert_eq!(
        m.get("doubtTask").map(String::as_str),
        Some("cmd:doubt_orchestrator_task")
    );
    assert_eq!(
        m.get("getCatalog").map(String::as_str),
        Some("cmd:get_command_catalog")
    );
}

#[test]
fn maps_wrapper_methods_to_tools() {
    use vox_graphify_reader::registry::transport_wrapper_map;
    let ts = "feedbackList(){ return invoke('invoke_mcp_tool', { tool: 'vox_feedback_list', args: {} }); }";
    let m = transport_wrapper_map(ts);
    assert_eq!(
        m.get("feedbackList").map(String::as_str),
        Some("tool:vox_feedback_list")
    );
}

#[test]
fn real_transport_yields_sane_wrapper_map() {
    use std::path::PathBuf;
    use vox_graphify_reader::registry::transport_wrapper_map;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let ts_src =
        std::fs::read_to_string(root.join("crates/vox-gui/ui/src/transport.ts")).expect("read transport.ts");
    let m = transport_wrapper_map(&ts_src);
    assert!(m.len() >= 20, "under-extracted wrappers: {}", m.len());
}

#[test]
fn real_files_yield_sane_counts() {
    use std::path::PathBuf;
    use vox_graphify_reader::registry::{mcp_tool_nodes, surface_nodes};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let surface_src = std::fs::read_to_string(
        root.join("crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts"),
    )
    .expect("read surfaceRegistry.generated.ts");
    let surfaces = surface_nodes(&surface_src);
    assert!(
        surfaces.len() >= 20,
        "under-extracted surfaces: {}",
        surfaces.len()
    );
    let dispatch_src =
        std::fs::read_to_string(root.join("crates/vox-orchestrator-mcp/src/dispatch.rs"))
            .expect("read dispatch.rs");
    let tools = mcp_tool_nodes(&dispatch_src);
    assert!(tools.len() >= 30, "under-extracted tools: {}", tools.len());
}
