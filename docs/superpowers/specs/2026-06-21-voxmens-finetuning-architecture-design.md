# VoxMens Fine-Tuning Architecture & Spot Pipeline — Design Spec

**Status:** Approved (brainstorm 2026-06-21). **Research basis:** `docs/src/architecture/voxmens-finetuning-boundaries-research-2026-06-21.md`. **Implementation plan:** `docs/superpowers/plans/2026-06-21-voxmens-finetuning-pipeline.md`.

## Goal

Make VoxMens a **resource-scalable hub-and-spoke** system that reaches **per-spoke Gemini-3-Flash / Claude-Sonnet-4.6 quality on each spoke's narrow domain** by fine-tuning **only where the base model lacks the capability**, and serving everything else (tools, skills, plugins, facts) through a **dynamic discovered layer** that needs **no retraining when capabilities are added**. Training runs on **cloud spot GPUs** (RunPod default) so we are no longer bound to a single local 4080 / 32B ceiling.

## The boundary (the core decision)

- **Fine-tune** = *capability or syntax the base model cannot be prompted into.* → Vox DSL, Rust idiom, and tool-call **behavior** (selection discipline + schema-valid arguments). Evidence: fine-tuning beats RAG for a novel/low-resource language (arXiv 2503.18760); fine-tuned small models top BFCL over GPT-4/Claude-3 (xLAM 2409.03215) and rival 16-70B on low-resource code (Agnostics 2508.04865).
- **Dynamic / discovered** = *anything enumerable or changing.* → the tool/skill/command/plugin catalog, facts, DB/API knowledge, evidence. Served via retrieval + schema, never weights. New skill = a registry row → retrieval surfaces it → the trained harness calls it. Zero retraining.

## Architecture

### Hub
One **Qwen3 dense ladder** lineage (4B → 8B → 14B → 32B; 72B/MoE when hardware allows). The rung is chosen by available VRAM (local) or the provisioned spot GPU (cloud). All spokes are LoRA adapters on this single base lineage, enabling multi-adapter hot-swap serving.

### Spokes — exactly 4 fine-tuned QLoRA adapters
| Spoke | Capability the base lacks | Verifiable reward / gate |
|---|---|---|
| `vox-lang` | Novel Vox DSL syntax/idiom | vox parse/compile rate |
| `rust` | Lower-resource Rust idiom in our workspace | `cargo build` reward |
| `tool-selection` | Choosing the right tool from a large catalog (failure mode #1) | selection accuracy |
| `argument-generation` | Producing schema-valid args for complex params (failure mode #2) | `tool_call_valid_json_rate` |

`tool-selection` + `argument-generation` together = the **harness**, decomposed per arXiv 2510.00229 (+46% on MCP-Bench vs monolithic). **Retired as fine-tuned spokes** (→ hub + dynamic layer): `chat`, `research`, `research-expert`, `rocks`, `populi-meta`.

### Dynamic / discovered layer (NOT fine-tuned)
- **Vocabulary (exists):** `tool-registry.canonical.yaml`, `SkillRegistry`, `input_schemas.rs` (schemars-derived JSON schemas).
- **Gap 1 — semantic tool RETRIEVAL:** embed task ↔ tool descriptions, inject only top-K tool schemas (today all tools are dumped in context). Research: dynamic tool retrieval is worth +23–104% (2512.17052) and scales the catalog without retraining (2509.20415).
- **Gap 2 — schema-CONSTRAINED decoding at generation:** today only post-hoc `JsonPrefix`/`StrictJson`. Adopt vLLM guided decoding / XGrammar so emitted tool-call args are schema-valid by construction. Do **not** hand-roll a grammar engine.

### Serving
**vLLM multi-LoRA**: one Qwen3 base in VRAM + the 4 adapters hot-swapped on demand (`VLLM_ALLOW_RUNTIME_LORA_UPDATING`, runtime load/unload endpoints, LRU, latency-hidden load). Wire existing `DomainRouter` / `route_by_signal` → adapter name. Do **not** build a custom merge/unmerge path.

### Cloud spot fine-tuning pipeline
Extend the existing `crates/vox-populi/src/mens/cloud/` module (Vast + RunPod clients + estimator + budget + watchdog already present). **RunPod is the default** (lower agony — programmatic API, fine-tuning templates, persistent storage, stable logs/checkpoints); **Vast.ai is opt-in** for ~20–40% cheaper spot with aggressive checkpointing (watchdog handles interruption). Complete the 3 missing pieces: **job-submission flow, log streaming, checkpoint sync.** One command:

```
vox mens train --remote --spoke <name> [--provider runpod|vast]
  → estimate cost → provision spot → sync corpus up → train QLoRA adapter
  → stream logs → sync checkpoints down → pull adapter → eval-gate → register in DomainRouter
```

### Evals
Add **BFCL** (Berkeley Function-Calling Leaderboard) for the harness adapters; keep MultiPL-E-style pass@k and per-spoke gates (`rust_compile_rate`, vox parse-rate, `tool_call_valid_json_rate`). Target = **per-spoke parity with Flash/Sonnet on that spoke's domain**, not general intelligence.

## V1 scope & deferrals

- **V1 = QLoRA + SFT only**, on the 4 spokes, RunPod default, the two dynamic-layer gaps, vLLM serving, BFCL + per-spoke gates.
- **Deferred to V2:** RLVR/GRPO with verifiable rewards (cargo build / vox compile / BFCL), on-policy distillation from a frontier teacher, and DPO/ORPO/FullSft (currently fail-closed in `AdapterMethodRegistry`).
- **Gate on data readiness:** each spoke trains only when its corpus passes a volume + diversity threshold.

## What we are explicitly NOT doing

- No separate chat spoke (Qwen3-instruct handles conversation on the hub).
- No fine-tuning per tool/skill/plugin (the whole point of the dynamic layer).
- No custom multi-LoRA merge engine (vLLM provides it).
- No new training backend (reuse CandleQlora plugin locally; cloud runs the same via container image).

## Risks

1. **Corpus volume** for tool-selection/arg-gen and Vox may be thin — mitigated by the synthetic `agentic_synth` (real surface) + a corpus-readiness gate.
2. **Spot interruption** on Vast — mitigated by frequent checkpoint sync + watchdog; RunPod default avoids most of it.
3. **Base-rung mismatch** between training GPU and local serving GPU — mitigated by training the rung that the *target* serving tier will run, recorded per adapter.
4. **One base for code+agentic+chat** — accepted; revisit only if measured chat/parity lags.
