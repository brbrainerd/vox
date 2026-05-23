---
title: "HF fine-tune gap matrix (SSOT ↔ code)"
description: "Official documentation for HF fine-tune gap matrix (SSOT ↔ code) for the Vox language. Detailed technical reference, architecture guides,"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---
# HF fine-tune gap matrix (SSOT ↔ code)

Maps **remaining** risks and **resolved** items to **modules** and **severity**. See [capability matrix](../archive/research-2026-q1/hf-finetune-capability-matrix.md) for the live feature table.

## Active gaps / risks

| Gap / risk | Location | Severity |
|------------|----------|----------|
| Burn: NF4 frozen base not wired into Mens train path | Primitives: `vox-tensor` `lora.rs` (QLoRA roadmap / f32 LoRA today); **full graph + merge:** `vox-populi` `mens/tensor/lora.rs`; workspace Burn **0.19** has quantization building blocks — **not** integrated as frozen NF4 bases for `LoraVoxTransformer` | **High** — **integration backlog** (not physics-limited); single-kernel QLoRA on Burn remains unscoped until designed against Burn quant APIs + optimizer/device story |
| Burn: `LoraAttention::merge()` when **`use_rope == true`** | `crates/vox-populi/src/mens/tensor/lora.rs` `merge()` — asserts / rustdoc: RoPE cannot fold into static merged linears | **Medium** (serve/merge for RoPE stacks only) |
| Candle: proxy stack (`o_proj` / `c_proj` + LM head), not full causal blocks | `candle_qlora_train.rs`, ADR 006/007 | **High** (cross-kernel parity) |
| qlora-rs API: sequential `QuantizedLinear` only | ADR 007 | **Medium** (full-graph Candle training) |
| Cross-stack logits parity | No end-to-end NF4 vs Burn **full-graph** LM assertion | **Medium** (primitives: matmul, **biased linear** (`candle_burn_f32_linear_lm_logits_parity`), **Tier B** NF4 dequant reference linear (`candle_burn_nf4_dequant_lm_reference_parity`), CE on shared f32 logits) |
| Burn `*.bin` ↔ Candle `candle_qlora_adapter.safetensors` | **No** automatic rename/layout bridge (`tensor/artifact_bridge.rs` + `merge_qlora` guard) | **By design** — operator must pick the kernel-appropriate merge command |

## Resolved / mitigated (was “gap”, now implemented)

| Item | Resolution |
|------|------------|
| Burn `LoraAttention::merge()` placeholder MHA | Real `MultiHeadAttention` merge for **non-RoPE** GPT-style attention; regression tests in `lora.rs` / Burn stack tests |
| Burn HF load beyond embeddings | GPT-2 decoder warm-start in `burn_hf_load.rs` (Q/K/V from `c_attn`, MLP, norms, `wpe`, `ln_f`, optional `lm_head`) |
| Merge UX: wrong adapter type | `merge-qlora` rejects `*.bin` with SSOT-linked copy from `tensor/artifact_bridge.rs` (`MERGE_QLORA_REJECTS_BURN_BIN`); aliases documented in SSOT / `ref-cli.md` |

## Related

- [Mens training SSOT](mens-training.md) — merge table and regression commands.
- [Mens LLM PR checklist](../archive/research-2026-q1/mens-llm-pr-checklist.md) — duplication, flags, layouts, merge, parity tiers.
- `crates/vox-populi/src/mens/tensor/finetune_contract.rs` — contract gates.


