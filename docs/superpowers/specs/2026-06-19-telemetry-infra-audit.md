---
category: "Telemetry Contracts"
---

# Telemetry Infrastructure Audit

**Date:** 2026-06-19  
**Status:** Audit complete — decisions locked.  
**Companion plan:** `docs/superpowers/plans/2026-06-19-centralized-telemetry-program.md`

---

## 1. In-repo infra inventory

### Files read

| Path | Purpose |
|------|---------|
| `infra/ci-runner/docker-compose.yml` | Self-hosted GitHub Actions runner stack (WSL2/Docker) |
| `infra/coolify/docker-compose.yml` | Coolify template for Codex-style HTTP API workloads on Turso |
| `infra/coolify/README.md` | Deployment guide for the Coolify template |
| `crates/vox-cli/src/commands/deploy.rs` | `vox deploy` — reads `Vox.toml [deploy]`, delegates to `vox-deploy-codegen` |
| `Cargo.toml` (workspace root) | Existing dep pins: `reqwest = "0.12"`, `governor = "0.10"` |

### What exists today

| Service | Exists? | Notes |
|---------|---------|-------|
| CI runner fleet | ✓ | `infra/ci-runner/` (Docker/WSL2, Task Scheduler autoscaler) |
| Coolify project | ✓ (template) | `infra/coolify/` — a Codex-BaaS-style HTTP API template; **NOT** a deployed vox-telemetry-server |
| ClickHouse | ✗ | No existing ClickHouse service anywhere in the repo's infra |
| OTLP Collector | ✗ | Not deployed |
| vox-telemetry-server | ✗ | Does not exist yet — to be created in Track C |

**Key finding:** `vox deploy` is a general-purpose deploy codegen tool (reads `Vox.toml [deploy]`, supports container/compose/k8s/bare-metal). It does **not** target a specific Coolify project automatically. A separate Coolify project must be created for the telemetry server, or it can be deployed via Hetzner self-managed Docker.

The `infra/coolify/docker-compose.yml` is a **Codex-BaaS template**, not a deployment of the Vox MCP image. The Vox MCP image (from `Dockerfile`) is deployed separately via Coolify's UI pointing at this repo — not through this compose template.

---

## 2. Ingest topology decision

**Decision: (b) `axum` + `clickhouse` crate (Rust)**

Rationale:
- Fewer moving parts: no OTel Collector process to manage, no exporter plugin chain.
- Fully auditable: the field allowlist re-applied server-side is the same taxonomy JSON; a single Rust crate handles decode + filter + insert.
- Typed schema: `clickhouse-rs` sends typed row structs; ClickHouse DDL is generated from the SSOT (C2).
- Swappable backend: the client sends standard OTLP/HTTP logs JSON; we can later add an OTel Collector in front of the axum service without changing the client.
- OTel Collector only wins if it's already deployed (it isn't).

**Architecture (Track C):**
```
vox client (OTLP/HTTP logs JSON, hand-encoded)
  → POST /v1/logs
  → axum ingest (vox-telemetry-server)
    → decode OTLP proto/JSON
    → server-side field allowlist re-applied (defense-in-depth)
    → clickhouse batch insert → events_raw (TTL 180 days)
    → materialized views (per-category rollups)
  → Grafana reads rollup views (k-anonymity enforced in query)
```

---

## 3. Hosting decision

**Decision: New Coolify project on FableForge, separate from existing Vox/Codex deployments.**

Rationale:
- FableForge is the existing managed host; creates a new project instead of sharing with the Vox MCP service to avoid blast-radius coupling.
- ClickHouse runs as a Docker service in the same Coolify project (single-node, volume-backed, internal network).
- Coolify manages TLS cert for the ingest endpoint (public HTTPS).
- Grafana also runs as a Docker service in the same project (read-only user on ClickHouse).
- The axum ingest binary is containerized via `vox deploy` (compose target, separate `Vox.toml`).

**Fallback:** If FableForge Coolify capacity is constrained at Deploy time (Track D), fall back to a Hetzner CX21 (2vCPU/4GB) with Docker Compose directly.

---

## 4. OpenTelemetry scope decision (client)

**Decision: Client carries NO `opentelemetry` SDK. OTLP/HTTP logs JSON is hand-encoded.**

The workspace already pins:
```toml
opentelemetry-otlp = { version = "0.29", features = ["http-proto", "reqwest-client"] }
```
This pin is consumed by `vox-foundation`'s `otel` feature (traces-only, `logs` feature absent).
Going 0.29 → 0.32 crosses two breaking otel releases and would require workspace-wide API-break fixes — **out of scope for this program**.

The client hand-encodes the OTLP/HTTP logs JSON envelope with `serde` + `reqwest` (both already in the workspace). This is sufficient: the server's axum endpoint decodes the same stable JSON format.

**No workspace `opentelemetry*` pin change required.** The 0.29 pins remain untouched.

The server repo (`vox-telemetry-server`, Track C) is a **separate repo** and may use whichever otel version it needs without affecting the Vox workspace.

---

## 5. Pinned versions (client-side — workspace Cargo.toml)

See `contracts/telemetry/pinned-versions.md` for the authoritative version table.

Summary:
- `reqwest = "0.12"` (already in workspace — reused for upload, feature-gated)
- `governor = "0.10"` (already in workspace — reused for rate limiting, feature-gated)
- `serde = "1"` + `serde_json = "1"` (already in workspace)
- `uuid = {version="1", features=["v4","serde"]}` (already in `vox-telemetry`)
- **No new client deps required.** `vox-telemetry-otlp`'s pure core is serde-only; its `remote` feature reuses existing `reqwest` and `governor` workspace pins.

---

## 6. Follow-ups not in scope

- OTel Collector deployment (would add ingest option; revisit post-MVP if needed)
- Multi-region ClickHouse replication (single-node MVP first)
- Data-subject access portal (post-MVP; noted in spec §8)
- Workspace otel 0.29→0.32 bump (separate program; traces stack migration)
