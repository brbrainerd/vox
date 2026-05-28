//! Language Server Protocol integration for Vox.
//!
//! Shared validation helpers used by the LSP binary and MCP / orchestrator quality gates.

use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, DiagnosticSeverity, NumberOrString,
    Position, Range, TextEdit, Uri, WorkspaceEdit,
};
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::Diagnostic as TypeckDiagnostic;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_ast_module;

pub mod capabilities;
pub mod code_lens;
pub mod completions;
pub mod grammar;
pub mod symbols;

pub use capabilities::server_capabilities;

/// Convert UTF-8 byte index to LSP line / column (character count per line, not UTF-16 code units).
pub fn byte_index_to_line_col(text: &str, index: usize) -> (u32, u32) {
    vox_compiler::ast::span::byte_offset_to_line_col_zero_based(text, index)
}

fn typeck_diagnostic_to_lsp(text: &str, err: TypeckDiagnostic) -> Diagnostic {
    let (sl, sc) = byte_index_to_line_col(text, err.span.start);
    let (el, ec) = byte_index_to_line_col(text, err.span.end);
    let start = Position {
        line: sl,
        character: sc,
    };
    let end = Position {
        line: el,
        character: ec,
    };
    let data = serde_json::json!({
        "suggestions": err.suggestions,
        "fixes": err.fixes.into_iter().map(|f| {
            let (sl, sc) = byte_index_to_line_col(text, f.span.start);
            let (el, ec) = byte_index_to_line_col(text, f.span.end);
            serde_json::json!({
                "label": f.label,
                "replacement": f.replacement,
                "range": Range {
                    start: Position { line: sl, character: sc },
                    end: Position { line: el, character: ec },
                }
            })
        }).collect::<Vec<_>>()
    });
    Diagnostic {
        range: Range { start, end },
        severity: Some(match err.severity {
            TypeckSeverity::Error => DiagnosticSeverity::ERROR,
            TypeckSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: err.code.clone().map(NumberOrString::String),
        code_description: None,
        source: Some("vox-lsp".to_string()),
        message: err.message,
        related_information: None,
        tags: None,
        data: Some(data),
    }
}

fn validate_document_impl(text: &str, include_hir: bool) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let tokens = lex(text);
    match parse(tokens) {
        Ok(module) => {
            diagnostics.extend(mesh_workflow_env_warnings(text, &module));
            let mut type_errors = typecheck_ast_module(text, &module);
            if include_hir {
                let hir = vox_compiler::hir::lower_module(&module);
                for e in vox_compiler::hir::validate_module(&hir) {
                    type_errors.push(TypeckDiagnostic::hir_invariant(
                        e.message,
                        e.span,
                        text,
                        e.correction_hint,
                    ));
                }
            }
            diagnostics.extend(
                type_errors
                    .into_iter()
                    .map(|err| typeck_diagnostic_to_lsp(text, err)),
            );
        }
        Err(parse_errors) => {
            for err in parse_errors {
                let (sl, sc) = byte_index_to_line_col(text, err.span.start);
                let (el, ec) = byte_index_to_line_col(text, err.span.end);
                let start = Position {
                    line: sl,
                    character: sc,
                };
                let end = Position {
                    line: el,
                    character: ec,
                };
                diagnostics.push(Diagnostic {
                    range: Range { start, end },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: None,
                    code_description: None,
                    message: err.to_string(),
                    source: Some("vox-lsp".to_string()),
                    ..Default::default()
                });
            }
        }
    }
    diagnostics
}

/// Run lexer → parser → typecheck and return LSP diagnostics (no side effects).
///
/// Note: This does **not** run HIR structural validation. For parity with the CLI
/// frontend (`run_frontend_str`), use [`validate_document_with_hir`].
pub fn validate_document(text: &str) -> Vec<Diagnostic> {
    validate_document_impl(text, false)
}

/// Full frontend validation: lexer → parser → typecheck → HIR lower → HIR validate.
///
/// Matches the diagnostic shape produced by `vox-cli` `run_frontend_str` for type/HIR issues
/// (parse errors are always surfaced as errors).
#[must_use]
pub fn validate_document_with_hir(text: &str) -> Vec<Diagnostic> {
    validate_document_impl(text, true)
}

/// Build quick-fix [`CodeAction`]s from diagnostics that carry structured `data.fixes`
/// (parity with the stdio LSP `textDocument/codeAction` handler).
#[must_use]
pub fn quickfixes_for_diagnostics(uri: Uri, diagnostics: &[Diagnostic]) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for diagnostic in diagnostics {
        let Some(ref data) = diagnostic.data else {
            continue;
        };
        let Ok(data) = serde_json::from_value::<serde_json::Value>(data.clone()) else {
            continue;
        };
        let Some(fixes) = data.get("fixes").and_then(|f| f.as_array()) else {
            continue;
        };
        for fix in fixes {
            let label = fix.get("label").and_then(|l| l.as_str()).unwrap_or("Fix");
            let replacement = fix
                .get("replacement")
                .and_then(|r| r.as_str())
                .unwrap_or("");
            let range = fix
                .get("range")
                .and_then(|r| serde_json::from_value::<Range>(r.clone()).ok());

            if let Some(range) = range {
                let mut changes = std::collections::HashMap::new();
                changes.insert(
                    uri.clone(),
                    vec![TextEdit {
                        range,
                        new_text: replacement.to_string(),
                    }],
                );

                let action = CodeAction {
                    title: label.to_string(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diagnostic.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        ..Default::default()
                    }),
                    is_preferred: Some(true),
                    ..Default::default()
                };
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }
    }

    actions
}

