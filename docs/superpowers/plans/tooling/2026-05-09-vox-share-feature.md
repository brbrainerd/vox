# `vox share` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `vox share` — a one-command public-URL tunnel for Vox apps. Default backend is Cloudflare Quick Tunnels, automatic fallback to `localhost.run` on failure, explicit `--backend tailscale` for power users with stable URLs. Built on the existing `vox bundle` single-binary substrate.

**Architecture:** A new crate `crates/vox-share/` holds a `TunnelBackend` trait and three implementations (Cloudflare, localhost.run, Tailscale Funnel). The CLI command `vox share` (in `crates/vox-cli/src/commands/share.rs`) orchestrates: bundle the app via existing `vox bundle` pipeline (or dev pipeline with `--dev`), spawn the bundled binary on a private port, spawn a reverse-proxy Axum process that adds auth + SSE detection + request logging and proxies to the bundled binary, then ask the chosen `TunnelBackend` to expose the proxy port to the public internet. Three child processes (app, proxy, tunnel client) coordinated through a shutdown channel.

**Tech Stack:** Rust (workspace crates), Axum (proxy), `cloudflared` Go binary (lazy-downloaded), system OpenSSH (`localhost.run`), `tailscale` CLI (Tailscale Funnel). The `cloudflared` binary is downloaded to `~/.cache/vox/cloudflared/<version>-<os>-<arch>`, SHA256-verified, redistributable under Apache-2.0.

---

## Decisions locked from brainstorming (2026-05-09)

| Decision | Choice |
|---|---|
| MVP scope | Robust + power-user — auto-fallback chain, SSE detection, auth, pre-flight checks, explicit `--backend` override |
| Providers (3) | Cloudflare Quick Tunnels (default), localhost.run (auto-fallback), Tailscale Funnel (`--backend tailscale`) |
| CLI shape | `vox share` top-level subcommand; defaults to `vox bundle` pipeline; `--dev` for fast iteration |
| Auth default | URL-embedded token (`?vox_share_token=...`); `--auth none` opts out, `--auth basic:user:pass` upgrades |
| SSE handling | Detect routes at startup; Cloudflare + SSE → auto-switch to localhost.run with one-line notice |
| Duration default | 8h; `--duration Nh` overrides; `--duration none` for unbounded |
| Foreground UX | Plain stdout, structured `[vox share]` and `[app]` prefixes |
| First-run UX | One-time `[Y/n]` consent banner; cloudflared auto-downloaded + SHA256-verified; state in `~/.config/vox/share-state.json` |

Companion research at [docs/src/architecture/gradio-streamlit-research-2026.md §6](../../../src/architecture/gradio-streamlit-research-2026.md) and [the share-feature research dossier](../../../../crates/vox-share/RESEARCH.md) (created in S1).

**Out of scope for this plan:** the Vox-hosted FRP relay at `*.vox.live`. That's the future direction (see [§Phase S10](#phase-s10--future-direction-vox-hosted-frp-relay)) but not built here.

---

## Status of related work

| Item | Status | Notes |
|---|---|---|
| `vox bundle` single-binary build | ✅ Already shipped | Per [crates/vox-cli/src/commands/bundle.rs] — produces self-contained executable with React assets via `rust_embed`. This is the foundation `vox share` builds on. |
| `vox container build/run` | ✅ Already shipped | OCI runtime abstraction via vox-container. Not used by `vox share`; share is a different category. |
| `vox deploy` (6 targets) | ✅ Already shipped | Production hosting. `vox share` is for ephemeral demos, not deploys. |
| Generated Axum binary `axum::serve` on `127.0.0.1:<PORT>` | ✅ Already shipped | Per `crates/vox-codegen/src/codegen_rust/emit/http.rs`. This is what `vox share` proxies to. |
| `cloudflared` / `ngrok` detection in `vox doctor` | ✅ Diagnostic only | `vox doctor` reports presence; no integration with serving pipeline. |

---

## Roadmap (phasing table)

| Phase | Work | Surfaces touched | Approx. size | Gate |
|---|---|---|---|---|
| **S1** Foundation: `vox-share` crate + `TunnelBackend` trait + LAN backend | New crate; CLI subcommand; backend abstraction; LAN backend (`bind 0.0.0.0`); stdout status output. | new `crates/vox-share/`, `crates/vox-cli/src/commands/share.rs`, CLI dispatch, integration tests | medium | `vox share --backend lan` works on a smoke-test app; LAN URL printed; Ctrl+C clean-shutdowns |
| **S2** Cloudflare backend + cloudflared lazy-download + first-run consent | cloudflared lazy-download with SHA256 verify; subprocess spawn; URL parse from stdout; consent banner; `~/.config/vox/share-state.json`. | `crates/vox-share/src/backends/cloudflare.rs`, `crates/vox-share/src/binary_cache.rs`, integration tests | large | Default `vox share` produces a working `*.trycloudflare.com` URL; first-run prompts once and persists |
| **S3** localhost.run backend + automatic fallback | SSH-based tunnel via stock OpenSSH; URL parse from SSH banner; fallback logic when Cloudflare backend fails. | `crates/vox-share/src/backends/localhost_run.rs`, fallback orchestration in `coordinator.rs` | medium | `vox share --backend localhost-run` works; killing Cloudflare backend mid-flight transparently switches |
| **S4** Tailscale Funnel backend (explicit only) | Detect `tailscale` CLI; check funnel-enabled; spawn `tailscale funnel <port>`. | `crates/vox-share/src/backends/tailscale.rs` | small | `vox share --backend tailscale` works on a machine with Tailscale enabled |
| **S5** Auth middleware (URL-token + basic-auth) | Axum middleware in proxy layer; token generator; URL synthesis. | `crates/vox-share/src/auth.rs`, proxy integration | medium | Default `vox share` produces URL with `?vox_share_token=`; without token, app returns 401 |
| **S6** SSE detection + auto-switch backend | Scan bundled binary's OpenAPI for `text/event-stream`; auto-switch on detection; explicit override flag. | `crates/vox-share/src/sse_detect.rs`, coordinator wiring | medium | Chat-app fixture with SSE auto-uses localhost.run when Cloudflare default chosen |
| **S7** Duration + auto-shutdown + countdown | 8h default; `--duration Nh\|none`; countdown line in stdout status; graceful shutdown of all child processes. | `crates/vox-share/src/lifecycle.rs` | small | `--duration 5s` smoke test exits at 5s ± 1s; countdown printed once per minute |
| **S8** Bundle/dev integration | Default: run `vox bundle` first if no fresh artifact; `--dev` uses dev server pipeline. | `crates/vox-cli/src/commands/share.rs` integration with bundle.rs | medium | Cold `vox share` runs bundle then shares; warm `vox share` reuses cache; `vox share --dev` skips bundle |
| **S9** Docs + abuse-policy + safety guide | New mdBook page + how-to guide + ToS reference; CLI help; abuse-and-takedown contact policy doc. | `docs/src/how-to/how-to-share.md`, `docs/src/architecture/share-policy-2026.md`, doc-pipeline regen | small | mdBook builds; `vox share --help` is informative |
| **S10** *Future* — Vox-hosted FRP relay (`*.vox.live`) | NOT IN THIS PLAN. Documented future direction; will get its own plan when prioritized. | n/a | n/a | n/a |

**Atomicity:** S1 is the substrate everything else hangs off; ship first. S2 is the headline feature; ship second. S3 unlocks the auto-fallback. S4-S7 are pairwise independent and can land in any order after S1+S2. S8 depends on S1 + the existing `vox bundle` pipeline being reliable. S9 lands last as the cap.

**Each phase ends in a green test suite, behind a flag where useful, and is independently shippable.** `vox share --backend lan` ships at end of S1. `vox share` (no flag) ships at end of S2 with Cloudflare. The fallback chain ships at end of S3. Auth ships at end of S5. The complete robust+power-user MVP is the union of S1-S9.

**This document holds detailed bite-sized TDD for S1, S2, and S3.** Phases S4-S9 each get a one-paragraph scope summary plus a follow-up plan trigger (write a fresh detailed plan when the prior phase lands and the next is unblocked). Same pattern as the [VUV-9 roadmap](2026-05-08-vuv-improvement-roadmap.md).

---

## File structure across all phases

