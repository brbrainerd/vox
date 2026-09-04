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

/// Parse `transport.ts` wrapper methods of the form `name(...){ ... invoke('CMD'... }`.
/// Maps each wrapper method name to its underlying command/tool target:
/// - `invoke('CMD'...)` → `name -> cmd:CMD`
/// - `invoke('invoke_mcp_tool', { tool: 'T'...)` → `name -> tool:T`
///
/// The wrappers in `transport.ts` are one-liners, so a region scan from each
/// method header to its `invoke(...)` call is sufficient.
pub fn transport_wrapper_map(ts_src: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    // The current open wrapper method name (set by a header line, cleared once an
    // invoke is matched or another header is seen). Wrappers are short, so the
    // first `invoke(...)` after a header is taken as that wrapper's target.
    let mut current: Option<String> = None;
    for line in ts_src.lines() {
        // A header line opens (or replaces) the current wrapper context.
        if let Some((name, after_open)) = method_header(line) {
            current = Some(name.to_string());
            // a single-line wrapper may also carry the invoke on the same line
            if try_map_invoke(&mut out, name, after_open) {
                current = None;
            }
            continue;
        }
        if let Some(name) = current.clone() {
            if try_map_invoke(&mut out, &name, line) {
                current = None;
            }
        }
    }
    out
}

/// Find the start of an `invoke(...)`/`Invoke(...)`/`invoke<...>(...)` call
/// (case-insensitive on the leading letter, so it matches both the raw
/// `invoke(...)` calls and transport.ts's `safeInvoke(...)` wrapper) —
/// distinguished from an `invoke_mcp_tool` string literal by requiring the
/// match be immediately followed by `(` or `<`, not an identifier character.
fn find_invoke_call(region: &str) -> Option<usize> {
    let bytes = region.as_bytes();
    let mut search_from = 0;
    while let Some(rel) = region[search_from..].find("nvoke") {
        let pos = search_from + rel;
        let prev_is_i = pos > 0 && matches!(bytes[pos - 1], b'i' | b'I');
        let after = pos + "nvoke".len();
        let next_is_call = after < bytes.len() && matches!(bytes[after], b'(' | b'<');
        if prev_is_i && next_is_call {
            return Some(pos - 1);
        }
        search_from = pos + "nvoke".len();
    }
    None
}

/// If `region` contains an `invoke(...)`/`invoke<…>(...)` call, record the mapping
/// for `name` and return true. Handles command and `invoke_mcp_tool` tool forms.
fn try_map_invoke(
    out: &mut std::collections::HashMap<String, String>,
    name: &str,
    region: &str,
) -> bool {
    // locate `invoke`/`Invoke` followed (possibly after a `<...>` generic) by `(`
    let Some(pos) = find_invoke_call(region) else {
        return false;
    };
    let after = region[pos + "invoke".len()..].trim_start();
    // skip an optional generic `<...>`
    let after = if let Some(stripped) = after.strip_prefix('<') {
        match stripped.find('>') {
            Some(gt) => stripped[gt + 1..].trim_start(),
            None => return false,
        }
    } else {
        after
    };
    let Some(rest) = after.strip_prefix('(') else {
        return false;
    };
    let Some(first) = first_quoted(rest) else {
        return false;
    };
    if first == "invoke_mcp_tool" {
        if let Some(tpos) = rest.find("tool:") {
            let after_tool = &rest[tpos + "tool:".len()..];
            if let Some(tool) = first_quoted(after_tool) {
                out.insert(name.to_string(), format!("tool:{tool}"));
                return true;
            }
        }
        false
    } else {
        out.insert(name.to_string(), format!("cmd:{first}"));
        true
    }
}

/// If `line` begins (after whitespace and optional `async`) with `ident(`,
/// return `(ident, slice_after_the_open_paren)`.
fn method_header(line: &str) -> Option<(&str, &str)> {
    let mut s = line.trim_start();
    if let Some(stripped) = s.strip_prefix("async ") {
        s = stripped.trim_start();
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let ident_char = c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
        if ident_char {
            i += 1;
        } else {
            break;
        }
    }
    if i == 0 || i >= bytes.len() || bytes[i] != b'(' {
        return None;
    }
    // first char must not be a digit
    if bytes[0].is_ascii_digit() {
        return None;
    }
    let name = &s[..i];
    // exclude JS keywords that are also followed by `(` (control flow / calls)
    if matches!(
        name,
        "if" | "for" | "while" | "switch" | "catch" | "return" | "await" | "function" | "invoke"
    ) {
        return None;
    }
    Some((name, &s[i + 1..]))
}

/// Return the contents of the first single- or double-quoted literal in `s`.
fn first_quoted(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let q = bytes.iter().position(|&c| c == b'\'' || c == b'"')?;
    let quote = bytes[q];
    let after = &s[q + 1..];
    let end = after.find(quote as char)?;
    Some(&after[..end])
}

/// Ingest the clap command catalog (`vox commands --format json --include-nested`,
/// i.e. the serialized `CommandCatalog`) as `cli:<group>:<command>` leaf nodes.
/// Top-level groups (path.len()==1) are emitted as `cli:<group>` group nodes; deeper
/// paths become leaves keyed by the first (group) and last (command) segments.
/// Malformed JSON yields an empty Vec — never panics (honesty: under-report).
pub fn cli_command_nodes(catalog_json: &str) -> Vec<RegistryNode> {
    let mut out = Vec::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(catalog_json) else {
        return out;
    };
    let Some(entries) = v.get("entries").and_then(|e| e.as_array()) else {
        return out;
    };
    for e in entries {
        let Some(path) = e.get("path").and_then(|p| p.as_array()) else {
            continue;
        };
        let segs: Vec<String> = path
            .iter()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect();
        match segs.as_slice() {
            [] => {}
            [group] => {
                let mut n = RegistryNode::new("cli", group, "cli-group");
                n.id = format!("cli:{group}");
                out.push(n);
            }
            [group, .., command] => {
                let mut n = RegistryNode::new("cli", command, "cli-command");
                n.id = format!("cli:{group}:{command}");
                out.push(n);
            }
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
