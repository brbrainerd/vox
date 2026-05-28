//! `PluginDir` — a temporary directory pre-populated with a Plugin.toml.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use vox_plugin_types::plugin_manifest::PluginManifest;

/// A temporary plugin root directory containing a valid `Plugin.toml`.
///
/// Cleaned up automatically when the value is dropped. Use [`PluginDir::path`]
/// to pass the directory to `vox-plugin-host` loader APIs.
pub struct PluginDir {
    _dir: TempDir,
    path: PathBuf,
}

impl PluginDir {
    /// Create a `PluginDir` from a Plugin.toml TOML source string.
    ///
    /// Parses the TOML eagerly so tests fail immediately on invalid manifests
    /// rather than only at loader call time.
    pub fn from_toml(toml_src: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let _manifest: PluginManifest =
            toml::from_str(toml_src).map_err(|e| format!("Plugin.toml parse error: {e}"))?;
        let dir = TempDir::new()?;
        let plugin_toml_path = dir.path().join("Plugin.toml");
        std::fs::write(&plugin_toml_path, toml_src)?;
        let path = dir.path().to_path_buf();
        Ok(Self { _dir: dir, path })
    }

    /// Path to the temporary plugin root directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Path to the `Plugin.toml` file inside the directory.
    pub fn manifest_path(&self) -> PathBuf {
        self.path.join("Plugin.toml")
    }

    /// Read back and parse the manifest. Useful for round-trip assertions.
    pub fn parse_manifest(&self) -> Result<PluginManifest, Box<dyn std::error::Error>> {
        let src = std::fs::read_to_string(self.manifest_path())?;
        Ok(toml::from_str(&src)?)
    }

    /// Write an empty file at `relative_path` inside the plugin dir.
    ///
    /// Use this to pre-create artifact stubs (e.g. `libfoo.so`) so that
    /// loaders that check file existence before loading don't fail early.
    pub fn touch(&self, relative_path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let target = self.path.join(relative_path.as_ref());
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, b"")?;
        Ok(())
    }
}

/// Assert that a Plugin.toml TOML string parses without error.
///
/// Panics with a readable error message on failure.
#[track_caller]
pub fn assert_manifest_parses(toml_src: &str) -> PluginManifest {
    toml::from_str(toml_src)
        .unwrap_or_else(|e| panic!("Plugin.toml failed to parse:\n{e}\n\nInput:\n{toml_src}"))
}
