use crate::extractor::LanguageExtractor;
use crate::features::{
    BodySignature, CallSite, ExtractedFeatures, FnDef, ImportLoc, LiteralContext, LiteralLoc, Loc,
    NumericLoc, UnitHint,
};
use anyhow::Result;
use proc_macro2::LineColumn;
use quote::quote;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprLit, Lit};
use vox_code_audit::rules::Language;
use xxhash_rust::xxh3::xxh3_64;

pub struct RustExtractor;

struct RustVisitor {
    features: ExtractedFeatures,
}

impl RustVisitor {
    fn span_to_loc(span: proc_macro2::Span) -> Loc {
        let lc: LineColumn = span.start();
        Loc {
            line: lc.line,
            col: lc.column,
        }
    }
}

impl<'ast> Visit<'ast> for RustVisitor {
    fn visit_expr_lit(&mut self, node: &'ast ExprLit) {
        match &node.lit {
            Lit::Str(s) => {
                self.features.string_literals.push(LiteralLoc {
                    value: s.value(),
                    loc: Self::span_to_loc(s.span()),
                    ctx: LiteralContext::Code,
                });
            }
            Lit::Int(i) => {
                if let Ok(v) = i.base10_parse::<i64>() {
                    self.features.numeric_literals.push(NumericLoc {
                        value: v as f64,
                        unit: None,
                        loc: Self::span_to_loc(i.span()),
                    });
                }
            }
            _ => {}
        }
        syn::visit::visit_expr_lit(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        // Recurse first so visit_expr_lit fires for all args, then we retag below
        syn::visit::visit_expr_call(self, node);

        if let Expr::Path(p) = node.func.as_ref() {
            let segs: Vec<String> = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();

            // Tag Duration::from_* numeric args with unit hints
            let unit = match segs.last().map(|s| s.as_str()) {
                Some("from_secs") | Some("from_secs_f64") => Some(UnitHint::Seconds),
                Some("from_millis") => Some(UnitHint::Millis),
                Some("from_nanos") => Some(UnitHint::Millis),
                _ => None,
            };
            if let Some(unit) = unit
                && let Some(arg) = node.args.first()
                && let Expr::Lit(ExprLit {
                    lit: Lit::Int(i), ..
                }) = arg
                && let Ok(v) = i.base10_parse::<i64>()
            {
                let loc = Self::span_to_loc(i.span());
                // Remove the untagged entry visit_expr_lit already added
                self.features.numeric_literals.retain(|n| {
                    !(n.unit.is_none()
                        && n.value == v as f64
                        && n.loc.line == loc.line
                        && n.loc.col == loc.col)
                });
                self.features.numeric_literals.push(NumericLoc {
                    value: v as f64,
                    unit: Some(unit),
                    loc,
                });
            }

            // Record call site
            self.features.call_sites.push(CallSite {
                path: segs,
                arity: node.args.len() as u8,
                loc: Self::span_to_loc(node.func.span()),
            });
        }
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        fn flatten_tree(tree: &syn::UseTree, prefix: Vec<String>, out: &mut Vec<Vec<String>>) {
            match tree {
                syn::UseTree::Path(p) => {
                    let mut next = prefix.clone();
                    next.push(p.ident.to_string());
                    flatten_tree(&p.tree, next, out);
                }
                syn::UseTree::Name(n) => {
                    let mut full = prefix;
                    full.push(n.ident.to_string());
                    out.push(full);
                }
                syn::UseTree::Group(g) => {
                    for item in &g.items {
                        flatten_tree(item, prefix.clone(), out);
                    }
                }
                _ => {}
            }
        }
        let mut paths = Vec::new();
        flatten_tree(&node.tree, Vec::new(), &mut paths);
        for p in paths {
            self.features.imports.push(ImportLoc {
                path: p,
                symbol: None,
                loc: Loc::default(),
            });
        }
        syn::visit::visit_item_use(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let body_src = quote!(#node).to_string();
        let hash = xxh3_64(body_src.as_bytes());
        let sig_src = node.sig.ident.to_string();
        let sig_hash = xxh3_64(sig_src.as_bytes());
        let line_count = body_src.lines().count() as u32;
        let loc = Self::span_to_loc(node.sig.ident.span());

        self.features.fn_definitions.push(FnDef {
            name: node.sig.ident.to_string(),
            body_hash: hash,
            sig_hash,
            loc,
        });
        self.features.body_signatures.push(BodySignature {
            hash,
            line_count,
            parent_fn: Some(node.sig.ident.to_string()),
            loc,
        });
        syn::visit::visit_item_fn(self, node);
    }
}

impl LanguageExtractor for RustExtractor {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures> {
        let mut visitor = RustVisitor {
            features: ExtractedFeatures::new(path.to_path_buf(), Language::Rust),
        };
        if let Ok(file) = syn::parse_file(content) {
            visitor.visit_file(&file);
        }
        visitor.features.crate_name = crate::extractor::crate_name_from_path(path);
        Ok(visitor.features)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract(src: &str) -> ExtractedFeatures {
        let e = RustExtractor;
        e.extract(std::path::Path::new("test.rs"), src).unwrap()
    }

    #[test]
    fn extracts_string_literal() {
        let f = extract(r#"fn foo() { let x = "hello world"; }"#);
        assert_eq!(f.string_literals.len(), 1);
        assert_eq!(f.string_literals[0].value, "hello world");
    }

    #[test]
    fn extracts_numeric_literal_with_seconds_unit() {
        let f = extract("fn t() { Duration::from_secs(30); }");
        assert_eq!(f.numeric_literals.len(), 1);
        assert_eq!(f.numeric_literals[0].value, 30.0);
        assert!(matches!(
            f.numeric_literals[0].unit,
            Some(UnitHint::Seconds)
        ));
    }

    #[test]
    fn extracts_numeric_literal_with_millis_unit() {
        let f = extract("fn t() { Duration::from_millis(100); }");
        assert!(!f.numeric_literals.is_empty());
        assert!(matches!(f.numeric_literals[0].unit, Some(UnitHint::Millis)));
    }

    #[test]
    fn skips_doc_string_literals() {
        // doc comments don't appear as string literals in the syn AST
        let f = extract(r#"/// doc comment fn foo() {}"#);
        assert_eq!(f.string_literals.len(), 0);
    }

    #[test]
    fn extracts_call_site_path() {
        let f = extract("fn t() { reqwest::Client::new(); }");
        assert_eq!(f.call_sites.len(), 1);
        assert_eq!(f.call_sites[0].path, vec!["reqwest", "Client", "new"]);
        assert_eq!(f.call_sites[0].arity, 0);
    }

    #[test]
    fn extracts_use_import() {
        let f = extract("use std::collections::HashMap;");
        assert_eq!(f.imports.len(), 1);
        assert_eq!(f.imports[0].path, vec!["std", "collections", "HashMap"]);
    }

    #[test]
    fn extracts_fn_definition_name() {
        let f = extract("fn default_true() -> bool { true }");
        assert_eq!(f.fn_definitions.len(), 1);
        assert_eq!(f.fn_definitions[0].name, "default_true");
    }
}
