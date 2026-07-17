---
title: "Vox Telemetry Server — Coolify Deployment & Extensible Docker Architecture"
description: "End-to-end plan to fold the vox-server telemetry ingest into the vox monorepo, deploy it to telemetry.voxlang.org on the existing Hetzner Coolify instance (separate Vox Foundation project, decoupled from FableForge), wire CI/CD with free-tier-aware compute placement, and mirror it locally."
category: "CI & Quality"
status: "current"
---

# Vox Telemetry Server — Coolify Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Get `telemetry.voxlang.org` live on the existing Hetzner Coolify instance — receiving redacted OTLP telemetry into ClickHouse over verified TLS — as a Vox Foundation-owned service fully decoupled from FableForge, with a reusable "Vox service on Coolify" pattern and a free-tier-aware CI/CD compute-placement policy.

**Architecture:** Fold the local-only `vox-server` repo (`C:/Users/Owner/vox-server`) into `vox-foundation/vox` as an *excluded* workspace at `server/telemetry/` (ClickHouse deps never touch the main `cargo build`/arch-check graph). Deploy as a Coolify **Docker Compose** app (own project "Vox Foundation", own subdomain, own data volume) mirroring the proven `vox-eval` pipeline: GHCR image → `docker/vox-telemetry.compose.yml` → Traefik TLS → Gate-3 health probe. ClickHouse stays internal-only; the public ingest is bearer-gated (write-only anti-abuse key, not a confidentiality boundary). Heavy/GPU CI runs on the local self-hosted fleet; the deploy critical path runs on free GitHub-hosted runners (Vox is a public repo → unlimited free minutes) so deploys never wait on the workstation.

**Tech Stack:** Rust (axum 0.7 + clickhouse 0.13), Docker Compose, Coolify + Traefik + Let's Encrypt, GitHub Actions, GHCR, ClickHouse 24.3.

**Source-of-truth facts (verified against the codebase 2026-06-20):**
- `vox-foundation/vox` is **PUBLIC** (unlimited free GitHub-hosted minutes); `brbrainerd/fableforge` is **PRIVATE** (2,000 free min/mo).
- Coolify instance: `http://178.156.212.19:8000`. Existing live service: `eval.voxlang.org` (app `COOLIFY_APP_UUID`).
- Telemetry source today: `C:/Users/Owner/vox-server/vox-server/` (nested crate `vox-server`, bin `vox-server`, lib `vox_server`). Routes: `POST /v1/logs`, `GET /healthz`. Listens `0.0.0.0:4318`. No auth, no boot-migration.
- `vox_server::schema::gen_ddl()` already emits `CREATE TABLE IF NOT EXISTS events_raw` + `CREATE MATERIALIZED VIEW IF NOT EXISTS …` and `load_taxonomy()` exists → boot-migration is a thin idempotent wrapper.
- Root `Cargo.toml`: `[workspace] members = ["crates/*","crates/workspace-hack"]`, has an `exclude = [...]` list, `default-members = ["crates/vox-cli"]`.
- Deploy template to mirror: `.github/workflows/docker-eval.yml`, `.github/workflows/deploy-hetzner.yml` (Gate 1 smoke → Gate 2 Coolify deploy/poll → Gate 3 TLS health → Gate 4 notify), `vox-eval.compose.yml`.
- Coolify secrets are vox-secrets-managed (`SecretId::CoolifyToken`, `CoolifyBaseUrl`, `CoolifyAppUuid`, …); raw `std::env::var` for *Coolify* creds in vox-repo Rust is prohibited (the telemetry server reading its *own* `CLICKHOUSE_*` env is fine — it is not vox-repo CLI code).
- AGENTS.md policy: **no new `.ps1`/`.sh`/`.py`** automation — convenience wrappers must be `.vox` or a `vox` subcommand. `docs/src/**` files require `title`/`description`/`category` frontmatter.

---

## File Structure

**Created:**
- `server/telemetry/` — folded-in telemetry crate (own `[workspace]`, own lockfile). Flattened from the double-nested `vox-server/vox-server/`.
- `server/telemetry/src/auth.rs` — bearer-token ingest gate.
- `docker/vox-telemetry.compose.yml` — Coolify prod SSOT (ClickHouse internal + vox-server + Traefik).
- `.github/workflows/docker-telemetry.yml` — GHCR image build (mirrors `docker-eval.yml`).
- `.github/workflows/deploy-telemetry.yml` — Coolify deploy + Gate-3 probe (mirrors `deploy-hetzner.yml`, telemetry-scoped).
- `docs/src/ci/compute-placement.md` — VPS vs local-fleet vs hosted policy + per-repo matrices (frontmatter required).
- `docs/src/ci/coolify-services.md` — the Vox-service registry (eval + telemetry rows).
- `scripts/telemetry-dev.vox` — local mirror up/down/migrate/tail wrapper (VoxScript, not shell).

**Modified:**
- `Cargo.toml` (root) — add `"server/telemetry"` to `[workspace].exclude`.
- `server/telemetry/src/schema.rs` — add `ensure_schema()`.
- `server/telemetry/src/main.rs` — call `ensure_schema()` on boot; mount bearer layer on `/v1/logs`.
- `crates/vox-telemetry/src/config.rs` (or wherever the uploader endpoint/token config lives) — add `otlp_endpoint` + `ingest_token`.
- `docs/src/ci/deploy-contract.md` — add the telemetry app secrets row.

---

## Phase 0 — Fold the repo (isolation first)

### Task 0.1: Move vox-server into `server/telemetry/` as an excluded workspace

**Files:**
- Create: `server/telemetry/**` (copied from `C:/Users/Owner/vox-server/vox-server/` flattened, plus repo-root files `migrations/`, `contracts/`, `dashboards/`, `grafana/`, `docker-compose.yml`, `Dockerfile`, `.env.example`)
- Modify: `Cargo.toml` (root) `[workspace].exclude`

- [ ] **Step 1: Copy the crate, flattening the double-nest**

