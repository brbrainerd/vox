use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

/// Recursively collect `.vox`, `.toml`, and `.json` files under `dir`,
/// skipping `.vox-cache/`, `target/`, and hidden directories.
fn collect_source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip caches, build output, and hidden dirs
        if name_str.starts_with('.') || name_str == "target" || name_str == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_source_files(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "vox" | "toml" | "json") {
                out.push(path);
            }
        }
    }
}

/// Unified deploy command — Zig-inspired single-command, any-target deployment.
///
/// Target resolution priority (highest wins):
/// 1. `--target` CLI flag
/// 2. `Vox.toml [deploy].target`
/// 3. Auto-detect: fall back to "container"
///
/// # Examples
///
/// ```text
/// vox deploy production                         # auto-detect target
/// vox deploy production --target container      # OCI + push
/// vox deploy production --target bare-metal     # systemd on SSH host
/// vox deploy production --target compose        # docker-compose up
/// vox deploy production --target k8s            # kubectl apply
/// vox deploy production --dry-run               # show plan, don't execute
/// ```
pub async fn run(
    env: &str,
    config_path: Option<&Path>,
    target_override: Option<&str>,
    dry_run: bool,
    hermetic: bool,
) -> Result<()> {
    let project_root = config_path
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // ── Load manifest ────────────────────────────────────────────────────────
    let manifest =
        vox_pm::VoxManifest::load(&project_root.join("Vox.toml")).unwrap_or_else(|_| {
            vox_pm::VoxManifest::scaffold("vox-app", "application")
        });
    let project_name = &manifest.package.name;
    let deploy_cfg = manifest.deploy.as_ref();

    // ── Artifact cache check ─────────────────────────────────────────────────
    let cache = vox_pm::ArtifactCache::default_for(&project_root)
        .context("Failed to open artifact cache")?;

    // Collect build inputs for cache keying via a lightweight recursive walk
    let mut input_files: Vec<std::path::PathBuf> = Vec::new();
    collect_source_files(&project_root, &mut input_files);
    let extra_inputs = vec![env, target_override.unwrap_or("auto")];
    let input_hash =
        vox_pm::ArtifactCache::compute_input_hash(&input_files, &extra_inputs)
            .unwrap_or_else(|_| "unknown".to_string());

    let cache_hit = matches!(
        cache.lookup(&input_hash),
        vox_pm::artifact_cache::CacheLookup::Hit { .. }
    );


    // ── Resolve deployment target ─────────────────────────────────────────────
    let target_str = vox_container::resolve_target_kind(
        target_override,
        deploy_cfg.and_then(|d| d.target.as_deref()),
    );

    println!(
        "{} {} {} {}",
        "🚀".bold(),
        "Deploying".bold().bright_white(),
        project_name.bright_cyan(),
        format!("→ {env} [{target_str}]").dimmed()
    );

    if cache_hit {
        println!("  {} Build artifacts cached — skipping compilation", "⚡".yellow());
    } else {
        println!("  {} Compiling project...", "🔨".blue());
        let main_file = project_root.join("src").join("main.vox");
        let out_dir = project_root.join("dist");

        if hermetic {
            println!("  {} Hermetic mode — executing build inside build container", "🔒".purple());
            if !dry_run {
                let cwd = std::env::current_dir().unwrap_or(project_root.clone());
                let status = std::process::Command::new("docker")
                    .args([
                        "run",
                        "--rm",
                        "-v",
                        &format!("{}:/app", cwd.display()),
                        "-w",
                        "/app",
                        "rust:1.80-slim", // Common base for Vox
                        "bash",
                        "-c",
                        "cargo run -p vox-cli -- build src/main.vox",
                    ])
                    .status()
                    .context("Failed to start hermetic docker container. Is Docker running?")?;

                if !status.success() {
                    anyhow::bail!("Hermetic build failed with exit code: {:?}", status.code());
                }
            }
        } else {
            if !dry_run {
                crate::commands::build::run_once(&main_file, &out_dir, false).await?;
            }
        }

        if !dry_run {
            let _ = cache.record_build(&input_hash, &format!("Deploy to {}", env), &[]);
        }
    }

    if dry_run {
        println!("  {} Dry-run mode — no changes will be made", "🔍".blue());
    }

    // ── Construct DeployTarget ────────────────────────────────────────────────
    match target_str {
        "container" => deploy_container(env, project_name, deploy_cfg, &project_root, dry_run).await,
        "bare-metal" => deploy_bare_metal(env, project_name, deploy_cfg, &project_root, dry_run).await,
        "compose" => deploy_compose(project_name, deploy_cfg, &project_root, dry_run).await,
        "kubernetes" => deploy_kubernetes(project_name, deploy_cfg, &project_root, dry_run).await,
        other => anyhow::bail!(
            "Unknown deployment target: {other:?}. \
             Use: container, bare-metal, compose, k8s"
        ),
    }
}

// ─── Container ───────────────────────────────────────────────────────────────