**Create (S1):**
- `crates/vox-share/Cargo.toml` — new workspace member.
- `crates/vox-share/src/lib.rs` — re-exports.
- `crates/vox-share/src/backend.rs` — `TunnelBackend` trait + `BackendKind` enum.
- `crates/vox-share/src/coordinator.rs` — process orchestration: app + proxy + tunnel.
- `crates/vox-share/src/proxy.rs` — Axum reverse proxy that adds auth + logs + proxies to inner app.
- `crates/vox-share/src/backends/mod.rs` — module index.
- `crates/vox-share/src/backends/lan.rs` — LAN backend (bind `0.0.0.0`).
- `crates/vox-share/src/state.rs` — `~/.config/vox/share-state.json` reader/writer.
- `crates/vox-share/src/error.rs` — error types.
- `crates/vox-share/RESEARCH.md` — research dossier copy (Cloudflare/localhost.run/Tailscale primary-source notes; included for future maintainers).
- `crates/vox-share/tests/lan_backend_test.rs` — integration test for LAN backend.
- `crates/vox-cli/src/commands/share.rs` — CLI subcommand.

**Create (S2):**
- `crates/vox-share/src/binary_cache.rs` — cloudflared lazy-download + SHA256 verify + cache dir management.
- `crates/vox-share/src/backends/cloudflare.rs` — Cloudflare Quick Tunnel backend.
- `crates/vox-share/src/consent.rs` — first-run consent banner + state persistence.
- `crates/vox-share/tests/cloudflare_backend_test.rs` — backend test (against a mock cloudflared binary).
- `crates/vox-share/tests/binary_cache_test.rs` — download/verify test.

**Create (S3):**
- `crates/vox-share/src/backends/localhost_run.rs` — SSH-based localhost.run backend.
- `crates/vox-share/tests/localhost_run_backend_test.rs` — backend test (mocked SSH).

**Create (S4-S9):**
- `crates/vox-share/src/backends/tailscale.rs` — Tailscale Funnel (S4).
- `crates/vox-share/src/auth.rs` — URL-token + basic-auth (S5).
- `crates/vox-share/src/sse_detect.rs` — OpenAPI SSE-route detection (S6).
- `crates/vox-share/src/lifecycle.rs` — duration / countdown / graceful shutdown (S7).
- `docs/src/how-to/how-to-share.md` — user guide (S9).
- `docs/src/architecture/share-policy-2026.md` — abuse / ToS / privacy doc (S9).

**Modify (across phases):**
- `crates/vox-cli/src/commands/mod.rs` — register `share` subcommand.
- `crates/vox-cli/src/main.rs` (or wherever clap dispatch is) — wire `Share(ShareArgs)` variant.
- `crates/vox-cli/Cargo.toml` — add `vox-share = { path = "../vox-share" }`.
- `Cargo.toml` (workspace) — add `crates/vox-share` to members.
- `docs/src/SUMMARY.md` (auto-regenerated, never hand-edited).

---

## Phase S1 — Foundation: `vox-share` crate + LAN backend

**Goal:** Ship the substrate. After S1, `vox share --backend lan` produces a working LAN URL on `0.0.0.0:7860`. No internet exposure. The orchestrator boots two child processes (the bundled app and a reverse-proxy Axum), prints status to stdout, handles Ctrl+C cleanly. The `TunnelBackend` trait exists with one implementation. Subsequent phases plug in additional backends.

**Why ship LAN first:** zero external dependencies, zero infrastructure, zero friction. Validates the entire orchestration pipeline. If the LAN backend doesn't work, no other backend will.

### Task 1: Create the workspace member

**Files:**
- Create: `crates/vox-share/Cargo.toml`
- Create: `crates/vox-share/src/lib.rs` (stub)
- Modify: `Cargo.toml` (workspace root) — add `crates/vox-share` to `members`

- [ ] **Step 1: Create `crates/vox-share/Cargo.toml`**

```toml
[package]
name = "vox-share"
version = "0.5.0"
edition = "2024"
publish = false
description = "vox share — public-URL tunneling for Vox apps. Cloudflare Quick Tunnels (default), localhost.run (fallback), Tailscale Funnel (explicit)."

[dependencies]
anyhow = "1"
axum = "0.7"
hyper = "1"
hyper-util = { version = "0.1", features = ["client", "client-legacy"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace"] }
tracing = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
async-trait = "0.1"
rand = "0.8"
hex = "0.4"

[dev-dependencies]
tempfile = "3"
reqwest = { version = "0.12", features = ["rustls-tls"], default-features = false }
```

Match the workspace's existing version pinning if newer/older versions are pinned at the workspace root. Run `grep -n '"axum"' Cargo.toml` at workspace root to check.

- [ ] **Step 2: Create `crates/vox-share/src/lib.rs` stub**

```rust
//! `vox share` — public-URL tunneling for Vox apps.
//!
//! Three backends:
//! - [`backends::lan`] — bind to `0.0.0.0`, no internet exposure (LAN-only)
//! - [`backends::cloudflare`] — Cloudflare Quick Tunnel via `*.trycloudflare.com` (default for `vox share`; added in S2)
//! - [`backends::localhost_run`] — SSH-based public URL via `*.lhr.life` (fallback; added in S3)
//! - [`backends::tailscale`] — Tailscale Funnel via `*.ts.net` (explicit; added in S4)
//!
//! See `RESEARCH.md` in this crate for primary-source notes on each provider's limits and ToS.

pub mod backend;
pub mod backends;
pub mod coordinator;
pub mod error;
pub mod proxy;
pub mod state;

pub use backend::{BackendKind, TunnelBackend, TunnelHandle};
pub use coordinator::{ShareConfig, ShareSession};
pub use error::{ShareError, ShareResult};
```

- [ ] **Step 3: Add to workspace members**

In the workspace root `Cargo.toml`, locate the `[workspace] members = [...]` array and add `"crates/vox-share"` (alphabetical position).

- [ ] **Step 4: Verify the workspace builds with the new (empty) crate**

Run: `cargo check -p vox-share 2>&1 | tail -10`

Expected: errors about missing modules (`backend`, `coordinator`, etc.) — this is correct since lib.rs declares them but they don't exist yet. We'll add them in subsequent tasks. The workspace itself should resolve the new crate.

If the workspace fails to load (e.g., "no targets to build" before module errors), the workspace registration is wrong — fix.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-share/Cargo.toml crates/vox-share/src/lib.rs Cargo.toml
git status
git commit -m "feat(share): scaffold vox-share crate (S1 task 1)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

Do NOT stage `.claude/settings.local.json`.

### Task 2: Define error type and backend trait

**Files:**
- Create: `crates/vox-share/src/error.rs`
- Create: `crates/vox-share/src/backend.rs`
- Test: `crates/vox-share/tests/backend_trait_test.rs`

- [ ] **Step 1: Write the failing test for the trait shape**

```rust
// crates/vox-share/tests/backend_trait_test.rs
//! Sanity test for the TunnelBackend trait shape.

use vox_share::{BackendKind, TunnelBackend};

#[test]
fn backend_kind_round_trips_via_str() {
    assert_eq!("lan".parse::<BackendKind>().unwrap(), BackendKind::Lan);
    assert_eq!("cloudflare".parse::<BackendKind>().unwrap(), BackendKind::Cloudflare);
    assert_eq!("localhost-run".parse::<BackendKind>().unwrap(), BackendKind::LocalhostRun);
    assert_eq!("tailscale".parse::<BackendKind>().unwrap(), BackendKind::Tailscale);
    assert!("frobnicate".parse::<BackendKind>().is_err());
}

#[test]
fn backend_kind_default_is_cloudflare() {
    assert_eq!(BackendKind::default(), BackendKind::Cloudflare);
}

/// Compile-time check that the trait is object-safe (we'll be Box<dyn TunnelBackend>-ing it).
fn _assert_object_safe(_: Box<dyn TunnelBackend>) {}
```

- [ ] **Step 2: Run, verify compile-fails**

Run: `cargo test -p vox-share --test backend_trait_test --no-run 2>&1 | tail -10`

Expected: errors about missing types (`BackendKind`, `TunnelBackend`, etc.).

- [ ] **Step 3: Create `error.rs`**

```rust
//! Error types for `vox share`.

use thiserror::Error;

pub type ShareResult<T> = Result<T, ShareError>;

#[derive(Debug, Error)]
pub enum ShareError {
    #[error("backend `{0}` is not available: {1}")]
    BackendUnavailable(&'static str, String),

    #[error("tunnel creation failed: {0}")]
    TunnelCreate(String),

    #[error("tunnel disconnected unexpectedly: {0}")]
    TunnelDisconnected(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("config: {0}")]
    Config(String),

    #[error("invalid backend: {0}")]
    InvalidBackend(String),
}
```

- [ ] **Step 4: Create `backend.rs`**

