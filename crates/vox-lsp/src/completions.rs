//! Completion engine for `vox-lsp`.
//!
//! Provides context-aware completions for keywords, decorators, types, and builtins.
//!
//! Keywords and lexer-backed decorators come from [`vox_compiler::language_surface`] (SSOT).

use tower_lsp_server::ls_types::*;
use vox_compiler::language_surface;

pub struct CompletionEngine;

impl CompletionEngine {
    /// Returns a list of all completions at a given position.
    /// Initially keyword-only, expanding to be context-aware.
    pub fn completions(_params: CompletionParams) -> CompletionList {
        let mut items = Vec::new();

        // 1. Keyword completions
        Self::add_keywords(&mut items);

        // 2. Decorator completions (if triggered by @)
        Self::add_decorators(&mut items);

        // 3. Type completions
        Self::add_types(&mut items);

        // 4. Builtin completions
        Self::add_builtins(&mut items);

        CompletionList {
            is_incomplete: false,
            items,
        }
    }

    fn add_keywords(items: &mut Vec<CompletionItem>) {
        for &(name, snippet) in language_surface::LSP_KEYWORD_SNIPPETS {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                insert_text: Some(snippet.to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
    }

    fn add_decorators(items: &mut Vec<CompletionItem>) {
        // Map docs to names for easy lookup
        let docs: std::collections::HashMap<&str, &str> = language_surface::LSP_DECORATOR_DOCS
            .iter()
            .cloned()
            .collect();

        for &(name, snippet) in language_surface::LSP_DECORATOR_SNIPPETS {
            let doc = docs.get(name).cloned().unwrap_or("");
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                documentation: Some(Documentation::String(doc.to_string())),
                insert_text: Some(snippet.to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
    }

    fn add_types(items: &mut Vec<CompletionItem>) {
        for &t in language_surface::SURFACE_TYPE_NAMES {
            items.push(CompletionItem {
                label: t.to_string(),
                kind: Some(CompletionItemKind::STRUCT),
                detail: Some("Built-in Type".to_string()),
                ..Default::default()
            });
        }
    }

    fn add_builtins(items: &mut Vec<CompletionItem>) {
        for &(name, sig) in language_surface::LSP_BUILTIN_SNIPPETS {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                insert_text: Some(sig.to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
    }
}
