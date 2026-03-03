# How-To: Deploy to Production

Learn how to package and deploy your Vox application using declarative environments and the `vox deploy` command.

## 0. Declarative Environment (Optional)

You can define your deployment environment directly in your `.vox` files using `@environment` blocks. This allows you to specify a base image, system packages, environment variables, exposed ports, and more, similar to a NixOS-like declarative specification.

```vox
@environment production:
    base: "node:22-alpine"
    packages: ["curl", "git", "imagemagick"]
    env:
        PORT: "8080"
        NODE_ENV: "production"
    expose: [8080, 443]
    volumes: ["/data"]
    copy:
        "nginx.conf" to "/etc/nginx/nginx.conf"
    run: ["echo 'Building...'"]
    cmd: ["npx", "tsx", "server.ts"]
```

### Bare Metal (systemd) Provider

For applications that run directly on Linux servers without Docker, set `base` to `"bare-metal"` and Vox will generate a systemd `.service` file instead of a Dockerfile:

```vox
@environment server:
    base: "bare-metal"
    workdir: "/opt/my-app"
    env:
        PORT: "8080"
    cmd: ["./my-app", "--port", "8080"]
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
image_name = "my-registry.io/my-vox-app"
registry   = "my-registry.io"
runtime    = "podman"  # optional: docker or podman (auto-detected if omitted)
```

Then run:
```bash
vox deploy
# or for a specific environment:
vox deploy --env staging
```

`vox deploy` automatically:
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
- [CLI Reference](api/vox-cli.md) — All `vox package` and `vox deploy` options.
- [Runtime Explanation](expl-architecture.md) — Understanding the runtime environment.
