//! Unified skill registry — in-memory + optional DB persistence.
//!
//! This is the single authoritative registry for skills in the Vox host process.
//! It supersedes `vox_skills::SkillRegistry` (which now re-exports from here).
//!
//! ## Variants
//!
//! * **Plugin-only** (no DB): constructed with [`SkillRegistry::new`].  Used by
//!   `discover()` at startup and tests that don't need persistence.
//! * **DB-backed**: constructed with [`SkillRegistry::new`] then
//!   [`SkillRegistry::set_db`] / [`SkillRegistry::with_db`].  The orchestrator
//!   attaches a `VoxDb` so installs and uninstalls are journalled to Codex.

use crate::errors::SkillNotInstalledError;
use crate::skill_manifest::{SkillCategory, SkillManifest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};
use vox_plugin_api::skill::LoadedSkill;
use vox_plugin_types::state_backend::PluginStateBackend;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// Result of a skill installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    /// Installed skill id.
    pub id: String,
    /// Installed version string.
    pub version: String,
    /// True when this id+version was already present (no-op install).
    pub already_installed: bool,
    /// Content hash of the bundle at install time.
    pub hash: String,
}

/// Result of a skill uninstallation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallResult {
    /// Skill id passed to [`SkillRegistry::uninstall`].
    pub id: String,
    /// True when a manifest was removed from the in-memory map.
    pub was_installed: bool,
}

// ---------------------------------------------------------------------------
// Internal registered-skill shape
// ---------------------------------------------------------------------------

/// A skill entry held in the unified registry.
#[derive(Debug, Clone)]
pub struct RegisteredSkill {
    /// Full skill metadata.
    pub manifest: SkillManifest,
    /// Raw SKILL.md body (present for plugin-discovered skills).
    pub body: Option<String>,
    /// Source that contributed this skill.
    pub source: SkillSource,
}

/// Where a registered skill came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillSource {
    /// Discovered from a plugin at `plugin_id`.
    Plugin { plugin_id: String },
    /// Installed from a `VoxSkillBundle` (marketplace / manual install).
    Bundle,
    /// Pulled from an OpenClaw mesh node.
    OpenClaw { node_id: String },
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Unified in-memory skill registry with optional persistent backend.
pub struct SkillRegistry {
    skills: Mutex<HashMap<String, RegisteredSkill>>,
    backend: Mutex<Option<Arc<dyn PluginStateBackend>>>,
}

impl SkillRegistry {
    /// In-memory registry without persistence.
    pub fn new() -> Self {
        Self {
            skills: Mutex::new(HashMap::new()),
            backend: Mutex::new(None),
        }
    }

    /// Fluent constructor: same as [`SkillRegistry::new`] then [`SkillRegistry::set_state_backend`].
    pub fn with_state_backend(self, backend: Arc<dyn PluginStateBackend>) -> Self {
        *self.backend.lock().unwrap_or_else(|e| e.into_inner()) = Some(backend);
        self
    }

    /// Attach a persistent backend (e.g. `vox-db::VoxDb`) after construction.
    pub fn set_state_backend(&self, backend: Arc<dyn PluginStateBackend>) {
        *self.backend.lock().unwrap_or_else(|e| e.into_inner()) = Some(backend);
    }

    /// Backwards-compat alias kept until Phase 7. Same as [`Self::with_state_backend`].
    pub fn with_db(self, backend: Arc<dyn PluginStateBackend>) -> Self {
        self.with_state_backend(backend)
    }

    /// Backwards-compat alias kept until Phase 7. Same as [`Self::set_state_backend`].
    pub fn set_db(&self, backend: Arc<dyn PluginStateBackend>) {
        self.set_state_backend(backend);
    }

