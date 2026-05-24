//! VUV / full-stack seam: `@endpoint` lowers to Tauri Rust commands and to `vox-client.ts`
//! invoke transport (Contract IR), while `CodegenOutput::api_client_ts` stays empty.
//!
//! Dashboard and `native-binary` remain Axum per ADR 024 / ADR 037; this test pins the
//! **generated app** desktop/mobile shell only.

use vox_codegen::codegen_rust::{RustAppShell, generate};
use vox_codegen::codegen_ts::vox_client::emit_vox_client;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

const ENDPOINT_VOX: &str = r#"@endpoint(kind: query) fn get_count() to int { return 0 }"#;

#[test]
fn tauri_rust_commands_match_vox_client_invoke() {
    let module = parse(lex(ENDPOINT_VOX)).expect("parse");
    let hir = lower_module(&module);

    let rust = generate(&hir, "demo_app", RustAppShell::TauriApp).expect("rust generate");
    assert!(
        rust.api_client_ts.is_empty(),
        "api_client_ts must stay empty; vox-client.ts is the SSOT"
    );

    let main = rust
        .files
        .get("src-tauri/src/main.rs")
        .expect("Tauri main.rs");
    assert!(
        main.contains("#[tauri::command]"),
        "expected #[tauri::command] in generated main.rs"
    );
    assert!(
        main.contains("async fn get_count("),
        "expected get_count command in generated main.rs"
    );

    let client = emit_vox_client(&hir);
    assert!(
        client.contains("isTauri()"),
        "vox-client must branch for Tauri webview"
    );
    assert!(
        client.contains("$tauri") && client.contains("\"get_count\""),
        "vox-client must invoke the same command name as Rust emit"
    );
}

#[test]
fn web_ir_lowers_endpoint_only_module() {
    let module = parse(lex(ENDPOINT_VOX)).expect("parse");
    let hir = lower_module(&module);
    let web = vox_codegen::web_ir::lower::lower_hir_to_web_ir(&hir);
    assert!(
        web.view_roots.is_empty(),
        "endpoint-only fixture should not introduce WebIR view roots"
    );
}
