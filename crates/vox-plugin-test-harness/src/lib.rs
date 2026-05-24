//! # vox-plugin-test-harness
//!
//! Shared test utilities for Vox plugin authors:
//!
//! - [`manifest_builder`] — fluent builders for Plugin.toml TOML strings
//! - [`plugin_dir`] — [`PluginDir`] temp-directory helper
//!
//! ## Quick start
//!
//! ```rust
//! use vox_plugin_test_harness::manifest_builder::CodeManifestBuilder;
//! use vox_plugin_test_harness::plugin_dir::{PluginDir, assert_manifest_parses};
//!
//! let toml = CodeManifestBuilder::new("test-plugin")
//!     .extension_point("MlBackend")
//!     .artifact("linux-x86_64", "libtest.so")
//!     .build();
//!
//! let dir = PluginDir::from_toml(&toml).expect("valid toml");
//! let manifest = dir.parse_manifest().expect("round-trip");
//! assert_eq!(manifest.plugin.id, "test-plugin");
//! ```

pub mod manifest_builder;
pub mod plugin_dir;

pub use plugin_dir::{PluginDir, assert_manifest_parses};

#[cfg(test)]
mod tests {
    use super::*;
    use manifest_builder::{CodeManifestBuilder, SkillManifestBuilder};

    #[test]
    fn code_builder_roundtrip() {
        let toml = CodeManifestBuilder::new("my-plugin")
            .name("My Plugin")
            .extension_point("MlBackend")
            .artifact("linux-x86_64", "libmy.so")
            .build();
        let dir = PluginDir::from_toml(&toml).expect("valid");
        let m = dir.parse_manifest().expect("round-trip");
        assert_eq!(m.plugin.id, "my-plugin");
        assert_eq!(m.plugin.name, "My Plugin");
    }

    #[test]
    fn skill_builder_roundtrip() {
        let toml = SkillManifestBuilder::new("my-skill")
            .skill_md("SKILL.md")
            .exposes("my_tool")
            .build();
        let m = assert_manifest_parses(&toml);
        assert_eq!(m.plugin.id, "my-skill");
    }

    #[test]
    fn plugin_dir_touch() {
        let toml = CodeManifestBuilder::new("touch-test")
            .artifact("linux-x86_64", "libtouch.so")
            .build();
        let dir = PluginDir::from_toml(&toml).expect("valid");
        dir.touch("libtouch.so").expect("touch");
        assert!(dir.path().join("libtouch.so").exists());
    }
}
