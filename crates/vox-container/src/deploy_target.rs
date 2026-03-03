//! # Deploy Target Abstraction
//!
//! Zig-inspired unified deployment target enum for Vox.
//!
//! All deployment paths in `vox deploy` reduce to a `DeployTarget` variant.
//! Each variant carries its own configuration and knows how to execute itself.
//!
//! ```text
//! vox deploy production
//!   └─ resolves Vox.toml [deploy] section
//!        └─ constructs DeployTarget::Container { ... }
//!             └─ build OCI image → push → done
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::runtime::ContainerRuntime;
use crate::BuildOpts;

/// A fully-resolved deployment target for a Vox application.
///
/// Constructed by the CLI from `Vox.toml` `[deploy]` plus any CLI overrides,
/// then executed via [`DeployTarget::execute`].
#[derive(Debug, Clone)]
pub enum DeployTarget {
    /// Build an OCI image and optionally push to a registry.
    Container(ContainerTarget),
    /// Copy the application to a remote host and install a systemd service.
    BareMetal(BareMetalTarget),
    /// Run `docker-compose` or `podman-compose` to bring up services.
    Compose(ComposeTarget),
    /// Apply Kubernetes manifests and roll out the deployment.
    Kubernetes(KubernetesTarget),
}

/// Configuration for an OCI container deployment.
#[derive(Debug, Clone)]
pub struct ContainerTarget {
    /// Container image tag, e.g. `"my-app:production"`.
    pub image_tag: String,
    /// Full registry path, e.g. `"ghcr.io/user/my-app:production"`.
    pub registry_tag: Option<String>,
    /// Registry hostname for `docker login`, e.g. `"ghcr.io"`.
    pub registry_host: Option<String>,
    /// Build context directory (usually project root).
    pub context_dir: PathBuf,
    /// Optional path to a Dockerfile.
    pub dockerfile: Option<PathBuf>,
    /// `--build-arg` key-value pairs.
    pub build_args: Vec<(String, String)>,
}

/// Configuration for a bare-metal (systemd) deployment.
#[derive(Debug, Clone)]
pub struct BareMetalTarget {
    /// SSH host, e.g. `"prod.example.com"`.
    pub host: String,
    /// SSH username.
    pub user: String,
    /// SSH port (default 22).
    pub port: u16,
    /// Remote directory to deploy into.
    pub deploy_dir: String,
    /// Name of the systemd service.
    pub service_name: String,
    /// Contents of the generated `.service` file.
    pub service_file_content: String,
}

/// Configuration for a Docker/Podman Compose deployment.
#[derive(Debug, Clone)]
pub struct ComposeTarget {
    /// Path to the compose file.
    pub compose_file: PathBuf,
    /// Compose project name.
    pub project_name: String,
    /// Subset of services to deploy (empty = all).
    pub services: Vec<String>,
    /// Whether to run in detached mode.
    pub detach: bool,
}

/// Configuration for a Kubernetes deployment.
#[derive(Debug, Clone)]
pub struct KubernetesTarget {
    /// Kubernetes cluster context name.
    pub cluster: Option<String>,
    /// Kubernetes namespace.
    pub namespace: String,
    /// Path to manifests directory or kustomization root.
    pub manifests_dir: PathBuf,
    /// Number of replicas to ensure.
    pub replicas: Option<u32>,
}

impl DeployTarget {
    /// Return a human-readable name for this target type.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Container(_) => "container",
            Self::BareMetal(_) => "bare-metal",
            Self::Compose(_) => "compose",
            Self::Kubernetes(_) => "kubernetes",
        }
    }

    /// Execute this deployment target.
    ///
    /// For `Container` targets, `runtime` must be provided.
    /// For other targets, `runtime` is unused.
    pub fn execute(
        &self,
        runtime: Option<&dyn ContainerRuntime>,
        dry_run: bool,
    ) -> Result<()> {
        match self {
            Self::Container(cfg) => execute_container(cfg, runtime, dry_run),
            Self::BareMetal(cfg) => execute_bare_metal(cfg, dry_run),
            Self::Compose(cfg) => execute_compose(cfg, dry_run),
            Self::Kubernetes(cfg) => execute_kubernetes(cfg, dry_run),
        }
    }
}

// ─── Container ───────────────────────────────────────────────────────────────

