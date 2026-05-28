---
title: "vox deploy CLI reference"
description: "Ship a Vox app via container, compose, kubernetes, bare-metal, fly, or coolify targets configured in Vox.toml."
category: "Language Reference"
status: "current"
last_updated: "2026-05-24"
schema_type: "TechArticle"
---

# `vox deploy` — ship a Vox app to a runtime

`vox deploy` reads the `[deploy]` section of your project's `Vox.toml`, builds
the right artifact, and ships it to the target runtime. Six target kinds are
supported today: `container`, `compose`, `kubernetes`, `bare-metal`, `fly`,
`coolify`.

For a one-line summary of *which* target to pick, jump to
[Choosing a target](#choosing-a-target).

## Quickstart

```bash
# 1. Scaffold a project with a deploy block pre-wired.
vox init --template web my-app
cd my-app

# 2. Inspect what deploy would do without invoking a runtime.
vox deploy --dry-run

# 3. When you're ready, run for real.
vox deploy
```

The web template scaffolds `Vox.toml` with `[deploy] target = "container"` and
commented `[deploy.fly]` / `[deploy.coolify]` blocks you can uncomment to switch
platforms.

## Flags

| Flag | Default | Meaning |
|---|---|---|
| `<environment>` (positional) | `production` | Image tag suffix (e.g. `staging`, `production`). |
| `--target <kind>` | from `[deploy].target` | Override the target kind for this run. |
| `--runtime <name>` | from `[deploy].runtime` | Override the container runtime (`auto`, `docker`, `podman`). Only honored when target is `container`. |
| `--dry-run` | `false` | Print the planned actions without invoking a runtime or mutating remote systems. **Does NOT require docker/podman to be installed**, even for container targets. |
| `--detach` | `false` | For `compose` targets: run `docker compose up` with `-d`. |
| `--locked` | `false` | Require `vox.lock` to exist before deploying. Use in CI to guarantee reproducible builds. |

## Choosing a target

| Target | Latency to ship | Portability | Ops complexity | Use when |
|---|---|---|---|---|
| `container` | Fastest local | Highest (OCI image runs anywhere) | None — you own the image | You want a build artifact you'll push elsewhere by hand. |
| `compose` | Fast | Medium (needs Docker) | Low | Single-host deploys, dev environments, small teams. |
| `kubernetes` | Medium | Medium (cluster-bound) | High | You already run a k8s cluster. |
| `bare-metal` | Medium | Low (host-specific) | Medium | SSH-accessible host, want systemd unit, no container. |
| `fly` | Fast | High (Fly hosts it) | None — Fly manages | "I want a URL, now." Hosted, global, billed by Fly. |
| `coolify` | Fast | Medium (your Coolify instance) | Low — Coolify manages | Self-hosted PaaS; you run a Coolify instance and want auto-deploy. |

## Target reference

### `container`

Builds an OCI container image and (optionally) pushes it to a registry. Does
not run the image — that's the next step in your pipeline.

```toml
[deploy]
target = "container"
runtime = "auto"                    # "auto" | "docker" | "podman"

[deploy.container]
# All fields optional — sensible defaults derive from the package name.
# image_name = "my-app"             # default: package.name
# registry   = "ghcr.io/your-org"   # if set, also pushes
# dockerfile = "Dockerfile.prod"    # default: Dockerfile (project root)
# build_args = { VOX_FEATURE_X = "1" }
```

**Dry-run output:**
```text
Deploying environment `production` via container target
  [dry-run] would build OCI image: my-app:production
  [dry-run] would push: ghcr.io/your-org/my-app:production
```

**Note:** `vox deploy --dry-run` does NOT require docker/podman to be installed.
The runtime detection is skipped on dry-run paths so CI / air-gapped
contributors can validate deploy plans without a container daemon.

### `compose`

Runs `docker compose up` against a `docker-compose.yml` you provide.

```toml
[deploy]
target = "compose"

[deploy.compose]
file         = "docker-compose.yml"  # default
project_name = "my-app"              # default: package.name
services     = ["web", "worker"]     # optional; empty = all services
```

Pair with `--detach` to launch in the background.

### `kubernetes`

Applies a directory of manifests via `kubectl apply -f <dir> --namespace <ns>`.

```toml
[deploy]
target = "kubernetes"

[deploy.kubernetes]
manifests_dir = "k8s/"
namespace     = "production"          # default: "default"
# cluster   = "prod-east"             # optional — sets kubectl --context
# replicas  = 3                       # informational; for templates that read it
```

### `bare-metal`

Generates a systemd unit, SCPs it to a host, and starts it. No container
runtime required on the target.

```toml
[deploy]
target = "bare-metal"

[deploy.bare-metal]
host         = "ops-1.example.com"
user         = "deploy"               # default: $USER / $USERNAME / "root"
port         = 22
deploy_dir   = "/opt/my-app"          # default: /opt/<package.name>
service_name = "my-app"               # default: package.name
```

### `fly`

Deploys via the `flyctl` CLI to Fly.io. Reuses your existing `flyctl auth`
session.

```toml
[deploy]
target = "fly"

[deploy.fly]
app_name = "my-app"                   # default: package.name
region   = "iad"                      # optional
# org    = "my-org"                   # optional
```

### `coolify`

Triggers a redeploy on a self-hosted Coolify instance via its API.

```toml
[deploy]
target = "coolify"

[deploy.coolify]
base_url      = "https://coolify.example.com"
app_uuid      = "abc-123-..."
token_env     = "COOLIFY_TOKEN"       # name of env var carrying the API token
force_rebuild = false
```

The token is read from the environment variable named by `token_env` (or from
the `vox-secrets` `CoolifyToken` slot as a fallback). Never commit the token
itself.

## Portable backend artifact lane (SBOM / signing)

For container-promoting targets (`container`, `compose`, `kubernetes`, `fly`,
`coolify`), two environment-driven gates can enforce supply-chain artifacts
before a non-dry-run deploy:

| Env var | When `1` / `true` | What it enforces |
|---|---|---|
| `VOX_BACKEND_ARTIFACT_SBOM_REQUIRED` | An SBOM file must exist at `<project>/<lane>/sbom.json` (or `sbom.spdx.json` / `sbom.cyclonedx.json`). | Aborts deploy if missing. |
| `VOX_BACKEND_ARTIFACT_SIGNING_REQUIRED` | A signing artifact must exist at `<project>/<lane>/signing.attestation.json` or `artifact.sig`. | Aborts deploy if missing. |

These gates are off by default; turn them on in CI to enforce the portability
lane described in
[`docs/src/reference/vox-portability-ssot.md`](vox-portability-ssot.md).

## See also

- [`vox new` / `vox init`](cli.md#init) — scaffold a project with a deploy block.
- [`vox doctor --project`](cli.md#doctor) — verify the project compiles clean
  before deploying.
- [`docs/src/reference/deployment-compose.md`](deployment-compose.md) — deeper
  dive on the compose target.
- CR-L7 integration test:
  [`crates/vox-cli/tests/cr_l7_new_deploy_doctor_e2e.rs`](../../../crates/vox-cli/tests/cr_l7_new_deploy_doctor_e2e.rs) —
  the contract test that exercises `vox new web → vox deploy --dry-run →
  vox doctor --project` end-to-end.
