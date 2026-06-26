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
