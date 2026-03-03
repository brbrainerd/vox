//! # vox-skills — Skill Marketplace and Plugin Architecture
//!
//! Provides a typed skill registry, skill bundle format parsing,
//! plugin lifecycle management, and an optional Vox Skills HTTP bridge.

pub mod builtins;
pub mod bundle;
pub mod hooks;
pub mod manifest;
pub mod parser;
pub mod plugin;
pub mod registry;

#[cfg(feature = "skills-registry")]
pub mod registry_api;

pub use builtins::install_builtins;
pub use bundle::{SkillBundle, VoxSkillBundle};
pub use hooks::{HookEvent, HookFn, HookRegistry};
pub use manifest::{SkillCategory, SkillManifest, SkillPermission};
pub use plugin::{Plugin, PluginKind, PluginManager};
pub use registry::{InstallResult, SkillRegistry, UninstallResult};

/// The canonical Vox Skills marketplace registry URL.
pub const SKILLS_REGISTRY_BASE: &str = "https://raw.githubusercontent.com/brbrainerd/vox/main/skills";

/// Errors from the skill system.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("Skill already installed: {0}")]
    AlreadyInstalled(String),
    #[error("Version conflict: installed={installed}, requested={requested}")]
    VersionConflict {
        installed: String,
        requested: String,
    },
    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("Permission denied: skill requires {0:?}")]
    PermissionDenied(SkillPermission),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML error: {0}")]
    Toml(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Hook error: {0}")]
    Hook(String),
}