    fn get_backend(&self) -> Option<Arc<dyn PluginStateBackend>> {
        self.backend
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    // -----------------------------------------------------------------------
    // Plugin-host path: register a LoadedSkill from discover()
    // -----------------------------------------------------------------------

    /// Register a [`LoadedSkill`] discovered from the plugin install directory.
    ///
    /// Both `LoadedSkill.manifest` and the host-side `SkillManifest` are now the same
    /// canonical type from `vox-plugin-types` (D-17 unification). If `author` is empty,
    /// it is filled in from `plugin_id` so registry callers always see a non-empty author.
    pub fn install(&self, skill: LoadedSkill) {
        let mut manifest = skill.manifest;
        if manifest.author.is_empty() {
            manifest.author = skill.plugin_id.clone();
        }
        let entry = RegisteredSkill {
            manifest: manifest.clone(),
            body: Some(skill.body),
            source: SkillSource::Plugin {
                plugin_id: skill.plugin_id,
            },
        };
        self.skills
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(manifest.id.clone(), entry);
    }

    /// Look up a skill by its plugin id (for the in-memory/discover path).
    pub fn lookup(&self, id: &str) -> Result<LoadedSkill, SkillNotInstalledError> {
        let skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
        let entry = skills.get(id).ok_or_else(|| SkillNotInstalledError {
            skill_id: id.to_string(),
        })?;
        // Reconstruct a LoadedSkill on the way out.
        // Both LoadedSkill.manifest and entry.manifest are now the same type (D-17).
        let plugin_id = match &entry.source {
            SkillSource::Plugin { plugin_id } => plugin_id.clone(),
            _ => id.to_string(),
        };
        Ok(LoadedSkill {
            plugin_id,
            format_version: 1,
            manifest: entry.manifest.clone(),
            body: entry.body.clone().unwrap_or_default(),
            exposed_tools: entry.manifest.tools.clone(),
        })
    }

    /// List plugin ids for all plugin-discovered skills.
    pub fn list_ids(&self) -> Vec<String> {
        let skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
        skills
            .values()
            .filter_map(|e| match &e.source {
                SkillSource::Plugin { plugin_id } => Some(plugin_id.clone()),
                _ => None,
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Full DB-backed path: install from VoxSkillBundle
    // -----------------------------------------------------------------------

    /// Install a skill from a `VoxSkillBundle`.
    ///
    /// Mirrors the old `vox_skills::SkillRegistry::install` including the
    /// `vox.mens` → `vox.populi` version-check guard and DB fire-and-forget.
    pub async fn install_bundle(
        &self,
        bundle: &crate::skill_bundle::VoxSkillBundle,
    ) -> Result<InstallResult, BundleInstallError> {
        let id = bundle.manifest.id.clone();
        let version = bundle.manifest.version.clone();
        let hash = bundle.content_hash();

        {
            let mut skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = skills.get(&id)
                && existing.manifest.version == version
            {
                info!(skill = %id, "Skill already installed at same version");
                return Ok(InstallResult {
                    id,
                    version,
                    already_installed: true,
                    hash,
                });
            }
            info!(skill = %id, version = %version, "Installing skill bundle");
            let mut manifest = bundle.manifest.clone();
            manifest.hash = Some(hash.clone());
            skills.insert(
                id.clone(),
                RegisteredSkill {
                    manifest,
                    body: Some(bundle.skill_md.clone()),
                    source: SkillSource::Bundle,
                },
            );
        }

        // Persist to VoxDB (fire-and-forget, no lock held)
        if let Some(db) = self.get_backend() {
            let manifest_json = serde_json::to_string(&bundle.manifest).unwrap_or_default();
            let skill_md = bundle.skill_md.clone();
            let id2 = id.clone();
            let ver2 = version.clone();
            tokio::spawn(async move {
                let _ = db
                    .publish_skill(&id2, &ver2, &manifest_json, &skill_md)
                    .await;
            });
        }

        Ok(InstallResult {
            id,
            version,
            already_installed: false,
            hash,
        })
    }

    /// Uninstall a skill by ID.
    ///
    /// **`vox.mens`** targets the same stored manifest as **`vox.populi`** (legacy id migration).
    pub async fn uninstall(&self, id: &str) -> Result<UninstallResult, UninstallError> {
        let canonical = if id == "vox.mens" { "vox.populi" } else { id };
        let was_installed = {
            let mut skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
            skills.remove(canonical).is_some()
        };
        if was_installed {
            info!(skill = %canonical, "Skill uninstalled");
            if let Some(db) = self.get_backend() {
                let id_owned = canonical.to_string();
                tokio::spawn(async move {
                    let _ = db.unpublish_skill(&id_owned).await;
                });
            }
        } else {
            warn!(skill = %id, "Tried to uninstall skill that was not installed");
        }
        Ok(UninstallResult {
            id: id.to_string(),
            was_installed,
        })
    }

    // -----------------------------------------------------------------------
    // Query surface
    // -----------------------------------------------------------------------

    /// Search skills by keyword (id, name, description, tags).
    pub fn search(&self, query: &str) -> Vec<SkillManifest> {
        let q = query.to_lowercase();
        let skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
        skills
            .values()
            .filter(|e| {
                let m = &e.manifest;
                m.id.to_lowercase().contains(&q)
                    || m.name.to_lowercase().contains(&q)
                    || m.description.to_lowercase().contains(&q)
                    || m.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .map(|e| e.manifest.clone())
            .collect()
    }

    /// List all installed skills, optionally filtered by category.
    pub fn list(&self, category: Option<&SkillCategory>) -> Vec<SkillManifest> {
        let skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
        skills
            .values()
            .filter(|e| category.is_none_or(|c| &e.manifest.category == c))
            .map(|e| e.manifest.clone())
            .collect()
    }

    /// Get a specific skill by ID.
    ///
    /// **`vox.mens`** resolves to **`vox.populi`** if present (bundled mesh skill id migration).
    pub fn get(&self, id: &str) -> Option<SkillManifest> {
        let skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
        skills.get(id).map(|e| e.manifest.clone()).or_else(|| {
            if id == "vox.mens" {
                skills.get("vox.populi").map(|e| e.manifest.clone())
            } else {
                None
            }
        })
    }

    // -----------------------------------------------------------------------
    // DB hydration
    // -----------------------------------------------------------------------

    /// Load all skills from the Codex `skill_manifests` table into memory.
    pub async fn hydrate_from_db(&self) -> Result<usize, HydrateError> {
        let db = match self.get_backend() {
            Some(db) => db,
            None => return Ok(0),
        };
        let entries = db
            .list_skill_manifests()
            .await
            .map_err(|e| HydrateError(e.to_string()))?;
        let mut skills = self.skills.lock().unwrap_or_else(|e| e.into_inner());
        let count = entries.len();
        for entry in entries {
            if let Ok(manifest) = serde_json::from_str::<SkillManifest>(&entry.manifest_json) {
                let id = manifest.id.clone();
                skills.insert(
                    id,
                    RegisteredSkill {
                        manifest,
                        body: None,
                        source: SkillSource::Bundle,
                    },
                );
            }
        }
        Ok(count)
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error returned by [`SkillRegistry::install_bundle`].
#[derive(Debug, thiserror::Error)]
#[error("bundle install error: {0}")]
pub struct BundleInstallError(pub String);

/// Error returned by [`SkillRegistry::uninstall`].
#[derive(Debug, thiserror::Error)]
#[error("uninstall error: {0}")]
pub struct UninstallError(pub String);

/// Error returned by [`SkillRegistry::hydrate_from_db`].
#[derive(Debug, thiserror::Error)]
#[error("hydrate_from_db error: {0}")]
pub struct HydrateError(pub String);

// ---------------------------------------------------------------------------
// Constructor helper
// ---------------------------------------------------------------------------

/// Returns a new empty registry in an [`Arc`]; not a process singleton.
#[must_use]
pub fn new_registry_arc() -> Arc<SkillRegistry> {
    Arc::new(SkillRegistry::new())
}

// ---------------------------------------------------------------------------
// promote_manifest / demote_manifest removed (D-17): LoadedSkill.manifest and the
// host-side SkillManifest are now the same canonical type from vox-plugin-types.
// Author backfill is done inline in install().

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_bundle::VoxSkillBundle;
    use crate::skill_manifest::{SkillCategory, SkillManifest};

    fn test_bundle(id: &str, category: SkillCategory) -> VoxSkillBundle {
        let m = SkillManifest::new(id, id, "1.0.0", "vox", "desc", category);
        VoxSkillBundle::new(m, "# Skill\nInstructions.")
    }

    #[tokio::test]
    async fn install_bundle_and_list() {
        let reg = SkillRegistry::new();
        let bundle = test_bundle("vox.compiler", SkillCategory::Compiler);
        let res = reg.install_bundle(&bundle).await.expect("install");
        assert_eq!(res.id, "vox.compiler");
        assert!(!res.already_installed);
        let skills = reg.list(None);
        assert_eq!(skills.len(), 1);
    }

    #[tokio::test]
    async fn double_install_same_version() {
        let reg = SkillRegistry::new();
        let bundle = test_bundle("vox.testing", SkillCategory::Testing);
        reg.install_bundle(&bundle).await.expect("first install");
        let res = reg.install_bundle(&bundle).await.expect("second install");
        assert!(res.already_installed);
    }

    #[tokio::test]
    async fn uninstall_removes_skill() {
        let reg = SkillRegistry::new();
        let bundle = test_bundle("vox.docs", SkillCategory::Documentation);
        reg.install_bundle(&bundle).await.expect("install");
        let res = reg.uninstall("vox.docs").await.expect("uninstall");
        assert!(res.was_installed);
        assert!(reg.get("vox.docs").is_none());
    }

    #[tokio::test]
    async fn search_finds_by_name() {
        let reg = SkillRegistry::new();
        let bundle = test_bundle("vox.git-helper", SkillCategory::Git);
        reg.install_bundle(&bundle).await.expect("install");
        let hits = reg.search("git");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].id, "vox.git-helper");
    }

    #[test]
    fn install_loaded_skill_and_lookup() {
        let reg = SkillRegistry::new();
        let loaded = LoadedSkill {
            plugin_id: "test.plugin".to_string(),
            format_version: 1,
            manifest: vox_plugin_api::skill::SkillManifest {
                id: "test.plugin".to_string(),
                name: "Test Plugin".to_string(),
                version: "0.1.0".to_string(),
                description: "A test".to_string(),
                tools: vec!["tool_a".to_string()],
                ..Default::default()
            },
            body: "# Test".to_string(),
            exposed_tools: vec![],
        };
        reg.install(loaded);
        let ids = reg.list_ids();
        assert_eq!(ids, vec!["test.plugin"]);
        let found = reg.lookup("test.plugin").expect("lookup");
        assert_eq!(found.plugin_id, "test.plugin");
    }

    #[test]
    fn no_backend_attached() {
        let reg = SkillRegistry::new();
        assert!(reg.get_backend().is_none());
    }
}
