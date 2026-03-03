//! Core trait and types for container runtime abstraction.

use std::path::PathBuf;

/// Options for building an OCI container image.
#[derive(Debug, Clone)]
pub struct BuildOpts {
    /// Directory containing the build context (usually where the Dockerfile lives).
    pub context_dir: PathBuf,
    /// Path to the Dockerfile. If `None`, uses `context_dir/Dockerfile`.
    pub dockerfile: Option<PathBuf>,
    /// Image tag, e.g. `"my-app:latest"`.
    pub tag: String,
    /// `--build-arg` key-value pairs.
    pub build_args: Vec<(String, String)>,
}

/// Options for running an OCI container.
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// Image to run (tag or ID).
    pub image: String,
    /// Port mappings as `(host, container)`.
    pub ports: Vec<(u16, u16)>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Volume mounts as `(host_path, container_path)`.
    pub volumes: Vec<(String, String)>,
    /// Run in detached mode.
    pub detach: bool,
    /// Container name.
    pub name: Option<String>,
    /// Remove container after exit.
    pub rm: bool,
}

impl Default for RunOpts {
    fn default() -> Self {
        Self {
            image: String::new(),
            ports: Vec::new(),
            env: Vec::new(),
            volumes: Vec::new(),
            detach: false,
            name: None,
            rm: true,
        }
    }
}

/// Unified interface for OCI-compatible container runtimes.
///
/// Implementations exist for Docker ([`crate::docker::DockerRuntime`]) and
/// Podman ([`crate::podman::PodmanRuntime`]).
pub trait ContainerRuntime: Send + Sync {
    /// Human-readable runtime name (`"docker"` or `"podman"`).
    fn name(&self) -> &str;

    /// Returns `true` when the runtime CLI is installed and reachable.
    fn available(&self) -> bool;

    /// Returns the CLI version string, or an error if not available.
    fn version(&self) -> anyhow::Result<String>;

    /// Build an OCI image from a Dockerfile. Returns the image ID on success.
    fn build(&self, opts: &BuildOpts) -> anyhow::Result<String>;

    /// Run a container from an image.
    fn run(&self, opts: &RunOpts) -> anyhow::Result<()>;

    /// Push an image to a registry.
    fn push(&self, tag: &str) -> anyhow::Result<()>;

    /// Tag an image with a new name.
    fn tag(&self, source: &str, target: &str) -> anyhow::Result<()>;

    /// Log into a container registry.
    fn login(&self, registry: &str, username: &str, token: &str) -> anyhow::Result<()>;
}
