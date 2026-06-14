//! Abstract syntax tree (AST) for the Vox language.
//!
//! This is a **standalone L0 leaf** (serde-only) re-exported by `vox-compiler` as
//! `vox_compiler::ast`, so downstream crates that only need the declaration AST (e.g. `vox-db`'s
//! DDL emitter) can depend on `vox-ast` without pulling in the whole compiler. The parser builds
//! these trees from tokens; the HIR phase lowers them; typeck and codegen consume HIR. The AST is
//! **lossy only where the parser
//! chooses not to model trivia**—names are **not** resolved: `Ident` and `Named` types carry
//! spellings as written, and it is up to later passes to bind them to definitions.
//!
//! # Layout
//! - [`decl`] — one file becomes a [`decl::Module`] of [`decl::Decl`] items (functions, tables, routes, …).
//! - [`expr`] — expression tree inside bodies; shares [`span::Span`] with statements and patterns.
//! - [`stmt`] — `let` / `return` / expression statements; blocks in `expr` embed statements.
//! - [`pattern`] — `let` and `match` patterns; must align with [`expr::Expr::Match`] arms.
//! - [`types`] — type **syntax** only (generics, `fn(…) -> …`); not the internal type algebra.
//!
//! # Spans
//! [`crate::span`] is **byte offsets into the original UTF-8 source** (`start` inclusive, `end` exclusive).
//! Diagnostics and LSP use these directly; they are not grapheme- or line-aware.
//!
//! See `docs/src/reference/lexicon.md` for naming aligned with the grammar.

/// Top-level declarations: functions, components, data models, routes, and UI.
pub mod decl;
/// Expression AST nodes (literals, calls, JSX, control flow).
pub mod expr;
/// Patterns for `let` bindings and `match` arms.
pub mod pattern;
/// Scalar → Rust / TypeScript / SQLite mapping (codegen SSOT).
pub mod scalar_mapping;
/// Byte-offset source spans.
pub mod span;
/// Statement AST nodes.
pub mod stmt;
/// Surface type expressions (`int`, generics, function types).
pub mod types;

/// Byte-offset span attached to parsed nodes.
pub use span::Span;

#[cfg(test)]
mod semcov_wave35_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> span::Span {
        span::Span::new(0, 0)
    }

    #[test]
    fn test_ast_dummy_span_integrity() {
        let span = dummy_span();
        assert_eq!(
            span.start, 0,
            "AST Dummy span must anchor at origin 0 byte-offset."
        );
        assert_eq!(span.end, 0, "AST Dummy span must anchor at length 0.");
    }

    #[test]
    fn test_ast_scalar_mapping_structure() {
        let expected_ident = String::from("int");
        let ident = types::TypeExpr::Named {
            name: expected_ident.clone(),
            span: dummy_span(),
        };
        if let types::TypeExpr::Named { name, span } = ident {
            assert_eq!(name, "int");
            assert_eq!(span.start, 0);
        } else {
            panic!("Expected TypeExpr::Named variant for surface scalar mapping.");
        }
    }
}
