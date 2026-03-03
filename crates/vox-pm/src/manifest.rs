#![allow(clippy::new_without_default)]
#![allow(clippy::should_implement_trait)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Represents the full `Vox.toml` manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxManifest {
    pub package: PackageSection,
    #[serde(default)]
    pub dependencies: BTreeMap<String, DependencySpec>,
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: BTreeMap<String, DependencySpec>,
    #[serde(default)]
    pub features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub bin: Vec<BinSpec>,
    #[serde(default)]
    pub workspace: Option<WorkspaceSection>,
    #[serde(default)]
    pub skills: BTreeMap<String, DependencySpec>,
    /// Orchestrator configuration overrides
    #[serde(default)]
    pub orchestrator: Option<toml::Table>,
    /// Deployment configuration
    #[serde(default)]
    pub deploy: Option<DeploySection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSection {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub targets: Vec<String>, // e.g. ["x86_64-windows-msvc", "aarch64-apple-darwin"]
}

fn default_version() -> String {
    "0.1.0".to_string()
}
fn default_kind() -> String {
    "library".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinSpec {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub doc: bool,
}

/// A dependency specification — either a simple version string or a detailed table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencySpec {
    /// Simple version string, e.g. `"^1.0"`
    Simple(String),
    /// Detailed specification with optional path, features, etc.
    Detailed(DetailedDependency),
}

impl DependencySpec {
    /// Get the version requirement string, if any.
    pub fn version_req(&self) -> Option<&str> {
        match self {
            DependencySpec::Simple(v) => Some(v.as_str()),
            DependencySpec::Detailed(d) => d.version.as_deref(),
        }
    }

    /// Check if this is a path dependency.
    pub fn is_path(&self) -> bool {
        matches!(self, DependencySpec::Detailed(d) if d.path.is_some())
    }

    /// Get the local path if this is a path dependency.
    pub fn path(&self) -> Option<&str> {
        match self {
            DependencySpec::Simple(_) => None,
            DependencySpec::Detailed(d) => d.path.as_deref(),
        }
    }

