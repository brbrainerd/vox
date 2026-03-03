//! Tests for HIR lowering, including import path handling and edge cases.

use vox_ast::decl::{Decl, ImportDecl, ImportPath, Module};
use vox_ast::span::Span;
use vox_hir::lower_module;

fn span() -> Span {
    Span { start: 0, end: 0 }
}

#[test]
fn import_single_segment() {
    let module = Module {
        declarations: vec![Decl::Import(ImportDecl {
            paths: vec![ImportPath {
                segments: vec!["foo".to_string()],
                span: span(),
            }],
            span: span(),
        })],
        span: span(),
    };
    let hir = lower_module(&module);
    assert_eq!(hir.imports.len(), 1);
    assert!(hir.imports[0].module_path.is_empty());
    assert_eq!(hir.imports[0].item, "foo");
}

#[test]
fn import_multi_segment() {
    let module = Module {
        declarations: vec![Decl::Import(ImportDecl {
            paths: vec![ImportPath {
                segments: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                span: span(),
            }],
            span: span(),
        })],
        span: span(),
    };
    let hir = lower_module(&module);
    assert_eq!(hir.imports.len(), 1);
    assert_eq!(hir.imports[0].module_path, vec!["a", "b"]);
    assert_eq!(hir.imports[0].item, "c");
}