```rust
//! Backend trait + kind enum.

use crate::error::ShareResult;
use async_trait::async_trait;
use std::str::FromStr;
use std::time::Duration;

/// Which backend to use for the share session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// LAN-only: bind 0.0.0.0; no internet exposure.
    Lan,
    /// Cloudflare Quick Tunnel via `*.trycloudflare.com` (default for `vox share`).
    Cloudflare,
    /// SSH-based public URL via `*.lhr.life`.
    LocalhostRun,
    /// Tailscale Funnel via `*.ts.net` (requires Tailscale account + funnel enabled).
    Tailscale,
}

impl Default for BackendKind {
    fn default() -> Self {
        Self::Cloudflare
    }
}

impl FromStr for BackendKind {
    type Err = crate::error::ShareError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lan" => Ok(Self::Lan),
            "cloudflare" => Ok(Self::Cloudflare),
            "localhost-run" => Ok(Self::LocalhostRun),
            "tailscale" => Ok(Self::Tailscale),
            other => Err(crate::error::ShareError::InvalidBackend(other.to_string())),
        }
    }
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Lan => "lan",
            Self::Cloudflare => "cloudflare",
            Self::LocalhostRun => "localhost-run",
            Self::Tailscale => "tailscale",
        })
    }
}

/// A handle to an active tunnel session. Drop = shutdown.
#[derive(Debug)]
pub struct TunnelHandle {
    /// The public URL the user can share. For LAN backend this is `http://<lan-ip>:<port>`.
    pub public_url: String,
    /// Backend that produced this handle.
    pub backend: BackendKind,
    /// Hint about how stable this URL is across reconnects.
    pub url_stability: UrlStability,
    /// Shutdown channel — sender dropped triggers backend shutdown.
    shutdown: tokio::sync::oneshot::Sender<()>,
}

#[derive(Debug, Clone, Copy)]
pub enum UrlStability {
    /// Same URL every run for the same machine/account (Tailscale, registered cloudflared).
    Stable,
    /// New URL each run (Quick Tunnel, anonymous localhost.run).
    PerSession,
}

impl TunnelHandle {
    pub fn new(
        public_url: String,
        backend: BackendKind,
        url_stability: UrlStability,
        shutdown: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        Self { public_url, backend, url_stability, shutdown }
    }

    /// Trigger graceful shutdown. Idempotent.
    pub fn shutdown(self) {
        let _ = self.shutdown.send(());
    }
}

/// A backend creates and manages a tunnel from `127.0.0.1:<port>` to a public URL.
#[async_trait]
pub trait TunnelBackend: Send + Sync {
    fn kind(&self) -> BackendKind;

    /// Verify prerequisites (binary present, account authorized, etc.). Called before `start`.
    async fn preflight(&self) -> ShareResult<()>;

    /// Start the tunnel. Returns once the public URL is known and routable.
    /// `local_port` is the localhost port the backend should forward.
    /// `connect_timeout` is the max time to wait for the URL to become available.
    async fn start(
        &self,
        local_port: u16,
        connect_timeout: Duration,
    ) -> ShareResult<TunnelHandle>;
}
```

- [ ] **Step 5: Add empty modules referenced by `lib.rs`**

To make the trait test compile, create stub files for the modules `lib.rs` declares. Each is a single-line `// stub for S1 task N`:

- `crates/vox-share/src/coordinator.rs`:
  ```rust
  //! Stub. Implemented in Task 5.
  pub struct ShareConfig;
  pub struct ShareSession;
  ```
- `crates/vox-share/src/proxy.rs`:
  ```rust
  //! Stub. Implemented in Task 4.
  ```
- `crates/vox-share/src/state.rs`:
  ```rust
  //! Stub. Implemented in S2.
  ```
- `crates/vox-share/src/backends/mod.rs`:
  ```rust
  //! Stub. LAN backend in Task 3; Cloudflare in S2; localhost.run in S3; Tailscale in S4.
  ```

- [ ] **Step 6: Run the test**

Run: `cargo test -p vox-share --test backend_trait_test 2>&1 | tail -10`

Expected: 2 tests pass, plus the compile-time `_assert_object_safe` check passes.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-share/src/
git commit -m "feat(share): TunnelBackend trait + BackendKind enum + error types (S1 task 2)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

### Task 3: Implement the LAN backend

**Files:**
- Create: `crates/vox-share/src/backends/lan.rs`
- Modify: `crates/vox-share/src/backends/mod.rs` — `pub mod lan;`
- Test: `crates/vox-share/tests/lan_backend_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-share/tests/lan_backend_test.rs
//! LAN backend integration test. Spawns a tiny Axum server on a random port,
//! invokes the LAN backend (which is a no-op pass-through), verifies the URL.

use std::time::Duration;
use vox_share::backends::lan::LanBackend;
use vox_share::{BackendKind, TunnelBackend};

#[tokio::test]
async fn lan_backend_returns_lan_url_pointing_at_local_port() {
    let backend = LanBackend::new();

    backend.preflight().await.expect("LAN preflight should always succeed");

    let port = 7860u16;
    let handle = backend.start(port, Duration::from_secs(1)).await
        .expect("LAN backend start should succeed unconditionally");

    assert_eq!(handle.backend, BackendKind::Lan);
    // URL should be http (not https) and contain the port.
    assert!(handle.public_url.starts_with("http://"));
    assert!(handle.public_url.contains(&format!(":{}", port)));
    // For LAN we expect either `0.0.0.0` or a real LAN IP. Pre-MVP just check both possibilities.
    assert!(
        handle.public_url.contains("0.0.0.0")
            || handle.public_url.chars().filter(|c| *c == '.').count() == 3,
        "LAN URL should contain 0.0.0.0 or a dotted IP, got: {}",
        handle.public_url
    );

    handle.shutdown();
}

#[tokio::test]
async fn lan_backend_kind_is_lan() {
    let backend = LanBackend::new();
    assert_eq!(backend.kind(), BackendKind::Lan);
}
```

- [ ] **Step 2: Run, verify compile-fails**

Run: `cargo test -p vox-share --test lan_backend_test --no-run 2>&1 | tail -15`

Expected: compile errors about `vox_share::backends::lan::LanBackend` not found.

- [ ] **Step 3: Implement `lan.rs`**

```rust
//! LAN backend — bind 0.0.0.0, return a LAN URL.
//!
//! No actual tunneling: the bundled app should already be bound to 0.0.0.0 by
//! the coordinator (see Task 5). The backend's job here is to discover a routable
//! LAN IP for the user-facing URL and produce a [`TunnelHandle`].

use crate::backend::{BackendKind, TunnelBackend, TunnelHandle, UrlStability};
use crate::error::ShareResult;
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct LanBackend;

impl LanBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TunnelBackend for LanBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Lan
    }

    async fn preflight(&self) -> ShareResult<()> {
        Ok(())
    }

    async fn start(
        &self,
        local_port: u16,
        _connect_timeout: Duration,
    ) -> ShareResult<TunnelHandle> {
        let lan_ip = detect_lan_ip().unwrap_or_else(|| "0.0.0.0".to_string());
        let public_url = format!("http://{}:{}", lan_ip, local_port);
        let (tx, _rx) = tokio::sync::oneshot::channel();
        // LAN backend has no background task; the rx is dropped immediately.
        // Coordinator owns the actual server bind; this handle is informational.
        Ok(TunnelHandle::new(
            public_url,
            BackendKind::Lan,
            UrlStability::Stable,
            tx,
        ))
    }
}

/// Best-effort discovery of a routable LAN IPv4 address.
///
/// Strategy: open a UDP socket to a public IP; the OS picks a routable local
/// address as the source. This works without sending any packets and without
/// requiring an actual route to the destination.
fn detect_lan_ip() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let local_addr = sock.local_addr().ok()?;
    let ip = local_addr.ip();
    if ip.is_unspecified() || ip.is_loopback() {
        None
    } else {
        Some(ip.to_string())
    }
}
```

- [ ] **Step 4: Wire into the backends module**

In `crates/vox-share/src/backends/mod.rs`:

```rust
//! Backends: LAN (S1), Cloudflare (S2), localhost.run (S3), Tailscale (S4).

pub mod lan;
```

- [ ] **Step 5: Run the test**

Run: `cargo test -p vox-share --test lan_backend_test 2>&1 | tail -10`

Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-share/src/backends/
git commit -m "feat(share): LAN backend (S1 task 3)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

### Task 4: Implement the reverse-proxy server

