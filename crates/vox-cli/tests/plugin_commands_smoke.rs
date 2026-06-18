//! Smoke tests for `vox plugin` and `vox bundle` CLI commands.

use std::process::Command;

// ── vox plugin list ─────────────────────────────────────────────────────────

#[test]
fn plugin_list_prints_catalog() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["plugin", "list"])
        .output()
        .expect("vox should run");
    assert!(out.status.success(), "vox plugin list should succeed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should contain at least one known plugin id.
    assert!(
        stdout.contains("skill-compiler"),
        "expected skill-compiler in plugin list output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Install root:"),
        "expected install root line in output, got:\n{stdout}"
    );
}

// ── vox bundle list ──────────────────────────────────────────────────────────

#[test]
fn bundle_list_prints_bundles() {
    let out = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["bundle", "list"])
        .output()
        .expect("vox should run");
    assert!(out.status.success(), "vox bundle list should succeed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("vox-fullstack"),
        "expected vox-fullstack in bundle list output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("9 bundle(s) defined."),
        "expected bundle count line, got:\n{stdout}"
    );
}

// ── vox plugin install --path + vox plugin remove ────────────────────────────

#[test]
fn plugin_install_path_and_remove() {
    // Use a unique VOX_PLUGINS_DIR so the test doesn't pollute the real install root.
    let tmp = std::env::temp_dir().join(format!("vox-plugin-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");

    // Resolve the noop-skill path (fixture lives in vox-plugin-host/tests/fixtures/noop-skill/).
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let noop_skill_path = workspace_root
        .join("crates")
        .join("vox-plugin-host")
        .join("tests")
        .join("fixtures")
        .join("noop-skill");

    let status = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args([
            "plugin",
            "install",
            "--path",
            &noop_skill_path.to_string_lossy(),
            "--yes",
        ])
        .env("VOX_PLUGINS_DIR", &tmp)
        .status()
        .expect("vox should run");
    assert!(status.success(), "vox plugin install --path should succeed");

    // Installed dir should exist.
    assert!(
        tmp.join("noop-skill").exists(),
        "noop-skill install dir should exist under tmp"
    );

    // Now remove it.
    let status = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["plugin", "remove", "noop-skill"])
        .env("VOX_PLUGINS_DIR", &tmp)
        .status()
        .expect("vox should run");
    assert!(status.success(), "vox plugin remove should succeed");

    assert!(
        !tmp.join("noop-skill").exists(),
        "noop-skill install dir should be gone after remove"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&tmp);
}
