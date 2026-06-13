# Semantic Behavior Map — `vox-deploy-codegen`

Deterministically synthesized from 37 distinct proven-behavior claims (of 37 extracted) across 7 symbols. 0 symbols have an explicit error-path proof; **7 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `generate_kubernetes_manifests`  (happy; EXTRACTED)
- [happy] includes application name in manifest  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes container image tag in manifest  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes namespace in manifest  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes Deployment kind in manifest  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes Service kind in manifest  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] exposes container port from spec in manifest  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes environment variables from spec in manifest  (crates/vox-deploy-codegen/src/generate.rs)

### `generate_dockerfile_from_spec`  (happy; EXTRACTED)
- [happy] uses apk package manager for Alpine images  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes all packages from spec in apk command  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] uses apt-get package manager for Debian images  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes all environment variables from spec  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] declares VOLUME directives for all volumes in spec  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] exposes all ports from exposed_ports in spec  (crates/vox-deploy-codegen/src/generate.rs)

### `generate_systemd_unit`  (happy; EXTRACTED)
- [happy] generates systemd unit file with [Unit] section  (crates/vox-deploy-codegen/src/bare_metal.rs)
- [happy] generates systemd unit file with [Service] section  (crates/vox-deploy-codegen/src/bare_metal.rs)
- [happy] includes WorkingDirectory directive from spec  (crates/vox-deploy-codegen/src/bare_metal.rs)
- [happy] includes Environment variables in unit file  (crates/vox-deploy-codegen/src/bare_metal.rs)
- [happy] includes ExecStart directive with command and args from spec  (crates/vox-deploy-codegen/src/bare_metal.rs)
- [happy] includes [Install] section with WantedBy=multi-user.target  (crates/vox-deploy-codegen/src/bare_metal.rs)

### `build_container_target`  (happy; EXTRACTED)
- [happy] produces correct image_tag from app_name and version  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] produces correct registry_tag when registry is provided  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] extracts registry_host from registry_tag  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] stores provided context_dir path  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] preserves build_args from input  (crates/vox-deploy-codegen/src/deploy_target.rs)

### `generate_default_dockerfile`  (happy; EXTRACTED)
- [happy] uses Debian bookworm-slim as base image  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] sets WORKDIR to /app  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] copies release binary vox-app to /app  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] exposes DEFAULT_APP_PORT  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] sets CMD to run /app/vox-app binary  (crates/vox-deploy-codegen/src/generate.rs)

### `resolve_target_kind`  (happy; EXTRACTED)
- [happy] returns 'container' when both arguments are None  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] returns 'bare-metal' when target_kind is Some('bare-metal')  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] returns 'compose' when orchestrator is Some('docker-compose')  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] returns 'kubernetes' when target_kind is Some('k8s') regardless of orchestrator  (crates/vox-deploy-codegen/src/deploy_target.rs)
- [happy] returns 'container' when target_kind is Some('auto')  (crates/vox-deploy-codegen/src/deploy_target.rs)

### `generate_compose_file`  (happy; EXTRACTED)
- [happy] omits version field from compose file  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes services section in compose file  (crates/vox-deploy-codegen/src/generate.rs)
- [happy] includes VOX_MESH_TOKEN environment variable template  (crates/vox-deploy-codegen/src/generate.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`build_container_target`** — only: _produces correct image_tag from app_name and version_
- **`generate_compose_file`** — only: _omits version field from compose file_
- **`generate_default_dockerfile`** — only: _uses Debian bookworm-slim as base image_
- **`generate_dockerfile_from_spec`** — only: _uses apk package manager for Alpine images_
- **`generate_kubernetes_manifests`** — only: _includes application name in manifest_
- **`generate_systemd_unit`** — only: _generates systemd unit file with [Unit] section_
- **`resolve_target_kind`** — only: _returns 'container' when both arguments are None_
