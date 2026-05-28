---
title: "Maintainability hotspot matrix (baseline)"
description: "Tracking matrix for the package and maintainability overhaul: duplicates, target consolidation, gating tests, and acceptance criteria."
category: "Architecture SSOTs"
last_updated: "2026-03-29"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Maintainability hotspot matrix

This document is the **baseline** for the package and maintainability rollout. Update rows as migrations land.

## Acceptance criteria (cross-cutting)

| Area | Criteria |
|------|----------|
| Bounded file reads | Same cap source (`vox_scaling_policy::ScalingPolicy::embedded().thresholds.max_file_bytes_hint`); same error messages for stat/over-cap/read/UTF-8 where `anyhow` is used |
| JSON Schema (CI/MCP) | Generated or shared validators match existing contract tests; MCP `input_schema` stays draft-07-compatible for strict clients |
| SSE / LLM streaming | Golden tests cover `data {` lines split across arbitrary byte chunks; no regression on `[DONE]` and delta content extraction |
| Retry / backoff | Documented caps and multipliers; activity codegen `ActivityOptions` unchanged unless accompanied by compiler+fixture updates |
| Process supervision | Managed binary resolution order unchanged; sidecar state file format unchanged |
| DB row mapping | `turso`/`StoreError` semantics preserved; one module at a time |

## Hotspot matrix

| ID | Hotspot | Owner crates / paths | Target consolidation | Gating tests / notes |
|----|---------|----------------------|------------------------|----------------------|
| H1 | Bounded UTF-8 reads | 14× `bounded_fs.rs`, `vox-cli/.../bounded_read.rs` | [`vox-bounded-fs`](../../../crates/vox-bounded-fs) | Per-crate tests; scaling TOESTUB |
| H2 | MCP `input_schema` vs params | `vox-mcp/tools/input_schemas.rs`, `params.rs` | `schemars`-first + documented overrides | `input_schemas` registry tests |
| H3 | JSON Schema validate boilerplate | `vox-cli` CI commands, `vox-toestub/suppression.rs` | [`vox-jsonschema-util`](../../../crates/vox-jsonschema-util) | Contract + scorecard tests |
| H4 | AI `generate` schema check | `vox-cli/commands/ai/generate.rs` | Same validator as CI or renamed lightweight API | Integration if present |
| H5 | SSE OpenAI streaming | `vox-runtime/llm/stream.rs`, `vox-ludus/.../transport.rs` | [`vox-openai-sse`](../../../crates/vox-openai-sse) (`Utf8LineBuffer`, `sse_data_line_delta`) | Chunk-boundary unit tests in crate |
| H6 | OpenAI wire types | `vox-runtime/llm/wire.rs`, `vox-mcp/llm_bridge/providers/openai.rs` | [`vox-openai-wire`](../../../crates/vox-openai-wire) | MCP + runtime compile |
| H7 | Retry/backoff | `activity.rs`, `openclaw.rs`, `social_retry.rs`, scholarly | [`vox-primitives`](../../../crates/vox-primitives) backoff; `backon` no-go (see `resilient_http`, `social_retry` docs) | Activity + publisher tests |
| H8 | Simple activity IDs | `activity.rs`, `vox-populi`, `populi_cli` | [`vox-primitives`](../../../crates/vox-primitives) `id` | Collision expectations |
| H9 | Process supervision | `vox-cli/process_supervision.rs` | `sysinfo` liveness; PATH via `which` crate (`path_lookup_executable`) | Manual / doctor flows |
| H10 | `reqwest::Client` defaults | Ludus, MCP, ARS, CLI, publisher | [`vox-reqwest-defaults`](../../../crates/vox-reqwest-defaults) | Timeout-sensitive integration |
| H11 | `row.get` mappers | `vox-db/store/ops_*.rs` | `vox_db::row_cols!` macro (pilot) | `vox-db` tests per module |
| H12 | Env / config parsing | `vox-config`, scattered `env::var` | `vox_config::env_parse` + Clavis for secrets | `vox ci clavis-parity`, doctor, [`clavis-ssot`](../reference/clavis-ssot.md) |

## Codegen and contract surfaces (do not drift silently)

- `vox-compiler` — `codegen_rust/emit/http.rs`, `with_emit.rs` (`ActivityOptions`)
- `contracts/cli/command-registry.yaml`, `contracts/mcp/tool-registry.canonical.yaml`
- Scaling policy: `contracts/scaling/policy.yaml` (embedded via `vox-scaling-policy`)

## Related

- [Environment variables (SSOT)](../reference/env-vars.md) — `VOX_DB_PATH`, OpenClaw sidecar env vars
- [AGENTS.md](../../../AGENTS.md) — Clavis secret resolution


