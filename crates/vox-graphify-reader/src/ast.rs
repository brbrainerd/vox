use std::path::Path;
use syn::visit::Visit;

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
                                if is_call {
                                    if let Some(ref source_fn) = current_fn {
                                        if let Some(function_node) =
                                            node.child_by_field_name("function")
                                        {
                                            if let Ok(callee) =
                                                function_node.utf8_text(content.as_bytes())
                                            {
                                                edges.push(ExtractedEdge {
                                                    source: source_fn.clone(),
                                                    target: callee.to_string(),
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
