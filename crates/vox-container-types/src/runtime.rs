//! Core trait and types for OCI container runtime abstraction.
//!
//! This is the deploy-facing container runtime trait, distinct from
//! `vox_skill_runtime::SkillRuntime` (which abstracts over WASM + container
//! runtimes for skill sandboxing).

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
    /// `--cpus` quota (e.g. `"1.5"`). `None` = unlimited.
    pub cpus: Option<String>,
    /// `--memory` cap (e.g. `"512m"`). `None` = unlimited.
    pub memory: Option<String>,
    /// `--pids-limit` cap. `None` = unlimited.
    pub pids_limit: Option<u32>,
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
            cpus: None,
            memory: None,
            pids_limit: None,
        }
    }
}

impl RunOpts {
    /// CLI resource-limit flags for `docker run` / `podman run`, in a stable order.
    /// Empty when no limits are set (behavior-preserving).
    pub fn resource_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(c) = &self.cpus {
            args.push("--cpus".to_string());
            args.push(c.clone());
        }
        if let Some(m) = &self.memory {
            args.push("--memory".to_string());
            args.push(m.clone());
        }
        if let Some(p) = self.pids_limit {
            args.push("--pids-limit".to_string());
            args.push(p.to_string());
        }
        args
    }
}

#[cfg(test)]
mod resource_args_tests {
    use super::*;

    #[test]
    fn no_limits_emits_nothing() {
        let opts = RunOpts::default();
        assert!(opts.resource_args().is_empty());
    }

    #[test]
    fn limits_emit_flags_in_order() {
        let opts = RunOpts {
            cpus: Some("1.5".into()),
            memory: Some("512m".into()),
            pids_limit: Some(128),
            ..RunOpts::default()
        };
        assert_eq!(
            opts.resource_args(),
            vec!["--cpus", "1.5", "--memory", "512m", "--pids-limit", "128"]
        );
    }
}

/// Unified interface for OCI-compatible container runtimes.
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
