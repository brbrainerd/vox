---
title: "How-To: Deploy to Production"
description: "Declarative deployment using the environment keyword and the vox deploy command for OCI-compatible containerization."
category: "How-To Guides"
last_updated: "2026-04-06"
status: "current"
training_eligible: true
 
schema_type: "HowTo"
keywords: ["deploy Vox app", "Vox production deployment", "Docker Vox", "self-host Vox"]
---
# How-To: Deploy to Production

Learn how to package and deploy your Vox application using declarative environments, the `vox deploy` command, and Fly.io for zero-configuration deployments.

## The Golden Path: Zero to Production

Vox's marquee product experience is going from an empty directory to a deployed application in under 10 minutes. 

```bash
# 1. Generate the TanStack Start and Rust scaffolding
vox new web my-app

# 2. Change into the directory
cd my-app

# 3. Deploy instantly (requires flyctl installed and authenticated)
vox deploy --target fly
```

This golden path guarantees the fastest iteration cycle for a solo developer building web applications.

## Declarative Environments

You can define your deployment environment directly in your `.vox` files using `environment` blocks. This allows you to specify a base image, system packages, environment variables, exposed ports, and more.

```vox
{{#include ../../../examples/golden/ref_orchestrator.vox:orchestrator_config}}
```

> [!NOTE]
> The **`npx tsx server.ts`** command is a **legacy / opt-in Node lane**. TypeScript codegen emits **`server.ts`** only when **`VOX_EMIT_EXPRESS_SERVER=1`** is set at build time; the default product path is the **generated Axum** binary plus **`vox-client.ts`** for HTTP endpoints. See [vox-fullstack-artifacts.md](../reference/vox-fullstack-artifacts.md).

### Bare Metal (systemd) Provider

For applications that run directly on Linux servers without Docker, set `base` to `"bare-metal"` and Vox will generate a systemd `.service` file instead of a Dockerfile:

```vox
// vox:skip
environment server {
    base "bare-metal"
    workdir "/opt/my-app"
    env PORT = "8080"
    cmd ["./my-app", "--port", "8080"]
}
```

Running `vox build` will emit a `server.service` file ready for deployment with `systemctl enable` and `systemctl start`.

Vox will automatically use these blocks to generate customized OCI-compatible Dockerfiles or systemd service files.

## 1. Registry Authentication

Before pushing images to a private registry, authenticate with `vox login`:

```bash
# Log in to the default VoxPM registry
vox login <your-api-token>

# Log in to a private OCI registry (e.g. GitHub Container Registry)
vox login <token-or-password> --registry ghcr.io --username myuser

# Log in to Docker Hub
vox login <password> --registry registry.hub.docker.com --username myuser
```

Credentials are stored in `~/.vox/auth.json`. When you run `vox deploy`, the CLI will automatically authenticate with the configured registry before pushing.

> [!TIP]
> For CI/CD pipelines, pass the token via stdin:
> ```bash
> echo "$REGISTRY_TOKEN" | vox login token --registry ghcr.io --username $REGISTRY_USER
> ```

## 2. Deploying with `vox deploy`

The simplest way to deploy your application is using the `vox deploy` command. This handles building your container image, authenticating with the registry, and pushing.

```toml
# Vox.toml
[deploy]
target = "fly"

[deploy.fly]
app_name = "my-app"
org = "personal"
region = "ams"
```

Then run:
```bash
vox deploy --target fly
```

`vox deploy --target fly` automatically:
1. Detects your local `flyctl` installation and authentication.
2. Packages the application using the local Vox compiler and toolchain.
3. Automatically deploys to Fly.io using your configured application name.

Alternatively, for traditional container-based deployments:
```toml
# Vox.toml
[deploy]
target = "container"
image_name = "my-registry.io/my-vox-app"
registry   = "my-registry.io"
runtime    = "podman"  # optional: docker or podman (auto-detected if omitted)
```

Then run:
```bash
vox deploy --target container
```

`vox deploy --target container` automatically:
1. Detects your container runtime (Podman preferred, Docker fallback)
2. Builds the OCI image
3. Authenticates with your registry using credentials from `vox login`
4. Tags and pushes the image

## 3. Manual Packaging

If you prefer building yourself, Vox generates an OCI-compatible Dockerfile:

```bash
vox package --kind docker
docker build -t my-vox-app .
```

## 4. Persistent Storage

Since Vox uses SQLite for the data layer and durability journal, ensure you mount a persistent volume if deploying as a container.

```yaml
# fly.toml example
[mounts]
  source = "vox_data"
  destination = "/data"
```

---

**Related Reference**:
- [Fullstack Artifacts](../reference/ref-syntax.md) — Rust-first containers vs Express `server.ts`.
- [CLI Reference](../reference/cli.md) — All `vox package` and `vox deploy` options.
- [Runtime Explanation](../explanation/expl-actors-workflows.md) — Understanding the runtime environment.

