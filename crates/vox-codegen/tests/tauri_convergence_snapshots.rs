//! Stable snapshots for Tauri convergence emit (ADR 037).

use vox_codegen::codegen_rust::{RustAppShell, generate};
use vox_compiler::hir::HirModule;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn tauri_convergence_snapshots() {
    let out = generate(&HirModule::default(), "pkg", RustAppShell::TauriApp)
        .expect("generate Tauri shell");
    let main = out
        .files
        .get("src-tauri/src/main.rs")
        .expect("src-tauri main.rs");
    insta::assert_snapshot!("tauri_app_main_rs", main);
    let build_rs = out.files.get("src-tauri/build.rs").expect("build.rs");
    insta::assert_snapshot!("tauri_app_build_rs", build_rs);
}

#[test]
fn tauri_setup_includes_schema_drift_validation_when_tables_exist() {
    let src = r#"
        @table type Task {
            title: str
        }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);
    let out = generate(&hir, "pkg", RustAppShell::TauriApp).expect("generate Tauri shell");
    let main = out
        .files
        .get("src-tauri/src/main.rs")
        .expect("src-tauri main.rs");

    assert!(
        main.contains("schema drift: table"),
        "tauri setup must include boot-time schema drift checks when tables exist",
    );
    assert!(
        main.contains("PRAGMA table_info"),
        "tauri setup should introspect live columns during boot",
    );
    assert!(
        main.contains("VOX_APP_DB_URL uses non-libsql backend"),
        "tauri setup should guard against non-libsql VOX_APP_DB_URL in this phase",
    );
}
