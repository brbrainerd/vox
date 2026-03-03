//! Plugin system — Plugin trait, PluginManager, and plugin kinds.
//!
//! A Plugin is a runtime-activatable unit: it can be a skill wrapper,
//! an external tool adapter, or a built-in Vox capability. The PluginManager
//! owns loading, unloading, and dispatching.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::SkillError;

/// Plugin kind discriminant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// A skill-backed plugin (loaded from a VoxSkillBundle)
    Skill,
    /// An external MCP server adapter
    McpAdapter,
    /// A native Rust plugin (linked into the binary)
    Native,
    /// A scripted plugin (Lua/Rhai/WASM future)
    Scripted,
}

/// Metadata about a loaded plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub id: String,
    pub kind: PluginKind,
    pub version: String,
    pub enabled: bool,
    pub tool_ids: Vec<String>,
}

/// Plugin lifecycle trait.
pub trait Plugin: Send + Sync {
    fn meta(&self) -> &PluginMeta;
    fn on_load(&self) -> Result<(), SkillError> {
        Ok(())
    }
    fn on_unload(&self) -> Result<(), SkillError> {
        Ok(())
    }
    fn tool_ids(&self) -> Vec<String> {
        self.meta().tool_ids.clone()
    }
}

/// A skill-backed plugin implementation.
pub struct SkillPlugin {
    meta: PluginMeta,
    pub skill_md: String,
}

impl SkillPlugin {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        skill_md: impl Into<String>,
        tool_ids: Vec<String>,
    ) -> Self {
        Self {
            meta: PluginMeta {
                id: id.into(),
                kind: PluginKind::Skill,
                version: version.into(),
                enabled: true,
                tool_ids,
            },
            skill_md: skill_md.into(),
        }
    }
}

impl Plugin for SkillPlugin {
    fn meta(&self) -> &PluginMeta {
        &self.meta
    }
    fn on_load(&self) -> Result<(), SkillError> {
        info!(plugin = %self.meta.id, "Skill plugin loaded");
        Ok(())
    }
    fn on_unload(&self) -> Result<(), SkillError> {
        info!(plugin = %self.meta.id, "Skill plugin unloaded");
        Ok(())
    }
}

/// Manager for all loaded plugins.
pub struct PluginManager {
    plugins: Mutex<HashMap<String, Box<dyn Plugin>>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Mutex::new(HashMap::new()),
        }
    }

    /// Load a plugin into the manager.
    pub fn load(&self, plugin: Box<dyn Plugin>) -> Result<(), SkillError> {
        plugin.on_load()?;
        let id = plugin.meta().id.clone();
        let mut plugins = self.plugins.lock().unwrap_or_else(|e| e.into_inner());
        if plugins.contains_key(&id) {
            warn!(plugin = %id, "Plugin already loaded, replacing");
        }
        plugins.insert(id, plugin);
        Ok(())
    }

    /// Unload a plugin by ID.
    pub fn unload(&self, id: &str) -> Result<(), SkillError> {
        let mut plugins = self.plugins.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(plugin) = plugins.remove(id) {
            plugin.on_unload()?;
            info!(plugin = %id, "Plugin unloaded");
        } else {
            warn!(plugin = %id, "Tried to unload unknown plugin");
        }
        Ok(())
    }

    /// List all loaded plugin metadata.
    pub fn list(&self) -> Vec<PluginMeta> {
        let plugins = self.plugins.lock().unwrap_or_else(|e| e.into_inner());
        plugins.values().map(|p| p.meta().clone()).collect()
    }

    /// Check if a plugin is loaded by ID.
    pub fn is_loaded(&self, id: &str) -> bool {
        let plugins = self.plugins.lock().unwrap_or_else(|e| e.into_inner());
        plugins.contains_key(id)
    }

    /// Get all tool IDs provided by all loaded plugins.
    pub fn all_tool_ids(&self) -> Vec<String> {
        let plugins = self.plugins.lock().unwrap_or_else(|e| e.into_inner());
        plugins.values().flat_map(|p| p.tool_ids()).collect()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plugin(id: &str) -> SkillPlugin {
        SkillPlugin::new(id, "1.0.0", "# Skill\nInstructions.", vec!["tool_a".into()])
    }

    #[test]
    fn load_and_list_plugins() {
        let mgr = PluginManager::new();
        mgr.load(Box::new(make_plugin("vox.compiler")))
            .expect("load");
        let list = mgr.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "vox.compiler");
        assert_eq!(list[0].kind, PluginKind::Skill);
    }

    #[test]
    fn unload_plugin() {
        let mgr = PluginManager::new();
        mgr.load(Box::new(make_plugin("vox.testing")))
            .expect("load");
        assert!(mgr.is_loaded("vox.testing"));
        mgr.unload("vox.testing").expect("unload");
        assert!(!mgr.is_loaded("vox.testing"));
    }

    #[test]
    fn all_tool_ids_aggregated() {
        let mgr = PluginManager::new();
        mgr.load(Box::new(SkillPlugin::new(
            "a",
            "1",
            "# A",
            vec!["tool_1".into(), "tool_2".into()],
        )))
        .expect("load a");
        mgr.load(Box::new(SkillPlugin::new(
            "b",
            "1",
            "# B",
            vec!["tool_3".into()],
        )))
        .expect("load b");
        let ids = mgr.all_tool_ids();
        assert_eq!(ids.len(), 3);
    }
}
