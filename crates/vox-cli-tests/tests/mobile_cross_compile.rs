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

/// Resolve the NDK toolchain or skip the test with a clear reason. Returns
/// `None` when the gate should silently pass (no NDK on this machine).
fn ndk_toolchain() -> Option<(String, PathBuf)> {
    if std::env::var_os("VOX_CLI_TESTS_SKIP_NDK").is_some() {
        eprintln!("skipping: VOX_CLI_TESTS_SKIP_NDK is set");
        return None;
    }
    let ndk_home = match std::env::var("ANDROID_NDK_HOME") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!("skipping: ANDROID_NDK_HOME not set");
            return None;
        }
    };
    if !Path::new(&ndk_home).is_dir() {
        eprintln!("skipping: ANDROID_NDK_HOME points at non-existent dir {ndk_home}");
        return None;
    }
    let cargo_ndk = match which("cargo-ndk") {
        Some(p) => p,
        None => {
            eprintln!("skipping: `cargo-ndk` not on PATH (install: `cargo install cargo-ndk`)");
            return None;
        }
    };
    Some((ndk_home, cargo_ndk))
}

/// Generic helper: build `package` for a given Android target and assert the
/// expected library artifact lands at `target/<triple>/release/<lib>`.
fn assert_cross_compiles(package: &str, target_triple: &str, expected_lib: &str) {
    let Some((ndk_home, cargo_ndk)) = ndk_toolchain() else {
        return;
    };
    let root = workspace_root();
    let output = Command::new(&cargo_ndk)
        .args([
            "ndk",
            "-t",
            target_triple,
            "build",
            "-p",
            package,
            "--release",
        ])
        .env("ANDROID_NDK_HOME", &ndk_home)
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| panic!("spawn cargo-ndk for {package}/{target_triple}: {e}"));
    assert!(
        output.status.success(),
        "cargo ndk build -p {package} -t {target_triple} FAILED\n--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let so = root
        .join("target")
        .join(target_triple)
        .join("release")
        .join(expected_lib);
    assert!(
        so.is_file(),
        "expected cdylib at {}; `cargo ndk` succeeded but produced no library",
        so.display()
    );
}

/// Aarch64 is the architecture that users actually ship to — the gate that
/// matters most. The other three architectures' tests are heavier but cheap
/// once the host build cache is warm.
#[test]
fn vox_runtime_rn_cross_compiles_to_aarch64_android() {
    assert_cross_compiles("vox-runtime-rn", "aarch64-linux-android", "libvox_runtime_rn.so");
}

/// 32-bit ARM (older devices + Wear OS).
#[test]
fn vox_runtime_rn_cross_compiles_to_armv7_android() {
    assert_cross_compiles("vox-runtime-rn", "armv7-linux-androideabi", "libvox_runtime_rn.so");
}

/// Used by the x86_64 emulator (the one EAS Build CI runs).
#[test]
fn vox_runtime_rn_cross_compiles_to_x86_64_android() {
    assert_cross_compiles("vox-runtime-rn", "x86_64-linux-android", "libvox_runtime_rn.so");
}

/// 32-bit x86 emulator (older Android Studio defaults).
#[test]
fn vox_runtime_rn_cross_compiles_to_i686_android() {
    assert_cross_compiles("vox-runtime-rn", "i686-linux-android", "libvox_runtime_rn.so");
}

/// vox-journal must also cross-compile cleanly — proves the new mobile-friendly
/// substrate is self-contained.
#[test]
fn vox_journal_cross_compiles_to_aarch64_android() {
    assert_cross_compiles("vox-journal", "aarch64-linux-android", "libvox_journal.so");
}

/// Build `vox-runtime-rn` as a static lib for `aarch64-apple-ios` (the arch
/// real iPhones ship). Unlike the Android gates this uses a plain
/// `cargo build --target` (no cargo-ndk); Apple's toolchain only runs on macOS.
///
/// PENDING — this project has no macOS host yet, so the gate cannot run here.
/// It is `#[ignore]`d (not deleted) so it surfaces in every `cargo test` run as
/// a visible reminder. **When a macOS host / CI runner exists: remove the
/// `#[ignore]`, run it, and wire it into the EAS Build CI matrix.** Full
/// build + lipo instructions live in
/// `docs/src/architecture/vox-runtime-rn-mobile-cross-compile.md` (§iOS).
#[test]
#[ignore = "iOS cross-compile requires a macOS host (none available yet) — see \
            docs/src/architecture/vox-runtime-rn-mobile-cross-compile.md; remove \
            #[ignore] once a macOS runner exists"]
fn vox_runtime_rn_cross_compiles_to_aarch64_ios() {
    if !cfg!(target_os = "macos") {
        eprintln!(
            "skipping: iOS cross-compile requires macOS; this host is {}",
            std::env::consts::OS
        );
        return;
    }
    let root = workspace_root();
    let output = Command::new("cargo")
        .args([
            "build",
            "-p",
            "vox-runtime-rn",
            "--release",
            "--target",
            "aarch64-apple-ios",
        ])
        .current_dir(&root)
        .output()
        .expect("spawn cargo build for aarch64-apple-ios");
    assert!(
        output.status.success(),
        "cargo build --target aarch64-apple-ios -p vox-runtime-rn FAILED\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    let lib = root
        .join("target")
        .join("aarch64-apple-ios")
        .join("release")
        .join("libvox_runtime_rn.a");
    assert!(
        lib.is_file(),
        "expected staticlib at {}; build succeeded but produced no .a",
        lib.display()
    );
}
