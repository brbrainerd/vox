---
title: "HF fine-tuning capability matrix (code-grounded)"
description: "Official documentation for HF fine-tuning capability matrix (code-grounded) for the Vox language. Detailed technical reference, architect"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
# HF fine-tuning capability matrix (code-grounded)

Single control plane: [`crates/vox-populi/src/mens/tensor/finetune_contract.rs`](../../../crates/vox-populi/src/mens/tensor/finetune_contract.rs) (`FineTuneContract`) + `execution_planner.rs` (`ExecutionPlanner`). Execution kernels: **Burn (wgpu LoRA)** vs **Candle (qlora-rs NF4)**.

| Capability | Burn kernel (`PopuliTrainBackend::BurnLora`) | Candle kernel (`PopuliTrainBackend::CandleQlora`) |
|------------|---------------------------------------------|--------------------------------------------------|
| **Training graph depth** | Full causal stack: `LoraVoxTransformer` → blocks → LM head (`tensor/lora.rs`). | **Proxy stack**: optional per-layer `o_proj` / GPT-2 `c_proj` as sequential `QuantizedLinear` + tied LM head; not full MHA/FFN blocks (`candle_qlora_train.rs`). |
| **Base quantization** | **None** in production path (f32 LoRA bases). NF4 base is **not** implemented (`lora.rs` module docs). | **NF4** frozen bases via **qlora-rs** on stacked linears + LM head. |
| **Tokenizer** | **Vox** (`VoxTokenizer` ChatML) default; **HF** `tokenizer.json` when `--tokenizer hf` + GPT-2 HF layout (contract-gated). | **HF** only (`tokenizer.json`); enforced in `qlora_preflight.rs`. |
| **Weight loading** | HF **warm-start**: token embeddings + **GPT-2** decoder blocks (Q/K/V split from `c_attn`, MLP, norms, `wpe`, `ln_f`, optional `lm_head`) when shapes match (`burn_hf_load.rs`). | mmap **f32** embedding table + selected projection keys from shards. |
| **Artifacts** | Burn `*.bin` checkpoints (`Checkpoint`); `merge-weights` → merged `VoxTransformer`. | `candle_qlora_adapter*.safetensors` **v2** + sidecar meta; **v3** unified schema (`adapter_schema_v3.rs`); `merge-qlora` subset merge. |
| **Merge fidelity** | `LoraAttention {:merge()` → Burn `MultiHeadAttention` with merged Q/K/V when **`use_rope == false`**; RoPE stacks cannot merge to static linears (see `lora.rs`). | Deterministic f32 delta merge for exported keys (`candle_qlora_merge.rs`). |
| **Cross-stack logits parity** | **Not** asserted end-to-end (NF4 vs f32 LoRA, different graphs). **Touchpoints:** `tests/candle_burn_f32_matmul_parity.rs` (**matmul**); `tests/candle_burn_f32_linear_lm_logits_parity.rs` (**biased linear** / LM-head-shaped f32 logits); `tests/candle_burn_nf4_dequant_lm_reference_parity.rs` (**Tier B:** qlora-rs NF4 round-trip → shared f32 `W` → Burn vs Candle LM-shaped linear); `tests/candle_burn_cross_entropy_parity.rs` (**CE** on shared logits). | Same integration tests. |

## Token / label policy

- **Shared helpers**: `tensor/training_text.rs` — `plain_system_prompt_response` (Candle), ChatML supervision strings + `hf_tokenize_chatml_supervised` (Burn + HF).
- **Candle objective**: last-token LM loss on concatenated plain text (see `candle_qlora_train.rs`).
- **Burn objective**: token-level CE with prompt masked at **-100** (ChatML boundary), Vox or HF tokenizer.

## Feature flags

| Build | Notes |
|-------|--------|
| `vox-populi/mens-gpu` | Burn + `tokenizers` + `safetensors` for HF-aware Burn path. |
| `vox-populi/mens-train` | `mens-gpu` + `candle-qlora` + qlora-rs (CLI **`gpu`** feature pulls this chain). |

## Related

- [Mobile edge AI SSOT](../reference/mobile-edge-ai.md) — off-device training vs on-device inference (LiteRT / Core ML), mens hints, `VOX_INFERENCE_PROFILE`.
- [Mens training SSOT](../reference/mens-training.md) — CLI entrypoints and regression tests.
- [HF fine-tune gap matrix](../reference/hf-finetune-gap-matrix.md) — remaining risks vs resolved items (SSOT ↔ code).
- [Mens LLM PR checklist](mens-llm-pr-checklist.md) — PR gate for LoRA duplication, layouts, parity tiers.
- ADR 006 / 007 — QLoRA graph scope and qlora-rs API gate.

## Burn production policy

Burn training is held as an opt-in research lane. Promotion to production requires scorecard evidence with explicit backend comparisons (`backend=burn` vs `backend=qlora`) over at least two benchmark cycles, including syntax + semantic KPI deltas and runtime repair KPIs.