fn execute_container(
    cfg: &ContainerTarget,
    runtime: Option<&dyn ContainerRuntime>,
    dry_run: bool,
) -> Result<()> {
    let runtime = runtime.context("Container deployment requires a container runtime")?;

    let opts = BuildOpts {
        context_dir: cfg.context_dir.clone(),
        dockerfile: cfg.dockerfile.clone(),
        tag: cfg.image_tag.clone(),
        build_args: cfg.build_args.clone(),
    };

    if dry_run {
        println!("  [dry-run] would build OCI image: {}", cfg.image_tag);
        if let Some(ref rt) = cfg.registry_tag {
            println!("  [dry-run] would push: {}", rt);
        }
        return Ok(());
    }

    println!("  Building OCI image: {}", cfg.image_tag);
    let image_id = runtime.build(&opts).context("OCI image build failed")?;
    println!("  ✓ Built: {}", image_id);

    if let Some(ref remote_tag) = cfg.registry_tag {
        if let Some(ref host) = cfg.registry_host {
            println!("  Pushing to {}…", host);
        }
        runtime.tag(&cfg.image_tag, remote_tag).context("Failed to tag image")?;
        runtime.push(remote_tag).context("Failed to push image")?;
        println!("  ✓ Pushed: {}", remote_tag);
    }

    Ok(())
}

// ─── Bare Metal ──────────────────────────────────────────────────────────────

fn execute_bare_metal(cfg: &BareMetalTarget, dry_run: bool) -> Result<()> {
    let ssh_target = format!("{}@{}", cfg.user, cfg.host);
    let service_file = format!("{}.service", cfg.service_name);

    if dry_run {
        println!("  [dry-run] would SCP service file → {}:{}/{}", ssh_target, cfg.deploy_dir, service_file);
        println!("  [dry-run] would run: systemctl enable --now {}", cfg.service_name);
        return Ok(());
    }

    // Write service file to temp location
    let tmp_path = std::env::temp_dir().join(&service_file);
    std::fs::write(&tmp_path, &cfg.service_file_content)
        .context("Failed to write temporary service file")?;

    // SCP service file to remote
    let scp_status = Command::new("scp")
        .args([
            "-P",
            &cfg.port.to_string(),
            &tmp_path.to_string_lossy().to_string(),
            &format!("{ssh_target}:{}/{service_file}", cfg.deploy_dir),
        ])
        .status()
        .context("scp not found; install OpenSSH client")?;

    if !scp_status.success() {
        anyhow::bail!("scp failed with exit code: {:?}", scp_status.code());
    }

    // Move service file into systemd and enable it
    let systemd_cmds = format!(
        "sudo mv {}/{service_file} /etc/systemd/system/{service_file} && \
         sudo systemctl daemon-reload && \
         sudo systemctl enable --now {service_name}",
        cfg.deploy_dir,
        service_name = cfg.service_name
    );

    let ssh_status = Command::new("ssh")
        .args([
            "-p",
            &cfg.port.to_string(),
            &ssh_target,
            &systemd_cmds,
        ])
        .status()
        .context("ssh not found; install OpenSSH client")?;

    if !ssh_status.success() {
        anyhow::bail!("ssh failed with exit code: {:?}", ssh_status.code());
    }

    let _ = std::fs::remove_file(&tmp_path);
    println!("  ✓ Service '{}' installed on {}", cfg.service_name, cfg.host);
    Ok(())
}

// ─── Compose ─────────────────────────────────────────────────────────────────

fn execute_compose(cfg: &ComposeTarget, dry_run: bool) -> Result<()> {
    // Try podman-compose first, fall back to docker compose
    let (bin, args_prefix): (&str, Vec<&str>) = if command_exists("podman-compose") {
        ("podman-compose", vec![])
    } else if command_exists("docker") {
        ("docker", vec!["compose"])
    } else {
        anyhow::bail!(
            "No compose runtime found. Install Podman (podman-compose) or Docker."
        );
    };

    let compose_file_str = cfg.compose_file.to_string_lossy();
    let mut args: Vec<String> = args_prefix.iter().map(|s| s.to_string()).collect();
    args.extend([
        "-f".to_string(),
        compose_file_str.to_string(),
        "-p".to_string(),
        cfg.project_name.clone(),
        "up".to_string(),
    ]);
    if cfg.detach {
        args.push("-d".to_string());
    }
    args.extend(cfg.services.clone());

    if dry_run {
        println!("  [dry-run] would run: {} {}", bin, args.join(" "));
        return Ok(());
    }

    let status = Command::new(bin)
        .args(&args)
        .status()
        .with_context(|| format!("Failed to run {bin}"))?;

    if !status.success() {
        anyhow::bail!("{} exited with: {:?}", bin, status.code());
    }

    println!("  ✓ Compose project '{}' is up", cfg.project_name);
    Ok(())
}