The source repo nests the crate one level deep (`vox-server/vox-server/`). Flatten so `server/telemetry/Cargo.toml` is the crate manifest with its own `[workspace]`.

Run (Bash tool):
```bash
mkdir -p C:/Users/Owner/vox/server/telemetry
cp -r C:/Users/Owner/vox-server/vox-server/. C:/Users/Owner/vox/server/telemetry/        # crate: src, tests, Cargo.toml
cp -r C:/Users/Owner/vox-server/migrations C:/Users/Owner/vox/server/telemetry/
cp -r C:/Users/Owner/vox-server/contracts  C:/Users/Owner/vox/server/telemetry/
cp -r C:/Users/Owner/vox-server/grafana    C:/Users/Owner/vox/server/telemetry/ 2>/dev/null || true
cp -r C:/Users/Owner/vox-server/dashboards C:/Users/Owner/vox/server/telemetry/ 2>/dev/null || true
cp C:/Users/Owner/vox-server/docker-compose.yml C:/Users/Owner/vox/server/telemetry/
cp C:/Users/Owner/vox-server/Dockerfile        C:/Users/Owner/vox/server/telemetry/
cp C:/Users/Owner/vox-server/.env.example      C:/Users/Owner/vox/server/telemetry/
cp C:/Users/Owner/vox-server/rust-toolchain.toml C:/Users/Owner/vox/server/telemetry/ 2>/dev/null || true
```

- [ ] **Step 2: Give `server/telemetry` its own self-contained workspace**

Edit `server/telemetry/Cargo.toml` so the flattened crate is both the workspace and the package (the source had a parent `[workspace]` + child member; now they merge). Prepend a `[workspace]` table and inline the former `[workspace.dependencies]`:

```toml
[workspace]
members = ["."]
resolver = "2"

[package]
name = "vox-server"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
description = "Vox telemetry ingest server — OTLP/HTTP → ClickHouse"

[[bin]]
name = "vox-server"
path = "src/main.rs"

[lib]
name = "vox_server"
path = "src/lib.rs"

[dependencies]
tokio              = { version = "1", features = ["full"] }
axum               = { version = "0.7" }
tower-http         = { version = "0.5", features = ["trace", "cors"] }
serde              = { version = "1", features = ["derive"] }
serde_json         = { version = "1" }
clickhouse         = { version = "0.13.3" }
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
anyhow             = "1"
thiserror          = "1"
dotenvy            = "0.15"
chrono             = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio = { version = "1", features = ["test-util"] }
```

- [ ] **Step 3: Exclude it from the outer vox workspace**

Edit root `Cargo.toml` — add `"server/telemetry"` to the existing `[workspace].exclude` array (alphabetically-near the other entries is fine):

```toml
exclude = [
    "crates/vox-arch-check/tests/fixtures/missing-desc",
    # ... existing entries ...
    "crates/vox-plugin-skill-v0",
    "server/telemetry",
]
```

- [ ] **Step 4: Verify the outer workspace ignores it and the inner builds standalone**

Run:
```bash
cd C:/Users/Owner/vox && cargo metadata --no-deps --format-version 1 | grep -c '"name":"vox-server"'
```
Expected: `0` (telemetry crate is NOT in the main workspace graph).

Run:
```bash
cd C:/Users/Owner/vox/server/telemetry && cargo build --bin vox-server
```
Expected: `Finished` (compiles standalone with its own lockfile).

- [ ] **Step 5: Commit**

```bash
cd C:/Users/Owner/vox
git add server/telemetry Cargo.toml
git commit -m "chore(telemetry): fold vox-server into server/telemetry as excluded workspace"
```

---

## Phase 1 — Harden the server (TDD)

### Task 1.1: Idempotent boot migration

**Files:**
- Modify: `server/telemetry/src/schema.rs`
- Modify: `server/telemetry/src/main.rs`
- Test: `server/telemetry/tests/schema_gen.rs` (extend)

- [ ] **Step 1: Write the failing test**

Append to `server/telemetry/tests/schema_gen.rs`:
```rust
#[test]
fn ddl_splits_into_executable_statements() {
    use vox_server::schema::{gen_ddl, load_taxonomy, split_statements};
    let tax = load_taxonomy().expect("taxonomy loads");
    let stmts = split_statements(&gen_ddl(&tax));
    // events_raw + at least one materialized view.
    assert!(stmts.len() >= 2, "expected >=2 statements, got {}", stmts.len());
    assert!(stmts.iter().any(|s| s.contains("CREATE TABLE IF NOT EXISTS events_raw")));
    assert!(stmts.iter().all(|s| !s.trim().is_empty()));
    // No bare comment-only fragments survive splitting.
    assert!(stmts.iter().all(|s| !s.trim_start().starts_with("--")));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd C:/Users/Owner/vox/server/telemetry && cargo test --test schema_gen ddl_splits_into_executable_statements`
Expected: FAIL — `split_statements` not found.

- [ ] **Step 3: Implement `split_statements` + `ensure_schema`**

Add to `server/telemetry/src/schema.rs`:
```rust
/// Split generated DDL into individual executable statements, dropping
/// comment-only lines. `gen_ddl` joins statements with ";\n\n".
pub fn split_statements(ddl: &str) -> Vec<String> {
    ddl.split(";\n\n")
        .map(|raw| {
            raw.lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Apply the generated schema idempotently on startup. Every statement is
/// `CREATE ... IF NOT EXISTS`, so this is safe on every boot.
pub async fn ensure_schema(ch: &clickhouse::Client) -> anyhow::Result<()> {
    let taxonomy = load_taxonomy()?;
    for stmt in split_statements(&gen_ddl(&taxonomy)) {
        ch.query(&stmt).execute().await?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd C:/Users/Owner/vox/server/telemetry && cargo test --test schema_gen ddl_splits_into_executable_statements`
Expected: PASS.

- [ ] **Step 5: Call `ensure_schema` on boot**

