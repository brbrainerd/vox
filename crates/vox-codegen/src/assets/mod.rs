//! Declarative asset manifest resolution for `vox compile` (`[bundle.assets]`).

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Resolved asset paths relative to a `Vox.toml` directory, staged under `target/generated/...`.
#[derive(Debug, Clone)]
pub struct AssetManifest {
    manifest_dir: PathBuf,
    /// Relative paths (from manifest dir) to copy into the staging tree.
    rel_paths: Vec<PathBuf>,
}

impl AssetManifest {
    /// Build a manifest from `[bundle.assets]` fragments (paths are relative to `manifest_dir`).
    #[must_use]
    pub fn from_bundle_fragment(
        manifest_dir: &Path,
        icons: Option<&str>,
        splash: Option<&str>,
        ml_models: Option<&Vec<String>>,
        fonts: Option<&Vec<String>>,
        _lazy: Option<bool>,
    ) -> Self {
        let mut rel_paths = Vec::new();
        if let Some(p) = icons {
            rel_paths.push(PathBuf::from(p));
        }
        if let Some(p) = splash {
            rel_paths.push(PathBuf::from(p));
        }
        if let Some(models) = ml_models {
            for m in models {
                rel_paths.push(PathBuf::from(m));
            }
        }
        if let Some(fs) = fonts {
            for f in fs {
                rel_paths.push(PathBuf::from(f));
            }
        }
        Self {
            manifest_dir: manifest_dir.to_path_buf(),
            rel_paths,
        }
    }

    /// Ensure every referenced path exists under the manifest directory.
    pub fn validate_preflight(&self) -> Result<()> {
        for rel in &self.rel_paths {
            let abs = self.manifest_dir.join(rel);
            if abs.exists() {
                continue;
            }
            anyhow::bail!(
                "bundle.assets: missing path `{}` (resolved from {})",
                rel.display(),
                abs.display()
            );
        }
        Ok(())
    }

    /// Copy declared assets into `dest_root`, preserving relative paths beneath `manifest_dir`.
    pub fn stage_under(&self, dest_root: &Path) -> Result<()> {
        std::fs::create_dir_all(dest_root).with_context(|| {
            format!(
                "create asset stage directory {}",
                dest_root.display()
            )
        })?;
        for rel in &self.rel_paths {
            let src = self.manifest_dir.join(rel);
            let dst = dest_root.join(rel);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("create parent dirs for {}", dst.display())
                })?;
            }
            copy_path_recursive(&src, &dst)
                .with_context(|| format!("stage asset {} -> {}", src.display(), dst.display()))?;
        }
        Ok(())
    }
}

fn copy_path_recursive(src: &Path, dst: &Path) -> Result<()> {
    let meta = src
        .metadata()
        .with_context(|| format!("stat {}", src.display()))?;
    if meta.is_dir() {
        std::fs::create_dir_all(dst).with_context(|| format!("mkdir {}", dst.display()))?;
        for entry in std::fs::read_dir(src).with_context(|| format!("read_dir {}", src.display()))?
        {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let child_src = entry.path();
            let child_dst = dst.join(entry.file_name());
            if file_type.is_dir() {
                copy_path_recursive(&child_src, &child_dst)?;
            } else if file_type.is_file() {
                std::fs::copy(&child_src, &child_dst).with_context(|| {
                    format!(
                        "copy {} -> {}",
                        child_src.display(),
                        child_dst.display()
                    )
                })?;
            }
        }
        Ok(())
    } else if meta.is_file() {
        std::fs::copy(src, dst).with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
        Ok(())
    } else {
        anyhow::bail!("unsupported asset type (not file/dir): {}", src.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_and_stage_icon_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let icon = dir.path().join("app-icon.png");
        std::fs::File::create(&icon).expect("touch");
        let m = AssetManifest::from_bundle_fragment(dir.path(), Some("app-icon.png"), None, None, None, None);
        m.validate_preflight().expect("ok");
        let stage = dir.path().join("stage");
        m.stage_under(&stage).expect("stage");
        assert!(stage.join("app-icon.png").is_file());
    }
}