**Files:**
- Modify: `crates/vox-share/src/proxy.rs`
- Test: `crates/vox-share/tests/proxy_test.rs`

The proxy is an Axum server that accepts public requests, optionally enforces auth (in S5), and forwards to the inner bundled app. In S1, the proxy is pass-through (no auth). Future phases bolt middleware onto this.

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-share/tests/proxy_test.rs
//! Reverse-proxy round-trip test. Spawns a tiny upstream Axum server, then
//! a `vox-share` proxy in front of it, sends a request through the proxy,
//! verifies the upstream's body comes back unchanged.

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use vox_share::proxy::ProxyConfig;

#[tokio::test]
async fn proxy_forwards_get_request_body_unchanged() {
    // Spawn upstream
    let upstream = Router::new().route("/hello", get(|| async { "hello-from-upstream" }));
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_port = upstream_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(upstream_listener, upstream).await.unwrap();
    });

    // Spawn proxy
    let cfg = ProxyConfig {
        upstream_addr: SocketAddr::from(([127, 0, 0, 1], upstream_port)),
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
    };
    let proxy_listener = TcpListener::bind(cfg.bind_addr).await.unwrap();
    let proxy_port = proxy_listener.local_addr().unwrap().port();
    let proxy_app = vox_share::proxy::build_app(cfg.clone());
    tokio::spawn(async move {
        axum::serve(proxy_listener, proxy_app).await.unwrap();
    });

    // Wait briefly for both servers to be ready.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Hit the proxy.
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/hello", proxy_port))
        .send()
        .await
        .expect("proxy should accept the request");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "hello-from-upstream");
}
```

- [ ] **Step 2: Run, verify compile-fails**

Run: `cargo test -p vox-share --test proxy_test --no-run 2>&1 | tail -15`

Expected: compile errors about `ProxyConfig`, `build_app`.

- [ ] **Step 3: Implement `proxy.rs`**

```rust
//! Reverse-proxy Axum server in front of the bundled Vox app.
//!
//! Adds a single hop on localhost so we can layer middleware (auth, logging,
//! request shaping) without touching the codegen'd app's `main.rs`. The tunnel
//! backend forwards to *this* proxy's port, not the bundled app's port directly.
//!
//! S1 ships the pass-through baseline. Future phases:
//! - S5: layer auth middleware (URL-token + basic-auth).
//! - S6: SSE-route detection / warning.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{StatusCode, Uri};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct ProxyConfig {
    /// Address of the upstream bundled Vox app.
    pub upstream_addr: SocketAddr,
    /// Where the proxy listens. The tunnel backend points at this address.
    pub bind_addr: SocketAddr,
}

#[derive(Clone)]
struct ProxyState {
    upstream_addr: SocketAddr,
    client: Client<hyper_util::client::legacy::connect::HttpConnector, Body>,
}

pub fn build_app(cfg: ProxyConfig) -> Router {
    let client = Client::builder(TokioExecutor::new())
        .build_http();
    let state = ProxyState {
        upstream_addr: cfg.upstream_addr,
        client,
    };
    Router::new().fallback(any(forward)).with_state(state)
}

async fn forward(
    axum::extract::State(state): axum::extract::State<ProxyState>,
    mut req: Request,
) -> Result<Response, (StatusCode, String)> {
    let path_and_query = req.uri()
        .path_and_query()
        .map(|x| x.as_str())
        .unwrap_or("/");
    let target = format!("http://{}{}", state.upstream_addr, path_and_query);
    *req.uri_mut() = Uri::try_from(target)
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("bad upstream URI: {}", e)))?;

    state.client
        .request(req)
        .await
        .map(|resp| resp.map(Body::new))
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("upstream error: {}", e)))
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p vox-share --test proxy_test 2>&1 | tail -15`

Expected: test passes. If hyper-util client API differs from this code, adapt — the integration shape is what matters.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-share/src/proxy.rs crates/vox-share/tests/proxy_test.rs
git commit -m "feat(share): reverse-proxy server (S1 task 4)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

### Task 5: Implement the coordinator

**Files:**
- Modify: `crates/vox-share/src/coordinator.rs`
- Test: `crates/vox-share/tests/coordinator_test.rs`

The coordinator is the conductor: it spawns the bundled app on a private port, spawns the proxy on a public-side port, asks the chosen `TunnelBackend` to expose the proxy, and orchestrates shutdown.

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-share/tests/coordinator_test.rs
//! Coordinator integration test using the LAN backend (zero infrastructure).

use std::path::PathBuf;
use std::time::Duration;
use vox_share::{BackendKind, ShareConfig};

#[tokio::test]
async fn coordinator_starts_lan_session_against_a_dummy_app() {
    // Build a tiny "app" binary on the fly: use a shell script (or echo) that
    // serves a static response. For the test, we'll skip the bundled-app
    // child and instead point the coordinator at an already-running upstream.
    //
    // (S8 wires up real `vox bundle` integration; for S1 the coordinator
    // is decoupled from binary lifecycle.)

    // Spawn a tiny upstream so the proxy has something to forward to.
    let upstream = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
    let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_port = upstream_listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(upstream_listener, upstream).await.unwrap(); });

    let cfg = ShareConfig {
        backend: BackendKind::Lan,
        upstream_port,
        proxy_port: 0, // OS-pick
        duration: Some(Duration::from_secs(2)),
        app_binary: None, // already running externally for this test
        connect_timeout: Duration::from_secs(2),
    };
    let session = vox_share::ShareSession::start(cfg).await
        .expect("LAN session should start");

    assert_eq!(session.tunnel_handle.backend, BackendKind::Lan);
    assert!(session.tunnel_handle.public_url.starts_with("http://"));
    assert!(session.tunnel_handle.public_url.contains(&format!(":{}", session.proxy_port)));

    // Hit the public URL — proxy forwards to upstream — gets "ok".
    // (For LAN backend the URL is whatever LAN IP we found; we test via the proxy_port directly.)
    let resp = reqwest::get(format!("http://127.0.0.1:{}/", session.proxy_port))
        .await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");

    session.shutdown().await;
}
```

- [ ] **Step 2: Run, verify compile-fails**

Run: `cargo test -p vox-share --test coordinator_test --no-run 2>&1 | tail -15`

Expected: compile errors about `ShareConfig`, `ShareSession`, etc.

- [ ] **Step 3: Implement `coordinator.rs`**

```rust
//! Coordinator: spawns app + proxy + tunnel-backend together.

use crate::backend::{BackendKind, TunnelBackend, TunnelHandle};
use crate::backends::lan::LanBackend;
use crate::error::{ShareError, ShareResult};
use crate::proxy::{build_app as build_proxy_app, ProxyConfig};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpListener;

/// Configuration for a share session.
#[derive(Debug, Clone)]
pub struct ShareConfig {
    pub backend: BackendKind,
    /// The localhost port the bundled Vox app is listening on.
    pub upstream_port: u16,
    /// Where to bind the proxy (0 = OS-pick).
    pub proxy_port: u16,
    /// Auto-shutdown after this duration. None = unbounded.
    pub duration: Option<Duration>,
    /// Path to the bundled app binary. None = assume already running.
    pub app_binary: Option<PathBuf>,
    /// Time to wait for the tunnel to come up.
    pub connect_timeout: Duration,
}

/// An active share session. Drop or call `shutdown` to clean up.
pub struct ShareSession {
    pub tunnel_handle: TunnelHandle,
    pub proxy_port: u16,
    proxy_shutdown: tokio::sync::oneshot::Sender<()>,
    duration_timer: Option<tokio::task::JoinHandle<()>>,
    /// Hold the app child process if we spawned one; drop = kill.
    _app_child: Option<tokio::process::Child>,
}

impl ShareSession {
    pub async fn start(cfg: ShareConfig) -> ShareResult<Self> {
        let app_child = if let Some(path) = &cfg.app_binary {
            let child = tokio::process::Command::new(path)
                .env("PORT", cfg.upstream_port.to_string())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| ShareError::Config(format!("spawn app binary: {}", e)))?;
            Some(child)
        } else {
            None
        };

        // Bind the proxy listener.
        let bind_addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], cfg.proxy_port));
        let listener = TcpListener::bind(bind_addr).await?;
        let actual_proxy_port = listener.local_addr()?.port();

        let proxy_cfg = ProxyConfig {
            upstream_addr: SocketAddr::from(([127, 0, 0, 1], cfg.upstream_port)),
            bind_addr,
        };
        let proxy_app = build_proxy_app(proxy_cfg);

        let (proxy_shutdown_tx, proxy_shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::serve(listener, proxy_app);
            tokio::select! {
                res = server => { let _ = res; }
                _ = proxy_shutdown_rx => { /* graceful */ }
            }
        });

        // Bring up the tunnel.
        let backend: Box<dyn TunnelBackend> = make_backend(cfg.backend);
        backend.preflight().await?;
        let tunnel_handle = backend.start(actual_proxy_port, cfg.connect_timeout).await?;

        // Optional auto-shutdown timer.
        let duration_timer = cfg.duration.map(|d| {
            tokio::spawn(async move {
                tokio::time::sleep(d).await;
            })
        });

        Ok(ShareSession {
            tunnel_handle,
            proxy_port: actual_proxy_port,
            proxy_shutdown: proxy_shutdown_tx,
            duration_timer,
            _app_child: app_child,
        })
    }

    pub async fn shutdown(self) {
        // Trigger tunnel shutdown.
        self.tunnel_handle.shutdown();
        // Cancel duration timer.
        if let Some(t) = self.duration_timer { t.abort(); }
        // Stop proxy.
        let _ = self.proxy_shutdown.send(());
        // Yield so spawned tasks observe shutdown.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn make_backend(kind: BackendKind) -> Box<dyn TunnelBackend> {
    match kind {
        BackendKind::Lan => Box::new(LanBackend::new()),
        // S2-S4 add the rest. Until then, error path:
        BackendKind::Cloudflare | BackendKind::LocalhostRun | BackendKind::Tailscale => {
            // Compile-time reachable but runtime-unreachable in S1: the CLI gates this in S2+.
            unimplemented!("backend {:?} ships in a later phase", kind)
        }
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p vox-share --test coordinator_test 2>&1 | tail -20`