In `server/telemetry/src/main.rs`, after `let state = AppState::new(ch)?;` add (the `AppState` clones the `Client`; if it consumes it, call `ensure_schema(&ch).await?` *before* `AppState::new`, capturing a clone first):
```rust
    // Apply schema idempotently on boot (CREATE … IF NOT EXISTS), so a fresh
    // Coolify volume is migrated without a separate one-shot step.
    let ch_for_migrate = ch.clone();
    vox_server::schema::ensure_schema(&ch_for_migrate).await?;
    let state = AppState::new(ch)?;
```
Adjust ordering so `ch` is cloned before being moved into `AppState`.

- [ ] **Step 6: Build + commit**

```bash
cd C:/Users/Owner/vox/server/telemetry && cargo build --bin vox-server
git -C C:/Users/Owner/vox add server/telemetry/src/schema.rs server/telemetry/src/main.rs server/telemetry/tests/schema_gen.rs
git -C C:/Users/Owner/vox commit -m "feat(telemetry): idempotent boot migration via ensure_schema"
```

### Task 1.2: Bearer-token ingest gate

**Files:**
- Create: `server/telemetry/src/auth.rs`
- Modify: `server/telemetry/src/lib.rs` (add `pub mod auth;`)
- Modify: `server/telemetry/src/main.rs` (mount layer on `/v1/logs` only)
- Test: `server/telemetry/tests/auth_gate.rs`

- [ ] **Step 1: Write the failing test**

Create `server/telemetry/tests/auth_gate.rs`:
```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // oneshot
use vox_server::auth::{bearer_layer, IngestToken};

fn app(token: Option<&str>) -> axum::Router {
    axum::Router::new()
        .route("/v1/logs", axum::routing::post(|| async { "accepted" }))
        .route_layer(bearer_layer(IngestToken(token.map(str::to_string))))
        .route("/healthz", axum::routing::get(|| async { "ok" }))
}

#[tokio::test]
async fn rejects_missing_bearer_when_token_set() {
    let res = app(Some("secret")).oneshot(
        Request::post("/v1/logs").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accepts_correct_bearer() {
    let res = app(Some("secret")).oneshot(
        Request::post("/v1/logs")
            .header("authorization", "Bearer secret")
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn healthz_is_open() {
    let res = app(Some("secret")).oneshot(
        Request::get("/healthz").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn no_token_configured_allows_all_for_local_dev() {
    let res = app(None).oneshot(
        Request::post("/v1/logs").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```

Add `tower = "0.4"` to `[dev-dependencies]` in `server/telemetry/Cargo.toml`.

- [ ] **Step 2: Run to verify it fails**

Run: `cd C:/Users/Owner/vox/server/telemetry && cargo test --test auth_gate`
Expected: FAIL — `vox_server::auth` does not exist.

- [ ] **Step 3: Implement the gate**

Create `server/telemetry/src/auth.rs`:
```rust
//! Bearer-token gate for `POST /v1/logs`.
//!
//! SECURITY NOTE: this token is a **write-only anti-abuse key** (Sentry-DSN
//! model), NOT a confidentiality boundary. The client ships it, so it is
//! extractable by design; its only jobs are coarse abuse-blocking and per-IP
//! rate limiting. Privacy is enforced upstream (client-side redaction) and
//! server-side (taxonomy allowlist re-applied per ingest). `/healthz` is
//! routed outside this layer and stays open.

use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::Response,
};

#[derive(Clone)]
pub struct IngestToken(pub Option<String>);

/// Build a `route_layer` that enforces the bearer token. When the token is
/// `None` (local dev), all requests pass.
pub fn bearer_layer(
    token: IngestToken,
) -> axum::middleware::FromFnLayer<
    fn(axum::extract::State<IngestToken>, Request, Next) -> _,
    IngestToken,
    (axum::extract::State<IngestToken>, Request),
> {
    from_fn_with_state(token, require_bearer)
}

async fn require_bearer(
    axum::extract::State(expected): axum::extract::State<IngestToken>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(expected) = expected.0.as_deref() else {
        return Ok(next.run(req).await); // dev: no token configured
    };
    let provided = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match provided {
        Some(t) if constant_time_eq(t.as_bytes(), expected.as_bytes()) => {
            Ok(next.run(req).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Length-independent byte compare. The token is not a confidentiality
/// boundary, but a constant-time compare costs nothing and avoids a trivial
/// timing oracle.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
```

> **Note for implementer:** the exact return type of `bearer_layer` is fiddly with `from_fn_with_state`. If the explicit signature above does not compile, simplify by NOT wrapping in a constructor — instead expose `require_bearer` + `IngestToken` and call `from_fn_with_state(token, require_bearer)` directly at the call site in `main.rs`, and have the test build the router the same way. Keep the test assertions identical.

Add `pub mod auth;` to `server/telemetry/src/lib.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cd C:/Users/Owner/vox/server/telemetry && cargo test --test auth_gate`
Expected: PASS (4 tests).

- [ ] **Step 5: Mount the layer in main.rs**

In `server/telemetry/src/main.rs`, read the token and apply it to `/v1/logs` only:
```rust
    let ingest_token = vox_server::auth::IngestToken(
        std::env::var("VOX_TELEMETRY_INGEST_TOKEN").ok().filter(|s| !s.is_empty()),
    );

    let app = Router::new()
        .route("/v1/logs", post(ingest_logs))
        .route_layer(axum::middleware::from_fn_with_state(
            ingest_token,
            vox_server::auth::require_bearer,
        ))
        .route("/healthz", get(healthz))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
```
(Make `require_bearer` `pub` if referenced from `main.rs`.)

- [ ] **Step 6: Build + commit**

```bash
cd C:/Users/Owner/vox/server/telemetry && cargo build --bin vox-server && cargo test
git -C C:/Users/Owner/vox add server/telemetry/src/auth.rs server/telemetry/src/lib.rs server/telemetry/src/main.rs server/telemetry/tests/auth_gate.rs server/telemetry/Cargo.toml
git -C C:/Users/Owner/vox commit -m "feat(telemetry): bearer-gate POST /v1/logs (write-only anti-abuse key)"
```

---

## Phase 2 — Production Docker SSOT

