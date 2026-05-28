//! Preflight for `vox compile --triple` / mobile targets.

use std::process::Command;

use super::super::common::Check;

pub fn run(triple: &str, checks: &mut Vec<Check>) {
    let rustup = if cfg!(target_os = "windows") {
        "rustup.exe"
    } else {
        "rustup"
    };

    let Ok(out) = Command::new(rustup)
        .args(["target", "list", "--installed"])
        .output()
    else {
        checks.push(Check::fail(
            "rustup",
            "rustup not found on PATH — install Rust toolchain",
        ));
        return;
    };

    let s = String::from_utf8_lossy(&out.stdout);
    if !s.lines().any(|l| l.trim() == triple) {
        checks.push(Check::fail(
            format!("rustup target `{triple}`"),
            format!("run: rustup target add {triple}"),
        ));
    } else {
        checks.push(Check::pass(
            format!("rustup target `{triple}`"),
            "installed",
        ));
    }

    if triple.contains("android") {
        let sdk = std::env::var("ANDROID_HOME")
            .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
            .unwrap_or_default();
        if sdk.is_empty() {
            checks.push(Check::fail(
                "Android SDK (ANDROID_HOME)",
                "set ANDROID_HOME or ANDROID_SDK_ROOT for mobile-android compile",
            ));
        } else {
            checks.push(Check::pass("Android SDK (ANDROID_HOME)", sdk));
        }
    }

    if triple.contains("apple-ios") || triple.contains("-ios") {
        let has_xc = Command::new("xcode-select")
            .arg("-p")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !has_xc {
            checks.push(Check::fail(
                "Xcode (xcode-select -p)",
                "install Xcode command-line tools for iOS targets",
            ));
        } else {
            checks.push(Check::pass("Xcode (xcode-select -p)", "ok"));
        }
    }

    if triple.contains("android")
        || triple.contains("apple-ios")
        || triple.contains("-ios")
    {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let ok = Command::new(&cargo)
            .args(["tauri", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            checks.push(Check::fail(
                "cargo tauri",
                "install Tauri CLI: `cargo install tauri-cli --locked` (v2; required for mobile/desktop packaging)",
            ));
        } else {
            checks.push(Check::pass("cargo tauri", "ok"));
        }
    }
}
