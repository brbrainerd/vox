---
title: "Interop tier policy"
description: "SSOT for the four-tier Vox interop model: core surfaces, approved bindings, package-managed Vox libraries, and explicit escape hatches."
category: "Architecture SSOTs"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Interop tier policy

Vox should keep interop predictable by treating foreign capability as a tiered system rather than one undifferentiated escape hatch.

## The four tiers

| Tier | Meaning | Examples |
|------|---------|----------|
| `tier0` | core Vox / std / builtin registry | `std.*`, builtin HTTP surfaces |
| `tier1` | approved wrappers exposed as narrow Vox namespaces | `OpenClaw`, future approved auth/json/http bindings |
| `tier2` | package-managed Vox libraries and skill bundles | Vox packages, reusable app-lane helper bundles |
| `tier3` | explicit escape hatches | `import rust:...`, WebIR interop nodes, islands, external MCP/OpenClaw |

## Rules

- Prefer the lowest tier that solves the bell-curve problem.
- Tier 3 does not become a substitute for Tier 1 wrapper design.
- `import rust:...` is Cargo manifest sugar, not a typed interop system.
- New common integrations should usually land as Tier 1 wrappers, not raw crate access.
- Runtime-internal crates (for example `tokio`, `axum`, `tower`) remain implementation details behind `WebIR` / `AppContract` / `RuntimeProjection`.
- High-debt ecosystems (for example broad SQL/ORM families) remain deferred until wrapper abstractions and representative demand justify first-class support.

## Curated package categories (bell curve)

When growing **tier2** surface area, prefer packages that match repetitive app lanes {

| Category | Typical capability | Notes |
|----------|-------------------|--------|
| HTTP / API client | outbound REST, JSON envelopes | Prefer bounded AppContract/server shapes first; use wrappers for provider SDKs. |
| Auth / sessions | cookies, OIDC-shaped flows | Keep policy in AppContract metadata where possible. |
| Serialization / validation | JSON, stable config | Align with `std.json` and contract tests before pulling large ecosystems. |
| Observability | tracing, metrics | Wire through `std.log` / runtime builtins on script paths; native `tracing` in host. |
| Background jobs | queues, retries | Workflow/activity language intent first; tier3 when an external broker is required. |

## Approved binding checklist

An approved wrapper should document:

1. namespace name
2. function signatures and argument arity
3. runtime or codegen mapping
4. docs page
5. tests
6. compatibility and migration policy

## Data-lane graduation criteria

For data crates to graduate from escape hatch/deferred to approved wrappers, all must be true:

1. The `turso+vox-db` lane cannot satisfy representative app/workflow needs.
2. A narrow Vox wrapper abstraction is specified (not raw ORM/query-builder mirroring).
3. Cross-target behavior and migration policy are explicit.
4. Debt-to-value score remains favorable in the Rust ecosystem support registry.

See also: [Rust ecosystem support contract](../reference/rust-ecosystem-support-contract.md).