### Task 2.1: `docker/vox-telemetry.compose.yml`

**Files:**
- Create: `docker/vox-telemetry.compose.yml`

- [ ] **Step 1: Write the compose SSOT**

```yaml
# Vox Telemetry — Coolify deployment (Vox Foundation project)
# Public ingest at telemetry.voxlang.org → vox-server :4318 → ClickHouse (internal only).
# Image SSOT: GHCR (built by .github/workflows/docker-telemetry.yml).
# Secrets (CLICKHOUSE_PASSWORD, VOX_TELEMETRY_INGEST_TOKEN) injected via Coolify project secrets.
# Docs: docs/src/ci/coolify-services.md
name: vox-telemetry

services:
  clickhouse:
    image: clickhouse/clickhouse-server:24.3-alpine
    container_name: vox-telemetry-clickhouse
    restart: unless-stopped
    # NO ports: and NO traefik labels — reachable ONLY by vox-server on the compose network.
    environment:
      CLICKHOUSE_DB: vox_telemetry
      CLICKHOUSE_USER: default
      CLICKHOUSE_PASSWORD: ${CLICKHOUSE_PASSWORD}
    volumes:
      - clickhouse_data:/var/lib/clickhouse   # Vox-owned data custody
    healthcheck:
      test: ["CMD", "wget", "--quiet", "--tries=1", "--spider", "http://localhost:8123/ping"]
      interval: 5s
      timeout: 3s
      retries: 20
    labels:
      - com.centurylinklabs.watchtower.enable=false

  vox-server:
    image: ghcr.io/vox-foundation/vox-telemetry-server:latest
    pull_policy: always
    container_name: vox-telemetry-server
    restart: unless-stopped
    depends_on:
      clickhouse:
        condition: service_healthy
    environment:
      CLICKHOUSE_URL: http://clickhouse:8123
      CLICKHOUSE_DB: vox_telemetry
      CLICKHOUSE_USER: default
      CLICKHOUSE_PASSWORD: ${CLICKHOUSE_PASSWORD}
      LISTEN_ADDR: 0.0.0.0:4318
      VOX_TELEMETRY_INGEST_TOKEN: ${VOX_TELEMETRY_INGEST_TOKEN}
      RUST_LOG: info
    ports:
      - "4318"   # exposed to Traefik only, not published to host
    labels:
      - "traefik.enable=true"
      # HTTPS router
      - "traefik.http.routers.vox-telemetry.rule=Host(`telemetry.voxlang.org`)"
      - "traefik.http.routers.vox-telemetry.entrypoints=https"
      - "traefik.http.routers.vox-telemetry.tls=true"
      - "traefik.http.routers.vox-telemetry.tls.certresolver=letsencrypt"
      # HTTP→HTTPS redirect
      - "traefik.http.routers.vox-telemetry-http.rule=Host(`telemetry.voxlang.org`)"
      - "traefik.http.routers.vox-telemetry-http.entrypoints=http"
      - "traefik.http.routers.vox-telemetry-http.middlewares=redirect-to-https"
      - "traefik.http.middlewares.redirect-to-https.redirectscheme.scheme=https"
      - "traefik.http.services.vox-telemetry.loadbalancer.server.port=4318"
      - com.centurylinklabs.watchtower.enable=true
    healthcheck:
      test: ["CMD", "wget", "--quiet", "--tries=1", "--spider", "http://localhost:4318/healthz"]
      interval: 30s
      timeout: 5s
      start_period: 20s

volumes:
  clickhouse_data:
```

- [ ] **Step 2: Validate compose syntax**

Run: `docker compose -f C:/Users/Owner/vox/docker/vox-telemetry.compose.yml config -q`
Expected: no output, exit 0 (env-var warnings for unset secrets are acceptable).

- [ ] **Step 3: Confirm the Dockerfile runtime has `wget`**

The healthcheck uses `wget`. `server/telemetry/Dockerfile` runtime stage is `alpine:3.20` which ships BusaBox `wget`. Verify:
Run: `grep -n "FROM alpine" C:/Users/Owner/vox/server/telemetry/Dockerfile`
Expected: a line `FROM alpine:3.20`. (BusyBox `wget` is present by default; no change needed.)

- [ ] **Step 4: Commit**

```bash
git -C C:/Users/Owner/vox add docker/vox-telemetry.compose.yml
git -C C:/Users/Owner/vox commit -m "feat(telemetry): Coolify compose SSOT (ClickHouse internal + Traefik TLS)"
```

---

## Phase 3 — CI/CD wiring (free-tier aware)

### Task 3.1: GHCR image build — `.github/workflows/docker-telemetry.yml`

**Files:**
- Create: `.github/workflows/docker-telemetry.yml`

- [ ] **Step 1: Author the workflow (mirror `docker-eval.yml`, telemetry-scoped)**

```yaml
name: Build & Push vox-telemetry-server Docker Image

on:
  push:
    branches: [main]
    paths:
      - "server/telemetry/**"
      - "docker/vox-telemetry.compose.yml"
      - ".github/workflows/docker-telemetry.yml"
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

permissions:
  contents: read
  packages: write

env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
  REGISTRY: ghcr.io
  IMAGE_NAME: vox-foundation/vox-telemetry-server

jobs:
  build-and-push:
    name: Build vox-telemetry-server image
    runs-on: ubuntu-latest   # public repo → free unlimited minutes; deploy path never waits on the workstation
    steps:
      - uses: actions/checkout@v7
      - uses: docker/setup-buildx-action@v4
      - name: Log in to GHCR
        uses: docker/login-action@v4
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Docker metadata
        id: meta
        uses: docker/metadata-action@v6
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=sha,prefix=main-
            type=raw,value=latest,enable={{is_default_branch}}
      - name: Build and push
        uses: docker/build-push-action@v7
        with:
          context: server/telemetry          # standalone crate; small build
          file: server/telemetry/Dockerfile
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=registry,ref=${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:latest
          cache-to: type=inline
      - name: Trivy scan (report-only)
        uses: aquasecurity/trivy-action@v0.36.0
        with:
          image-ref: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:latest
          format: 'table'
          severity: 'CRITICAL,HIGH'
          ignore-unfixed: true
          exit-code: '0'
```

