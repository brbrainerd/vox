# Zig-Inspired Deployment Architecture

Vox's deployment story is modelled after the Zig compiler's core insight: **one command, any target, zero manual configuration**.

## Background: What We Learned from Zig

The Zig compiler achieves a remarkable user experience through several interlocking design decisions:

| Zig Design | Vox Equivalent |
|---|---|
| `zig build -Dtarget=<triple>` — one command, any native target | `vox deploy <env>` — one command, any deploy target |
| Self-contained binary bundling Clang + libc headers | Auto-detection + auto-healing for container runtimes, Python, Node |
| SHA-256 content-addressed artifact cache | `.vox-cache/artifacts/` — skip rebuild when inputs unchanged |
| Hermetic builds (isolated from host) | `--hermetic` mode — build inside a container for reproducibility |
| Declarative `build.zig` — single source of truth | Declarative `Vox.toml [deploy]` — single source of truth |

## Unified Deployment Command

All deployment targets are driven by a single command:

```bash
vox deploy <env>                              # auto-detect target from Vox.toml
vox deploy production --target container      # OCI image → Docker/Podman → registry
vox deploy production --target bare-metal     # systemd service file on SSH host
vox deploy production --target compose        # docker-compose.yml + docker compose up
vox deploy production --target k8s            # Kubernetes manifests + kubectl apply
vox deploy production --hermetic              # build inside container for reproducibility
vox deploy production --dry-run               # show what would happen, don't do it
```

## `Vox.toml` Deployment Configuration

```toml
[deploy]
# The deployment target type: "container", "bare-metal", "compose", "k8s", or "auto"
target = "auto"
# Container runtime preference: "docker", "podman", or "auto" (prefers Podman)
runtime = "auto"

[deploy.container]
image_name = "my-app"
registry   = "ghcr.io/user"

[deploy.bare-metal]
host         = "prod.example.com"
user         = "deploy"
service_name = "my-app"
deploy_dir   = "/opt/my-app"

[deploy.compose]
project_name = "my-app"
services     = ["app", "db"]

[deploy.kubernetes]
cluster   = "prod"
namespace = "default"
replicas  = 3
```

## Artifact Cache

Vox stores build outputs in a content-addressed cache, keyed by SHA-3/512 of all inputs:

```
.vox-cache/
├── manifests/    # <input-hash> → artifact metadata (JSON)
└── artifacts/    # <input-hash>/ directories with build outputs
```

When `vox build` or `vox deploy` runs:
1. Hash all source files + `Vox.toml` + dependency versions
2. Look up the hash in `.vox-cache/manifests/`
3. **Cache hit** → skip compilation entirely, go straight to packaging/deploy
4. **Cache miss** → full build, write outputs to `.vox-cache/artifacts/<hash>/`

This mirrors Zig's `.zig-cache/` with SHA-256 manifests and object directories.

## Bare-Metal Deployment Detail

When `target = "bare-metal"`, `vox deploy` generates and installs a systemd service:

1. Compiles the Vox application
2. Generates a `.service` file from the `@environment` declaration
3. SCPs the binary and service file to `<host>`
4. Runs `systemctl daemon-reload && systemctl enable --now <service-name>` via SSH

## Key Crates

| Crate | Role |
|---|---|
| `vox-container` | `ContainerRuntime` trait, Docker/Podman, bare-metal systemd, `DeployTarget` enum |
| `vox-pm` | `ArtifactCache` (content-addressed build cache), `VoxManifest`/`DeploySection` |
| `vox-cli` | Unified `vox deploy` command dispatching to all target types |

## Reducing Technical Debt

Before this architecture, deployment was scattered across four commands and files:

- `vox deploy` → `deploy.rs` (only OCI)
- `vox deploy-infra` → `deploy_infra.rs` (Terraform + Compose generation)
- `vox container` → `container.rs` (raw runtime operations)
- Bare-metal was buried in `vox-container/src/bare_metal.rs`, unreachable from CLI

All of this is now unified under `vox deploy` with target dispatch logic in `vox-container::deploy_target`.
