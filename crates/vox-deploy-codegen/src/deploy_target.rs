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

use crate::BuildOpts;
use crate::ContainerRuntime;

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
    /// Deploy to Fly.io using `flyctl`.
    Fly(FlyTarget),
    /// Deploy to Coolify using its API.
    Coolify(CoolifyTarget),
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

/// Configuration for a Fly.io deployment.
#[derive(Debug, Clone)]
pub struct FlyTarget {
    /// App name.
    pub app_name: String,
    /// Organization to deploy to.
    pub org: Option<String>,
    /// Region to deploy to.
    pub region: Option<String>,
    /// Project root to run `flyctl deploy` in.
    pub project_root: PathBuf,
}

/// Configuration for a Coolify deployment.
#[derive(Debug, Clone)]
pub struct CoolifyTarget {
    pub base_url: String,
    pub token: String,
    pub app_uuid: String,
    pub force_rebuild: bool,
    pub wait_timeout_secs: Option<u64>,
}

impl DeployTarget {
    /// Return a human-readable name for this target type.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Container(_) => "container",
            Self::BareMetal(_) => "bare-metal",
            Self::Compose(_) => "compose",
            Self::Kubernetes(_) => "kubernetes",
            Self::Fly(_) => "fly",
            Self::Coolify(_) => "coolify",
        }
    }

    /// Execute this deployment target.
    ///
    /// For `Container` targets, `runtime` must be provided.
    /// For other targets, `runtime` is unused.
    pub fn execute(&self, runtime: Option<&dyn ContainerRuntime>, dry_run: bool) -> Result<()> {
        match self {
            Self::Container(cfg) => execute_container(cfg, runtime, dry_run),
            Self::BareMetal(cfg) => execute_bare_metal(cfg, dry_run),
            Self::Compose(cfg) => execute_compose(cfg, dry_run),
            Self::Kubernetes(cfg) => execute_kubernetes(cfg, dry_run),
            Self::Fly(cfg) => execute_fly(cfg, dry_run),
            Self::Coolify(cfg) => execute_coolify(cfg, dry_run),
        }
    }
}

// ─── Container ───────────────────────────────────────────────────────────────

