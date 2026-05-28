//! Opt-in: `pnpm install` + `vite build` for a golden `.vox` fixture (requires network on first run).
#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

use vox_cli::commands::build;
use vox_cli::frontend;

#[tokio::test]
#[ignore = "set VOX_WEB_VITE_SMOKE=1 and ensure pnpm is on PATH — owner: integration-tests sunset: 2026-12-31"]
async fn full_stack_minimal_vite_production_build() {
    let vite_smoke_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxWebViteSmoke);
    assert_eq!(
        vite_smoke_resolved.expose(),
        Some("1"),
        "set VOX_WEB_VITE_SMOKE=1 to run this test"
    );

    let mut repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // If we're inside the crate (local test or normal cargo test), go to workspace root.
    if repo.ends_with("vox-integration-tests") || repo.ends_with("crates/vox-integration-tests") {
        repo.pop();
        repo.pop();
    }
    let vox_file = repo.join("crates/vox-integration-tests/tests/fixtures/full_stack_minimal.vox");
    assert!(
        vox_file.is_file(),
        "missing fixture: {}",
        vox_file.display()
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let ts_out = tmp.path().join("ts");
    build::run(
        &vox_file,
        &ts_out,
        None,
        None,
        false,
        false,
        vox_cli::cli_args::BuildMode::App,
        vox_codegen::codegen_rust::RustAppShell::default(),
    )
    .await
    .expect("vox build");

    let app = ts_out.join("app");
    frontend::scaffold_react_app(&app, &ts_out, false).expect("scaffold");

    let pnpm = frontend::pnpm_executable();
    let st = Command::new(pnpm)
        .args(["install", "--prefer-offline"])
        .current_dir(&app)
        .status()
        .unwrap_or_else(|e| panic!("pnpm install failed to spawn ({pnpm:?}): {e}"));
    assert!(st.success(), "pnpm install failed");

    let st = Command::new(pnpm)
        .args(["run", "build"])
        .current_dir(&app)
        .status()
        .expect("pnpm run build spawn");
    assert!(st.success(), "pnpm run build failed");
}