> **Build-context note:** `server/telemetry/Dockerfile` was authored when the crate sat at repo root (`COPY . .`). Because `context: server/telemetry`, the existing `COPY . .` + `cargo build --release --bin vox-server` still works unchanged. The `COPY --from=builder /build/contracts/ /app/contracts/` line requires `server/telemetry/contracts/` to exist (copied in Phase 0). Verify in Step 2.

- [ ] **Step 2: Verify the Dockerfile build locally before pushing**

Run: `cd C:/Users/Owner/vox && docker build -f server/telemetry/Dockerfile -t vox-telemetry-server:test server/telemetry`
Expected: `naming to … vox-telemetry-server:test` (build succeeds; `contracts/` present).

- [ ] **Step 3: Commit**

```bash
git -C C:/Users/Owner/vox add .github/workflows/docker-telemetry.yml
git -C C:/Users/Owner/vox commit -m "ci(telemetry): GHCR image build workflow"
```

### Task 3.2: Deploy + Gate-3 probe — `.github/workflows/deploy-telemetry.yml`

**Files:**
- Create: `.github/workflows/deploy-telemetry.yml`

- [ ] **Step 1: Author the deploy workflow**

This mirrors `deploy-hetzner.yml`'s Gate-2/Gate-3 logic but is telemetry-scoped: it triggers on telemetry paths, uses a new `COOLIFY_TELEMETRY_APP_UUID` secret, waits for the image build, and probes `https://telemetry.voxlang.org/healthz` (body `ok`). Reuse the **exact** trigger/poll bash from `deploy-hetzner.yml` (it is hardened against HTML login pages, missing read scope, etc.), substituting the app UUID env and the public probe URL.

```yaml
name: Deploy Telemetry (Coolify)

on:
  workflow_run:
    workflows: ["Build & Push vox-telemetry-server Docker Image"]
    types: [completed]
    branches: [main]
  workflow_dispatch:
    inputs:
      skip_public_health_probe:
        type: boolean
        default: false

env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

concurrency:
  group: deploy-telemetry-${{ github.ref }}
  cancel-in-progress: true

jobs:
  deploy-coolify:
    name: "Gate 2: Deploy & Poll (telemetry)"
    # Only run if the image build succeeded (or manual dispatch).
    if: ${{ github.event_name == 'workflow_dispatch' || github.event.workflow_run.conclusion == 'success' }}
    runs-on: ubuntu-latest
    env:
      COOLIFY_BASE_URL: ${{ secrets.COOLIFY_BASE_URL }}
      COOLIFY_TOKEN: ${{ secrets.COOLIFY_TOKEN }}
      COOLIFY_READ_TOKEN: ${{ secrets.COOLIFY_READ_TOKEN }}
      COOLIFY_APP_UUID: ${{ secrets.COOLIFY_TELEMETRY_APP_UUID }}
    steps:
      - name: Trigger Coolify deploy
        id: trigger
        run: |
          # �***COPY the full "Trigger Coolify deploy" bash from deploy-hetzner.yml verbatim***
          # It is app-UUID-agnostic (uses $COOLIFY_APP_UUID, here mapped to the telemetry app).
          # ... see .github/workflows/deploy-hetzner.yml job deploy-coolify step "Trigger Coolify deploy"
          echo "::error::Implementer: paste the verified trigger bash here"; exit 1
      - name: Poll Deployment Status
        id: poll
        run: |
          # ↑ COPY the full "Poll Deployment Status" bash from deploy-hetzner.yml verbatim.
          echo "::error::Implementer: paste the verified poll bash here"; exit 1

  health-check:
    name: "Gate 3: Health Check (telemetry)"
    needs: [deploy-coolify]
    if: ${{ always() && needs.deploy-coolify.result == 'success' }}
    runs-on: ubuntu-latest
    steps:
      - name: Wait for container to settle
        run: sleep 15
      - name: Probe public telemetry host (HTTPS, verified TLS)
        if: ${{ github.event_name != 'workflow_dispatch' || github.event.inputs.skip_public_health_probe != 'true' }}
        env:
          PROBE_URL: ${{ secrets.COOLIFY_PUBLIC_TELEMETRY_HEALTH_URL }}
        run: |
          set -euo pipefail
          URL="${PROBE_URL:-https://telemetry.voxlang.org/healthz}"
          echo "Public HTTPS probe (verified CA, no -k): $URL"
          ok=""
          for i in $(seq 1 24); do
            tmp="$(mktemp)"
            http="$(curl -sS -o "$tmp" -w '%{http_code}' "$URL" || echo "000")"
            body="$(tr -d '\r\n' < "$tmp" 2>/dev/null || true)"; rm -f "$tmp"
            if [ "$http" = "200" ] && [ "$body" = "ok" ]; then
              echo "✅ HTTP 200, body 'ok'"; ok="1"; break
            fi
            echo "[$i/24] HTTP $http body='$body'"; sleep 10
          done
          [ -n "$ok" ] || { echo "::error::telemetry /healthz probe failed (~4m). Check Traefik Host rule, Let's Encrypt cert, container healthcheck."; exit 1; }
```

