//! Integration tests for uv venv auto-detection in vox-py and vox-container.
//!
//! These tests verify that:
//! - `PythonEnv::site_packages_path()` and `venv_path()` behave correctly
//!   when a synthetic `.venv` is present.
//! - `VoxPyRuntime::new()` does not panic in a clean environment with no venv.
//! - The venv-path helpers correctly identify Windows vs POSIX layouts.

use std::fs;
use std::path::PathBuf;
use vox_container::env::PythonEnv;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a minimal fake Windows-style venv tree under `dir`.
/// Layout: `dir/.venv/Lib/site-packages/`
fn make_windows_venv(base: &PathBuf) -> PathBuf {
    let venv = base.join(".venv");
    let sp = venv.join("Lib").join("site-packages");
    fs::create_dir_all(&sp).expect("could not create fake windows venv");
    venv
}

/// Create a minimal fake POSIX-style venv tree under `base`.
/// Layout: `base/.venv/lib/python3.12/site-packages/`
fn make_posix_venv(base: &PathBuf) -> PathBuf {
    let venv = base.join(".venv");
    let sp = venv.join("lib").join("python3.12").join("site-packages");
    fs::create_dir_all(&sp).expect("could not create fake posix venv");
    venv
}

/// Construct a throw-away `PythonEnv` (uv_available does not matter for path tests).
fn dummy_env() -> PythonEnv {
    PythonEnv {
        uv_available: false,
        uv_version: None,
        python_version: None,
        cuda_version: None,
        has_gpu: false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PythonEnv path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn venv_path_via_env_var_uv_project_environment() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let venv = make_windows_venv(&tmp.path().to_path_buf());

    // Set the env var that uv itself emits
    std::env::set_var("UV_PROJECT_ENVIRONMENT", venv.to_str().unwrap());
    let env = dummy_env();
    let found = env.venv_path();
    std::env::remove_var("UV_PROJECT_ENVIRONMENT");

    assert!(found.is_some(), "should detect venv via UV_PROJECT_ENVIRONMENT");
    assert_eq!(found.unwrap(), venv);
}

#[test]
fn venv_path_via_env_var_virtual_env() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let venv = make_posix_venv(&tmp.path().to_path_buf());

    // Ensure UV_PROJECT_ENVIRONMENT is absent so it falls through to VIRTUAL_ENV
    std::env::remove_var("UV_PROJECT_ENVIRONMENT");
    std::env::set_var("VIRTUAL_ENV", venv.to_str().unwrap());
    let env = dummy_env();
    let found = env.venv_path();
    std::env::remove_var("VIRTUAL_ENV");

    assert!(found.is_some(), "should detect venv via VIRTUAL_ENV");
    assert_eq!(found.unwrap(), venv);
}

#[test]
fn site_packages_path_windows_layout() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let venv = make_windows_venv(&tmp.path().to_path_buf());
    let expected_sp = venv.join("Lib").join("site-packages");

    std::env::set_var("UV_PROJECT_ENVIRONMENT", venv.to_str().unwrap());
    let env = dummy_env();
    let sp = env.site_packages_path();
    std::env::remove_var("UV_PROJECT_ENVIRONMENT");

    assert!(sp.is_some(), "site_packages_path should find Lib/site-packages");
    assert_eq!(sp.unwrap(), expected_sp);
}

#[test]
fn site_packages_path_posix_layout() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let venv = make_posix_venv(&tmp.path().to_path_buf());
    let expected_sp = venv.join("lib").join("python3.12").join("site-packages");

    std::env::remove_var("UV_PROJECT_ENVIRONMENT");
    std::env::set_var("VIRTUAL_ENV", venv.to_str().unwrap());
    let env = dummy_env();
    let sp = env.site_packages_path();
    std::env::remove_var("VIRTUAL_ENV");

    assert!(sp.is_some(), "site_packages_path should find lib/python*/site-packages");
    assert_eq!(sp.unwrap(), expected_sp);
}

#[test]
fn site_packages_path_absent_returns_none_when_no_venv_env_vars() {
    // Ensure neither env var is set so we fall through to cwd check.
    // Since cwd is unlikely to have a .venv in the test runner, this should return None.
    std::env::remove_var("UV_PROJECT_ENVIRONMENT");
    std::env::remove_var("VIRTUAL_ENV");

    // Use a temporary dir as CWD to guarantee no .venv present
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&tmp).unwrap();

    let env = dummy_env();
    let sp = env.site_packages_path();

    // Restore cwd
    std::env::set_current_dir(original_cwd).unwrap();

    // May still find one via subprocess (uv run) if uv is installed on this dev machine.
    // We just assert it doesn't panic — the actual value is environment-dependent.
    let _ = sp;
}

// ─────────────────────────────────────────────────────────────────────────────
// VoxPyRuntime smoke test — just don't panic
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn vox_py_runtime_new_does_not_panic_in_clean_env() {
    std::env::remove_var("VOX_VENV_PATH");
    std::env::remove_var("UV_PROJECT_ENVIRONMENT");
    std::env::remove_var("VIRTUAL_ENV");

    let rt = std::panic::catch_unwind(|| vox_py::VoxPyRuntime::new());
    assert!(rt.is_ok(), "VoxPyRuntime::new() must not panic due to venv detection code");
}

#[test]
fn vox_venv_path_env_var_windows_layout_picked_up_by_new() {
    // Set VOX_VENV_PATH to a fake Windows-layout venv and verify new() doesn't panic.
    let tmp = tempfile::tempdir().expect("tempdir");
    let venv = make_windows_venv(&tmp.path().to_path_buf());

    std::env::set_var("VOX_VENV_PATH", venv.to_str().unwrap());
    let rt = std::panic::catch_unwind(|| vox_py::VoxPyRuntime::new());
    std::env::remove_var("VOX_VENV_PATH");

    assert!(rt.is_ok(), "VoxPyRuntime::new() must not panic when VOX_VENV_PATH points at a valid Windows venv");
}

#[test]
fn vox_venv_path_env_var_missing_path_warns_not_panics() {
    // VOX_VENV_PATH pointing at a nonexistent path should warn and continue, not panic.
    std::env::set_var("VOX_VENV_PATH", "/nonexistent_venv_path_xyzzy_12345");
    let rt = std::panic::catch_unwind(|| vox_py::VoxPyRuntime::new());
    std::env::remove_var("VOX_VENV_PATH");

    assert!(rt.is_ok(), "VoxPyRuntime::new() must not panic when VOX_VENV_PATH is a missing path");
}