Expected: test passes.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-share/src/coordinator.rs crates/vox-share/tests/coordinator_test.rs
git commit -m "feat(share): coordinator (S1 task 5)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

### Task 6: Wire `vox share` CLI subcommand (LAN-only for S1)

**Files:**
- Create: `crates/vox-cli/src/commands/share.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs` — `pub mod share;`
- Modify: `crates/vox-cli/src/main.rs` (or wherever clap dispatch lives) — register subcommand
- Modify: `crates/vox-cli/Cargo.toml` — add `vox-share = { path = "../vox-share" }`
- Test: `crates/vox-cli/tests/share_cli_test.rs`

- [ ] **Step 1: Write the failing CLI smoke test**

```rust
// crates/vox-cli/tests/share_cli_test.rs
use std::process::Command;

#[test]
fn share_help_lists_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["share", "--help"])
        .output()
        .expect("vox binary should be runnable");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("share"), "help output should reference share: {}", stdout);
    assert!(
        stdout.contains("public") || stdout.contains("tunnel") || stdout.contains("LAN")
            || stdout.contains("URL"),
        "help should describe what share does: {}", stdout
    );
}

#[test]
fn share_help_lists_backend_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["share", "--help"])
        .output()
        .expect("vox binary should be runnable");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--backend"),
        "help should include --backend flag: {}", stdout);
}
```

- [ ] **Step 2: Run, verify FAIL**

Run: `cargo test -p vox-cli --test share_cli_test 2>&1 | tail -10`

Expected: FAIL — `share` not a known subcommand.

- [ ] **Step 3: Add the dependency**

In `crates/vox-cli/Cargo.toml`, add `vox-share = { path = "../vox-share" }` to `[dependencies]`.

- [ ] **Step 4: Create `share.rs`**

```rust
//! `vox share` — public-URL tunnel for Vox apps.
//!
//! S1: LAN backend only. S2 adds Cloudflare default. S3 adds localhost.run fallback.

use anyhow::Result;
use clap::Args;
use std::time::Duration;
use vox_share::{BackendKind, ShareConfig, ShareSession};

#[derive(Args, Debug)]
pub struct ShareArgs {
    /// Tunnel backend. Default: cloudflare (Cloudflare Quick Tunnel; in S2+).
    /// Available: lan (S1), cloudflare (S2), localhost-run (S3), tailscale (S4).
    #[arg(long, default_value = "lan")]
    pub backend: String,

    /// Port to bind the bundled app. Defaults to 7860 (Gradio convention).
    #[arg(long, default_value = "7860")]
    pub port: u16,

    /// Auto-shutdown after this duration (e.g. "8h", "30m", "none"). Default: 8h.
    #[arg(long, default_value = "8h")]
    pub duration: String,

    /// Use dev server pipeline instead of `vox bundle` (faster iteration; not production-shape).
    #[arg(long)]
    pub dev: bool,
}

pub async fn run(args: ShareArgs) -> Result<()> {
    let backend: BackendKind = args.backend.parse()
        .map_err(|e| anyhow::anyhow!("invalid --backend `{}`: {}", args.backend, e))?;

    let duration = parse_duration(&args.duration)?;

    println!("[vox share] Starting share session...");
    println!("[vox share] Backend: {}", backend);

    // S8 wires up real bundle integration. For S1, document the gap and refuse if app_binary
    // would be needed. LAN backend doesn't need a binary if the user has one running, but
    // for the CLI the expected flow is "vox share runs my app" — that's S8.
    // For S1, error helpfully:
    if !matches!(backend, BackendKind::Lan) {
        anyhow::bail!(
            "backend `{}` ships in a later phase. Use `--backend lan` for now.",
            backend
        );
    }

    // S1 stub: there's no app to spawn yet (S8). Print a helpful note and exit.
    println!("[vox share] (S1) LAN backend ready, but no app pipeline yet.");
    println!("[vox share] Pipeline integration ships in S8.");
    println!("[vox share] For now, run your bundled binary on port {} and share will proxy in S5+.", args.port);

    let cfg = ShareConfig {
        backend,
        upstream_port: args.port,
        proxy_port: 0,
        duration,
        app_binary: None,
        connect_timeout: Duration::from_secs(10),
    };

    let session = ShareSession::start(cfg).await
        .map_err(|e| anyhow::anyhow!("share session start: {}", e))?;

    println!("[vox share] Public URL: {}", session.tunnel_handle.public_url);
    println!("[vox share] Local: http://127.0.0.1:{}", session.proxy_port);
    println!("[vox share] Press Ctrl+C to stop.");

    // Wait for Ctrl+C or duration timeout.
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("[vox share] Shutdown requested.");
        }
        _ = async {
            if let Some(d) = duration {
                tokio::time::sleep(d).await;
            } else {
                std::future::pending::<()>().await;
            }
        } => {
            println!("[vox share] Duration elapsed; shutting down.");
        }
    }

    session.shutdown().await;
    println!("[vox share] Done.");
    Ok(())
}

fn parse_duration(s: &str) -> Result<Option<Duration>> {
    if s == "none" { return Ok(None); }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: u64 = num.parse().map_err(|_| anyhow::anyhow!("bad duration `{}`", s))?;
    Ok(Some(match unit {
        "s" => Duration::from_secs(n),
        "m" => Duration::from_secs(n * 60),
        "h" => Duration::from_secs(n * 3600),
        _ => anyhow::bail!("duration unit must be s/m/h or `none`, got `{}`", unit),
    }))
}
```

- [ ] **Step 5: Register the subcommand**

In `crates/vox-cli/src/commands/mod.rs`, add `pub mod share;` next to the other module declarations.

In the clap-derived enum (likely in `crates/vox-cli/src/main.rs` or `lib.rs` — check Task 5 of VUV-9 plan for how `migrate` was wired), add:

```rust
Share(commands::share::ShareArgs),
```

In the dispatch match:

```rust
Command::Share(args) => commands::share::run(args).await?,
```

- [ ] **Step 6: Run the smoke tests**

Run: `cargo build -p vox-cli 2>&1 | tail -10`
Expected: clean build.

Run: `cargo test -p vox-cli --test share_cli_test 2>&1 | tail -10`
Expected: 2 tests pass.

Run manually: `cargo run -p vox-cli -- share --help 2>&1 | head -20`
Expected: clap help showing `--backend`, `--port`, `--duration`, `--dev`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/share.rs crates/vox-cli/src/commands/mod.rs \
        crates/vox-cli/src/main.rs crates/vox-cli/Cargo.toml \
        crates/vox-cli/tests/share_cli_test.rs
git status
git commit -m "feat(cli): vox share subcommand skeleton (S1 task 6)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

### Task 7: Add the research dossier as in-tree reference

**Files:**
- Create: `crates/vox-share/RESEARCH.md`

The research from this brainstorming session is load-bearing for design decisions. Future maintainers need to find it without spelunking through git history.

