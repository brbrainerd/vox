use crate::manifest::{ManifestError, VoxManifest};
use std::path::{Path, PathBuf};

/// Represents a Vox workspace containing multiple packages.
#[derive(Debug)]
pub struct VoxWorkspace {
    /// The root directory containing the workspace `Vox.toml`.
    pub root_dir: PathBuf,
    /// The root manifest.
    pub root_manifest: VoxManifest,
    /// All discovered member packages.
    pub members: Vec<WorkspaceMember>,
}

#[derive(Debug)]
pub struct WorkspaceMember {
    /// The package's manifest.
    pub manifest: VoxManifest,
    /// The directory containing the package's `Vox.toml`.
    pub dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("Manifest error: {0}")]
    Manifest(#[from] ManifestError),
    #[error("Not a workspace")]
    NotAWorkspace,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Glob pattern error: {0}")]
    Glob(#[from] glob::PatternError),
    #[error("Glob matching error: {0}")]
    GlobMatch(#[from] glob::GlobError),
}

impl VoxWorkspace {
    /// Load a workspace from a given root path.
    pub fn load(root: &Path) -> Result<Self, WorkspaceError> {
        let root_manifest_path = root.join("Vox.toml");
        let root_manifest = VoxManifest::load(&root_manifest_path)?;

        let workspace_config = root_manifest
            .workspace
            .as_ref()
            .ok_or(WorkspaceError::NotAWorkspace)?;

        let mut members = Vec::new();

        for pattern in &workspace_config.members {
            // Treat the pattern relative to the root directory
            let root_str = root.to_str().unwrap_or(".");
            let glob_pattern = if pattern.contains('*') {
                format!("{}/{}", root_str, pattern)
            } else {
                format!("{}/{}/Vox.toml", root_str, pattern)
            };

            for entry in glob::glob(&glob_pattern)? {
                let path = entry?;
                // If the glob resolved to a directory, append Vox.toml
                let toml_path = if path.is_dir() {
                    path.join("Vox.toml")
                } else {
                    path.clone()
                };

                if toml_path.exists() {
                    if let Ok(manifest) = VoxManifest::load(&toml_path) {
                        members.push(WorkspaceMember {
                            manifest,
                            dir: toml_path.parent().unwrap_or(Path::new("")).to_path_buf(),
                        });
                    }
                }
            }
        }

        Ok(Self {
            root_dir: root.to_path_buf(),
            root_manifest,
            members,
        })
    }
    /// Validate the workspace against the project's architectural schema (vox-schema.json).
    pub fn validate_architecture(&self) -> Result<Vec<String>, WorkspaceError> {
        let schema_path = self.root_dir.join("vox-schema.json");
        if !schema_path.exists() {
            return Ok(Vec::new()); // No schema to validate against
        }

        let schema_content = std::fs::read_to_string(schema_path)?;
        let schema: serde_json::Value = serde_json::from_str(&schema_content)
            .map_err(|e| WorkspaceError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;

        let mut violations = Vec::new();
        let crates_map = schema.get("crates").and_then(|v| v.as_object());

        if let Some(crates) = crates_map {
            for member in &self.members {
                let member_name = member.manifest.package.name.clone();
                if let Some(crate_config) = crates.get(&member_name) {
                    if let Some(pattern) = crate_config.get("path_pattern").and_then(|v| v.as_str()) {
                        // Check if member.dir matches the expected pattern
                        // For simplicity, we check if member.dir is a sub-path of the pattern's base
                        let pattern_base = pattern.split("/**").next().unwrap_or(pattern);
                        let expected_dir = self.root_dir.join(pattern_base);

                        if !member.dir.starts_with(&expected_dir) && !expected_dir.starts_with(&member.dir) {
                            violations.push(format!(
                                "Crate '{}' is in '{}', but vox-schema.json expects it under '{}'",
                                member_name,
                                member.dir.display(),
                                expected_dir.display()
                            ));
                        }
                    }
                } else {
                    violations.push(format!(
                        "Crate '{}' is not registered in vox-schema.json",
                        member_name
                    ));
                }
            }
        }

        Ok(violations)
    }
}
