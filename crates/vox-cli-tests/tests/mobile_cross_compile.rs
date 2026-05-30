//! Cross-compile gate: `vox-runtime-rn` must build for `aarch64-linux-android`.
//!
//! Skipped automatically when the toolchain is absent so contributors without
//! NDK installed don't see false-positive failures.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root resolvable")
        .to_path_buf()
}

fn which(name: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Build vox-runtime-rn for aarch64-linux-android via `cargo ndk` and assert
/// the resulting `.so` exists. Skipped when the NDK toolchain isn't available.
#[test]
fn mobile_cross_compile_aarch64_android_succeeds() {
    if std::env::var_os("VOX_CLI_TESTS_SKIP_NDK").is_some() {
        eprintln!("skipping: VOX_CLI_TESTS_SKIP_NDK is set");
        return;
    }
    let ndk_home = match std::env::var("ANDROID_NDK_HOME") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!("skipping: ANDROID_NDK_HOME not set");
            return;
        }
    };
    if !Path::new(&ndk_home).is_dir() {
        eprintln!("skipping: ANDROID_NDK_HOME points at non-existent dir {ndk_home}");
        return;
    }
    let Some(cargo_ndk) = which("cargo-ndk") else {
        eprintln!("skipping: `cargo-ndk` not on PATH (install: `cargo install cargo-ndk`)");
        return;
    };

    let root = workspace_root();
    let output = Command::new(&cargo_ndk)
        .args([
            "ndk",
            "-t",
            "aarch64-linux-android",
            "build",
            "-p",
            "vox-runtime-rn",
            "--release",
        ])
        .env("ANDROID_NDK_HOME", &ndk_home)
        .current_dir(&root)
        .output()
        .expect("spawn cargo-ndk");
    assert!(
        output.status.success(),
        "cargo ndk build -p vox-runtime-rn -t aarch64-linux-android FAILED\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let so = root
        .join("target")
        .join("aarch64-linux-android")
        .join("release")
        .join("libvox_runtime_rn.so");
    assert!(
        so.is_file(),
        "expected cdylib at {}, but the file is missing — `cargo ndk` succeeded but produced no .so",
        so.display()
    );
}
