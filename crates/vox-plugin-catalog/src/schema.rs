//! Catalog schema: plugin, bundle, and component entry types parsed from `catalog.toml`.

use serde::{Deserialize, Serialize};

/// Lifecycle status of a catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CatalogStatus {
    /// Early preview; API or behavior may change.
    Alpha,
    /// Feature-complete but not yet production-hardened.
    Beta,
    /// Production-ready; breaking changes follow semver.
    #[default]
    Stable,
    /// Maintained for compatibility; prefer the replacement if one is listed.
    Deprecated,
}

/// One entry in the plugin catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PluginCatalogEntry {
    /// Globally unique short id, e.g. "mens-candle-cuda" or "skill-compiler".
    pub id: String,

    /// Which payload kind this plugin ships.
    pub payload_kind: PayloadKind,

    /// One-line human description.
    pub description: String,

    /// Lifecycle stage. Defaults to `stable` when absent.
    #[serde(default)]
    pub status: CatalogStatus,

    /// For `code` payloads: extension-point trait names this plugin provides.
    #[serde(default)]
    pub extension_points: Option<Vec<String>>,

    /// For `skill` payloads: MCP tool names this skill exposes to agents.
    #[serde(default)]
    pub exposes_tools: Option<Vec<String>>,

    /// Optional capability tag (e.g. "nvidia-gpu") informational only.
    #[serde(default)]
    pub requires_tag: Option<String>,

    /// Where to fetch the plugin from for `vox plugin install <id>`.
    /// Always present for first-party plugins (1a guarantee — every plugin
    /// is standalone-installable, not bundle-only).
    pub default_source: String,

    /// Advisory list of first-party bundles that pre-install this plugin.
    /// Shown by `vox plugin info`. Does not gate standalone install.
    #[serde(default)]
    pub bundled_in: Vec<String>,
}

/// Discriminator for plugin payload kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PayloadKind {
    Code,
    Skill,
    Composite,
}

/// One distribution-bundle entry in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BundleEntry {
    pub id: String,
    pub description: String,

    /// Lifecycle stage. Defaults to `stable` when absent.
    #[serde(default)]
    pub status: CatalogStatus,

    /// Optional parent bundle whose plugin set is inherited.
    #[serde(default)]
    pub extends: Option<String>,

    /// Plugins added on top of any inherited set. May be empty.
    #[serde(default)]
    pub plugins: Vec<String>,
}

/// One installable *component*: a first-party Vox binary that is NOT a cdylib
/// plugin (it implements no extension-point trait and is not loaded by the
/// plugin host). Components are optional companion executables — currently just
/// the Tauri GUI — that ship alongside the host binary and are installed on
/// demand via `vox plugin install <id>` / `vox gui` install-if-absent. CLI-only
/// users never fetch them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Component {
    /// Globally unique short id, e.g. "gui".
    pub id: String,

    /// Installed executable file name without extension (consumers append
    /// ".exe" on Windows), e.g. "vox-gui".
    pub binary: String,

    /// One-line human description.
    pub description: String,

    /// Lifecycle stage. Defaults to `stable` when absent.
    #[serde(default)]
    pub status: CatalogStatus,

    /// Platform constraints. Empty vectors mean "no constraint".
    #[serde(default)]
    pub requires: ComponentRequires,

    /// Where to fetch the component for `vox plugin install <id>`. Mirrors the
    /// plugin `default-source` convention: `local:<path>` or `github:owner/repo`.
    pub default_source: String,
}

/// Platform gating for a [`Component`]. The host OS must appear in `os` (when
/// non-empty) AND host arch in `arch` (when non-empty).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ComponentRequires {
    /// Allowed `std::env::consts::OS` values (e.g. "windows","macos","linux").
    /// Empty = any OS.
    #[serde(default)]
    pub os: Vec<String>,

    /// Allowed `std::env::consts::ARCH` values (e.g. "x86_64","aarch64").
    /// Empty = any arch.
    #[serde(default)]
    pub arch: Vec<String>,
}
