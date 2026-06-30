use std::collections::HashMap;
use std::path::Path;
use syn::visit::Visit;

/// Frontend callees that mark a backend boundary crossing. The arg0 string literal becomes
/// the target. TODO: a declarative config when a 3rd boundary kind lands — see spec §non-fork.
const BOUNDARY_CALLEES: &[&str] = &["invoke", "callTool"];

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ExtractedNode {
    pub id: String,
    pub label: String,
    pub kind: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ExtractedEdge {
    pub source: String,
    pub target: String,
    #[serde(default = "default_confidence")]
    pub confidence: String,
}

pub(crate) fn default_confidence() -> String {
    "resolved".into()
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ExtractedGraph {
    pub nodes: Vec<ExtractedNode>,
    pub edges: Vec<ExtractedEdge>,
}

/// Bump when the extraction scheme changes (node-id format, edge rules). Folded into the
/// per-file cache key in `rebuild` so unchanged files re-extract instead of returning a
/// graph built under the old scheme.
pub const EXTRACTOR_VERSION: &str = "3";

/// Qualify a symbol with its module path. Empty `module_id` yields the bare symbol so the
/// legacy `extract_ast` wrapper keeps its old output.
pub(crate) fn qualify(module_id: &str, sym: &str) -> String {
    if module_id.is_empty() {
        sym.to_string()
    } else {
        format!("{module_id}::{sym}")
    }
}

struct RustVisitor {
    module_id: String,
    nodes: Vec<ExtractedNode>,
    edges: Vec<ExtractedEdge>,
    current_fn: Option<String>,
}

impl<'ast> Visit<'ast> for RustVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let fn_name = node.sig.ident.to_string();
        let id = qualify(&self.module_id, &fn_name);
        self.nodes.push(ExtractedNode {
            id: id.clone(),
            label: fn_name,
            kind: "fn".to_string(),
        });
        let old_fn = self.current_fn.replace(id);
        syn::visit::visit_item_fn(self, node);
        self.current_fn = old_fn;
    }
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let struct_name = node.ident.to_string();
        self.nodes.push(ExtractedNode {
            id: qualify(&self.module_id, &struct_name),
            label: struct_name,
            kind: "struct".to_string(),
        });
        syn::visit::visit_item_struct(self, node);
    }
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(ref expr_path) = *node.func {
            if let Some(ref current_fn) = self.current_fn {
                if let Some(segment) = expr_path.path.segments.last() {
                    self.edges.push(ExtractedEdge {
                        source: current_fn.clone(),
                        target: segment.ident.to_string(), // BARE; resolved in rebuild
                        confidence: "resolved".into(),
                    });
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

/// Back-compat wrapper: bare ids. Used by `ast_tests` (which assert on `label`, unaffected).
pub fn extract_ast(path: &Path, content: &str) -> ExtractedGraph {
    extract_ast_in_module(path, content, "")
}

/// Per-file AST. Definition ids and edge sources are qualified with `module_id`; edge
/// targets are left bare for global resolution in `rebuild`.
pub fn extract_ast_in_module(path: &Path, content: &str, module_id: &str) -> ExtractedGraph {
    extract_ast_in_module_with_wrappers(path, content, module_id, &HashMap::new())
}

/// Like `extract_ast_in_module`, but with a `voxTransport.<method>` → target map so wrapper
/// calls become declared boundary edges. `invoke`/`callTool` boundary crossings are detected
/// from their arg0 string literal regardless of the map.
pub fn extract_ast_in_module_with_wrappers(
    path: &Path,
    content: &str,
    module_id: &str,
    wrappers: &HashMap<String, String>,
) -> ExtractedGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    if path.extension().map_or(false, |ext| ext == "rs") {
        if let Ok(file) = syn::parse_file(content) {
            let mut visitor = RustVisitor {
                module_id: module_id.to_string(),
                nodes: Vec::new(),
                edges: Vec::new(),
                current_fn: None,
            };
            visitor.visit_file(&file);
            nodes = visitor.nodes;
            edges = visitor.edges;
        }
    } else {
        #[cfg(feature = "tree-sitter-grammars")]
        {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let language = match ext {
                    "ts" | "js" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
                    "tsx" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX),
                    "py" => Some(tree_sitter_python::LANGUAGE),
                    _ => None,
                };
                // tree-sitter-typescript 0.23.2 node-kind names (discovered via A0d
                // print-test against LANGUAGE_TSX; use these verbatim in B1/D1):
                //   JSX self-closing element : "jsx_self_closing_element"
                //     - component name        : child_by_field_name("name") -> "identifier"
                //       (Capitalized => component; lowercase => DOM tag)
                //     - opening element       : "jsx_opening_element"
                //   import statement          : "import_statement"
                //     - named import ident    : "import_clause" > "named_imports" >
                //                               "import_specifier"; the bound name is the
                //                               specifier's child_by_field_name("name") -> "identifier"
                //   call_expression           : "call_expression"
                //     - args field            : "arguments" (positional children, named)
                //     - string literal kind   : "string" (text via "string_fragment" child)
                //     - object literal kind    : "object"
                //     - object entry kind      : "pair" (key "property_identifier", value e.g. "string")
                if let Some(lang) = language {
                    let mut parser = tree_sitter::Parser::new();
                    if parser.set_language(&lang.into()).is_ok() {
                        if let Some(tree) = parser.parse(content, None) {
                            let mut cursor = tree.walk();
                            let mut stack = vec![tree.root_node()];
                            let mut current_fn: Option<String> = None;
                            while let Some(node) = stack.pop() {
                                let is_fn_def = matches!(
                                    node.kind(),
                                    "function_declaration"
                                        | "method_definition"
                                        | "function_definition"
                                );
                                let is_class = node.kind() == "class_definition";
                                let is_call = matches!(node.kind(), "call_expression" | "call");

                                if is_fn_def || is_class {
                                    if let Some(name_node) = node.child_by_field_name("name") {
                                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                                            let id = qualify(module_id, name);
                                            nodes.push(ExtractedNode {
                                                id: id.clone(),
                                                label: name.to_string(),
                                                kind: if is_class { "struct" } else { "fn" }
                                                    .to_string(),
                                            });
                                            if is_fn_def {
                                                current_fn = Some(id);
                                            }
                                        }
                                    }
                                }
                                // JSX element usage => composition edge (only for
                                // Capitalized names; lowercase = DOM tags).
                                let is_jsx = matches!(
                                    node.kind(),
                                    "jsx_self_closing_element" | "jsx_opening_element"
                                );
                                if is_jsx {
                                    if let Some(ref source_fn) = current_fn {
                                        if let Some(name_node) = node.child_by_field_name("name") {
                                            if let Ok(name) =
                                                name_node.utf8_text(content.as_bytes())
                                            {
                                                if name
                                                    .chars()
                                                    .next()
                                                    .is_some_and(|c| c.is_uppercase())
                                                {
                                                    edges.push(ExtractedEdge {
                                                        source: source_fn.clone(),
                                                        target: name.to_string(),
                                                        confidence: "resolved".into(),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                                if is_call {
                                    if let Some(ref source_fn) = current_fn {
                                        if let Some(function_node) =
                                            node.child_by_field_name("function")
                                        {
                                            if let Ok(callee) =
                                                function_node.utf8_text(content.as_bytes())
                                            {
                                                let bare =
                                                    callee.rsplit('.').next().unwrap_or(callee);
                                                let args = node.child_by_field_name("arguments");
                                                if BOUNDARY_CALLEES.contains(&bare) {
                                                    // invoke('cmd') / callTool('cmd'); special-case
                                                    // invoke('invoke_mcp_tool', { tool: '...' }).
                                                    if let Some(arg0) = args.and_then(|a| {
                                                        arg_string_literal(a, 0, content)
                                                    }) {
                                                        if arg0 == "invoke_mcp_tool" {
                                                            if let Some(tool) = args.and_then(|a| {
                                                                arg_object_string_field(
                                                                    a, 1, "tool", content,
                                                                )
                                                            }) {
                                                                edges.push(ExtractedEdge {
                                                                    source: source_fn.clone(),
                                                                    target: format!("tool:{tool}"),
                                                                    confidence: "declared".into(),
                                                                });
                                                            }
                                                        } else {
                                                            edges.push(ExtractedEdge {
                                                                source: source_fn.clone(),
                                                                target: format!("cmd:{arg0}"),
                                                                confidence: "declared".into(),
                                                            });
                                                        }
                                                    }
                                                    // Boundary call handled: do NOT also emit the
                                                    // bare-call edge (prevents the double-count).
                                                    for child in node.children(&mut cursor) {
                                                        stack.push(child);
                                                    }
                                                    continue;
                                                } else if callee.starts_with("voxTransport.") {
                                                    if let Some(target) = wrappers.get(bare) {
                                                        edges.push(ExtractedEdge {
                                                            source: source_fn.clone(),
                                                            target: target.clone(),
                                                            confidence: "declared".into(),
                                                        });
                                                    }
                                                    for child in node.children(&mut cursor) {
                                                        stack.push(child);
                                                    }
                                                    continue;
                                                }
                                                edges.push(ExtractedEdge {
                                                    source: source_fn.clone(),
                                                    target: callee.to_string(),
                                                    confidence: "resolved".into(),
                                                });
                                            }
                                        }
                                    }
                                }
                                for child in node.children(&mut cursor) {
                                    stack.push(child);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ExtractedGraph { nodes, edges }
}

/// Text of a `string` literal node, stripped of its quote delimiters. Uses the A0d-recorded
/// kind names (string literal == "string").
#[cfg(feature = "tree-sitter-grammars")]
fn string_literal_value(node: tree_sitter::Node, content: &str) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }
    let raw = node.utf8_text(content.as_bytes()).ok()?;
    let trimmed = raw
        .strip_prefix('\'')
        .or_else(|| raw.strip_prefix('"'))
        .or_else(|| raw.strip_prefix('`'))
        .unwrap_or(raw);
    let trimmed = trimmed
        .strip_suffix('\'')
        .or_else(|| trimmed.strip_suffix('"'))
        .or_else(|| trimmed.strip_suffix('`'))
        .unwrap_or(trimmed);
    Some(trimmed.to_string())
}

/// Value of the positional argument at `idx` in a `call_expression`'s `arguments` node, if it
/// is a string literal. Template-string / computed args → None (honest miss).
#[cfg(feature = "tree-sitter-grammars")]
fn arg_string_literal(args: tree_sitter::Node, idx: usize, content: &str) -> Option<String> {
    let mut cursor = args.walk();
    let arg = args.named_children(&mut cursor).nth(idx)?;
    string_literal_value(arg, content)
}

/// Value of `field`'s string entry inside the object literal positional argument at `idx`.
#[cfg(feature = "tree-sitter-grammars")]
fn arg_object_string_field(
    args: tree_sitter::Node,
    idx: usize,
    field: &str,
    content: &str,
) -> Option<String> {
    let mut cursor = args.walk();
    let obj = args.named_children(&mut cursor).nth(idx)?;
    if obj.kind() != "object" {
        return None;
    }
    let mut obj_cursor = obj.walk();
    for pair in obj.named_children(&mut obj_cursor) {
        if pair.kind() != "pair" {
            continue;
        }
        let key = pair.child_by_field_name("key")?;
        let key_text = key.utf8_text(content.as_bytes()).ok()?;
        if key_text == field {
            let value = pair.child_by_field_name("value")?;
            return string_literal_value(value, content);
        }
    }
    None
}