async fn deploy_container(
    env: &str,
    project_name: &str,
    deploy_cfg: Option<&vox_pm::DeploySection>,
    project_root: &Path,
    dry_run: bool,
) -> Result<()> {
    // Resolve runtime preference
    let rt_pref = deploy_cfg
        .and_then(|d| d.runtime.as_deref())
        .unwrap_or("auto")
        .parse::<vox_container::detect::RuntimePreference>()
        .unwrap_or_default();

    let runtime = vox_container::detect_runtime(rt_pref)
        .context("No container runtime found. Install Docker or Podman.")?;

    println!(
        "  {} {} ({})",
        "Runtime:".dimmed(),
        runtime.name().bright_white(),
        runtime.version().unwrap_or_else(|_| "unknown".into()).dimmed()
    );

    // Resolve image name / registry from manifest
    let image_name = deploy_cfg
        .and_then(|d| d.effective_image_name())
        .unwrap_or(project_name);
    let registry = deploy_cfg.and_then(|d| d.effective_registry());
    let dockerfile = deploy_cfg
        .and_then(|d| d.container.as_ref())
        .and_then(|c| c.dockerfile.as_deref());
    let build_args = deploy_cfg
        .and_then(|d| d.container.as_ref())
        .map(|c| c.build_args.clone())
        .unwrap_or_default();

    let target = vox_container::build_container_target(
        project_name,
        env,
        Some(image_name),
        registry,
        dockerfile,
        &build_args,
        project_root,
    );

    // Attempt registry login if credentials are cached
    if let Some(ref host) = target.registry_host {
        if let Some(auth) = crate::commands::login::get_auth(host) {
            println!("  {} {}", "Authenticating:".dimmed(), host.bright_white());
            let _ = runtime.login(
                host,
                auth.username.as_deref().unwrap_or("token"),
                &auth.token,
            );
        }
    }

    let deploy_target = vox_container::DeployTarget::Container(target);
    deploy_target.execute(Some(runtime.as_ref()), dry_run)?;

    println!("  {} Deployed to '{env}'", "✓".green().bold());
    Ok(())
}

// ─── Bare Metal ──────────────────────────────────────────────────────────────

async fn deploy_bare_metal(
    _env: &str,
    project_name: &str,
    deploy_cfg: Option<&vox_pm::DeploySection>,
    _project_root: &Path,
    dry_run: bool,
) -> Result<()> {
    let bm_cfg = deploy_cfg
        .and_then(|d| d.bare_metal.as_ref());

    let host = bm_cfg
        .and_then(|b| b.host.as_deref())
        .context(
            "Bare-metal deployment requires [deploy.bare-metal].host in Vox.toml\n\
             Example:\n\n  [deploy.bare-metal]\n  host = \"prod.example.com\"\n  user = \"deploy\""
        )?;

    let user = bm_cfg
        .and_then(|b| b.user.as_deref())
        .unwrap_or("deploy");
    let port = bm_cfg.and_then(|b| b.port).unwrap_or(22);
    let service_name = bm_cfg
        .and_then(|b| b.service_name.as_deref())
        .unwrap_or(project_name);
    let deploy_dir = bm_cfg
        .and_then(|b| b.deploy_dir.as_deref())
        .unwrap_or_else(|| "/opt/vox-app");

    // Load the compiled AST to dynamically fetch `@environment` details
    let main_file = _project_root.join("src").join("main.vox");
    let mut parsed_env_vars = Vec::new();
    let mut parsed_workdir = None;
    let mut parsed_cmd = Vec::new();

    if let Ok(result) = crate::pipeline::run_frontend(&main_file, false).await {
        if let Some(target_env) = result
            .hir
            .environments
            .iter()
            .find(|e| e.name == _env)
            .or_else(|| result.hir.environments.iter().find(|e| e.name == "default"))
        {
            parsed_env_vars = target_env.env_vars.clone();
            parsed_workdir = target_env.workdir.clone();
            parsed_cmd = target_env.cmd.clone();
            if !parsed_env_vars.is_empty() {
                println!("  {} Found {} environment variable(s) in AST", "ℹ".blue(), parsed_env_vars.len());
            }
        }
    }

    let spec = vox_container::generate::EnvironmentSpec {
        base_image: "bare-metal".to_string(),
        workdir: Some(parsed_workdir.unwrap_or_else(|| deploy_dir.to_string())),
        env_vars: parsed_env_vars,
        cmd: if parsed_cmd.is_empty() {
            vec![format!("{}/{}", deploy_dir, project_name)]
        } else {
            parsed_cmd
        },
        ..Default::default()
    };

    let service_content = vox_container::bare_metal::generate_systemd_unit(&spec, service_name);

    println!("  {} {}@{}:{}", "SSH host:".dimmed(), user.bright_white(), host.bright_white(), port);
    println!("  {} {}", "Service:".dimmed(), service_name.bright_white());
    println!("  {} {}", "Deploy dir:".dimmed(), deploy_dir.bright_white());

    let target = vox_container::BareMetalTarget {
        host: host.to_string(),
        user: user.to_string(),
        port,
        deploy_dir: deploy_dir.to_string(),
        service_name: service_name.to_string(),
        service_file_content: service_content,
    };

    let deploy_target = vox_container::DeployTarget::BareMetal(target);
    deploy_target.execute(None, dry_run)?;

    if !dry_run {
        println!("  {} Deployed bare-metal to '{}@{}'", "✓".green().bold(), user, host);
    }
    Ok(())
}

