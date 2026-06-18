//! Build script for `voxup`.
//!
//! Propagates the TARGET environment variable to the compiler.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| {
        #[cfg(target_arch = "x86_64")]
        let arch = "x86_64";
        #[cfg(target_arch = "aarch64")]
        let arch = "aarch64";

        #[cfg(target_os = "windows")]
        let os = "pc-windows-msvc";
        #[cfg(target_os = "linux")]
        let os = "unknown-linux-gnu";
        #[cfg(target_os = "macos")]
        let os = "apple-darwin";

        format!("{arch}-{os}")
    });
    println!("cargo:rustc-env=TARGET={target}");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-changed=build.rs");
}
