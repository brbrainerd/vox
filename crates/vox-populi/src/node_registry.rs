//! [`NodeRecord`], [`LocalRegistry`], and re-exports from [`vox_populi_types`].
//!
//! Pure-data types (`NodeRecord`, `PopuliRegistryFile`, `PopuliRegistryError`, helpers)
//! live in [`vox_populi_types`] per ADR-042. This module owns only the file-backed
//! persistence layer ([`LocalRegistry`]) which requires `std::fs` I/O.

use std::path::{Path, PathBuf};

use crate::now_ms;

// ── Re-exports from vox-populi-types (ADR-042) ───────────────────────────────

pub use vox_populi_types::{
    MAX_MAINTENANCE_FOR_MS, NodeRecord, PopuliRegistryError, PopuliRegistryFile,
    filter_registry_by_max_stale_ms, node_maintenance_blocks_new_work,
    sweep_expired_maintenance_on_nodes,
};

// ── LocalRegistry ─────────────────────────────────────────────────────────────

/// Local file-backed registry (single-writer; suitable for shared Docker volume in dev).
#[derive(Debug)]
pub struct LocalRegistry {
    path: PathBuf,
}

impl LocalRegistry {
    /// Default path under the user home: `~/.vox/cache/populi/local-registry.json`.
    #[must_use]
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join(".vox")
            .join("cache")
            .join("populi")
            .join("local-registry.json")
    }

    /// Open registry at `path` (file may not exist yet).
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Prefer `VOX_MESH_REGISTRY_PATH`, else [`LocalRegistry::default_path`].
    #[must_use]
    pub fn resolved_default_path() -> PathBuf {
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshRegistryPath)
            .expose()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(Self::default_path)
    }

    /// Path on disk.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load or return empty registry.
    pub fn load(&self) -> Result<PopuliRegistryFile, PopuliRegistryError> {
        if !self.path.is_file() {
            return Ok(PopuliRegistryFile {
                schema_version: 1,
                nodes: Vec::new(),
                queue_depth: None,
            });
        }
        let raw = std::fs::read_to_string(&self.path).map_err(PopuliRegistryError::Io)?;
        let parsed: PopuliRegistryFile =
            serde_json::from_str(&raw).map_err(|e| PopuliRegistryError::Json(e.to_string()))?;
        Ok(parsed)
    }

    /// Replace registry contents atomically (write temp + rename).
    pub fn save(&self, reg: &PopuliRegistryFile) -> Result<(), PopuliRegistryError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(PopuliRegistryError::Io)?;
        }
        let json = serde_json::to_string_pretty(reg)
            .map_err(|e| PopuliRegistryError::Json(e.to_string()))?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json.as_bytes()).map_err(PopuliRegistryError::Io)?;
        std::fs::rename(&tmp, &self.path).map_err(PopuliRegistryError::Io)?;
        Ok(())
    }

    /// Upsert a node by `id` and persist.
    pub fn upsert_node(&self, mut record: NodeRecord) -> Result<(), PopuliRegistryError> {
        record.last_seen_unix_ms = now_ms();
        let mut reg = self.load()?;
        reg.schema_version = 1;
        if let Some(i) = reg.nodes.iter().position(|n| n.id == record.id) {
            reg.nodes[i] = record;
        } else {
            reg.nodes.push(record);
        }
        self.save(&reg)
    }
}
