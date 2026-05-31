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

// The back-button handler is emitted through the `@vox/runtime` adapter
// contract (commit f65fc27d5d), NOT direct Tauri `listen('vox-back-button')`.
// The adapter wraps the platform listener (Tauri event API on desktop, RN
// BackHandler on mobile) so a single source runs on both targets.

#[test]
fn back_button_decl_emits_runtime_adapter_hook() {
    let src = r#"
@query fn handle_back() to bool { return true }
@back_button {
    on_press: handle_back
}
"#;
    let ts = emit(src);
    assert!(ts.contains("voxRuntime.onBackButton("), "got:\n{ts}");
    assert!(ts.contains("handle_back("), "got:\n{ts}");
    assert!(
        ts.contains("@vox/runtime"),
        "expected @vox/runtime adapter import, got:\n{ts}"
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
    assert!(ts.contains("voxRuntime.onBackButton("), "got:\n{ts}");
    assert!(ts.contains("navigate_home("), "got:\n{ts}");
}