// ─── Kubernetes ──────────────────────────────────────────────────────────────

fn execute_kubernetes(cfg: &KubernetesTarget, dry_run: bool) -> Result<()> {
    if !command_exists("kubectl") {
        anyhow::bail!(
            "kubectl not found. Install from https://kubernetes.io/docs/tasks/tools/"
        );
    }

    let manifests = cfg.manifests_dir.to_string_lossy();
    let mut kubectl_args = vec!["apply".to_string(), "-f".to_string(), manifests.to_string()];

    if cfg.namespace != "default" {
        kubectl_args.extend(["-n".to_string(), cfg.namespace.clone()]);
    }

    if let Some(ref cluster) = cfg.cluster {
        kubectl_args.extend(["--context".to_string(), cluster.clone()]);
    }

    if dry_run {
        kubectl_args.push("--dry-run=client".to_string());
        println!("  [dry-run] would run: kubectl {}", kubectl_args.join(" "));
        return Ok(());
    }

    let status = Command::new("kubectl")
        .args(&kubectl_args)
        .status()
        .context("Failed to run kubectl")?;

    if !status.success() {
        anyhow::bail!("kubectl apply failed with: {:?}", status.code());
    }

    // Optionally scale to desired replicas
    if let Some(replicas) = cfg.replicas {
        // Best-effort scale — user's manifests may manage this themselves
        println!("  ℹ️  Desired replicas: {replicas} (ensure your manifests reflect this)");
    }

    println!("  ✓ Kubernetes manifests applied to namespace '{}'", cfg.namespace);
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Resolve which deploy target string to use, given the raw deploy section,
/// the environment name, and any CLI override.
pub fn resolve_target_kind(
    target_override: Option<&str>,
    deploy_target: Option<&str>,
) -> &'static str {
    let raw = target_override.or(deploy_target).unwrap_or("auto");
    match raw.to_lowercase().as_str() {
        "container" | "docker" | "podman" => "container",
        "bare-metal" | "baremetal" | "systemd" => "bare-metal",
        "compose" | "docker-compose" | "podman-compose" => "compose",
        "k8s" | "kubernetes" | "kube" => "kubernetes",
        _ => "container", // auto defaults to container
    }
}

/// Build a [`ContainerTarget`] from manifest values and CLI overrides.
pub fn build_container_target(
    project_name: &str,
    env: &str,
    image_name: Option<&str>,
    registry: Option<&str>,
    dockerfile: Option<&str>,
    extra_build_args: &[(String, String)],
    context_dir: &Path,
) -> ContainerTarget {
    let base_name = image_name.unwrap_or(project_name);
    let image_tag = format!("{base_name}:{env}");

    let (registry_tag, registry_host) = if let Some(reg) = registry {
        let full = format!("{reg}/{base_name}:{env}");
        // Extract hostname (first component before `/`)
        let host = reg.split('/').next().unwrap_or(reg).to_string();
        (Some(full), Some(host))
    } else {
        (None, None)
    };

    ContainerTarget {
        image_tag,
        registry_tag,
        registry_host,
        context_dir: context_dir.to_path_buf(),
        dockerfile: dockerfile.map(PathBuf::from),
        build_args: extra_build_args.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_target_kind() {
        assert_eq!(resolve_target_kind(None, None), "container");
        assert_eq!(resolve_target_kind(Some("bare-metal"), None), "bare-metal");
        assert_eq!(resolve_target_kind(None, Some("docker-compose")), "compose");
        assert_eq!(resolve_target_kind(Some("k8s"), Some("docker")), "kubernetes");
        assert_eq!(resolve_target_kind(Some("auto"), None), "container");
    }

    #[test]
    fn test_build_container_target() {
        let t = build_container_target(
            "myapp",
            "prod",
            None,
            Some("ghcr.io/test"),
            None,
            &[("FOO".into(), "bar".into())],
            Path::new("/tmp"),
        );

        assert_eq!(t.image_tag, "myapp:prod");
        assert_eq!(t.registry_tag.as_deref(), Some("ghcr.io/test/myapp:prod"));
        assert_eq!(t.registry_host.as_deref(), Some("ghcr.io"));
        assert_eq!(t.context_dir, Path::new("/tmp"));
        assert_eq!(t.build_args, vec![("FOO".into(), "bar".into())]);
    }
}
