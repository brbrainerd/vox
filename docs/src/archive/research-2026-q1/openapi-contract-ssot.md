---
title: "OpenAPI contract SSOT (Populi, MCP, Codex)"
description: "How committed OpenAPI YAML stays authoritative and how we validate and optionally generate clients."
category: "Architecture SSOTs"
status: "current"
sort_order: 0
last_updated: "2026-03-29"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# OpenAPI contract SSOT

## Principle

**Committed YAML under `contracts/` remains the published contract** for Populi, MCP HTTP gateway, Codex, and similar surfaces. Runtime code and tests prove alignment; we do not silently derive the contract from Axum routes without an explicit ADR.

## Layers of enforcement

1. **Structural parse** — The spec must deserialize as OpenAPI 3.x. We use the `openapiv3` crate in tests (see `crates/vox-populi/tests/openapi_paths.rs`, test `openapi_spec_parses_as_openapiv3`) so invalid YAML or schema shape fails early.
2. **Path / schema parity** — Integration tests keep an explicit list of paths (and key schemas) aligned with `transport::router` and DTO serde keys. This catches drift that a parse-only check would miss.
3. **CI substring guards** — `vox ci` still uses targeted substring checks for Codex (`OPENAPI_SUBSTRINGS` in `crates/vox-cli/src/commands/ci/constants.rs`) as a **cheap backstop**. Over time, prefer replacing these with `openapiv3` + operation-id or tag assertions where possible.

## Optional: generated clients

**When to adopt `progenitor` (or similar):**

- After path stability and auth middleware story are clear.
- Start with **read-only** or **internal** crates (e.g. `PopuliHttpClient` shape in `crates/vox-populi/src/http_client.rs`) -> shrink repetitive `reqwest` calls.

**Risks:** naming of types, feature flags (`transport`, `mens`), and hand-written auth headers must stay in thin wrappers.

## What we are not doing (without ADR)

- **utoipa-from-routes as SSOT** — Fine for greenfield; inverting SSOT from committed YAML requires an explicit decision and publish pipeline for the generated spec.

## References

- `contracts/populi/control-plane.openapi.yaml`
- `contracts/mcp/http-gateway.openapi.yaml`
- `contracts/codex-api.openapi.yaml`
- `crates/vox-populi/tests/openapi_paths.rs`
- `crates/vox-mcp/tests/http_gateway_openapi_paths.rs`


