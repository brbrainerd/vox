//! `PluginStateBackend` impl for `VoxDb`.
//!
//! Lets the L3 plugin host (`vox-plugin-host`) accept the trait
//! (`Arc<dyn PluginStateBackend>`) instead of a concrete `Arc<VoxDb>`, breaking
//! the layering inversion documented in
//! `docs/src/architecture/2026-05-08-workspace-reorg-design.md`.

use async_trait::async_trait;
use vox_plugin_types::state_backend::{
    PluginStateBackend, PluginStateError, PluginStateSkillEntry,
};

#[async_trait]
impl PluginStateBackend for crate::VoxDb {
    async fn publish_skill(
        &self,
        id: &str,
        version: &str,
        manifest_json: &str,
        skill_md: &str,
    ) -> Result<(), PluginStateError> {
        crate::VoxDb::publish_skill(self, id, version, manifest_json, skill_md)
            .await
            .map_err(|e| PluginStateError::new(e.to_string()))
    }

    async fn unpublish_skill(&self, id: &str) -> Result<(), PluginStateError> {
        crate::VoxDb::unpublish_skill(self, id)
            .await
            .map_err(|e| PluginStateError::new(e.to_string()))
    }

    async fn list_skill_manifests(&self) -> Result<Vec<PluginStateSkillEntry>, PluginStateError> {
        let entries = crate::VoxDb::list_skill_manifests(self)
            .await
            .map_err(|e| PluginStateError::new(e.to_string()))?;
        Ok(entries
            .into_iter()
            .map(|e| PluginStateSkillEntry {
                id: e.id,
                version: e.version,
                manifest_json: e.manifest_json,
                skill_md: e.skill_md,
            })
            .collect())
    }

    async fn claim_skill_identity(
        &self,
        identity: &str,
        owner: &str,
    ) -> Result<(), PluginStateError> {
        crate::VoxDb::claim_skill_identity(self, identity, owner)
            .await
            .map_err(|e| PluginStateError::new(e.to_string()))
    }

    async fn get_skill_reliability(&self, skill_id: &str) -> Result<Option<f64>, PluginStateError> {
        crate::VoxDb::get_skill_reliability(self, skill_id)
            .await
            .map_err(|e| PluginStateError::new(e.to_string()))
    }
}
