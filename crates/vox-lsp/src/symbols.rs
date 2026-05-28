//! Document symbols for `vox-lsp`.
//!
//! Walks the AST and maps declarations to LSP symbols (outline view).

use tower_lsp_server::ls_types::*;
pub use vox_compiler::ast::Span;
pub use vox_compiler::ast::decl::*;

pub struct SymbolEngine;

impl SymbolEngine {
    /// Walk a parsed module and emit a list of symbols.
    pub fn symbols(module: &Module, text: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        for decl in &module.declarations {
            if let Some(symbol) = Self::decl_to_symbol(decl, text) {
                symbols.push(symbol);
            }
        }

        symbols
    }

    fn decl_to_symbol(decl: &Decl, text: &str) -> Option<DocumentSymbol> {
        match decl {
            Decl::Function(f) => Some(Self::make_symbol(
                &f.name,
                &f.span,
                SymbolKind::FUNCTION,
                vec![],
                text,
            )),
            Decl::TypeDef(t) => Some(Self::make_symbol(
                &t.name,
                &t.span,
                SymbolKind::STRUCT,
                vec![],
                text,
            )),
            Decl::Const(c) => Some(Self::make_symbol(
                &c.name,
                &c.span,
                SymbolKind::CONSTANT,
                vec![],
                text,
            )),
            Decl::HttpRoute(h) => {
                let name = format!("{:?} {}", h.method, h.path);
                Some(Self::make_symbol(
                    &name,
                    &h.span,
                    SymbolKind::INTERFACE,
                    vec![],
                    text,
                ))
            }
            Decl::McpTool(m) => Some(Self::make_symbol(
                &m.func.name,
                &m.func.span,
                SymbolKind::FUNCTION,
                vec![],
                text,
            )),
            Decl::Table(t) => Some(Self::make_symbol(
                &t.name,
                &t.span,
                SymbolKind::STRUCT,
                vec![],
                text,
            )),
            Decl::Collection(c) => Some(Self::make_symbol(
                &c.name,
                &c.span,
                SymbolKind::STRUCT,
                vec![],
                text,
            )),
            Decl::Config(c) => Some(Self::make_symbol(
                &c.name,
                &c.span,
                SymbolKind::OBJECT,
                vec![],
                text,
            )),
            _ => None,
        }
    }

    fn make_symbol(
        name: &str,
        span: &Span,
        kind: SymbolKind,
        children: Vec<DocumentSymbol>,
        text: &str,
    ) -> DocumentSymbol {
        let range = Self::span_to_range(span, text);

        #[allow(deprecated)]
        DocumentSymbol {
            name: name.to_string(),
            detail: None,
            kind,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    fn span_to_range(span: &Span, text: &str) -> Range {
        let (start_line, start_col) = crate::byte_index_to_line_col(text, span.start);
        let (end_line, end_col) = crate::byte_index_to_line_col(text, span.end);

        Range {
            start: Position {
                line: start_line,
                character: start_col,
            },
            end: Position {
                line: end_line,
                character: end_col,
            },
        }
    }
}
