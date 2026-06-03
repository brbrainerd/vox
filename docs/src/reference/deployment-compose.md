---
title: "Deployment: Docker, Compose, Coolify, CI (SSOT)"
description: "Official documentation for Deployment: Docker, Compose, Coolify, CI (SSOT) for the Vox language. Detailed technical reference, architectu"
category: "Language Reference"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# Deployment: Docker, Compose, Coolify, CI (SSOT)

**Single navigation hub** for container images, Compose files, hosted deploy (Coolify), CI checks, and how they relate to **mens** and **mobile/edge** (which are *not* the same shape as a Linux OCI image).

## Compose profiles (which file when)

| Profile | Purpose | Compose / template | Default image / build | Ports (typical) |
|---------|---------|--------------------|------------------------|-----------------|
| **MCP single-node** | Run `vox mcp` with API keys + optional Codex (Turso) | Repo root [`docker-compose.yml`](../../../docker-compose.yml) | Root [`Dockerfile`](../../../Dockerfile) (`CMD vox mcp`) | **3000** |
| **MCP + mens (multi-service)** | Control plane + MCP + worker; shared registry volume | [`examples/mens-compose.yml`](../../../docker-compose.yml) | Same `Dockerfile` with build-arg `VOX_CLI_FEATURES=mens,script-execution` | **9847** (mens), **3000** (MCP) |
| **Codex API (BaaS template)** | Self-hosted Codex-style HTTP API on Turso (placeholder service name) | [`infra/coolify/docker-compose.yml`](../../../infra/coolify/docker-compose.yml) | **`VOX_CODEX_IMAGE`** (you build/push); not the default `vox` MCP image unless you retag/repurpose | **8080** (template) |
| **Generated app stack** | `vox deploy` / `vox-container` sample (Node + nginx + optional mens env) | Emitted by [`generate_compose_file`](../../../crates/vox-deploy-codegen/src/generate.rs) | Project `Dockerfile` from `@environment` / package flow | **3000** + **80/443** |

**Do not** assume root `docker-compose.yml` and `infra/coolify/docker-compose.yml` are interchangeable: they target **different workloads** (MCP vs Codex API template). See [Codex BaaS](../archive/research-2026-q1/codex-baas.md) and [infra/coolify/README.md](../adr/index.md).

Optional split-plane sidecar: run **`vox-orchestrator-d`** alongside `vox-mcp` and set `VOX_ORCHESTRATOR_DAEMON_SOCKET` on MCP to the daemon TCP endpoint. Use `VOX_MCP_ORCHESTRATOR_RPC_READS=1` / `VOX_MCP_ORCHESTRATOR_RPC_WRITES=1` only when both services share the same repo/db context and startup probe confirms matching `repository_id`.

## OCI image (repo `Dockerfile`)

- **Binary:** `vox` (release), optional features via `VOX_CLI_FEATURES` (e.g. `mens,script-execution`).
- **Data:** volume **`/root/.vox`**; align with `VOX_DB_*` / local SQLite layout per [ADR 004](../adr/004-codex-arca-turso-ssot.md).
- **Mens sidecar (single container):** `VOX_MESH_MESH_SIDECAR=1` + entrypoint [`infra/containers/entrypoints/vox-entrypoint.vox`](../../../infra/containers/entrypoints/vox-entrypoint.vox); exposes **9847** when used.
- **Health:** `vox doctor --probe` (see root `Dockerfile` and [`infra/containers/Dockerfile.populi`](../../../infra/containers/Dockerfile.populi) `HEALTHCHECK`).

## Environment SSOT (Compose-friendly)

- **Codex / Turso:** `VOX_DB_URL`, `VOX_DB_TOKEN`, `VOX_DB_PATH` — [env-vars SSOT](env-vars.md), [ADR 004](../adr/004-codex-arca-turso-ssot.md).
- **Mens:** full `VOX_MESH_*` table — [mens SSOT](populi.md). Optional **`VOX_ORCHESTRATOR_MESH_CONTROL_URL`** for MCP to read mens nodes (see [`examples/mens-compose.yml`](../../../docker-compose.yml)). With a client-suitable URL, **`vox-mcp`** also **HTTP join/heartbeat** to the control plane (see mens SSOT **`VOX_MESH_HTTP_*`**). **Overlay / WAN personal clusters:** [Populi overlay runbook](../operations/populi-overlay-personal-cluster-runbook.md).
- **Optional mens env block (one text SSOT):** [`infra/containers/vox-compose-populi-environment.block.yaml`](../../../infra/containers/vox-compose-populi-environment.block.yaml) — embedded into generated Compose in `vox-container`; keep [`examples/mens-compose.yml`](../../../docker-compose.yml) semantically aligned (comments in that file point here).
- **Inference / mobile:** `VOX_INFERENCE_PROFILE` and LAN/cloud patterns — [mobile / edge AI SSOT](mobile-edge-ai.md) (phones do **not** run this `Dockerfile`).

