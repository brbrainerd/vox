//! Registry ingest: parse `#[tauri::command]` functions via `syn` and flag
//! those that are defined but never registered with `generate_handler!`.

#[derive(Clone, Debug, PartialEq)]
pub struct RegistryNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub unregistered: bool,
}

impl RegistryNode {
    fn new(prefix: &str, name: &str, kind: &str) -> Self {
        RegistryNode {
            id: format!("{prefix}:{name}"),
            label: name.to_string(),
            kind: kind.to_string(),
            unregistered: false,
        }
    }
}

/// Parse `#[tauri::command]` fns with syn; flag those absent from `registered`.
pub fn tauri_command_nodes(src: &str, registered: &[&str]) -> Vec<RegistryNode> {
    let mut out = Vec::new();
    let Ok(file) = syn::parse_file(src) else {
        return out;
    };
    for item in file.items {
        if let syn::Item::Fn(f) = item {
            let is_cmd = f
                .attrs
                .iter()
                .any(|a| a.path().segments.iter().any(|s| s.ident == "command"));
            if is_cmd {
                let name = f.sig.ident.to_string();
                let mut n = RegistryNode::new("cmd", &name, "command");
                n.unregistered = !registered.contains(&name.as_str());
                out.push(n);
            }
        }
    }
    out
}
