//! Opt-in: build a golden fixture, then `pnpm exec playwright test` (requires browsers + `pnpm install` in this crate).
#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

use vox_cli::commands::build;
use vox_cli::frontend;

#[tokio::test]
#[ignore = "set VOX_GUI_PLAYWRIGHT=1; run `pnpm install` + `pnpm exec playwright install chromium` in crates/vox-integration-tests — owner: integration-tests sunset: 2026-12-31"]
async fn golden_route_screenshot_and_a11y() {
    let playwright_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGuiPlaywright);
    assert_eq!(
        playwright_resolved.expose(),
        Some("1"),
        "set VOX_GUI_PLAYWRIGHT=1 to run this test"
    );

    let mut repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

    let artifacts = tempfile::tempdir().expect("artifact tempdir");
    let it_crate = repo.join("crates/vox-integration-tests");
    assert!(it_crate.is_dir(), "missing {}", it_crate.display());

    let st = Command::new(pnpm)
        .args(["install", "--prefer-offline"])
        .current_dir(&it_crate)
        .status()
        .unwrap_or_else(|e| panic!("pnpm install (e2e) failed to spawn: {e}"));
    assert!(st.success(), "pnpm install in vox-integration-tests failed");

    let st = Command::new(pnpm)
        .args([
            "exec",
            "playwright",
            "test",
            "--config",
            "playwright.config.ts",
        ])
        .current_dir(&it_crate)
        .env("VOX_PLAYWRIGHT_APP_DIR", &app)
        .env("VOX_PLAYWRIGHT_OUT_DIR", artifacts.path())
        .status()
        .expect("playwright spawn");
    assert!(st.success(), "playwright test failed");

    assert!(
        artifacts.path().join("route.png").is_file(),
        "expected route.png under {}",
        artifacts.path().display()
    );
    assert!(
        artifacts.path().join("a11y.json").is_file(),
        "expected a11y.json under {}",
        artifacts.path().display()
    );
}
