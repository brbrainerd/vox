//! Workspace `Vox.toml` fragments used by `vox compile` (`[workspace]`, `[bundle]`, asset hints).
//!
//! Distinct from [`super::VoxConfig`] (toolchain / inference prefs): this parses **project**
//! manifest tables while ignoring unknown top-level keys (`[package]`, `[deploy]`, …).

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Cargo-style workspace member paths relative to the manifest directory.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct WorkspaceTomlFragment {
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct BundleTomlFragment {
    pub identifier: Option<String>,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
    pub desktop: Option<toml::Value>,
    pub mobile: Option<toml::Value>,
    pub assets: Option<BundleAssetsToml>,
    pub plugins: Option<toml::Value>,
    pub signing: Option<toml::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct BundleAssetsToml {
    pub icons: Option<String>,
    pub splash: Option<String>,
    pub ml_models: Option<Vec<String>>,
    pub fonts: Option<Vec<String>>,
    pub lazy: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ProjectVoxTomlDehydrated {
    workspace: Option<WorkspaceTomlFragment>,
    bundle: Option<BundleTomlFragment>,
}

/// Parsed `[workspace]` + `[bundle]` from a single `Vox.toml`.
#[derive(Debug, Clone)]
pub struct ProjectManifest {
    pub workspace: Option<WorkspaceTomlFragment>,
    pub bundle: Option<BundleTomlFragment>,
    pub manifest_path: PathBuf,
}

impl ProjectManifest {
    /// Load `path` (typically `Vox.toml`). Missing file returns empty manifest.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(Self {
                workspace: None,
                bundle: None,
                manifest_path: path.to_path_buf(),
            });
        };
        let parsed: ProjectVoxTomlDehydrated = toml::from_str(&text).unwrap_or_default();
        Ok(Self {
            workspace: parsed.workspace,
            bundle: parsed.bundle,
            manifest_path: path.to_path_buf(),
        })
    }

    /// Resolve `[workspace.members]` to absolute paths (skipped if missing).
    pub fn member_manifest_paths(&self) -> Vec<PathBuf> {
        let Some(ws) = &self.workspace else {
            return Vec::new();
        };
        let base = self
            .manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        ws.members
            .iter()
            .map(|m| base.join(m).join("Vox.toml"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_workspace_and_bundle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("Vox.toml");
        let mut f = std::fs::File::create(&p).expect("create");
        writeln!(
            f,
            r#"
[package]
name = "demo"
version = "0.1.0"

[workspace]
members = ["pkgs/a"]

[bundle]
identifier = "com.example.x"
display_name = "Demo"
"#
        )
        .expect("write");

        let m = ProjectManifest::load(&p).expect("load");
        assert_eq!(
            m.workspace.as_ref().unwrap().members,
            vec!["pkgs/a".to_string()]
        );
        assert_eq!(
            m.bundle.as_ref().unwrap().identifier.as_deref(),
            Some("com.example.x")
        );
        let members = m.member_manifest_paths();
        assert_eq!(members.len(), 1);
        assert!(members[0].ends_with(Path::new("pkgs/a/Vox.toml")));
    }
}