    /// Get requested features.
    pub fn features(&self) -> &[String] {
        match self {
            DependencySpec::Simple(_) => &[],
            DependencySpec::Detailed(d) => &d.features,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedDependency {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSection {
    #[serde(default)]
    pub members: Vec<String>,
}

/// Deployment configuration from `[deploy]` section in Vox.toml.
///
/// Supports multiple deployment targets with per-target sub-sections:
///
/// ```toml
/// [deploy]
/// target  = "auto"   # "container" | "bare-metal" | "compose" | "k8s" | "auto"
/// runtime = "auto"   # "docker" | "podman" | "auto"
///
/// [deploy.container]
/// image_name = "my-app"
/// registry   = "ghcr.io/user"
///
/// [deploy.bare-metal]
/// host         = "prod.example.com"
/// user         = "deploy"
/// service_name = "my-app"
/// deploy_dir   = "/opt/my-app"
///
/// [deploy.kubernetes]
/// cluster   = "prod"
/// namespace = "default"
/// replicas  = 3
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeploySection {
    /// Deployment target type: "container", "bare-metal", "compose", "k8s", or "auto".
    #[serde(default)]
    pub target: Option<String>,
    /// Container runtime preference: "auto", "docker", or "podman".
    #[serde(default)]
    pub runtime: Option<String>,
    /// OCI image name for builds (legacy flat field, merged into `container`).
    #[serde(default)]
    pub image_name: Option<String>,
    /// Registry URL for pushing images (legacy flat field, merged into `container`).
    #[serde(default)]
    pub registry: Option<String>,
    /// OCI container deployment configuration.
    #[serde(default)]
    pub container: Option<ContainerDeployConfig>,
    /// Bare-metal / systemd deployment configuration.
    #[serde(default, rename = "bare-metal")]
    pub bare_metal: Option<BareMetalDeployConfig>,
    /// Docker Compose / Podman Compose deployment configuration.
    #[serde(default)]
    pub compose: Option<ComposeDeployConfig>,
    /// Kubernetes deployment configuration.
    #[serde(default)]
    pub kubernetes: Option<KubernetesDeployConfig>,
}

impl DeploySection {
    /// Resolve the effective image name: prefers `[deploy.container].image_name`,
    /// falls back to the legacy flat `image_name` field.
    pub fn effective_image_name(&self) -> Option<&str> {
        self.container
            .as_ref()
            .and_then(|c| c.image_name.as_deref())
            .or(self.image_name.as_deref())
    }

    /// Resolve the effective registry: prefers `[deploy.container].registry`,
    /// falls back to the legacy flat `registry` field.
    pub fn effective_registry(&self) -> Option<&str> {
        self.container
            .as_ref()
            .and_then(|c| c.registry.as_deref())
            .or(self.registry.as_deref())
    }
}

/// OCI container-specific deploy configuration (`[deploy.container]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContainerDeployConfig {
    /// OCI image name, e.g. `"my-app"`.
    #[serde(default)]
    pub image_name: Option<String>,
    /// Registry URL for pushing images, e.g. `"ghcr.io/user"`.
    #[serde(default)]
    pub registry: Option<String>,
    /// Additional Docker/Podman `--build-arg` key-value pairs.
    #[serde(default)]
    pub build_args: Vec<(String, String)>,
    /// Path to the Dockerfile, relative to project root. Defaults to `Dockerfile`.
    #[serde(default)]
    pub dockerfile: Option<String>,
}

/// Bare-metal (systemd) deploy configuration (`[deploy.bare-metal]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BareMetalDeployConfig {
    /// SSH hostname or IP of the target server.
    pub host: Option<String>,
    /// SSH username. Defaults to the current OS user.
    #[serde(default)]
    pub user: Option<String>,
    /// SSH port. Defaults to `22`.
    #[serde(default)]
    pub port: Option<u16>,
    /// Systemd service name. Defaults to the project name.
    #[serde(default)]
    pub service_name: Option<String>,
    /// Remote directory to deploy into. Defaults to `/opt/<project-name>`.
    #[serde(default)]
    pub deploy_dir: Option<String>,
}

/// Docker / Podman Compose deploy configuration (`[deploy.compose]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComposeDeployConfig {
    /// Compose project name. Defaults to the project name from `[package]`.
    #[serde(default)]
    pub project_name: Option<String>,
    /// Path to the compose file. Defaults to `docker-compose.yml`.
    #[serde(default)]
    pub file: Option<String>,
    /// Compose service names to deploy. If empty, all services are deployed.
    #[serde(default)]
    pub services: Vec<String>,
}

/// Kubernetes deploy configuration (`[deploy.kubernetes]`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KubernetesDeployConfig {
    /// Kubernetes cluster name (as in `~/.kube/config`).
    #[serde(default)]
    pub cluster: Option<String>,
    /// Kubernetes namespace. Defaults to `"default"`.
    #[serde(default)]
    pub namespace: Option<String>,
    /// Number of pod replicas. Defaults to `1`.
    #[serde(default)]
    pub replicas: Option<u32>,
    /// Path to a Kustomization directory or manifest directory.
    #[serde(default)]
    pub manifests_dir: Option<String>,
}

impl VoxManifest {
    /// Parse a `Vox.toml` from a string.
    pub fn from_str(content: &str) -> Result<Self, ManifestError> {
        toml::from_str(content).map_err(ManifestError::Parse)
    }

    /// Load a `Vox.toml` from a file path.
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(path).map_err(ManifestError::Io)?;
        Self::from_str(&content)
    }

    /// Find and load `Vox.toml` by searching from the given directory upwards.
    pub fn discover(start_dir: &Path) -> Result<(Self, std::path::PathBuf), ManifestError> {
        let mut dir = start_dir.to_path_buf();
        loop {
            let candidate = dir.join("Vox.toml");
            if candidate.exists() {
                let manifest = Self::load(&candidate)?;
                return Ok((manifest, candidate));
            }
            if !dir.pop() {
                return Err(ManifestError::NotFound);
            }
        }
    }

    /// Serialize back to TOML string.
    pub fn to_toml_string(&self) -> Result<String, ManifestError> {
        toml::to_string_pretty(self).map_err(ManifestError::Serialize)
    }

    /// Generate a scaffold manifest for `vox init`.
    pub fn scaffold(name: &str, kind: &str) -> Self {
        Self {
            package: PackageSection {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                kind: kind.to_string(),
                description: Some(format!("A Vox {kind}")),
                license: Some("MIT".to_string()),
                authors: Vec::new(),
                repository: None,
                homepage: None,
                keywords: Vec::new(),
                targets: Vec::new(),
            },
            dependencies: BTreeMap::new(),
            dev_dependencies: BTreeMap::new(),
            features: BTreeMap::new(),
            bin: Vec::new(),
            workspace: None,
            skills: BTreeMap::new(),
            orchestrator: None,
            deploy: None,
        }
    }

    /// Add a dependency (mutating).
    pub fn add_dependency(&mut self, name: &str, spec: DependencySpec) {
        self.dependencies.insert(name.to_string(), spec);
    }

    /// Remove a dependency (mutating). Returns true if it existed.
    pub fn remove_dependency(&mut self, name: &str) -> bool {
        self.dependencies.remove(name).is_some()
    }
}

