use serde_json::{Value, json};

pub fn overlay_test_targets(
    graph: &Value,
    _file_path: &str,
    test_content: &str,
) -> Result<Value, String> {
    let mut updated = graph.clone();
    let mut targets = std::collections::HashMap::new();

    // Parse references of functions in the test content
    if let Ok(file) = syn::parse_file(test_content) {
        use syn::visit::Visit;
        struct TestVisitor {
            current_test: Option<String>,
            calls: std::collections::HashMap<String, Vec<String>>,
        }
        impl<'ast> Visit<'ast> for TestVisitor {
            fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
                let name = node.sig.ident.to_string();
                if name.starts_with("test_") {
                    let old = self.current_test.replace(name.clone());
                    syn::visit::visit_item_fn(self, node);
                    self.current_test = old;
                } else {
                    syn::visit::visit_item_fn(self, node);
                }
            }
            fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
                if let syn::Expr::Path(ref expr_path) = *node.func {
                    if let Some(ref current_test) = self.current_test {
                        if let Some(segment) = expr_path.path.segments.last() {
                            let callee = segment.ident.to_string();
                            self.calls
                                .entry(callee)
                                .or_default()
                                .push(current_test.clone());
                        }
                    }
                }
                syn::visit::visit_expr_call(self, node);
            }
        }
        let mut visitor = TestVisitor {
            current_test: None,
            calls: std::collections::HashMap::new(),
        };
        visitor.visit_file(&file);
        targets = visitor.calls;
    }

    if let Some(nodes) = updated.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|i| i.as_str()) {
                if let Some(test_names) = targets.get(id) {
                    node.as_object_mut()
                        .unwrap()
                        .insert("targeted_by".to_string(), json!(test_names));
                }
            }
        }
    }

    Ok(updated)
}
