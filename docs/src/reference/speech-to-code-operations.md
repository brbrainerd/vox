---
title: "Speech-to-code — operations, security, rollout"
description: "Dashboards, privacy, canary, and release gates for spoken Vox codegen."
category: "Language Reference"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Operations

## Observability

- Emit **correlation IDs** from Oratio/MCP (`correlation_id` JSON fields) and join with `RUST_LOG=vox_mcp_speech=debug`.
- KPI schema: [`contracts/speech-to-code/kpi-baseline.schema.json`](../../../contracts/speech-to-code/kpi-baseline.schema.json).
- Benchmark manifest: [`contracts/speech-to-code/benchmark-fixtures.manifest.txt`](../../../contracts/speech-to-code/benchmark-fixtures.manifest.txt).
- Schema drift guards: `cargo test -p vox-integration-tests --test speech_schema_parity`.
- Optional **canary gate**: set `VOX_SPEECH_CANARY_KPI` to a KPI JSON file and run `cargo test -p vox-integration-tests --test speech_canary` — thresholds default from [`canary_policy.example.json`](../../../contracts/speech-to-code/canary_policy.example.json).

## Security and privacy

- MCP **`vox_validate_file`** resolves relative paths against the bound repository root and rejects canonical paths outside it (including traversal via `..` and absolute paths in other trees).
- Avoid persisting raw audio in shared logs; redact paths if needed. MCP `vox_oratio_listen` logs **path basename only** for protected path-like tokens when LLM polish rejects a correction.
- Speech trace / training rows: follow repo retention policy; use `mens/schemas/speech_to_code_trace.schema.json` only for **opt-in** export.
- Labeling rubric (human QA): [`contracts/speech-to-code/labeling_rubric.md`](../../../contracts/speech-to-code/labeling_rubric.md).

## Release gates

- **Compile**: `cargo check -p vox-mcp -p vox-oratio -p vox-lsp -p vox-audio-ingress` (and `cargo check -p vox-cli --features oratio-mic` when shipping mic capture).
- **Quality**: MCP `validate_file` and `vox_generate_code` must use `validate_document_with_hir`; `vox_speech_to_code` delegates to the same codegen path.
- **Contract**: MCP registry includes `vox_speech_to_code` (`contracts/mcp/tool-registry.canonical.yaml`); integration tests `speech_schema_parity` / manifest guards stay green.
- **Regression**: run `cargo test -p vox-oratio -p vox-lsp -p vox-corpus` speech-related tests.

### Incremental rollout stages

1. **Transcript-only:** HTTP ingress + MCP transcribe; no automated codegen.
2. **Draft codegen:** `vox_speech_to_code` with `validate:false` for exploratory drafts only.
3. **Validated codegen (default path):** `validate:true` (default), bounded retries, HIR gate unchanged.
4. **Broader tooling:** expand intent/routing; keep destructive repo operations behind explicit human confirmation outside this tool.

## Canary / rollback (MENS)

- Promote speech-tuned checkpoints only when compile-pass@k on the frozen benchmark set improves vs baseline.
- Roll back if p95 latency or error-rate SLO regresses (define per deployment).

See [`speech-to-code-pipeline.md`](speech-to-code-pipeline.md).

