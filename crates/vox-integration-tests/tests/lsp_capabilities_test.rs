//! Exercise LSP library surfaces advertised in `initialize` (no subprocess).

use std::str::FromStr as _;

use tower_lsp_server::ls_types::{
    CompletionParams, Diagnostic, DiagnosticSeverity, PartialResultParams, Position, Range,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

#[test]
fn initialize_capabilities_match_implemented_handlers() {
    let caps = vox_lsp::server_capabilities();
    assert!(caps.hover_provider.is_some());
    assert!(caps.completion_provider.is_some());
    assert!(caps.document_symbol_provider.is_some());
    assert!(caps.code_lens_provider.is_some());
    assert!(caps.code_action_provider.is_some());
    assert!(caps.semantic_tokens_provider.is_some());
    assert!(
        caps.document_formatting_provider.is_none(),
        "document formatting is not implemented — do not advertise until wired"
    );
}

#[test]
fn semantic_tokens_encode_fn_keyword() {
    let src = "fn main() to int { return 0 }";
    let data = vox_lsp::grammar::encode_semantic_tokens(src);
    assert!(
        !data.is_empty(),
        "expected semantic tokens for minimal module"
    );
}

#[test]
fn completion_lists_surface_keyword_fn() {
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: Uri::from_str("file:///tmp/a.vox").expect("uri"),
            },
            position: Position {
                line: 0,
                character: 0,
            },
        },
        context: None,
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let list = vox_lsp::completions::CompletionEngine::completions(params);
    assert!(
        list.items.iter().any(|i| i.label == "fn"),
        "expected `fn` keyword completion"
    );
}

#[test]
fn hover_resolves_print_builtin() {
    assert!(vox_lsp::builtin_hover_markdown("print").is_some());
}

#[test]
fn document_symbols_outline_fn() {
    let src = "fn demo() to int { return 1 }\n";
    let module = parse(lex(src)).expect("parse");
    let syms = vox_lsp::symbols::SymbolEngine::symbols(&module, src);
    assert!(
        syms.iter().any(|s| s.name == "demo"),
        "expected outline symbol for demo()"
    );
}

#[test]
fn code_lens_emits_for_at_test() {
    let src = "@test fn foo() to int { return 1 }\n";
    let module = parse(lex(src)).expect("parse");
    let lenses = vox_lsp::code_lens::code_lenses_for_module(&module, src);
    assert!(
        lenses.iter().any(|l| l.command.as_ref().is_some_and(|c| c.command == "vox.runTest")),
        "expected run-test code lens"
    );
}

#[test]
fn quickfixes_roundtrip_from_diagnostic_data() {
    let uri = Uri::from_str("file:///tmp/fixture.vox").expect("uri");
    let diagnostic = Diagnostic {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 1,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        message: "example".into(),
        source: Some("vox-lsp".into()),
        data: Some(serde_json::json!({
            "fixes": [{
                "label": "Apply fix",
                "replacement": "ok",
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 1}
                }
            }]
        })),
        ..Default::default()
    };

    let actions = vox_lsp::quickfixes_for_diagnostics(uri, std::slice::from_ref(&diagnostic));
    assert_eq!(actions.len(), 1);
}
