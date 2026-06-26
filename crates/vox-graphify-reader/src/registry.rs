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

/// Parse an MCP dispatch table: for each line, take the quoted literal left of
/// `=>`; if it starts with `vox_`, emit a `tool:` node.
pub fn mcp_tool_nodes(src: &str) -> Vec<RegistryNode> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in src.lines() {
        let Some(arrow) = line.find("=>") else {
            continue;
        };
        let left = &line[..arrow];
        // take the last quoted literal on the left of `=>`
        let Some(close) = left.rfind('"') else {
            continue;
        };
        let Some(open) = left[..close].rfind('"') else {
            continue;
        };
        let name = &left[open + 1..close];
        if name.starts_with("vox_") && seen.insert(name.to_string()) {
            out.push(RegistryNode::new("tool", name, "tool"));
        }
    }
    out
}

/// Parse the generated surface registry: match `viewKey:` (NOT `id:`), skip when
/// the value starts with `null`, else take the quoted id → `surface:` node.
pub fn surface_nodes(src: &str) -> Vec<RegistryNode> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in src.lines() {
        let Some(idx) = line.find("viewKey:") else {
            continue;
        };
        let rest = line[idx + "viewKey:".len()..].trim_start();
        if rest.starts_with("null") {
            continue;
        }
        // take the first quoted literal (single or double quote)
        let bytes = rest.as_bytes();
        let Some(q) = bytes.iter().position(|&c| c == b'\'' || c == b'"') else {
            continue;
        };
        let quote = bytes[q];
        let after = &rest[q + 1..];
        let Some(end) = after.find(quote as char) else {
            continue;
        };
        let id = &after[..end];
        if !id.is_empty() && seen.insert(id.to_string()) {
            out.push(RegistryNode::new("surface", id, "surface"));
        }
    }
    out
}
