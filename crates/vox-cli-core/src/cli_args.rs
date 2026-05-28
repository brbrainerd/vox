//! Shared CLI argument types and enums for various commands.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Build mode (`app` or `library`).
#[derive(Clone, Copy, Debug, ValueEnum, Default, Serialize, Deserialize, PartialEq)]
pub enum BuildMode {
    /// Emit app code + components (default).
    #[default]
    App,
    /// Emit UI-agnostic models, schemas, and client fetchers.
    Library,
}

/// Bundling mode: `app` (web + backend) or `script` (binary only).
#[derive(Clone, Copy, Debug, ValueEnum, Default, Serialize, Deserialize, PartialEq)]
pub enum BundleMode {
    /// Web application with React frontend and Axum backend.
    #[default]
    App,
    /// Native binary script for mesh/CLI execution.
    Script,
}

/// `vox compile --target …` packaging lane.
#[derive(Clone, Copy, Debug, ValueEnum, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CompileKind {
    /// Axum + embedded SPA single binary (same as `vox bundle-app`).
    #[default]
    NativeBinary,
    /// Desktop installer path: bundle + Tauri packaging hints under `target/generated/tauri-packaging/`.
    Desktop,
    /// Android mobile (Tauri mobile toolchain; emits hints + requires Android SDK).
    MobileAndroid,
    /// iOS mobile (requires macOS + Xcode).
    MobileIos,
    /// OCI/server packaging — use `vox deploy` instead (compile prints guidance).
    Server,
    /// `fn main()` script binary (`script-execution` feature).
    Script,
    /// WASI wasm artifact (`script-execution`, isolation wasm).
    Wasi,
}

/// `vox upgrade` lane: release binary vs local repository checkout.
#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradeLane {
    /// Checksums-verified release archive into `CARGO_HOME/bin` (default).
    #[default]
    Release,
    /// Fetch / fast-forward then `cargo install`.
    Repo,
}
