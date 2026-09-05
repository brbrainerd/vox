use std::path::PathBuf;
use thiserror::Error;

/// Plugin-missing error. `Display` calls into [`crate::format_install_hint`]
/// so the rendered message includes the workspace-local install command
/// when the caller is running from a Vox workspace checkout — see that
/// helper for the exact format and detection logic.
#[derive(Debug)]
pub struct PluginMissingError {
    pub plugin_id: &'static str,
    pub extension_point: &'static str,
}

impl std::fmt::Display for PluginMissingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "This Vox feature requires the '{}' plugin (extension point '{}'), which is not installed.\n\nTo install it, run:\n\n{}",
            self.plugin_id,
            self.extension_point,
            crate::format_install_hint(self.plugin_id, None)
        )
    }
}

impl std::error::Error for PluginMissingError {}

#[derive(Debug)]
pub struct SkillNotInstalledError {
    pub skill_id: String,
}

impl std::fmt::Display for SkillNotInstalledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Skill '{}' is not installed.\n\nTo install it, run:\n\n{}",
            self.skill_id,
            crate::format_install_hint(&self.skill_id, None)
        )
    }
}

impl std::error::Error for SkillNotInstalledError {}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("plugin manifest at {path:?} failed to parse: {source}")]
    ManifestParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("plugin dylib at {path:?} failed to dlopen: {source}")]
    DlopenFailed {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    #[error("plugin '{0}' has mismatched ABI: {0:?}")]
    AbiMismatch(AbiMismatchError),
    #[error("plugin init returned an error: {0}")]
    InitFailed(String),
    #[error("io error reading {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The plugin's own manifest `version` does not match the running core's
    /// `CARGO_PKG_VERSION`. Unlike the ABI-range check in
    /// [`crate::loader::Loader::load`], there is no compatibility window
    /// here: a stale or newer plugin binary is refused rather than risking
    /// an incompatible in-memory layout with the host.
    #[error(
        "plugin '{plugin_id}' version {found} does not match running core version {expected}; reinstall the plugin to match this vox build"
    )]
    VersionMismatch {
        plugin_id: String,
        expected: String,
        found: String,
    },
}

#[derive(Debug, Error)]
#[error("plugin '{id}' has ABI version {plugin_abi}, host supports {host_abi_min}..={host_abi}")]
pub struct AbiMismatchError {
    pub id: String,
    pub plugin_abi: u32,
    /// Newest ABI the host speaks ([`vox_plugin_api::VOX_PLUGIN_ABI_VERSION`]).
    pub host_abi: u32,
    /// Oldest ABI the host still accepts ([`vox_plugin_api::VOX_PLUGIN_ABI_MIN_SUPPORTED`]).
    pub host_abi_min: u32,
}
