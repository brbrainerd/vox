use vox_codegen::codegen_ts::emitter::generate;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

fn emit(src: &str) -> String {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    generate(&hir)
        .expect("emit")
        .files
        .iter()
        .map(|(_, c)| c.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn push_decl_emits_push_notifications_listener() {
    let src = r#"
@mutation fn store_token(token: str) to str { return token }
@push {
    on_register: store_token
}
"#;
    let ts = emit(src);
    assert!(ts.contains("PushNotifications"), "got:\n{ts}");
    assert!(ts.contains("store_token("), "got:\n{ts}");
}
