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

struct RustVisitor {
    nodes: Vec<ExtractedNode>,
    edges: Vec<ExtractedEdge>,
    current_fn: Option<String>,
}

impl<'ast> Visit<'ast> for RustVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let fn_name = node.sig.ident.to_string();
        self.nodes.push(ExtractedNode {
            id: fn_name.clone(),
            label: fn_name.clone(),
            kind: "fn".to_string(),
        });

        let old_fn = self.current_fn.replace(fn_name);
        syn::visit::visit_item_fn(self, node);
        self.current_fn = old_fn;
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let struct_name = node.ident.to_string();
        self.nodes.push(ExtractedNode {
            id: struct_name.clone(),
            label: struct_name,
            kind: "struct".to_string(),
        });
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(ref expr_path) = *node.func {
            if let Some(ref current_fn) = self.current_fn {
                if let Some(segment) = expr_path.path.segments.last() {
                    let callee = segment.ident.to_string();
                    self.edges.push(ExtractedEdge {
                        source: current_fn.clone(),
                        target: callee,
                    });
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

pub fn extract_ast(path: &Path, content: &str) -> ExtractedGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    if path.extension().map_or(false, |ext| ext == "rs") {
        if let Ok(file) = syn::parse_file(content) {
            let mut visitor = RustVisitor {
                nodes: Vec::new(),
                edges: Vec::new(),
                current_fn: None,
            };
            visitor.visit_file(&file);
            nodes = visitor.nodes;
            edges = visitor.edges;
        }
    } else {
        // Multi-language tree-sitter fallback (under tree-sitter-grammars feature)
        #[cfg(feature = "tree-sitter-grammars")]
        {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let language = match ext {
                    "ts" | "js" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
                    "tsx" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX),
                    _ => None,
                };
                if let Some(lang) = language {
                    let mut parser = tree_sitter::Parser::new();
                    if parser.set_language(&lang.into()).is_ok() {
                        if let Some(tree) = parser.parse(content, None) {
                            let mut cursor = tree.walk();
                            // Simple traversal to extract functions/calls
                            let mut stack = vec![tree.root_node()];
                            let mut current_fn: Option<String> = None;
                            while let Some(node) = stack.pop() {
                                if node.kind() == "function_declaration"
                                    || node.kind() == "method_definition"
                                {
                                    if let Some(name_node) = node.child_by_field_name("name") {
                                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                                            nodes.push(ExtractedNode {
                                                id: name.to_string(),
                                                label: name.to_string(),
                                                kind: "fn".to_string(),
                                            });
                                            current_fn = Some(name.to_string());
                                        }
                                    }
                                }
                                if node.kind() == "call_expression" {
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
