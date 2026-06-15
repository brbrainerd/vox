//! `vox deploy` — execute `Vox.toml` `[deploy]` via [`vox_deploy_codegen`].

use crate::cli_args::DeployArgs;
use crate::commands::pm_lifecycle::lockfile_path;
use anyhow::{Context, Result};
use std::path::PathBuf;
use vox_container::{ContainerRuntime, detect_runtime};
use vox_deploy_codegen::generate::EnvironmentSpec;
use vox_deploy_codegen::{
    BareMetalTarget, ComposeTarget, DeployTarget, KubernetesTarget, build_container_target,
    generate_systemd_unit, resolve_target_kind,
};
use vox_package::VoxManifest;

/// `vox deploy` — build/push OCI images, run compose, apply Kubernetes manifests, or bare-metal systemd.
pub async fn run(args: DeployArgs) -> Result<()> {
    let manifest_path = PathBuf::from("Vox.toml");
    let manifest = VoxManifest::load(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "No Vox.toml found. Run `vox init` first.")?;

    let deploy = manifest.deploy.as_ref().context(
        "No [deploy] section in Vox.toml. Add [deploy] with target and target-specific tables.",
    )?;

    if args.locked {
        let p = lockfile_path();
        if !p.exists() {
            anyhow::bail!(
                "missing `{}` — run `vox lock` first (or omit `--locked`)",
                p.display()
            );
        }
    }

    let project_root = std::env::current_dir()?;
    let env_name = args.environment.as_str();
    let target_kind = resolve_target_kind(args.target.as_deref(), deploy.target.as_deref());

    let mut runtime_holder: Option<Box<dyn ContainerRuntime>> = None;

    let target = match target_kind {
        "container" => {
            let pref: vox_container::detect::RuntimePreference = deploy
                .runtime
                .as_deref()
                .or(args.runtime.as_deref())
                .unwrap_or("auto")
                .parse()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            // `--dry-run` describes the planned actions without invoking a
            // container runtime — required for CI (CR-L7) and air-gapped
            // contributors who have no docker/podman installed.
            if !args.dry_run {
                runtime_holder = Some(
                    detect_runtime(pref).context("container runtime detection / availability")?,
                );
            }

            let build_args: Vec<(String, String)> = deploy
                .container
                .as_ref()
                .map(|c| c.build_args.clone())
                .unwrap_or_default();

            let dockerfile = deploy
                .container
                .as_ref()
                .and_then(|c| c.dockerfile.as_deref());

            let ct = build_container_target(
                &manifest.package.name,
                env_name,
                deploy.effective_image_name(),
                deploy.effective_registry(),
                dockerfile,
                &build_args,
                &project_root,
            );
            DeployTarget::Container(ct)
        }
        "compose" => {
            let cfg = deploy
                .compose
                .as_ref()
                .context("deploy target is compose but [deploy.compose] is missing")?;
            let project_name = cfg
                .project_name
                .clone()
                .unwrap_or_else(|| manifest.package.name.clone());
            let file = cfg.file.as_deref().unwrap_or("docker-compose.yml");
            DeployTarget::Compose(ComposeTarget {
                compose_file: project_root.join(file),
                project_name,
                services: cfg.services.clone(),
                detach: args.detach,
            })
        }
        "kubernetes" => {
            let cfg = deploy
                .kubernetes
                .as_ref()
                .context("deploy target is kubernetes but [deploy.kubernetes] is missing")?;
            let manifests_dir = cfg
                .manifests_dir
                .as_deref()
                .context("[deploy.kubernetes].manifests_dir is required")?;
            DeployTarget::Kubernetes(KubernetesTarget {
                cluster: cfg.cluster.clone(),
                namespace: cfg
                    .namespace
                    .clone()
                    .unwrap_or_else(|| "default".to_string()),
                manifests_dir: project_root.join(manifests_dir),
                replicas: cfg.replicas,
            })
        }
        "bare-metal" => {
            let cfg = deploy
                .bare_metal
                .as_ref()
                .context("deploy target is bare-metal but [deploy.bare-metal] is missing")?;
            let host = cfg
                .host
                .as_deref()
                .context("[deploy.bare-metal].host is required")?;
            let service_name = cfg
                .service_name
                .clone()
                .unwrap_or_else(|| manifest.package.name.clone());
            let deploy_dir = cfg
                .deploy_dir
                .clone()
                // vox-arch-check: allow abs-path
                .unwrap_or_else(|| format!("/opt/{}", manifest.package.name));

            let spec = EnvironmentSpec {
                base_image: "bare-metal".to_string(),
                workdir: Some(deploy_dir.clone()),
                ..Default::default()
            };
            let service_file_content = generate_systemd_unit(&spec, &service_name);

            let user = cfg.user.clone().unwrap_or_else(default_ssh_user);

            DeployTarget::BareMetal(BareMetalTarget {
                host: host.to_string(),
                user,
                port: cfg.port.unwrap_or(22),
                deploy_dir,
                service_name,
                service_file_content,
            })
        }
        "fly" => {
            let cfg = deploy.fly.as_ref().cloned().unwrap_or_default();
            let app_name = cfg
                .app_name
                .clone()
                .unwrap_or_else(|| manifest.package.name.clone());

            vox_deploy_codegen::DeployTarget::Fly(vox_deploy_codegen::deploy_target::FlyTarget {
                app_name,
                org: cfg.org.clone(),
                region: cfg.region.clone(),
                project_root: project_root.clone(),
            })
        }
        "coolify" => {
            let cfg = deploy
                .coolify
                .as_ref()
                .context("deploy target is coolify but [deploy.coolify] is missing")?;

            vox_deploy_codegen::DeployTarget::Coolify(
                vox_deploy_codegen::deploy_target::CoolifyTarget {
                    base_url: cfg.base_url.clone().unwrap_or_default(),
                    token: std::env::var(&cfg.token_env).unwrap_or_else(|_| {
                        vox_secrets::resolve_secret(vox_secrets::SecretId::CoolifyToken)
                            .expose()
                            .unwrap_or_default()
                            .to_string()
                    }),
                    app_uuid: cfg.app_uuid.clone().unwrap_or_default(),
                    force_rebuild: cfg.force_rebuild,
                    wait_timeout_secs: Some(900),
                },
            )
        }
        _ => anyhow::bail!("unsupported deploy target kind: {target_kind}"),
    };

    let runtime_ref = runtime_holder
        .as_deref()
        .map(|r| r as &dyn ContainerRuntime);

    enforce_portable_backend_artifact_lane(&project_root, target_kind, args.dry_run)
        .context("portable backend artifact lane guard")?;

    println!(
        "Deploying environment `{}` via {} target",
        env_name,
        target.kind_name()
    );
    target
        .execute(runtime_ref, args.dry_run)
        .context("deploy execution failed")?;
    Ok(())
}