#[derive(Debug)]
pub enum ManifestError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
    NotFound,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Io(e) => write!(f, "I/O error: {e}"),
            ManifestError::Parse(e) => write!(f, "Parse error: {e}"),
            ManifestError::Serialize(e) => write!(f, "Serialize error: {e}"),
            ManifestError::NotFound => write!(f, "Vox.toml not found"),
        }
    }
}

impl std::error::Error for ManifestError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
[package]
name = "my-app"
"#;
        let manifest = VoxManifest::from_str(toml).unwrap();
        assert_eq!(manifest.package.name, "my-app");
        assert_eq!(manifest.package.version, "0.1.0");
        assert_eq!(manifest.package.kind, "library");
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn test_parse_full_manifest() {
        let toml = r#"
[package]
name = "my-app"
version = "1.2.3"
kind = "application"
description = "My cool app"
license = "MIT"
authors = ["alice", "bob"]
keywords = ["web", "ai"]

[dependencies]
text-summarizer = "^1.0"
auth-utils = { version = "0.2", features = ["oauth"] }
my-local-lib = { path = "../my-lib" }

[dev-dependencies]
test-helpers = "0.1"

[features]
default = ["auth"]
auth = ["auth-utils"]
"#;
        let manifest = VoxManifest::from_str(toml).unwrap();
        assert_eq!(manifest.package.name, "my-app");
        assert_eq!(manifest.package.version, "1.2.3");
        assert_eq!(manifest.package.kind, "application");
        assert_eq!(manifest.dependencies.len(), 3);

        // Simple version dep
        assert!(matches!(
            manifest.dependencies.get("text-summarizer"),
            Some(DependencySpec::Simple(v)) if v == "^1.0"
        ));

        // Detailed dep with features
        match manifest.dependencies.get("auth-utils") {
            Some(DependencySpec::Detailed(d)) => {
                assert_eq!(d.version.as_deref(), Some("0.2"));
                assert_eq!(d.features, vec!["oauth"]);
            }
            _ => panic!("expected detailed dep"),
        }

        // Path dep
        let local = manifest.dependencies.get("my-local-lib").unwrap();
        assert!(local.is_path());
        assert_eq!(local.path(), Some("../my-lib"));

        // Features
        assert_eq!(manifest.features.len(), 2);
        assert_eq!(manifest.features["default"], vec!["auth"]);
    }

    #[test]
    fn test_scaffold() {
        let manifest = VoxManifest::scaffold("hello-world", "application");
        assert_eq!(manifest.package.name, "hello-world");
        assert_eq!(manifest.package.kind, "application");
        let toml_str = manifest.to_toml_string().unwrap();
        assert!(toml_str.contains("hello-world"));
    }

    #[test]
    fn test_add_remove_dependency() {
        let mut manifest = VoxManifest::scaffold("test", "library");
        manifest.add_dependency("foo", DependencySpec::Simple("^1.0".into()));
        assert_eq!(manifest.dependencies.len(), 1);
        assert!(manifest.remove_dependency("foo"));
        assert!(!manifest.remove_dependency("foo"));
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn test_roundtrip() {
        let manifest = VoxManifest::scaffold("roundtrip-test", "skill");
        let toml_str = manifest.to_toml_string().unwrap();
        let parsed = VoxManifest::from_str(&toml_str).unwrap();
        assert_eq!(parsed.package.name, "roundtrip-test");
        assert_eq!(parsed.package.kind, "skill");
    }
}
