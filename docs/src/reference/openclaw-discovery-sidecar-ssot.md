---
title: "OpenClaw Discovery and Sidecar SSOT"
description: "SSOT for OpenClaw endpoint resolution order, discovery env vars and cache behavior, managed openclaw-gateway sidecar install and doctor controls, failure modes, and vox ci openclaw-contract fixture locations."
category: "Language Reference"

schema_type: "TechArticle"
---

# OpenClaw Discovery + Sidecar SSOT

This document is the single-source-of-truth for how Vox resolves OpenClaw endpoints and how managed sidecar installation behaves.

## Resolution precedence

Vox resolves OpenClaw endpoints in this order:

1. explicit command arguments (when provided)
2. environment / Clavis overrides
3. upstream discovery (`/.well-known/openclaw.json`)
4. deterministic local defaults

The shared resolver lives in `crates/vox-skills/src/openclaw_discovery.rs` and is consumed by CLI, MCP, and runtime adapter connect paths.

## Discovery inputs

- `VOX_OPENCLAW_WELL_KNOWN_URL` (optional explicit well-known URL)
- `VOX_OPENCLAW_URL` (optional HTTP gateway override)
- `VOX_OPENCLAW_WS_URL` (optional WS gateway override)
- `VOX_OPENCLAW_CATALOG_LIST_URL` (optional catalog list override)
- `VOX_OPENCLAW_CATALOG_SEARCH_URL` (optional catalog search override)

## Discovery cache behavior

- resolver caches a normalized snapshot with TTL
- stale fetch failures fall back to last-known-good cache when present
- if cache is unavailable, deterministic defaults are used

## Managed sidecar policy

Managed sidecar binary name:

- `openclaw-gateway` (`openclaw-gateway.exe` on Windows)

Release lane behavior:

- bootstrap/upgrade search release `checksums.txt` for matching sidecar assets for the current target triple
- sidecar asset is only installed when present and checksum verification passes
- sidecar install is best-effort and does not block `vox` binary install

Opt-out:

- set `VOX_OPENCLAW_SIDECAR_DISABLE=1` (or `true`)
- set `VOX_OPENCLAW_SIDECAR_EXPECT_VERSION=<version>` to have `vox openclaw doctor`
  report sidecar version drift (`match` / `mismatch`) against the detected
  sidecar `openclaw-gateway --version` output

Runtime supervision SSOT:

- `crates/vox-cli/src/process_supervision.rs` centralizes managed binary resolution,
  detached spawn, version probing, and process-tree termination used by OpenClaw doctor,
  daemon dispatch, and Populi lifecycle commands.
- OpenClaw doctor persists sidecar runtime state at
  `.vox/process-supervision/openclaw-gateway.state.json` (PID + binary path + start time),
  reuses live recorded PIDs when present, and prunes stale state before respawn.
- Explicit sidecar lifecycle controls are exposed via
  `vox openclaw sidecar status|start|stop`.
- Startup probe policy for `vox openclaw doctor --auto-start` is configurable via:
  - `VOX_OPENCLAW_SIDECAR_START_MAX_ATTEMPTS` (default `3`)
  - `VOX_OPENCLAW_SIDECAR_START_BACKOFF_MS` (default `500`)

## Operational failure modes

- **Well-known endpoint unavailable**: resolver falls back to last-known-good cache,
  then deterministic local defaults if no cache exists.
- **Catalog URL shape drift**: explicit env overrides (`VOX_OPENCLAW_CATALOG_*`) remain
  highest-priority recovery path without code changes.
- **Sidecar missing on PATH**: `vox openclaw doctor --auto-start` performs best-effort spawn
  and reports readiness fields instead of failing hard.
- **Sidecar version drift**: `VOX_OPENCLAW_SIDECAR_EXPECT_VERSION` allows explicit runtime
  mismatch visibility in doctor output for rollout gating.

## Contract fixtures

OpenClaw contract CI validates both protocol and discovery fixtures {

- `contracts/openclaw/protocol/*`
- `contracts/openclaw/discovery/*`

Guard command:

- `vox ci openclaw-contract`