/// OCI-facing deploy targets that participate in the portable backend artifact lane.
fn portable_backend_promotion_target_kind(kind: &str) -> bool {
    matches!(
        kind,
        "container" | "compose" | "kubernetes" | "fly" | "coolify"
    )
}

fn enforce_portable_backend_artifact_lane(
    project_root: &std::path::Path,
    target_kind: &str,
    dry_run: bool,
) -> Result<()> {
    if dry_run || !portable_backend_promotion_target_kind(target_kind) {
        return Ok(());
    }
    let sbom_required =
        vox_config::env_parse::resolve_config_bool("VOX_BACKEND_ARTIFACT_SBOM_REQUIRED", false);
    let signing_required =
        vox_config::env_parse::resolve_config_bool("VOX_BACKEND_ARTIFACT_SIGNING_REQUIRED", false);
    if !sbom_required && !signing_required {
        return Ok(());
    }

    let lane_dir = vox_config::paths::repo_backend_artifact_dir(project_root);
    if sbom_required {
        let candidates = [
            lane_dir.join("sbom.json"),
            lane_dir.join("sbom.spdx.json"),
            lane_dir.join("sbom.cyclonedx.json"),
        ];
        if !candidates.iter().any(|p| p.is_file()) {
            anyhow::bail!(
                "VOX_BACKEND_ARTIFACT_SBOM_REQUIRED is enabled but no SBOM file found under {} \
                 (expected sbom.json, sbom.spdx.json, or sbom.cyclonedx.json). \
                 See docs/src/reference/vox-portability-ssot.md.",
                lane_dir.display()
            );
        }
    }
    if signing_required {
        let candidates = [
            lane_dir.join("signing.attestation.json"),
            lane_dir.join("artifact.sig"),
        ];
        if !candidates.iter().any(|p| p.is_file()) {
            anyhow::bail!(
                "VOX_BACKEND_ARTIFACT_SIGNING_REQUIRED is enabled but no signing material found under {} \
                 (expected signing.attestation.json or artifact.sig). \
                 See docs/src/reference/vox-portability-ssot.md.",
                lane_dir.display()
            );
        }
    }
    Ok(())
}

fn default_ssh_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "root".to_string())
}
