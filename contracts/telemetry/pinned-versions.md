---
category: "Telemetry Contracts"
---

# Telemetry Dependency Version Pins

**Audited:** 2026-06-19  
**Decision:** Client hand-encodes OTLP/HTTP logs JSON (no opentelemetry SDK on client). All client deps are ALREADY in the workspace — no new pins needed.

## Client-side (workspace Cargo.toml — existing pins, reused)

| Crate | Workspace pin | Used by | Note |
|-------|--------------|---------|------|
| `reqwest` | `"0.12"` | `vox-telemetry-otlp` (feature `remote` only) | TLS: `rustls-tls`; feature `json` + `stream` already enabled |
| `governor` | `"0.10"` | `vox-telemetry-otlp` (feature `remote` only) | Rate-limit upload bursts |
| `serde` | `"1"` | `vox-telemetry-otlp` (pure core, always) | Derives |
| `serde_json` | `"1"` | `vox-telemetry-otlp` (pure core, always) | OTLP JSON encoding |
| `uuid` | `"1"` (features: v4, serde) | `vox-telemetry::config` | install_id — already a direct dep |
| `tokio` | workspace | `vox-telemetry-otlp` (feature `remote` only) | Async upload |

**No new `[workspace.dependencies]` entries required.** All of the above are already pinned in the workspace root `Cargo.toml`.

## Client-side (explicitly NOT added)

| Crate | Reason excluded |
|-------|----------------|
| `opentelemetry*` (any version) | Client hand-encodes OTLP JSON. Workspace already pins 0.29 (traces-only, consumed by vox-foundation). Adding the `logs` feature or bumping to 0.32 is a workspace-global breaking change — **out of scope**. |
| `opentelemetry-proto` | Server-only dep (Track C, separate repo) |
| `clickhouse` | Server-only dep |

## Server-side (`vox-telemetry-server` repo — separate repo, its own Cargo.toml)

These versions must be pinned in the server repo, NOT in the Vox workspace:

| Crate | Recommended version | Note |
|-------|--------------------|----|
| `clickhouse` | `"0.13.3"` | Async client for ClickHouse 23.x+ |
| `opentelemetry-proto` | `"0.7"` (latest as of 2026-06) | OTLP log decoding (server-side) |
| `axum` | `"0.8"` | Ingest HTTP server |
| `tokio` | `"1"` | Async runtime |
| `serde` + `serde_json` | `"1"` | OTLP JSON decode |

## ClickHouse version

| Component | Version | Note |
|-----------|---------|------|
| ClickHouse server | `23.8` LTS (Docker: `clickhouse/clickhouse-server:23.8-alpine`) | LTS release, good TTL + MV support |

## OTLP wire format

Client sends **OTLP/HTTP logs** (JSON framing, `Content-Type: application/json`). This is the stable public OTLP format; compatible with any OTel Collector or custom axum receiver.

Endpoint URL recorded here after Track D deployment: `TBD` (update after D2 completes).