## Runtimes: Docker vs Podman

- **CLI / deploy:** [`vox-container`](../../../crates/vox-container/src/lib.rs) implements **`ContainerRuntime`** for Docker and Podman; Compose execution prefers **`podman-compose`** then **`docker compose`** ([`deploy_target.rs`](../../../crates/vox-deploy-codegen/src/deploy_target.rs)).
- **CI:** GitHub self-hosted jobs use **Docker** (see [workflow enumeration](../ci/workflow-enumeration.md)). Validate Podman locally for rootless/volume/DNS differences before claiming parity.

## Coolify

- Coolify deploys **Docker Compose** bundles; use `${VAR}` / `${VAR:-default}` so secrets and toggles stay in the UI — [Coolify environment variables](https://coolify.io/docs/knowledge-base/environment-variables), [Compose on Coolify](https://coolify.io/docs/knowledge-base/docker/compose).
- Vox template: [`infra/coolify/`](../../../infra/coolify/) — read the README for image vs `Dockerfile` MCP split and build-time vs runtime vars.

## CI (GitHub)

- **GitHub:** `docker compose … config` on the mens example + `docker build` default and mens feature matrix — [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml).

> The former GitLab CI mirror was retired and deleted on 2026-06-03; GitHub Actions is the sole CI surface.

## Related docs

- [Vox portability SSOT](vox-portability-ssot.md) — normative portability guarantees, SSOT boundaries, and conformance expectations.
- [Cross-platform Vox — lanes & Docker matrix (SSOT)](../archive/research-2026-q1/vox-cross-platform-runbook.md) — script worker vs app vs mobile; feature matrix.
- [How to deploy](../how-to/how-to-deploy.md) — `vox deploy`, `Vox.toml`, registry login.
- [Zig-inspired deployment](../explanation/zig-inspired-deployment.md) — unified `vox deploy` targets and crates.
- [Mens SSOT](populi.md), [orchestration unified SSOT](orchestration-unified.md), [Populi overlay personal cluster runbook](../operations/populi-overlay-personal-cluster-runbook.md), [remote execution rollout checklist](../operations/populi-remote-execution-rollout-checklist.md).
- [Mobile / edge AI SSOT](mobile-edge-ai.md).

## Do’s and don’ts (short)

- **Do** keep variable names identical to [env-vars SSOT](env-vars.md) / mens / ADR 004.
- **Do** use persistent volumes for `/root/.vox` (or documented `VOX_DB_PATH`) in production Compose.
- **Don’t** embed secrets in committed defaults; use substitution + CI/secret stores.
- **Don’t** document “run the MCP `Dockerfile` on mobile”; use mobile-edge SSOT profiles and mens HTTP from the app.

## Remote mobile operations boundary

When teams need phone-based project management:

- Run Vox services on a remote host (Docker/Compose, VM, or bare-metal).
- Expose a hardened network control plane for bounded operations from mobile clients.
- Front the optional MCP HTTP gateway with a trusted reverse proxy and TLS termination; keep `vox-mcp` itself private-bind where possible.
- For strict proxy signaling, pair `VOX_MCP_HTTP_REQUIRE_FORWARDED_HTTPS=1` with a proxy-set `X-Forwarded-Proto: https`; only trust forwarded client IPs when ingress is fully controlled.
- Keep repository/toolchain state on the host; mobile clients should not be expected to run Cargo/git/`vox` locally.

See [MCP HTTP gateway contract](mcp-http-gateway-contract.md), [Crate API: vox-mcp](../reference/cli.md), and [env vars SSOT](env-vars.md) for the complete control-plane policy surface.

This deployment SSOT remains about server/container runtime surfaces; it does not redefine phones as first-class OCI runtime hosts.

