//! Code lens generation for Vox test declarations.
//!
//! Produces `▶ Run test` / `▶ Run property` code lenses above each
//! `@test` and `@forall` declaration in a `.vox` document.

use tower_lsp_server::ls_types::{CodeLens, Command, Position, Range};
use vox_compiler::ast::decl::{Decl, Module};

/// Build all code lenses for a parsed Vox module.
///
/// Each `@test` declaration gets a `▶ Run test` lens.
/// Each `@forall` declaration gets a `▶ Run property` lens.
/// The command name is `vox.runTest` so VS Code extension can intercept it.
pub fn code_lenses_for_module(module: &Module, text: &str) -> Vec<CodeLens> {
    let mut lenses = Vec::new();

    for decl in &module.declarations {
        match decl {
            Decl::Test(t) => {
                let span = &t.func.span;
                let (line, _col) = crate::byte_index_to_line_col(text, span.start);
                let range = Range {
                    start: Position { line, character: 0 },
                    end: Position { line, character: 0 },
                };
                let label = if t.label.is_empty() {
                    t.func.name.clone()
                } else {
                    t.label.clone()
                };
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title: format!("▶ Run test: {label}"),
                        command: "vox.runTest".to_string(),
                        arguments: Some(vec![serde_json::json!({
                            "kind": "test",
                            "label": label,
                            "fn_name": t.func.name,
                        })]),
                    }),
                    data: None,
                });
            }
            Decl::Forall(f) => {
                let span = &f.func.span;
                let (line, _col) = crate::byte_index_to_line_col(text, span.start);
                let range = Range {
                    start: Position { line, character: 0 },
                    end: Position { line, character: 0 },
                };
                let label = if f.label.is_empty() {
                    f.func.name.clone()
                } else {
                    f.label.clone()
                };
                lenses.push(CodeLens {
                    range,
                    command: Some(Command {
                        title: format!("▶ Run property ({} iters): {label}", f.iterations),
                        command: "vox.runTest".to_string(),
                        arguments: Some(vec![serde_json::json!({
                            "kind": "forall",
                            "label": label,
                            "fn_name": f.func.name,
                            "iterations": f.iterations,
                        })]),
                    }),
                    data: None,
                });
            }
            _ => {}
        }
    }

    lenses
}
