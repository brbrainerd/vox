/// Task 1.4: @webhook must not be silently dropped during codegen.
///
/// The `@webhook` decorator is parsed, structurally typechecked, and HIR-lowered,
/// but the Rust codegen backend always emits `endpoint.webhook = None` — no
/// secret-verification, replay-window, or idempotency logic is generated.
/// This test confirms that the pipeline emits a diagnostic with code
/// `vox/decorator/webhook-runtime-unimplemented` so the gap is loud, not silent.
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::lint_ast_declarations;

#[test]
fn webhook_is_not_silently_dropped() {
    // @webhook(provider: stripe) on a @server fn with Vox return-type syntax.
    // Uses the stripe provider so no secret_var is required (avoids the
    // `vox/webhook/missing-secret-var` error from boilerplate_grafts.rs).
    // @server must come first (the parser dispatches @server → parse_server_endpoint
    // → parse_fn_decl; @webhook is consumed inside parse_fn_decl's decorator loop).
    let src = "@server\n@webhook(provider: stripe)\nfn on_event() to str { return \"ok\" }\n";
    let tokens = lex(src);
    let module = parse(tokens).expect("@webhook + @server fn must parse without errors");
    let diags = lint_ast_declarations(&module, src);
    let codes: Vec<String> = diags.iter().filter_map(|d| d.code.clone()).collect();
    assert!(
        codes.iter().any(|c| c.starts_with("vox/decorator/webhook")),
        "@webhook must emit a diagnostic starting with 'vox/decorator/webhook', got {codes:?}"
    );
}
