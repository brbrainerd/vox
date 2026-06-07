//! Pure-types surface for the vox plugin system.
//!
//! Designed to be a leaf dep: no async runtime, no DB client, no abi_stable.
//! Crates that need only the manifest/skill/state-backend shapes can depend
//! here without pulling in `vox-plugin-api`'s full ABI machinery or `vox-db`.
//!
//! Re-exported by `vox-plugin-api` (manifest types) and `vox-plugin-host`
//! (skill manifest + state-backend trait) for backwards compatibility.

pub mod plugin_manifest;
pub mod skill_manifest;
pub mod state_backend;
pub mod target;

pub use plugin_manifest::{
    CodePayload, CompositePayload, HostRequirement, NativeLib, PayloadProvides, PayloadRequires,
    PluginHeader, PluginManifest, PluginPayload, SkillPayload, SkillTools,
};
pub use skill_manifest::{SkillCategory, SkillManifest, SkillPermission};
pub use state_backend::{PluginStateBackend, PluginStateError, PluginStateSkillEntry};
pub use target::{PLUGIN_TARGET_TRIPLES, current_target_triple, plugin_artifact_filename};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_manifest_default_has_empty_id() {
        let m = SkillManifest::default();
        assert_eq!(m.id, "");
        assert!(m.permissions.is_empty());
        assert!(m.tools.is_empty());
    }
}
