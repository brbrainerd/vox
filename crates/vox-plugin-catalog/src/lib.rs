//! SSOT catalog of all first-party Vox plugins and distribution bundles.
//!
//! See `docs/src/architecture/plugin-system-redesign-2026.md`.

pub mod docs;
pub mod schema;

use schema::{BundleEntry, Component, PluginCatalogEntry, SkillBundleEntry};
use serde::Deserialize;
use std::sync::OnceLock;

/// Embedded raw catalog source. Validated at build time by `build.rs`.
const CATALOG_SRC: &str = include_str!("../catalog.toml");

#[derive(Deserialize)]
struct CatalogFile {
    #[serde(default, rename = "plugin")]
    plugins: Vec<PluginCatalogEntry>,
    #[serde(default, rename = "bundle")]
    bundles: Vec<BundleEntry>,
    #[serde(default, rename = "component")]
    components: Vec<Component>,
    #[serde(default, rename = "skill-bundle")]
    skill_bundles: Vec<SkillBundleEntry>,
}

fn parsed() -> &'static CatalogFile {
    static CACHED: OnceLock<CatalogFile> = OnceLock::new();
    CACHED.get_or_init(|| {
        toml::from_str::<CatalogFile>(CATALOG_SRC)
            .expect("catalog.toml should parse — build.rs validates this")
    })
}

/// All first-party plugins declared in `catalog.toml`.
pub fn all_plugins() -> &'static [PluginCatalogEntry] {
    &parsed().plugins
}

/// All distribution bundles declared in `catalog.toml`.
pub fn all_bundles() -> &'static [BundleEntry] {
    &parsed().bundles
}

/// All optional first-party components (companion binaries such as the GUI)
/// declared in `catalog.toml`. Components are not cdylib plugins; they are
/// standalone executables installed on demand and never built for CLI-only users.
pub fn all_components() -> &'static [Component] {
    &parsed().components
}

/// All bundled interop skills declared in `catalog.toml` (`[[skill-bundle]]`).
pub fn all_skill_bundles() -> &'static [SkillBundleEntry] {
    &parsed().skill_bundles
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("unknown bundle: {0}")]
    UnknownBundle(String),
    #[error("unknown plugin '{plugin}' referenced by bundle '{bundle}'")]
    UnknownPlugin { bundle: String, plugin: String },
    #[error("bundle '{0}' has a cyclic extends chain")]
    CyclicExtends(String),
}

/// Resolve a bundle id to its full plugin set, walking the `extends` chain
/// and deduplicating by plugin id. Order: parent plugins first, then child
/// additions. First-occurrence wins for duplicates.
pub fn bundle_resolved(id: &str) -> Result<Vec<&'static PluginCatalogEntry>, ResolveError> {
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut chain: Vec<&'static BundleEntry> = Vec::new();
    let mut current = id.to_string();
    loop {
        let bundle = all_bundles()
            .iter()
            .find(|b| b.id == current)
            .ok_or_else(|| ResolveError::UnknownBundle(current.clone()))?;
        if !seen_ids.insert(bundle.id.clone()) {
            return Err(ResolveError::CyclicExtends(id.to_string()));
        }
        chain.push(bundle);
        match &bundle.extends {
            Some(parent) => current = parent.clone(),
            None => break,
        }
    }
    // Walk parents-first.
    let mut out: Vec<&'static PluginCatalogEntry> = Vec::new();
    let mut included: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for bundle in chain.iter().rev() {
        for plugin_id in &bundle.plugins {
            if included.insert(plugin_id.as_str()) {
                let plugin = all_plugins()
                    .iter()
                    .find(|p| &p.id == plugin_id)
                    .ok_or_else(|| ResolveError::UnknownPlugin {
                        bundle: bundle.id.clone(),
                        plugin: plugin_id.clone(),
                    })?;
                out.push(plugin);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod bundle_membership_tests {
    use super::bundle_resolved;

    #[test]
    fn vox_server_bundle_includes_runtime_container() {
        let plugins =
            bundle_resolved("vox-server").expect("vox-server bundle must resolve without error");
        assert!(
            plugins.iter().any(|p| p.id == "runtime-container"),
            "expected runtime-container in vox-server bundle; got: {:?}",
            plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn vox_server_bundle_preserves_existing_members() {
        // Regression guard: adding runtime-container must not evict existing plugins.
        let plugins =
            bundle_resolved("vox-server").expect("vox-server bundle must resolve without error");
        let ids: Vec<&str> = plugins.iter().map(|p| p.id.as_str()).collect();
        for required in [
            "populi-mesh",
            "skill-orchestrator",
            "skill-memory",
            "webhook",
        ] {
            assert!(
                ids.contains(&required),
                "existing member '{required}' was evicted from vox-server bundle; current: {ids:?}"
            );
        }
    }

    #[test]
    fn vox_server_bundle_resolves_without_error() {
        // Sanity: every plugin id listed in vox-server must exist in all_plugins().
        bundle_resolved("vox-server").expect("vox-server bundle should resolve cleanly");
    }
}
