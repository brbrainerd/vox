//! `VoxConfig` — Single Source of Truth for all Vox toolchain settings.
//!
//! Precedence (highest → lowest):
//!   ENV VARS > Vox.toml (workspace) > ~/.vox/config.toml (global) > compiled defaults
//!
//! CLI flags must be applied by the caller *after* calling `VoxConfig::load()`.
//! See: `docs/agents/config-hierarchy.md`

mod gamify_web;
mod impl_ops;
mod persist;
mod toml_schema;
// Keep the internal module name (`vox_config`) private; callers should import `vox_config::VoxConfig`.
mod vox_config;

pub use gamify_web::{BuildTarget, GamifyMode, WebRunMode};
pub use vox_config::VoxConfig;

/// Test-only accessor for the file path `VoxConfig::save()` writes to.
#[cfg(test)]
pub(crate) fn persist_test_global_config_path() -> Option<std::path::PathBuf> {
    persist::global_config_path()
}