fn execute_container(
    cfg: &ContainerTarget,
    runtime: Option<&dyn ContainerRuntime>,
    dry_run: bool,
) -> Result<()> {
    // Dry-run paths must not require an installed container runtime — CI
    // (CR-L7) and air-gapped contributors should be able to validate the
    // deploy plan without docker/podman present.
    if dry_run {
        println!("  [dry-run] would build OCI image: {}", cfg.image_tag);
        if let Some(ref rt) = cfg.registry_tag {
            println!("  [dry-run] would push: {}", rt);
        }
        return Ok(());
    }

    let runtime = runtime.context("Container deployment requires a container runtime")?;

    let opts = BuildOpts {
        context_dir: cfg.context_dir.clone(),
        dockerfile: cfg.dockerfile.clone(),
        tag: cfg.image_tag.clone(),
        build_args: cfg.build_args.clone(),
    };

    println!("  Building OCI image: {}", cfg.image_tag);
    let image_id = runtime.build(&opts).context("OCI image build failed")?;
    println!("  ✓ Built: {}", image_id);

    if let Some(ref remote_tag) = cfg.registry_tag {
        if let Some(ref host) = cfg.registry_host {
            println!("  Pushing to {}…", host);
        }
        runtime
            .tag(&cfg.image_tag, remote_tag)
            .context("Failed to tag image")?;
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
        println!(
            "  [dry-run] would SCP service file → {}:{}/{}",
            ssh_target, cfg.deploy_dir, service_file
        );
        println!(
            "  [dry-run] would run: systemctl enable --now {}",
            cfg.service_name
        );
        return Ok(());
    }

    // Write service file to temp location
    let tmp_path = std::env::temp_dir().join(&service_file);
    std::fs::write(&tmp_path, &cfg.service_file_content)
        .context("Failed to write temporary service file")?;

    // SCP service file to remote
    let scp_status = Command::new("scp")
        .arg("-P")
        .arg(cfg.port.to_string())
        .arg(tmp_path.as_os_str())
        .arg(format!("{ssh_target}:{}/{service_file}", cfg.deploy_dir))
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
        .args(["-p", &cfg.port.to_string(), &ssh_target, &systemd_cmds])
        .status()
        .context("ssh not found; install OpenSSH client")?;

    if !ssh_status.success() {
        anyhow::bail!("ssh failed with exit code: {:?}", ssh_status.code());
    }

    let _ = std::fs::remove_file(&tmp_path);
    println!(
        "  ✓ Service '{}' installed on {}",
        cfg.service_name, cfg.host
    );
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
        anyhow::bail!("No compose runtime found. Install Podman (podman-compose) or Docker.");
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
        anyhow::bail!("kubectl not found. Install from https://kubernetes.io/docs/tasks/tools/");
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

    println!(
        "  ✓ Kubernetes manifests applied to namespace '{}'",
        cfg.namespace
    );
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
        "fly" | "flyio" => "fly",
        "coolify" => "coolify",
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

// ─── Fly.io ──────────────────────────────────────────────────────────────────

fn execute_fly(cfg: &FlyTarget, dry_run: bool) -> Result<()> {
    println!("  Deploying to Fly.io: {}", cfg.app_name);
    if dry_run {
        println!("  [dry-run] would run: flyctl deploy");
        return Ok(());
    }

    let fly_check = Command::new("flyctl").arg("version").output();
    if fly_check.is_err() {
        anyhow::bail!(
            "flyctl is not installed. Please install it from https://fly.io/docs/flyctl/install/"
        );
    }

    let fly_toml_path = cfg.project_root.join("fly.toml");
    if !fly_toml_path.exists() {
        println!("  [setup] No fly.toml found. Running flyctl launch...");
        let mut launch_cmd = Command::new("flyctl");
        launch_cmd
            .arg("launch")
            .arg("--no-deploy")
            .arg("--name")
            .arg(&cfg.app_name);

        if let Some(ref org) = cfg.org {
            launch_cmd.arg("--org").arg(org);
        }
        if let Some(ref region) = cfg.region {
            launch_cmd.arg("--region").arg(region);
        }

        launch_cmd.current_dir(&cfg.project_root);

        let launch_status = launch_cmd.status().context("Failed to run flyctl launch")?;
        if !launch_status.success() {
            anyhow::bail!("flyctl launch failed.");
        }
    }

    println!("  [deploy] Running flyctl deploy...");
    let mut deploy_cmd = Command::new("flyctl");
    deploy_cmd.arg("deploy").current_dir(&cfg.project_root);

    let deploy_status = deploy_cmd.status().context("Failed to run flyctl deploy")?;
    if !deploy_status.success() {
        anyhow::bail!("flyctl deploy failed.");
    }

    println!("  ✓ Successfully deployed to Fly.io!");
    Ok(())
}

// ─── Coolify ─────────────────────────────────────────────────────────────────

fn execute_coolify(cfg: &CoolifyTarget, dry_run: bool) -> Result<()> {
    println!("  Deploying to Coolify: {}/{}", cfg.base_url, cfg.app_uuid);
    if dry_run {
        println!("  [dry-run] would trigger Coolify deployment via API");
        return Ok(());
    }

    let url = format!(
        "{}/api/v1/deploy?uuid={}{}",
        cfg.base_url.trim_end_matches('/'),
        cfg.app_uuid,
        if cfg.force_rebuild { "&force=true" } else { "" }
    );

    let output = Command::new("curl")
        .args([
            "-s",
            "-X",
            "GET",
            "-H",
            &format!("Authorization: Bearer {}", cfg.token),
            &url,
        ])
        .output()
        .context("Failed to trigger Coolify deployment")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Coolify API call failed: {}", err);
    }

    let response = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&response).unwrap_or(serde_json::Value::Null);
    let deploy_uuid = json["deploymentUuid"].as_str().map(|s| s.to_string());

    println!("  ✓ Deployment triggered successfully on Coolify.");

    if let Some(timeout_secs) = cfg.wait_timeout_secs {
        let deploy_uuid = match deploy_uuid {
            Some(u) => u,
            None => {
                println!(
                    "  Could not extract deployment UUID to poll. Proceeding without polling."
                );
                return Ok(());
            }
        };

        println!(
            "  Polling deployment {} for completion (timeout {}s)...",
            deploy_uuid, timeout_secs
        );
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed().as_secs() > timeout_secs {
                anyhow::bail!(
                    "Coolify deployment timed out after {} seconds",
                    timeout_secs
                );
            }
            std::thread::sleep(std::time::Duration::from_secs(10));

            let status_url = format!(
                "{}/api/v1/deployments/{}",
                cfg.base_url.trim_end_matches('/'),
                deploy_uuid
            );

            let out = Command::new("curl")
                .args([
                    "-s",
                    "-X",
                    "GET",
                    "-H",
                    &format!("Authorization: Bearer {}", cfg.token),
                    &status_url,
                ])
                .output()?;

            let res = String::from_utf8_lossy(&out.stdout);
            let stat_json: serde_json::Value =
                serde_json::from_str(&res).unwrap_or(serde_json::Value::Null);
            let status = stat_json["status"].as_str().unwrap_or("");

            if status == "finished" || status == "success" {
                println!("  ✓ Deployment finished successfully.");
                break;
            } else if status == "failed" || status == "error" {
                println!("  ❌ Deployment failed!");

                let logs_url = format!(
                    "{}/api/v1/applications/{}/logs",
                    cfg.base_url.trim_end_matches('/'),
                    cfg.app_uuid
                );
                let logs_out = Command::new("curl")
                    .args([
                        "-s",
                        "-H",
                        &format!("Authorization: Bearer {}", cfg.token),
                        &logs_url,
                    ])
                    .output();

                if let Ok(l_out) = logs_out {
                    let log_res = String::from_utf8_lossy(&l_out.stdout);
                    let l_json: serde_json::Value =
                        serde_json::from_str(&log_res).unwrap_or(serde_json::Value::Null);
                    if let Some(logs) = l_json["logs"].as_str() {
                        eprintln!("\n--- COOLIFY LOGS ---\n{}\n--------------------", logs);
                    }
                }
                anyhow::bail!("Coolify deployment reported failure status.");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_target_kind() {
        assert_eq!(resolve_target_kind(None, None), "container");
        assert_eq!(resolve_target_kind(Some("bare-metal"), None), "bare-metal");
        assert_eq!(resolve_target_kind(None, Some("docker-compose")), "compose");
        assert_eq!(
            resolve_target_kind(Some("k8s"), Some("docker")),
            "kubernetes"
        );
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