// ─── Compose ─────────────────────────────────────────────────────────────────

async fn deploy_compose(
    project_name: &str,
    deploy_cfg: Option<&vox_pm::DeploySection>,
    project_root: &Path,
    dry_run: bool,
) -> Result<()> {
    let compose_cfg = deploy_cfg.and_then(|d| d.compose.as_ref());

    let compose_file = compose_cfg
        .and_then(|c| c.file.as_deref())
        .map(|f| project_root.join(f))
        .unwrap_or_else(|| {
            // Auto-detect: prefer docker-compose.yml, fall back to compose.yml
            let candidate = project_root.join("docker-compose.yml");
            if candidate.exists() { candidate } else { project_root.join("compose.yml") }
        });

    if !compose_file.exists() {
        // Generate a default compose file
        let content = vox_container::generate::generate_compose_file();
        std::fs::write(&compose_file, content)
            .context("Failed to write docker-compose.yml")?;
        println!("  {} Generated docker-compose.yml", "✓".green());
    }

    let pname = compose_cfg
        .and_then(|c| c.project_name.as_deref())
        .unwrap_or(project_name);
    let services = compose_cfg
        .map(|c| c.services.clone())
        .unwrap_or_default();

    println!("  {} {}", "Compose file:".dimmed(), compose_file.display().to_string().bright_white());
    println!("  {} {}", "Project:".dimmed(), pname.bright_white());

    let target = vox_container::ComposeTarget {
        compose_file,
        project_name: pname.to_string(),
        services,
        detach: true,
    };

    let deploy_target = vox_container::DeployTarget::Compose(target);
    deploy_target.execute(None, dry_run)?;

    if !dry_run {
        println!("  {} Compose project '{pname}' deployed", "✓".green().bold());
    }
    Ok(())
}

// ─── Kubernetes ──────────────────────────────────────────────────────────────

async fn deploy_kubernetes(
    project_name: &str,
    deploy_cfg: Option<&vox_pm::DeploySection>,
    project_root: &Path,
    dry_run: bool,
) -> Result<()> {
    let k8s_cfg = deploy_cfg.and_then(|d| d.kubernetes.as_ref());

    let manifests_dir = k8s_cfg
        .and_then(|k| k.manifests_dir.as_deref())
        .map(|d| project_root.join(d))
        .unwrap_or_else(|| project_root.join("k8s"));

    let namespace = k8s_cfg
        .and_then(|k| k.namespace.as_deref())
        .unwrap_or("default");

    if !manifests_dir.exists() {
        println!("  {} Generating default Kubernetes manifests in '{}'...", "ℹ".blue(), manifests_dir.display());
        std::fs::create_dir_all(&manifests_dir).context("Failed to create k8s/ directory")?;
        let image_tag = deploy_cfg
            .and_then(|d| d.effective_image_name())
            .unwrap_or(project_name);

        // Load AST to get environment-specific ports/env-vars
        let main_file = project_root.join("src").join("main.vox");
        let mut spec = vox_container::generate::EnvironmentSpec::default();
        if let Ok(result) = crate::pipeline::run_frontend(&main_file, false).await {
            if let Some(env_node) = result
                .hir
                .environments
                .iter()
                .find(|e| e.name == "production" || e.name == "default")
            {
                spec.env_vars = env_node.env_vars.clone();
                spec.exposed_ports = env_node.exposed_ports.clone();
            }
        }

        let manifests_content = vox_container::generate::generate_kubernetes_manifests(
            project_name,
            image_tag,
            namespace,
            &spec
        );
        std::fs::write(manifests_dir.join("deployment.yaml"), manifests_content)
            .context("Failed to write deployment.yaml")?;
    }

    let cluster = k8s_cfg.and_then(|k| k.cluster.as_deref());
    let replicas = k8s_cfg.and_then(|k| k.replicas);

    println!("  {} {}", "Namespace:".dimmed(), namespace.bright_white());
    println!("  {} {}", "Manifests:".dimmed(), manifests_dir.display().to_string().bright_white());
    if let Some(c) = cluster {
        println!("  {} {}", "Cluster:".dimmed(), c.bright_white());
    }

    let target = vox_container::KubernetesTarget {
        cluster: cluster.map(str::to_string),
        namespace: namespace.to_string(),
        manifests_dir,
        replicas,
    };

    let deploy_target = vox_container::DeployTarget::Kubernetes(target);
    deploy_target.execute(None, dry_run)?;

    if !dry_run {
        println!(
            "  {} Project '{}' applied to namespace '{namespace}'",
            "✓".green().bold(),
            project_name
        );
    }
    Ok(())
}
