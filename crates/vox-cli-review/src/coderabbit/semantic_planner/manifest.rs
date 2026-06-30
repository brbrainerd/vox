//! Persist semantic plan JSON under `.coderabbit/`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::SemanticManifest;

/// Write `semantic-manifest.json` and return its path.
pub fn write_semantic_manifest(repo: &Path, manifest: &SemanticManifest) -> Result<PathBuf> {
    let cr_dir = repo.join(".coderabbit");
    std::fs::create_dir_all(&cr_dir).ok();
    let manifest_path = cr_dir.join("semantic-manifest.json");
    let json = serde_json::to_string_pretty(manifest).context("serialize manifest")?;
    std::fs::write(&manifest_path, &json).context("write manifest")?;
    Ok(manifest_path)
}