> **Implementer:** the two `echo "::error::… paste …"` placeholders MUST be replaced by copying the corresponding step bodies from `deploy-hetzner.yml` (the `deploy-coolify` job's "Trigger Coolify deploy" and "Poll Deployment Status" steps) verbatim — they are battle-tested and app-UUID-agnostic. Do not re-author the Coolify API logic from scratch.

- [ ] **Step 2: Lint the workflow**

Run: `cd C:/Users/Owner/vox && npx --yes @action-validator/cli .github/workflows/deploy-telemetry.yml 2>/dev/null || echo "validator unavailable — yaml-lint instead"`
Then: `docker run --rm -i ghcr.io/rhysd/actionlint:latest < /dev/null 2>/dev/null; cat .github/workflows/deploy-telemetry.yml | head -1`
Expected: YAML parses; no placeholder `::error::` lines remain after Step 1.

- [ ] **Step 3: Commit**

```bash
git -C C:/Users/Owner/vox add .github/workflows/deploy-telemetry.yml
git -C C:/Users/Owner/vox commit -m "ci(telemetry): Coolify deploy + Gate-3 TLS health probe"
```

### Task 3.3: Secrets registry — extend `deploy-contract.md` + vox-secrets

**Files:**
- Modify: `docs/src/ci/deploy-contract.md`
- Modify: `crates/vox-secrets/src/**` (wherever `SecretId` enum + GHA mapping live)

- [ ] **Step 1: Add the telemetry secrets row to the deploy contract**

In `docs/src/ci/deploy-contract.md`, in the "Secrets (vox-secrets Managed)" table, add:

| Secret ID | GHA Secret Name | Description |
|---|---|---|
| `CoolifyTelemetryAppUuid` | `COOLIFY_TELEMETRY_APP_UUID` | Coolify Docker-Compose app UUID for `telemetry.voxlang.org` (Vox Foundation project). |
| `VoxTelemetryIngestToken` | `VOX_TELEMETRY_INGEST_TOKEN` | Write-only ingest anti-abuse key (Sentry-DSN model). Injected as a Coolify project secret AND shipped in client config. |
| `ClickhouseTelemetryPassword` | `CLICKHOUSE_PASSWORD` | ClickHouse password for the telemetry project (Coolify project secret only). |
| _(optional)_ | `COOLIFY_PUBLIC_TELEMETRY_HEALTH_URL` | Overrides `https://telemetry.voxlang.org/healthz` for Gate-3. |

- [ ] **Step 2: Add `SecretId` variants (if vox CLI needs to resolve them)**

Locate the `SecretId` enum: `grep -rn "enum SecretId" crates/vox-secrets/src`. Add `CoolifyTelemetryAppUuid` and `VoxTelemetryIngestToken` variants following the existing `CoolifyAppUuid` pattern (string key + GHA env mapping). Build: `cargo build -p vox-secrets`.

> Only needed if a `vox` subcommand resolves these locally. The deploy workflows read them straight from GitHub Secrets, so this step is optional for the deploy path — implement it only when wiring `vox telemetry dev` (Phase 7) or a future `vox ci coolify-telemetry`.

- [ ] **Step 3: Commit**

```bash
git -C C:/Users/Owner/vox add docs/src/ci/deploy-contract.md crates/vox-secrets 2>/dev/null
git -C C:/Users/Owner/vox commit -m "docs(deploy): register telemetry Coolify app + ingest-token secrets"
```

---

## Phase 4 — Compute-placement policy (doc + apply to new workflows)

### Task 4.1: Author `docs/src/ci/compute-placement.md`

**Files:**
- Create: `docs/src/ci/compute-placement.md`

- [ ] **Step 1: Write the policy doc**

Frontmatter + the decision rule + both matrices. Content:

```markdown
---
title: "CI/CD Compute Placement Policy"
description: "Where each CI, CD, and nightly job runs — Hetzner VPS (always-on) vs local self-hosted fleet (CPU/disk/GPU) vs GitHub-hosted (neutral/free) — for vox (public) and FableForge (private), chosen by gating resource."
category: "CI & Quality"
status: "current"
---

# CI/CD Compute Placement Policy

## Decision rule — classify by *gating resource*, then place

| Gating resource | Host | Why |
|---|---|---|
| CPU-parallel (compile, clippy --all-targets, mutation, test matrix) | Local fleet | 32 threads vs few shared vCPU |
| Disk-IO / cache (cargo target, Docker layers, sccache, graphify) | Local fleet | 4 TB NVMe vs ~160–240 GB |
| GPU (inference, LoRA/QLoRA, qwen nightly, ComfyUI) | Local fleet | RTX 4080S; VPS has none |
| RAM-heavy (Next.js build, Playwright/Stagehand) | Local fleet | 64 GB vs ~16 GB |
| Uptime / network (deploy CD, health/TLS probes, DB maintenance, dep-bots) | Hetzner VPS | Must fire regardless of workstation state |
| Reproducibility on neutral infra (portability gate, cross-OS release, security scans) | GitHub-hosted | No private-hardware dependency |

## Free-tier economics
- **vox = PUBLIC** → unlimited free GitHub-hosted minutes. Use hosted for the entire **deploy critical path** (image build, Coolify trigger, Gate-3) so deploys never wait on the workstation. Local fleet only for raw speed / GPU on latency-tolerant heavy jobs.
- **FableForge = PRIVATE** → 2,000 free min/mo. Keep only the light merge gate + deploy trigger on hosted; push all heavy jobs to the local fleet to conserve minutes.

## vox placement
| Tier | Jobs |
|---|---|
| Local fleet (`[self-hosted, linux, x64]`) | ci.yml build+clippy+test, mutation-nightly, compile-matrix, bench-nightly (pinned for HW consistency), qwen35-native-nightly (GPU), ml_data_extraction |
| Hetzner VPS | deploy triggers + Gate-3 probes, nightly ClickHouse maintenance (TTL/OPTIMIZE, backup→object storage), live-endpoint uptime, link/dep bots |
| GitHub-hosted | Gate-1 portability build, docker-telemetry/docker-eval image builds, release-* cross-OS, mobile EAS, codeql/scorecard/gitleaks; ci-fallback-hosted.yml = safety valve |

## FableForge placement
| Tier | Jobs |
|---|---|
| Local fleet | nextjs-build-check, e2e-*/stagehand/semantic-vrt, dystopia-core-full-tests, studio-pipeline (GPU), test-coverage |
| Hetzner VPS | deploy-hetzner/convex-deploy/deploy-guard triggers, nightly-live-audit, archive-cron/deploy-archiver, coderabbit-ingest, cursor-sync |
| GitHub-hosted | lint/typecheck merge gate, coverage-check reporting |

## Invariants
1. The merge gate never hard-depends on the workstation (Gate-1 portability + ci-fallback-hosted keep PRs unblockable).
2. bench-nightly is pinned to one host (local) so timings stay comparable.
3. DB maintenance + backups run where the data lives (Hetzner → object storage).
```

- [ ] **Step 2: Confirm new telemetry workflows already comply**

`docker-telemetry.yml` and `deploy-telemetry.yml` both use `runs-on: ubuntu-latest` (deploy critical path on free hosted) — matches the policy. No change needed.

- [ ] **Step 3: Commit**

```bash
git -C C:/Users/Owner/vox add docs/src/ci/compute-placement.md
git -C C:/Users/Owner/vox commit -m "docs(ci): compute-placement policy (VPS vs local fleet vs hosted)"
```

> **Out of scope (follow-up):** migrating the `runs-on` of the ~40 existing workflows to match the matrices. This plan documents the policy and applies it to the new telemetry workflows; a separate sweep PR should reconcile existing workflows, one workflow per commit, with CI green between each.

---

## Phase 5 — Client wiring (point Vox at the live endpoint)

### Task 5.1: OTLP endpoint + ingest token in client config (TDD)

**Files:**
- Modify: `crates/vox-telemetry/src/config.rs` (or the uploader config module — locate with `grep -rn "OTLP_ENDPOINT\|otlp_endpoint\|upload" crates/vox-telemetry-otlp/src crates/vox-telemetry/src`)
- Test: alongside that module

- [ ] **Step 1: Locate the uploader endpoint resolution**

Run: `grep -rn "OTLP_ENDPOINT\|endpoint\|ingest" crates/vox-telemetry-otlp/src crates/vox-telemetry/src | head -20`
Identify where the uploader reads its target URL today (Track B `B5: feature-gated uploader`).

- [ ] **Step 2: Write the failing test**

In the config module's test section:
```rust
#[test]
fn otlp_endpoint_defaults_to_production_telemetry_host() {
    let cfg = TelemetryUploadConfig::from_env_or_default(&Default::default());
    assert_eq!(cfg.endpoint, "https://telemetry.voxlang.org/v1/logs");
}

#[test]
fn ingest_token_threads_into_authorization_header() {
    let cfg = TelemetryUploadConfig { endpoint: "x".into(), ingest_token: Some("k".into()) };
    assert_eq!(cfg.authorization_header().as_deref(), Some("Bearer k"));
}
```
(Adapt type/method names to the actual config struct.)

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p vox-telemetry-otlp otlp_endpoint_defaults` (or the owning crate)
Expected: FAIL.

- [ ] **Step 4: Implement**

Add `endpoint` (default `https://telemetry.voxlang.org/v1/logs`) and `ingest_token` (from `VOX_TELEMETRY_INGEST_TOKEN`, else baked default), plus `authorization_header()` returning `Some("Bearer <token>")` when set. Wire the uploader's reqwest request to set the `Authorization` header from `authorization_header()`.

- [ ] **Step 5: Run to verify it passes** — Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git -C C:/Users/Owner/vox add crates/vox-telemetry-otlp crates/vox-telemetry 2>/dev/null
git -C C:/Users/Owner/vox commit -m "feat(telemetry): client points at telemetry.voxlang.org with bearer ingest token"
```

---

## Phase 6 — Ops rollout (manual checklist — run by an operator with Coolify access)

These steps are not code; execute them in order and record outcomes. They are gated on Phases 0–5 being merged to `main`.

- [ ] **Step 1: DNS** — Create an `A` record `telemetry.voxlang.org → 178.156.212.19` (same IP as `eval.voxlang.org`). Verify: `nslookup telemetry.voxlang.org` resolves to the VPS IP.

- [ ] **Step 2: Coolify project** — In Coolify (`http://178.156.212.19:8000`), create a **new project** "Vox Foundation" (separate from "FableForge Production"). This is the decoupling boundary.

- [ ] **Step 3: Coolify resource** — Add Resource → **Docker Compose** → source = GitHub `vox-foundation/vox`, branch `main`, compose path `docker/vox-telemetry.compose.yml`. (Coolify reads the compose from the repo; the image is pulled from GHCR per `pull_policy: always`.)

- [ ] **Step 4: Coolify secrets** — In the resource's Environment, set `CLICKHOUSE_PASSWORD` (generate a strong value) and `VOX_TELEMETRY_INGEST_TOKEN` (generate; this is the value that also goes into the client default + the `VOX_TELEMETRY_INGEST_TOKEN` GitHub Secret).

- [ ] **Step 5: TLS** — Ensure the Traefik `letsencrypt` certresolver is active for the new host (same resolver eval uses). Trigger first deploy from the Coolify UI.

- [ ] **Step 6: Capture the app UUID** — From the resource URL/API, copy the application UUID. Set GitHub repo secrets on `vox-foundation/vox`: `COOLIFY_TELEMETRY_APP_UUID`, `VOX_TELEMETRY_INGEST_TOKEN`, and (if not already global) `CLICKHOUSE_PASSWORD`.

- [ ] **Step 7: Verify Gate 3** — Run the `Deploy Telemetry (Coolify)` workflow via `workflow_dispatch`. Confirm Gate-3 probe goes green:
  `curl -sS https://telemetry.voxlang.org/healthz` → `ok` (verified TLS, no `-k`).

- [ ] **Step 8: End-to-end smoke** — POST a single OTLP `vox.command` event with the bearer header:
  ```bash
  curl -sS -X POST https://telemetry.voxlang.org/v1/logs \
    -H "Authorization: Bearer $VOX_TELEMETRY_INGEST_TOKEN" \
    -H "Content-Type: application/json" \
    --data @server/telemetry/tests/fixtures/sample_vox_command.json
  ```
  Then verify the row landed (ClickHouse is internal, so query via a Coolify exec or a temporary `clickhouse-client` in the project network):
  `SELECT count() FROM vox_telemetry.events_raw WHERE event_name='vox.command'` → ≥ 1.
  (If no fixture exists, reuse the E3 payload from the prior session's ledger AGH-0017/0019.)

- [ ] **Step 9: Flip a real client** — In a local Vox config, set consent = Granted and master telemetry on; perform one command; confirm a new row appears within the spool/upload interval.

---

## Phase 7 — Local mirror, service registry, memory

### Task 7.1: `vox telemetry dev` local-mirror wrapper (VoxScript)

**Files:**
- Create: `scripts/telemetry-dev.vox`

- [ ] **Step 1: Author a VoxScript wrapper** (honors AGENTS.md "no new .ps1/.sh/.py")

It wraps the folded-in `server/telemetry/docker-compose.yml` (the local mirror — ClickHouse + vox-server + Grafana, `full` profile), runs migrations (now automatic on boot), and tails logs. Accepts `up` / `down` / `logs` subcommands, shelling out to `docker compose` via Vox's process API. Document that the local mirror runs **standalone** (not merged into FableForge's compose) — different ports (telemetry `4318/8123/3000` vs FableForge `3210/3211/6791/8200`) so both can run at once.

- [ ] **Step 2: Smoke the wrapper**

Run: `vox run scripts/telemetry-dev.vox -- up` then `curl -s localhost:4318/healthz` → `ok`. `vox run scripts/telemetry-dev.vox -- down`.

- [ ] **Step 3: Commit**

```bash
git -C C:/Users/Owner/vox add scripts/telemetry-dev.vox
git -C C:/Users/Owner/vox commit -m "feat(telemetry): vox telemetry dev local-mirror wrapper"
```

### Task 7.2: Service registry doc

**Files:**
- Create: `docs/src/ci/coolify-services.md`

- [ ] **Step 1: Write the registry**

```markdown
---
title: "Vox Coolify Service Registry"
description: "Every public Vox service deployed to the Hetzner Coolify instance: subdomain, compose SSOT, GHCR image, app-UUID secret, and Gate-3 health URL. The 6-artifact template for adding a new service."
category: "CI & Quality"
status: "current"
---

# Vox Coolify Service Registry

| Service | Subdomain | Compose SSOT | GHCR image | App-UUID secret | Health URL |
|---|---|---|---|---|---|
| Eval sandbox | eval.voxlang.org | vox-eval.compose.yml | ghcr.io/brbrainerd/vox-eval (personal namespace — org free tier can't make container packages public) | COOLIFY_APP_UUID | https://eval.voxlang.org/health |
| Telemetry | telemetry.voxlang.org | docker/vox-telemetry.compose.yml | ghcr.io/vox-foundation/vox-telemetry-server | COOLIFY_TELEMETRY_APP_UUID | https://telemetry.voxlang.org/healthz |

## Add-a-service template (6 artifacts)
1. `server/<svc>/` excluded workspace + Dockerfile
2. `.github/workflows/docker-<svc>.yml` → GHCR (runs-on ubuntu-latest; public repo = free)
3. `docker/vox-<svc>.compose.yml` (Traefik HTTPS+redirect+healthcheck+watchtower; datastores internal-only + named volume)
4. `.github/workflows/deploy-<svc>.yml` (Coolify deploy + Gate-3, reuse deploy-hetzner bash)
5. Secrets: `COOLIFY_<SVC>_APP_UUID` (+ service secrets) as Coolify project secrets + GitHub Secrets
6. A row in this table

**Invariants:** own Coolify Docker-Compose app + own Gate-3 probe; one subdomain under a Vox-owned domain; datastores never routed; separate Coolify project per org-ownership boundary.
```

- [ ] **Step 2: Commit**

```bash
git -C C:/Users/Owner/vox add docs/src/ci/coolify-services.md
git -C C:/Users/Owner/vox commit -m "docs(ci): Vox Coolify service registry + add-a-service template"
```

### Task 7.3: Update ledger + memory

- [ ] **Step 1:** Append an `AGH-00xx` entry to `docs/superpowers/antigravity-handoff-ledger.md` recording the deployment (delivered artifacts, the decoupling boundary, the free-tier placement decision, and the manual ops steps that remain human-gated).
- [ ] **Step 2:** Update the telemetry memory file (`project_centralized_telemetry_spec_plan_2026_06_19.md`) to note the Coolify deployment plan + `telemetry.voxlang.org` + the Vox Foundation Coolify project; and add a one-line `MEMORY.md` pointer if a new file is warranted.
- [ ] **Step 3: Commit.**

---

## Self-Review

**Spec coverage:** ✅ live on Coolify (Phases 2,3,6) · ✅ ClickHouse hosting decision = internal-only container w/ named volume (Section 2 / Task 2.1) · ✅ local mirror (Task 7.1) · ✅ extensible Docker architecture (Task 7.2 template) · ✅ privacy (Task 1.2 bearer + ClickHouse isolation + existing redaction) · ✅ decouple Vox/FableForge (separate Coolify project — Phase 6 Step 2; nothing Rust in FableForge) · ✅ nightly + CI/CD placement for both repos (Phase 4) · ✅ free-tier delegation (Phase 4 economics).

**Placeholder scan:** the only intentional "fill-in" is `deploy-telemetry.yml`'s two bash steps, which MUST be copied verbatim from `deploy-hetzner.yml` (Task 3.2 Step 1 note) — this is deliberate reuse of battle-tested code, not a vague placeholder; the step says exactly which job/steps to copy.

**Type/name consistency:** `vox-server` (bin) / `vox_server` (lib) / image `ghcr.io/vox-foundation/vox-telemetry-server` / app-UUID secret `COOLIFY_TELEMETRY_APP_UUID` / ingest env `VOX_TELEMETRY_INGEST_TOKEN` — used consistently across compose, workflows, secrets table, and client config.

**Open data point to confirm during rollout:** the exact Hetzner VPS plan (vCPU/RAM/disk) — it governs how much, if any, build/cache work the VPS can absorb before the policy's "heavy → local" rule must be enforced harder. Confirm via `ssh` `nproc` / `free -h` / `df -h` and annotate `compute-placement.md`.