fn vox_populi_enabled_from_env() -> bool {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshEnabled)
        .expose()
        .map(|v: &str| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

/// When `VOX_MESH_ENABLED` is unset/false, warn on `mesh_*` activity calls inside `workflow` bodies.
fn mesh_workflow_env_warnings(
    text: &str,
    module: &vox_compiler::ast::decl::Module,
) -> Vec<Diagnostic> {
    use vox_compiler::ast::decl::Decl;
    use vox_compiler::ast::expr::Expr;
    use vox_compiler::ast::span::Span;
    use vox_compiler::ast::stmt::Stmt;

    if vox_populi_enabled_from_env() {
        return Vec::new();
    }

    fn collect_mesh_calls_expr(expr: &Expr, out: &mut Vec<Span>) {
        match expr {
            Expr::Ident { name, span } if name.starts_with("mesh_") => {
                out.push(*span);
            }
            Expr::Call { callee, args, .. } => {
                collect_mesh_calls_expr(callee, out);
                for a in args {
                    collect_mesh_calls_expr(&a.value, out);
                }
            }
            Expr::MethodCall { object, args, .. } => {
                collect_mesh_calls_expr(object, out);
                for a in args {
                    collect_mesh_calls_expr(&a.value, out);
                }
            }
            Expr::Binary { left, right, .. } => {
                collect_mesh_calls_expr(left, out);
                collect_mesh_calls_expr(right, out);
            }
            Expr::Unary { operand, .. } => collect_mesh_calls_expr(operand, out),
            Expr::Block { stmts, .. } => {
                for s in stmts {
                    collect_mesh_calls_stmt(s, out);
                }
            }
            Expr::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_mesh_calls_expr(condition, out);
                for s in then_body {
                    collect_mesh_calls_stmt(s, out);
                }
                if let Some(es) = else_body {
                    for s in es {
                        collect_mesh_calls_stmt(s, out);
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_mesh_calls_stmt(stmt: &Stmt, out: &mut Vec<Span>) {
        match stmt {
            Stmt::Let { value, .. } => collect_mesh_calls_expr(value, out),
            Stmt::Assign { value, target, .. } => {
                collect_mesh_calls_expr(target, out);
                collect_mesh_calls_expr(value, out);
            }
            Stmt::Expr { expr, .. } => collect_mesh_calls_expr(expr, out),
            Stmt::Return { value: Some(e), .. } => collect_mesh_calls_expr(e, out),
            Stmt::While {
                condition, body, ..
            } => {
                collect_mesh_calls_expr(condition, out);
                for s in body {
                    collect_mesh_calls_stmt(s, out);
                }
            }
            _ => {}
        }
    }

    let mut spans: Vec<Span> = Vec::new();
    for decl in &module.declarations {
        if let Decl::Workflow(w) = decl {
            for s in &w.body {
                collect_mesh_calls_stmt(s, &mut spans);
            }
        }
    }

    spans
        .into_iter()
        .map(|span| {
            let (sl, sc) = byte_index_to_line_col(text, span.start);
            let (el, ec) = byte_index_to_line_col(text, span.end);
            Diagnostic {
                range: Range {
                    start: Position {
                        line: sl,
                        character: sc,
                    },
                    end: Position {
                        line: el,
                        character: ec,
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: None,
                code_description: None,
                source: Some("vox-lsp".to_string()),
                message: "Mens activity call: enable VOX_MESH_ENABLED=1 (and mens control-plane env) at runtime for mens hooks.".to_string(),
                related_information: None,
                tags: None,
                data: None,
            }
        })
        .collect()
}

/// Identifier-like character (ASCII alnum + underscore).
#[inline]
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Word at LSP `(line, character)` using **0-based** lines and **character = column in Unicode scalars** (same convention as [`byte_index_to_line_col`]).
#[must_use]
pub fn word_at_position(text: &str, line: u32, character: u32) -> Option<String> {
    let line_str = text.lines().nth(line as usize)?;
    let col = character as usize;
    let chars: Vec<char> = line_str.chars().collect();
    if col >= chars.len() {
        return None;
    }
    if !is_ident_char(chars[col]) {
        return None;
    }
    let mut start = col;
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = col + 1;
    while end < chars.len() && is_ident_char(chars[end]) {
        end += 1;
    }
    Some(chars[start..end].iter().collect())
}

/// True if `line` contains a `Speech.transcribe` / `Speech . transcribe` call (word boundaries).
#[must_use]
pub fn line_has_speech_transcribe(line: &str) -> bool {
    for (idx, _) in line.match_indices("Speech") {
        if idx > 0
            && let Some(prev) = line[..idx].chars().next_back()
            && (prev.is_ascii_alphanumeric() || prev == '_')
        {
            continue;
        }
        let after_speech = &line[idx + "Speech".len()..];
        let after_speech = after_speech.trim_start();
        let Some(after_dot) = after_speech.strip_prefix('.') else {
            continue;
        };
        let after_dot = after_dot.trim_start();
        let Some(rest) = after_dot.strip_prefix("transcribe") else {
            continue;
        };
        let boundary_ok = match rest.chars().next() {
            None => true,
            Some('(') => true,
            Some(c) if c.is_whitespace() => true,
            Some(c) => !is_ident_char(c),
        };
        if boundary_ok {
            return true;
        }
    }
    false
}

/// Markdown hover using the **current line** so `transcribe` only hovers in `Speech.transcribe` context.
#[must_use]
pub fn builtin_hover_markdown_in_line(line: &str, word: &str) -> Option<String> {
    match word {
        "transcribe" => {
            if line_has_speech_transcribe(line) {
                builtin_hover_markdown("transcribe")
            } else {
                None
            }
        }
        _ => builtin_hover_markdown(word),
    }
}

/// Short markdown hovers for selected builtins (Oratio, HTTP, stdlib).
#[must_use]
pub fn builtin_hover_markdown(word: &str) -> Option<String> {
    match word {
        "Speech" => Some(
            "**Speech** — Oratio (Candle Whisper, pure Rust). \
             `Speech.transcribe(path: str) → Result[str]` returns **refined** transcript text. \
             Rust codegen uses `vox-oratio`; TS/browser cannot call it directly."
                .to_string(),
        ),
        "transcribe" => Some(
            "**transcribe** — On `Speech`, runs STT via **Vox Oratio** (HF Candle). \
             Server/Rust only; in TS use `@server` or `POST /api/audio/transcribe` — see repo **`examples/oratio/codexAudioTranscribe.ts`**."
                .to_string(),
        ),
        "HTTP" => Some(
            "**HTTP** — builtin client module: `get` / `post` / `put` / `delete` (paths return `Result[Response]`)."
                .to_string(),
        ),
        "OpenClaw" => Some(
            "**OpenClaw** — builtin gateway module (WS-first): \
             `list_skills()`, `call(method, params_json)`, `subscribe(domain)`, \
             `unsubscribe(domain)`, `notify(domain, message)`."
                .to_string(),
        ),
        "print" => Some("**print** — `print(value: str) → Unit`.".to_string()),
        "len" => Some("**len** — length of a collection.".to_string()),
        "ret" => Some("**DEPRECATED**: use `return` instead. `ret` will be removed in a future version.".to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;

    #[test]
    fn word_at_position_finds_ident() {
        let t = "fn demo():\n    Speech.transcribe(p)\n";
        assert_eq!(word_at_position(t, 1, 4).as_deref(), Some("Speech"));
    }

    #[test]
    fn line_has_speech_transcribe_detects_call() {
        assert!(line_has_speech_transcribe("    Speech.transcribe(path)"));
        assert!(line_has_speech_transcribe("Speech . transcribe (x)"));
        assert!(!line_has_speech_transcribe("FooSpeech.transcribe(x)"));
        assert!(!line_has_speech_transcribe("let transcribe = 1"));
    }

    #[test]
    fn hover_transcribe_only_with_speech_receiver() {
        let line = "    Speech.transcribe(p)";
        assert!(builtin_hover_markdown_in_line(line, "transcribe").is_some());
        assert!(builtin_hover_markdown_in_line("other.transcribe(p)", "transcribe").is_none());
    }

    // Mutex guard for environment mutations in tests that set VOX_MESH_ENABLED.
    static MESH_WARNING_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn mesh_activity_warning_test() {
        let doc = "workflow w() { mesh_snapshot() }";
        let _lock = MESH_WARNING_ENV_LOCK
            .lock()
            .expect("mens env mutex poisoned");

        // When mens is DISABLED, we expect a WARNING diagnostic.
        // SAFETY: single-threaded under the mutex guard above.
        unsafe {
            std::env::set_var("VOX_MESH_ENABLED", "0");
        }
        let diags = validate_document(doc);
        assert!(
            diags
                .iter()
                .any(|d| d.severity == Some(DiagnosticSeverity::WARNING)
                    && d.message.contains("Mens activity call")),
            "Expected mens call warning when VOX_MESH_ENABLED=0, got: {:?}",
            diags
        );

        // When mens is ENABLED, no mens WARNING should fire.
        // SAFETY: single-threaded under the mutex guard above.
        unsafe {
            std::env::set_var("VOX_MESH_ENABLED", "1");
        }
        let diags = validate_document(doc);
        assert!(
            !diags
                .iter()
                .any(|d| d.severity == Some(DiagnosticSeverity::WARNING)
                    && d.message.contains("Mens activity call")),
            "Expected no mens call warning when VOX_MESH_ENABLED=1, got: {:?}",
            diags
        );

        // SAFETY: restore to neutral state.
        unsafe {
            std::env::remove_var("VOX_MESH_ENABLED");
        }
    }
}
