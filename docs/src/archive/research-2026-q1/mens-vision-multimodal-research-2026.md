---
title: "Mens vision and multimodal inputs (research 2026)"
description: "TrainingPair limits, orchestrator vision hints vs attachments, corpus-lab screenshot-to-JSON rubrics, Candle text-only native stack vs remote VLMs, telemetry and privacy boundaries."
category: "Architecture SSOTs"
status: "research"
sort_order: 16
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Aligns Mens JSONL contracts, orchestrator routing, and optional vision QA without conflating native QLoRA with multimodal weights."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Mens vision and multimodal inputs (research 2026)

## Executive summary

Vox today separates three layers that are easy to conflate:

1. **Orchestrator model selection** — Remote catalogs (for example OpenRouter) expose `supports_vision` when upstream reports image input modalities. Prompt text can also trigger heuristics (`infer_prompt_capability_hints` in `vox-orchestrator`).
2. **Native Mens Candle QLoRA and `vox mens serve` / Schola** — Decoder-only **text** generation with a Hugging Face tokenizer; no in-tree image encoder in the Candle inference engine.
3. **Mens training JSONL** — `TrainingPair` in `vox-tensor` carries UTF-8 strings only (`prompt`, `response`, optional `turns[].content`). There is no first-class attachment field today.

**Recommendation:** Treat **vision** as an optional **evidence pipeline** that produces **small structured JSON** (rubric output, layout hashes, a11y snapshots) beside compiler metrics. Route **raw multimodal inference** to **remote** VLMs until `TrainingPair` (or a successor row type) and loaders are explicitly versioned and bounded.

## Ground truth in repository

| Concern | Location / behavior |
| --- | --- |
| Text-only inference enum | `vox-populi`: `InferenceModel` (`Qwen2` / `Qwen35` variants) in `candle_inference_serve.rs` — autoregressive text, KV cache, no vision tower. |
| JSONL row shape | `vox-tensor` `data.rs`: `TrainingPair` — no `image_url`, `mime`, or `bytes_sha256` fields. |
| Vision routing heuristics | `vox-orchestrator` `dei_shim/selection/resolve.rs`: substring-based `(requires_vision, requires_web_search)` from prompt text only. |
| OpenRouter vision flag | `vox-orchestrator` `catalog.rs`: `supports_vision` from `architecture.input_modalities` containing `"image"`. |
| Compiler + golden gate | `vox-compiler` tests `golden_vox_examples.rs` — parse, HIR, WebIR validate, Syntax-K; unrelated to pixels. |
| Screenshot / browser | `vox-runtime` browser builtins; MCP `browser_screenshot` — pixels leave the trust boundary unless policy wraps them. |

## Design directions

### A. Agent-to-agent handoff (near-term, low coupling)

- **Coding agent** produces `.vox` and compiler diagnostics (or `VoxIrModule` path when emitted).
- **Vision specialist** (remote VLM) receives **screenshot + fixed rubric** and returns **JSON** validated against a small JSON Schema (widget list, visible errors, primary CTA, route hint).
- Store `vision_rubric.json` keyed by `fixture_id` and `sha3(screenshot bytes)` next to corpus batch reports; **do not** embed raw pixels in git-tracked JSONL.

### B. Explicit task hints (orchestrator)

- Prefer **client-supplied** `requires_vision` and an `attachment_manifest` (MIME type, content hash, optional URI) over substring inference for high-stakes routes.
- When heuristics are used, log `hint_source: heuristic` vs `explicit` for later evaluation.

### C. `TrainingPair` v2 (research schema, not implemented here)

Document-only requirements for a future serde shape:

- Optional `attachments: [{ kind, mime, sha256, max_bytes, redaction_tier }]`.
- Version field `training_pair_schema` for loaders (`VOX_MENS_TRAIN_JSONL_STRICT=1` behavior must be defined per version).
- Interaction with HF chat templates for Qwen-class VL models (special image tokens) — see [mens-qwen-family-migration-research-2026.md](mens-qwen-family-migration-research-2026.md) and Hugging Face `Qwen3_5Config` multimodal token ids in upstream docs.

### D. Cheaper than VL where possible

- Playwright **accessibility tree** or DOM snapshot JSON may answer many “what is on screen?” questions without a VLM; compare cost and flakiness before defaulting to vision models in CI.

## Privacy, telemetry, artifacts

- Raw screenshots are **workspace artifacts** — follow [workspace artifact retention](../../../contracts/operations/workspace-artifact-retention.v1.yaml) and `vox ci artifact-audit` guidance in contributor governance.
- Any telemetry row that references vision must avoid embedding image bytes; align with [telemetry trust SSOT](telemetry-trust-ssot.md) and opt-in persistence flags.

## See also

- [GUI, v0/islands, vision, and Mens Qwen — virtuous-cycle implementation plan (2026)](vox-gui-vision-virtuous-cycle-implementation-plan-2026.md) — execution waves and 50+ concrete work items.
- [Vox corpus lab (research 2026)](vox-corpus-lab-research-2026.md) — tiers, batch lanes, eval harness sketch.
- [Mens Qwen family migration (research 2026)](mens-qwen-family-migration-research-2026.md) — text vs multimodal configs upstream.
- [Mens training data contract](../reference/mens-training-data-contract.md) — `validate-batch`, quarantine, lanes.
- [Vox source → Mens pipeline SSOT](vox-source-to-mens-pipeline-ssot.md) — lexer vs HF tokenizer separation.
- [Mens training SSOT / reference](../reference/mens-training.md) — Candle QLoRA-first, serve matrix.

## Open questions

1. Should `vox_vision_rubric` be a first-class **mix lane** in `mens/config/mix.yaml`, or a separate JSONL source consumed only by eval jobs?
2. Who owns JSON Schema for rubric output — `vox-corpus`, `vox-eval`, or `contracts/eval/`?
3. Minimum redaction rules before any screenshot hash is logged to `research_metrics`.


