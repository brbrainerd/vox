---
title: "Qwen 3.6 integration research (groundwork)"
description: "Pre-implementation checklist for Qwen 3.6 vs 3.5: primary sources, HF config and weight layout, Vox integration matrix (native QLoRA vs remote API), risks (context, reasoning, tools, multimodal, closed weights)."
category: "Architecture SSOTs"
last_updated: "2026-04-08"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Qwen 3.6 integration research (groundwork)

This note is **planning and verification only**. It does not claim shipped Qwen 3.6 behavior in Vox. Third-party summaries (blogs, aggregators, model-router copy) often lag or misstate **open-weight** availability and **config** details—treat them as hypotheses until pinned to **primary** artifacts below.

**Current Vox SSOT for native Candle QLoRA** remains **Qwen 3.5** (`Qwen/Qwen3.5-4B` and related tiers); see [`mens-training.md`](../reference/mens-training.md).

## 1. Source-of-truth checklist (before any code)

Verify and record links + revision dates for:

| Item | Why it matters for Vox |
|------|-------------------------|
| Official Qwen / Alibaba model card or release post | License, context limits, modality claims, “thinking” / reasoning behavior |
| Hugging Face model hub entries (if any) | Whether **weights** exist for local train/merge/serve; `config.json`, `tokenizer_config.json`, chat template |
| `model_type` and key layout in `config.json` | Drives [`hf_load.rs`](../../../crates/vox-populi/src/mens/tensor/hf_load.rs) and [`hf_keymap.rs`](../../../crates/vox-populi/src/mens/tensor/hf_keymap.rs) |
| Attention layout (dense, hybrid linear/full, MoE) | Whether 3.6 reuses **Qwen 3.5** hybrid patterns or needs a new `HfArchitecture` variant |
| Special tokens (tool, vision, reasoning, EOS) | Tokenization, masking for SFT, completion boundaries in Schola / orchestrator |
| Context length (advertised vs practical) | VRAM, sequence packing, checkpointing policy for local QLoRA |

If **no** Hugging Face–compatible weights appear for a given SKU, native Mens paths in this repo remain **out of scope** for that SKU until that changes.

## 2. Vox integration matrix (planning)

| Surface | When 3.6 is in scope | Preconditions |
|---------|----------------------|-----------------|
| **`vox mens train` / Candle QLoRA** | HF (or compatible) **safetensors + config** that match or extend existing Qwen 3.5 parsing | Successful `qlora_preflight`; possible new `HfArchitecture::Qwen36` or mapped alias to `Qwen35` if keys are compatible |
| **`vox-schola serve` / merged adapters** | Same as above + merge manifest parity | Adapter schema and `candle_qlora_merge` family detection |
| **Orchestrator / remote inference (BYOK, HTTP)** | **API-only** or OpenRouter-style ids are fine without local weights | Provider prefix handling (see `provider_family_strengths` in [`spec.rs`](../../../crates/vox-orchestrator/src/models/spec.rs)); tokenizer + tool schema documented by provider |
| **Multimodal** | Not a separate stack from 3.5 | Extends the same contracts as [`qwen35-multimodal-phase2-backlog.md`](qwen35-multimodal-phase2-backlog.md) (vision/video tokens, corpus, trainer, serve) |

## 3. Risks and vagaries (confirm against official docs)

- **Long context**: Advertised millions of tokens vs what local QLoRA can train at a given `seq_len` and batch; optimizer state and activation memory.
- **Reasoning / chain-of-thought**: Extra tokens or template segments affect supervised fine-tuning masks and logprob boundaries; may differ from Qwen 3.5 “thinking” toggles.
- **Tool calling**: JSON schema or special tokens may drift from 3.5 Instruct; orchestrator and eval gates need explicit fixtures per model id.
- **Closed-weight or hosted-only SKUs**: No local merge of adapters without a **compatible open base**; plan for remote-only routing and cost/quotas.
- **MoE or new block types**: May invalidate assumptions in proxy-stack or full-graph QLoRA preflight; strict preflight should fail closed with a clear operator message.

## 4. Optional follow-up (implementation phase, later)

- After official `config.json` is available, add explicit parsing in [`hf_load.rs`](../../../crates/vox-populi/src/mens/tensor/hf_load.rs) (e.g. `HfArchitecture::Qwen36` **or** map to `Qwen35` if key namespaces match `model.language_model.layers.*`).
- Extend [`qlora_preflight.rs`](../../../crates/vox-populi/src/mens/tensor/qlora_preflight.rs) with architecture-specific guards and diagnostics.
- Update [`contracts/mens/training-presets.v1.yaml`](../../../contracts/mens/training-presets.v1.yaml) and docs only when a **concrete** default 3.6 base is chosen for the product.

## 5. Related docs

- [Qwen3.5 multimodal Phase 2 backlog](qwen35-multimodal-phase2-backlog.md) — multimodal contracts shared across Qwen generations until proven otherwise.
- [Mens native training SSOT](../reference/mens-training.md) — current default base model and CLI expectations.