- [ ] **Step 1: Create `RESEARCH.md`**

Write a 200-300-word distilled version of the tunnel-options research with:
- Cloudflare Quick Tunnels: 200 in-flight cap, no SSE, no URL stability, Apache-2.0 cloudflared, no pure-Rust client
- localhost.run: SSH-based, no client to ship, speed-throttled, complementary failure surface
- Tailscale Funnel: account required, stable URL, Personal-plan free, only ports 443/8443/10000
- Disqualified options: bore (TCP only), playit/ngrok/zrok (account-gated), localtunnel (unmaintained), serveo (flaky)
- Future direction: Vox-hosted FRP relay (HuggingFace's `gradio.live` model)

Cite primary sources inline.

- [ ] **Step 2: Commit**

```bash
git add crates/vox-share/RESEARCH.md
git commit -m "docs(share): research dossier on tunnel options (S1 task 7)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

### S1 acceptance gate

- [ ] `cargo test -p vox-share` — all tests pass
- [ ] `cargo test -p vox-cli --test share_cli_test` — passes
- [ ] `cargo run -p vox-cli -- share --backend lan --duration 5s` — produces a LAN URL, exits cleanly after 5s
- [ ] `cargo run -p vox-cli -- share --help` — informative help output
- [ ] No regressions in pre-existing test failures (the `bug_a_match_arms_repro.rs` and `web_ir_lower_emit_test.rs` failures from VUV-9 are pre-existing and unrelated)

---

## Phase S2 — Cloudflare backend + cloudflared lazy-download + first-run consent

**Goal:** Default `vox share` produces a working `*.trycloudflare.com` URL. cloudflared binary is downloaded lazily on first use, SHA256-verified, cached. First run shows a `[Y/n]` consent banner; subsequent runs are silent.

### Task 1: Implement `binary_cache.rs` for cloudflared download + verify

**Files:**
- Create: `crates/vox-share/src/binary_cache.rs`
- Modify: `crates/vox-share/src/lib.rs` — `pub mod binary_cache;`
- Test: `crates/vox-share/tests/binary_cache_test.rs`

The cache lives at `${XDG_CACHE_HOME:-$HOME/.cache}/vox/cloudflared/cloudflared-{version}-{os}-{arch}{.exe}`. Pinned version per platform; checksum hardcoded in source.

- [ ] **Step 1: Define the pinned version**

Hardcode in source:

```rust
/// Pinned cloudflared release we download.
/// To update: pick a release at https://github.com/cloudflare/cloudflared/releases,
/// download all platforms, run `sha256sum` on each, paste below.
pub const CLOUDFLARED_VERSION: &str = "2026.04.0";

pub fn cloudflared_url_and_checksum(os: &str, arch: &str) -> Option<(String, &'static str)> {
    // (filename, sha256) per (os, arch)
    let (filename, checksum) = match (os, arch) {
        ("linux", "x86_64") =>
            ("cloudflared-linux-amd64", "<TODO-FILL-IN-FROM-RELEASE>"),
        ("linux", "aarch64") =>
            ("cloudflared-linux-arm64", "<TODO-FILL-IN-FROM-RELEASE>"),
        ("macos", "x86_64") =>
            ("cloudflared-darwin-amd64.tgz", "<TODO-FILL-IN-FROM-RELEASE>"),
        ("macos", "aarch64") =>
            ("cloudflared-darwin-arm64.tgz", "<TODO-FILL-IN-FROM-RELEASE>"),
        ("windows", "x86_64") =>
            ("cloudflared-windows-amd64.exe", "<TODO-FILL-IN-FROM-RELEASE>"),
        _ => return None,
    };
    let url = format!(
        "https://github.com/cloudflare/cloudflared/releases/download/{}/{}",
        CLOUDFLARED_VERSION, filename
    );
    Some((url, checksum))
}
```

The `<TODO-FILL-IN-FROM-RELEASE>` placeholders MUST be replaced with real SHA256 hashes from `https://github.com/cloudflare/cloudflared/releases/download/{VERSION}/sha256sums.txt` before merge. The CI smoke test in S2 task 5 verifies the recorded checksum matches Cloudflare's published sums.

- [ ] **Step 2: Implement download/verify/cache**

```rust
pub async fn ensure_cloudflared() -> ShareResult<PathBuf> {
    let cache_dir = cache_dir()?;
    std::fs::create_dir_all(&cache_dir)?;

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let (url, expected_sha) = cloudflared_url_and_checksum(os, arch)
        .ok_or_else(|| ShareError::Config(format!("unsupported platform {}-{}", os, arch)))?;

    let bin_name = format!("cloudflared-{}-{}-{}{}",
        CLOUDFLARED_VERSION, os, arch,
        if os == "windows" { ".exe" } else { "" });
    let bin_path = cache_dir.join(&bin_name);

    if bin_path.exists() && verify_sha256(&bin_path, expected_sha)? {
        return Ok(bin_path);
    }

    download_and_verify(&url, expected_sha, &bin_path).await?;
    set_executable(&bin_path)?;
    Ok(bin_path)
}

fn cache_dir() -> ShareResult<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".cache")))
        .ok_or_else(|| ShareError::Config("could not determine cache dir".into()))?;
    Ok(base.join("vox").join("cloudflared"))
}

fn verify_sha256(path: &Path, expected: &str) -> ShareResult<bool> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex::encode(hasher.finalize());
    Ok(actual.eq_ignore_ascii_case(expected))
}

async fn download_and_verify(url: &str, expected_sha: &str, dest: &Path) -> ShareResult<()> {
    let resp = reqwest::get(url).await
        .map_err(|e| ShareError::Config(format!("download {}: {}", url, e)))?;
    if !resp.status().is_success() {
        return Err(ShareError::Config(format!(
            "download {}: HTTP {}", url, resp.status())));
    }
    let bytes = resp.bytes().await
        .map_err(|e| ShareError::Config(format!("read body: {}", e)))?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha) {
        return Err(ShareError::Config(format!(
            "SHA256 mismatch: expected {}, got {}", expected_sha, actual)));
    }
    std::fs::write(dest, &bytes)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> ShareResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(path)?.permissions();
    perm.set_mode(perm.mode() | 0o111);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}
#[cfg(not(unix))]
fn set_executable(_: &Path) -> ShareResult<()> { Ok(()) }
```

Add `sha2 = "0.10"`, `hex = "0.4"`, `reqwest = { version = "0.12", features = ["rustls-tls"], default-features = false }`, `dirs = "5"` to `crates/vox-share/Cargo.toml`.

- [ ] **Step 3: Test against a controlled fixture**

In `crates/vox-share/tests/binary_cache_test.rs`, write a test that uses `wiremock` (or a simple `axum` server in the test) to serve a known-checksum binary, then verify `ensure_cloudflared`-style logic accepts it and rejects a tampered version. Use `XDG_CACHE_HOME` env override to point at a tempdir.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-share/src/binary_cache.rs crates/vox-share/src/lib.rs \
        crates/vox-share/Cargo.toml crates/vox-share/tests/binary_cache_test.rs
git commit -m "feat(share): cloudflared binary cache with SHA256 verify (S2 task 1)"
```

### Task 2: Implement Cloudflare backend

**Files:**
- Create: `crates/vox-share/src/backends/cloudflare.rs`
- Test: `crates/vox-share/tests/cloudflare_backend_test.rs`

- [ ] **Step 1: Subprocess + URL parse**

Spawn `cloudflared tunnel --url http://127.0.0.1:<port> --no-autoupdate` and parse the URL from stderr (cloudflared writes the URL to stderr in lines like `INF +--------------------------------------+\nINF | https://random-otter.trycloudflare.com |\nINF +--------------------------------------+`).

Regex: `https://[a-z0-9-]+\.trycloudflare\.com`

Implementation pattern: tokio::process::Command::new + AsyncBufReadExt over stderr, scan for the URL, surface via oneshot::Sender, hold the child until `TunnelHandle::shutdown` triggers `child.kill()`.

- [ ] **Step 2: Test with a mock cloudflared binary**

Create a tiny shell script (or Rust binary) that mimics cloudflared's URL-printing behavior, set `VOX_CLOUDFLARED_PATH` env override to point at it, and verify the backend parses the URL. The mock prints a fake URL after a short delay; the test asserts the URL is captured.

- [ ] **Step 3: Add `VOX_CLOUDFLARED_PATH` override to `binary_cache.rs`** for testability:

```rust
pub async fn ensure_cloudflared() -> ShareResult<PathBuf> {
    if let Ok(custom) = std::env::var("VOX_CLOUDFLARED_PATH") {
        let p = PathBuf::from(custom);
        if !p.exists() { return Err(ShareError::Config(format!("VOX_CLOUDFLARED_PATH does not exist: {}", p.display()))); }
        return Ok(p);
    }
    // ...existing logic
}
```

- [ ] **Step 4: Commit**

### Task 3: Implement first-run consent banner

**Files:**
- Create: `crates/vox-share/src/consent.rs`
- Modify: `crates/vox-share/src/state.rs` — `~/.config/vox/share-state.json` reader/writer

- [ ] **Step 1: Define the state shape**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShareState {
    /// Has the user accepted the Cloudflare ToS / public-exposure notice?
    pub cloudflare_consent_v1: bool,
    /// Version of the consent text accepted (for re-prompting on policy changes).
    pub consent_text_version: u32,
}
```

- [ ] **Step 2: Implement the banner**

If `state.cloudflare_consent_v1 == false`, print the banner from S0 design (downloading + ToS notice + URL exposure) and read a single line from stdin. Anything that doesn't start with `n` or `N` is acceptance. Save state + proceed.

If stdin is not a TTY (CI, piped input), require an `--accept-tos` flag to proceed. Don't deadlock waiting on stdin.

- [ ] **Step 3: Commit**

### Task 4: Wire Cloudflare backend into the coordinator

Update `make_backend()` in `coordinator.rs`:

```rust
fn make_backend(kind: BackendKind) -> Box<dyn TunnelBackend> {
    match kind {
        BackendKind::Lan => Box::new(LanBackend::new()),
        BackendKind::Cloudflare => Box::new(CloudflareBackend::new()),
        BackendKind::LocalhostRun | BackendKind::Tailscale => unimplemented!(),
    }
}
```

Update CLI default: `#[arg(long, default_value = "cloudflare")]`.

### Task 5: CI smoke test for checksum drift

Add `crates/vox-share/tests/cloudflared_checksum_drift_test.rs` that fetches `https://github.com/cloudflare/cloudflared/releases/download/{VERSION}/sha256sums.txt`, parses it, and compares against the hardcoded values in `binary_cache.rs`. Test marked `#[ignore]` by default; CI runs with `--ignored`. Catches the case where someone bumps `CLOUDFLARED_VERSION` without updating checksums.

### S2 acceptance gate

- [ ] `cargo test -p vox-share` — all tests pass (LAN + Cloudflare + binary cache + consent)
- [ ] `cargo run -p vox-cli -- share --backend cloudflare --duration 30s` (manual; cannot fully automate without internet) — produces a `*.trycloudflare.com` URL, the URL is reachable from a phone, exits cleanly after 30s
- [ ] `vox share` (no flag) defaults to Cloudflare
- [ ] First-run prompts for consent; second run is silent
- [ ] CI checksum drift test passes against current cloudflared release

---

## Phase S3 — localhost.run backend + automatic fallback

**Goal:** When Cloudflare backend fails (network error, ToS-block, regional incident), automatically retry with localhost.run. Also exposed as explicit `--backend localhost-run`.

### Task 1: Detect stock SSH

Per platform: Linux/macOS check `which ssh`. Windows: check both `ssh.exe` in PATH (bundled with Windows 10 1803+) and provide actionable error if absent ("Install OpenSSH client: Settings → Apps → Optional Features → Add → OpenSSH Client").

### Task 2: Implement localhost.run backend

Spawn `ssh -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=60 -R 80:localhost:<port> nokey@localhost.run`. Parse the URL from stdout (line like `Connect to https://random-otter.lhr.life or...`). On reconnect, the URL changes; emit a notice and update `TunnelHandle.public_url` (but the original Sender is invalidated — caller must reacquire).

For S3 v1, don't auto-reconnect; if the SSH connection drops, emit `TunnelDisconnected` and let coordinator decide.

### Task 3: Implement fallback chain

In coordinator: when `BackendKind::Cloudflare` is selected and `start()` returns `TunnelCreate` or `BackendUnavailable`, retry with `LocalhostRun`. Print a notice: `[vox share] Cloudflare backend unavailable: <reason>; falling back to localhost.run`.

Disable fallback if `--backend X` was explicitly set (vs defaulted) — exposed via a `ShareConfig.allow_fallback: bool` field defaulting to `true` when CLI uses default backend, `false` when CLI passes explicit `--backend`.

### Task 4: Tests

- Mock SSH harness for backend test (just like cloudflared mock).
- Coordinator test: register a "always-fails" Cloudflare mock, verify fallback to LAN/localhost-run works.

### S3 acceptance gate

- [ ] `vox share --backend localhost-run` produces an `lhr.life` URL
- [ ] Default `vox share` with Cloudflare disabled (force-fail mock) auto-falls-back to localhost.run
- [ ] Explicit `--backend cloudflare` does NOT fallback (errors out instead)

---

## Phase S4 — Tailscale Funnel backend (explicit only)

**Goal:** `vox share --backend tailscale` exposes the proxy on the user's `*.ts.net` URL via Tailscale Funnel.

**Files involved:**
- Create: `crates/vox-share/src/backends/tailscale.rs`

**Approach:**
- Detect `tailscale` CLI; helpful error if absent ("Install Tailscale: https://tailscale.com/download")
- Run `tailscale serve --bg https http://127.0.0.1:<port>` (set up a serve), then `tailscale funnel <port> on` (enable public exposure on port 443).
- Discover the user's `tailnet` name via `tailscale status --json` and synthesize the URL `https://<machine-name>.<tailnet>.ts.net`
- Preflight: verify Funnel is enabled in the user's tailnet (HTTP probe of admin API or parse `tailscale funnel status`)

**Why a separate plan:** Tailscale's CLI surface is rich and changes; preflight (account state, funnel-enabled, port restrictions to 443/8443/10000) needs careful error messages. ~10-15 TDD steps.

**Trigger to write the plan:** S3 lands.

---

## Phase S5 — Auth middleware

**Goal:** Default `vox share` URL embeds a token (`?vox_share_token=<random16>`); without it, requests get 401. `--auth basic:user:pass` upgrades to HTTP basic. `--auth none` opts out.

**Files involved:**
- Create: `crates/vox-share/src/auth.rs`
- Modify: `crates/vox-share/src/proxy.rs` — accept an `AuthLayer` parameter

**Approach:**
- AuthMode enum: `None`, `UrlToken(String)`, `Basic(String, String)`
- Token: 16 random hex chars from `rand::thread_rng()`
- Middleware: `tower::Layer` impl, returns 401 with `WWW-Authenticate: Basic realm="vox share"` for the basic-auth path; for token, checks `?vox_share_token=` query param (or `Cookie: vox_share_token=...` to handle browser SPA navigation), 401 with a tiny HTML page on miss
- URL synthesis: when token mode, the public URL passed to user includes `?vox_share_token=<token>`

**Why a separate plan:** middleware ordering with the existing proxy layer needs care; the auth layer must come BEFORE the proxy so 401 short-circuits without hitting upstream. ~10 TDD steps.

**Trigger to write the plan:** S3 lands.

---

## Phase S6 — SSE detection + auto-switch backend

**Goal:** On `vox share --backend cloudflare` (or default), scan the bundled binary's OpenAPI spec (already emitted by Vox per [`crates/vox-codegen/src/codegen_ts/openapi_emit.rs`]) for routes that produce `text/event-stream`. If any present, print `[vox share] App uses streaming; auto-selected --backend localhost-run for SSE compatibility` and switch.

**Files involved:**
- Create: `crates/vox-share/src/sse_detect.rs`
- Modify: `crates/vox-share/src/coordinator.rs`

**Approach:**
- The bundled binary is built by `vox bundle`. The OpenAPI spec lives at a known path (e.g., `target/generated/openapi.json` or wherever `vox emit openapi` puts it — check existing emitter)
- Scan: `responses[*].content["text/event-stream"]` is the JSON path
- Override flag: `--allow-buffered-streaming` to suppress auto-switch (power user)

**Why a separate plan:** the OpenAPI scan touches existing emit pipeline; integration with `vox bundle` flow needs care. ~6 TDD steps.

**Trigger to write the plan:** S5 lands.

---

## Phase S7 — Duration + auto-shutdown + countdown

**Goal:** `--duration 8h` (default), `--duration none`, countdown printed every minute when remaining time < 1h, every 5 minutes otherwise.

**Files involved:**
- Create: `crates/vox-share/src/lifecycle.rs`

**Approach:**
- `tokio::time::sleep` for the bound; signal channel to coordinator on expiry
- Countdown: separate task printing `[vox share] {N} remaining` on a tickle interval
- Graceful shutdown: SIGTERM to child processes (if possible), wait briefly, then SIGKILL fallback

**Why a separate plan:** graceful child-process shutdown is platform-dependent; Windows process trees need special handling. ~8 TDD steps.

**Trigger to write the plan:** S6 lands.

---

## Phase S8 — Bundle/dev integration

**Goal:** `vox share` defaults to running `vox bundle` first (cached), `--dev` uses dev pipeline. The coordinator spawns the resulting binary as a child process.

**Files involved:**
- Modify: `crates/vox-cli/src/commands/share.rs` — call into `vox bundle` pipeline
- Modify: `crates/vox-share/src/coordinator.rs` — accept the binary path from CLI

**Approach:**
- Cold cache: run `vox bundle`, write artifact to `target/share-bundle/<app-name>{.exe}`
- Warm cache: hash `.vox` source tree; if hash matches `target/share-bundle/.last-hash`, reuse
- `--dev`: invoke whatever `vox dev` currently does (find in [crates/vox-cli/src/commands/dev.rs])
- Spawn binary with `PORT=<chosen>` env var (matches existing Axum codegen behavior)

**Why a separate plan:** the bundle hash strategy needs care to avoid stale builds; integration with the existing `vox bundle` command should reuse its codepath rather than duplicating. ~10 TDD steps.

**Trigger to write the plan:** S7 lands.

---

## Phase S9 — Docs + abuse-policy + safety guide

**Goal:** mdBook how-to page, abuse/ToS reference doc, prominent safety warnings in CLI help.

**Files involved:**
- Create: `docs/src/how-to/how-to-share.md` — user-facing how-to
- Create: `docs/src/architecture/share-policy-2026.md` — abuse policy, ToS references, takedown-contact policy
- Modify: CLI help text in `share.rs` to link to the docs

**Approach:**
- How-to: cover all three backends, auth modes, duration, common pitfalls (SSE/Cloudflare, corporate SWGs blocking trycloudflare.com, public-link safety)
- Policy: cite Cloudflare ToS, localhost.run free-tier terms, Tailscale Funnel terms; describe how Vox responds to abuse complaints (we don't operate the relay, but we surface the upstream provider's contact)
- Run `cargo run -p vox-doc-pipeline` to regenerate SUMMARY.md / architecture-index.md / feed.xml (per the [VUV-9 plan's auto-generated-doc rule](2026-05-08-vuv-improvement-roadmap.md))

**Why a separate plan:** docs need to be written after the implementation reality is known (don't document features that ended up working differently). ~5 TDD steps.

**Trigger to write the plan:** S8 lands.

---

## Phase S10 — Future direction: Vox-hosted FRP relay (`*.vox.live`)

**NOT IN THIS PLAN.** Documented here so it isn't lost.

**The idea:** fork [`huggingface/frp`](https://github.com/huggingface/frp) (Apache-2.0, the same FRP server that powers `*.gradio.live`), deploy on a Vox-foundation-owned VPS (Hetzner ~$5/mo handles thousands of clients), allocate `*.vox.live` subdomains. CLI ships a Rust-built FRP client (or vendored frpc binary) instead of cloudflared.

**Benefits:** stable URLs (configurable expiry, Gradio uses 72h), SSE works (FRP is byte-stream relay), branded UX, control over abuse handling, independence from Cloudflare's policy stability.

**Costs:** infrastructure ops (TLS cert renewal, VPS reliability, abuse contact, possibly DDoS), engineering (~1 engineer-week for setup, plus an ongoing abuse-response process), domain + cert costs.

**Trigger to write the plan:** when (a) `vox share` MAU > some threshold worth investing in branded UX, OR (b) Cloudflare changes Quick Tunnel terms in a way that breaks our default. Until then, the existing three-backend chain is sufficient.

---

## Cross-cutting concerns

### Tunnel-binary distribution

Per [Gradio's lazy-download pattern](https://github.com/gradio-app/gradio/issues/11928), `cloudflared` is downloaded on first use, not bundled with the Vox CLI installer. This keeps the Vox CLI install footprint small. `~/.cache/vox/cloudflared/` is the cache. SHA256 verification before execution is non-negotiable.

### Abuse / ToS surface

Vox's `vox share` default uses Cloudflare's Quick Tunnels. Cloudflare's [Online Services Terms of Use](https://www.cloudflare.com/website-terms/) apply to the user's traffic. Vox does NOT proxy or relay traffic; the user's app talks directly to Cloudflare's edge via the local cloudflared process. Vox's role is limited to packaging the launcher.

If Cloudflare deprecates Quick Tunnels or adds an account gate (as ngrok did in 2023), `vox share` falls back to `localhost.run`. The plan's S2 task 1 includes a "weekly CI smoke test" to detect breakage early.

### Pre-existing breakage to ignore

The `vox-compiler` test failures referencing private `codegen_ts` (in `bug_a_match_arms_repro.rs` and `web_ir_lower_emit_test.rs`) are pre-existing from before this plan's branch. Don't try to fix them as part of share work.

### MENS retraining

This plan adds new public CLI surface (`vox share`, its flags) and new vocabulary. If the MENS training corpus contains examples that emit "wrong" share invocations (e.g., older drafts of this design), retrain after S2 lands. Standard discipline per [VUV-9 cross-cutting concerns](2026-05-08-vuv-improvement-roadmap.md#cross-cutting-concerns).

---

## Risks and non-goals

**Risks:**

- **Cloudflare adds account gate to Quick Tunnels.** Probability medium (Cloudflare has been tightening free tier policies). Mitigation: S3 fallback chain to localhost.run; S10 self-hosted FRP relay if breakage is total.
- **`*.trycloudflare.com` widely blocked by corporate SWGs.** Already true per [BleepingComputer 2024 report](https://www.bleepingcomputer.com/news/security/hackers-abuse-free-trycloudflare-to-deliver-remote-access-malware/). Mitigation: surface this prominently in the consent banner; suggest `--backend tailscale` for users behind hostile corporate SWGs.
- **Cloudflare's no-SSE limitation surprises users.** Already true. Mitigation: S6 SSE detection auto-switches.
- **cloudflared binary version drift breaks our CLI.** Mitigation: pin version, weekly CI smoke test, enable user-side `vox share --upgrade-cloudflared` opt-in (S2 follow-up).
- **Tailscale Funnel restrictions** (only ports 443/8443/10000, only `127.0.0.1`). Mitigation: preflight error messages with actionable instructions.
- **localhost.run speed-throttling.** For ML demos with images, the throttle is tangibly slow. Mitigation: documented in S9; acceptable for a fallback.
- **Auth middleware vs SSE compatibility.** URL-token middleware reading `?vox_share_token=` is fine for SSE (the query param is in the initial GET). Basic-auth is fine. Don't break SSE.

**Non-goals:**

- A Rust-native FRP client implementation. Out of scope; cloudflared is Go and works.
- The Vox-hosted FRP relay (S10). Documented as future direction but not built here.
- Replacing `vox deploy` (which is for production hosting, including Coolify). `vox share` is for ephemeral demos only.
- A general HTTP proxy for arbitrary localhost servers. `vox share` is scoped to Vox apps (built via `vox bundle`).
- WebRTC peer-to-peer or pure-Rust tunnel. Skipped per research; not viable as default.

---

## Self-review checklist

- [x] Spec coverage — all eight decisions from brainstorming are mapped to phases.
- [x] Open questions — none remain (the future relay is a deferred phase, not an open question).
- [x] Placeholder scan — `<TODO-FILL-IN-FROM-RELEASE>` placeholders are explicitly called out as MUST-FILL-BEFORE-MERGE in S2 task 1; that's a runtime-data placeholder (not a plan-spec placeholder).
- [x] Type consistency — `BackendKind`, `TunnelBackend`, `TunnelHandle`, `ShareConfig`, `ShareSession` named consistently across S1-S3.
- [x] Naming policy — `vox share` is a new public name; once registry exists from VUV-9, an entry MAY be added if we ever rename it. Today: stable.

---

## Execution handoff

Plan complete and saved at `docs/superpowers/plans/tooling/2026-05-09-vox-share-feature.md`.

**Two execution options:**

1. **Subagent-driven (recommended)** — fresh subagent per task, two-stage review between tasks. Uses `superpowers:subagent-driven-development`.
2. **Inline** — execute in this session with checkpoints. Uses `superpowers:executing-plans`.

**Which approach?**
