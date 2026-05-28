//! Stable snapshots for Tauri convergence emit (ADR 037).

use vox_codegen::codegen_rust::{RustAppShell, generate};
use vox_compiler::hir::HirModule;

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
