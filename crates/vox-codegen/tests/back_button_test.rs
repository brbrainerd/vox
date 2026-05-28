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
fn back_button_decl_emits_tauri_listen() {
    let src = r#"
@query fn handle_back() to bool { return true }
@back_button {
    on_press: handle_back
}
"#;
    let ts = emit(src);
    assert!(ts.contains("listen('vox-back-button'"), "got:\n{ts}");
    assert!(ts.contains("handle_back("), "got:\n{ts}");
    assert!(
        ts.contains("@tauri-apps/api/event"),
        "expected Tauri event API, got:\n{ts}"
    );
}

#[test]
fn back_button_with_fallback_emits_fallback_call() {
    let src = r#"
@query fn handle_back() to bool { return false }
@mutation fn navigate_home() to str { return "/" }
@back_button {
    on_press: handle_back
    fallback: navigate_home
}
"#;
    let ts = emit(src);
    assert!(ts.contains("listen('vox-back-button'"), "got:\n{ts}");
    assert!(ts.contains("navigate_home("), "got:\n{ts}");
}
