//! Contract gates for the `clients/runtime-*` npm packages.
//!
//! Each package implements `@vox/runtime-types::VoxRuntime` for its target
//! shell (Tauri desktop, Expo mobile). Drift in either implementation would
//! mean an emitted Vox app fails to type-check OR (worse) silently calls
//! into a method whose runtime behavior differs from the contract.
//!
//! These tests shell out to `tsc --noEmit` against the package's own
//! `tsconfig.test.json`, which type-checks the package source plus a
//! contract test file that asserts every method has the expected signature.
//!
//! Skipped when `VOX_CLI_TESTS_SKIP_TSC=1`.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Workspace root, computed once.
fn workspace_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        // crates/vox-cli-tests/ is two levels under the workspace root.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root resolvable from CARGO_MANIFEST_DIR")
            .to_path_buf()
    })
}

/// Locate `npx`. Returns `None` if not on PATH (tests then skip gracefully).
///
/// On Windows prefer the `.cmd` shim first — a bare-name file with no
/// extension is usually a shell script and raises "os error 193" when
/// spawned directly as a Win32 process. This mirrors the priority used by
/// the harness's own `which_executable`.
fn find_npx() -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) {
        &[".cmd", ".exe", ".bat", ""]
    } else {
        &[""]
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in exts {
            let candidate = dir.join(format!("npx{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn run_package_contract(package_dir: &str, tsconfig: &str) {
    if std::env::var_os("VOX_CLI_TESTS_SKIP_TSC").is_some() {
        return;
    }
    let pkg_path = workspace_root().join(package_dir);
    assert!(
        pkg_path.is_dir(),
        "package dir missing: {}",
        pkg_path.display()
    );
    let Some(npx) = find_npx() else {
        eprintln!("warning: `npx` not on PATH; skipping {package_dir} contract gate");
        return;
    };
    let output = Command::new(&npx)
        .args([
            "--yes",
            "-p",
            "typescript@5",
            "--",
            "tsc",
            "-p",
            tsconfig,
            "--noEmit",
        ])
        .current_dir(&pkg_path)
        .output()
        .unwrap_or_else(|e| panic!("spawn tsc for {package_dir}: {e}"));
    assert!(
        output.status.success(),
        "tsc failed for {package_dir}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// `@vox/runtime-rn` must satisfy `VoxRuntime`. Asserted via a contract test
/// that imports the exported `voxRuntime` value and assigns it to a
/// `VoxRuntime`-typed binding; any divergence is a compile-time error.
#[test]
fn runtime_rn_satisfies_vox_runtime_contract() {
    run_package_contract("clients/runtime-rn", "tsconfig.test.json");
}

/// `@vox/runtime` (Tauri impl) must satisfy `VoxRuntime`. Same regime as the
/// RN contract test — proves the desktop adapter and the mobile adapter
/// implement the SAME contract, so a Vox source that compiles for one
/// target will compile for the other.
#[test]
fn runtime_web_satisfies_vox_runtime_contract() {
    run_package_contract("clients/runtime-web", "tsconfig.test.json");
}
