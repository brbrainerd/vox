---
title: "ADR 003 — Native Rust Training Over Python"
description: "Historical move off Python/Unsloth; current Mens training is native Candle + qlora-rs QLoRA via vox mens train (see mens-training SSOT)."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 003 — Native Rust Training Over Python

**Status**: Accepted; amended 2026-04-06  
**Date**: 2026-03-02 (original decision)  
**Author**: Bert Brainerd

**Current product path:** Large-model **QLoRA** fine-tuning runs **entirely in Rust** — **Candle**, **qlora-rs**, and **`vox mens train`** (`--backend qlora`, `--tokenizer hf` by default). **Python / Unsloth** described below is **historical context** only, not an operator requirement.

---

## Historical context (why we left Python)

The original Mens training pipeline used `mens/training/train.py` (Python, Unsloth, QLoRA). That caused:

1. **Environment friction**: Python version conflicts, uv/pip pinning, CUDA version mismatches  
2. **Slow iteration**: Python-based tokenizer was ~10× slower than native Rust for our dogfood path  
3. **Philosophical mismatch**: Vox could not dogfood training if the loop lived in another language  
4. **CI complexity**: Separate Python setup and heavy deps on every CI run  

**Original decision (March 2026):** Move the bulk of the pipeline to native Rust (**Burn 0.19** for scratch LoRA / experimentation), and initially assumed Python might remain for some large-model QLoRA work.

**Amendment:** Native **Candle + qlora-rs** now covers **HF-weight QLoRA** in-tree. See [ADR 006 — Mens full-graph Candle QLoRA with qlora-rs](006-mens-full-graph-qlora-qlora-rs.md), [ADR 007 — qlora-rs multi-layer training API](007-qlora-rs-multi-layer-training-api.md), and the SSOT [Mens native training](../reference/mens-training.md).

---

## Current architecture (summary)

| Concern | Historical (pre–native QLoRA) | Current |
|--------|-------------------------------|---------|
| Tokenizer (dogfood / VoxTokenizer JSONL) | Python | Rust (`VoxTokenizer` in `vox-tensor`) |
| Data loading (JSONL) | Python loop | Rust `JsonlDataLoader` |
| Synthetic / CLI data generation | `scripts/datagen.py` | `vox generate-data` (Rust) |
| Scratch / Burn LoRA (small model, wgpu) | Python training loop | `vox training native` / Burn paths in `vox-tensor` (legacy vs `vox mens train` dispatch — see SSOT) |
| **HF QLoRA (large models)** | Python (Unsloth) | **Rust:** `vox mens train` → **`CandleQlora`** + **qlora-rs**; weights via **Rust `hf-hub`** |
| Corpus extraction | Python | `vox mens corpus extract` (Rust) |
| Training validation | Python | `vox mens corpus eval` (Rust via `vox-eval`) |

**Dispatch note:** `vox mens train` is the canonical operator CLI. **`PopuliTrainBackend::BurnLora` is rejected at runtime**; the supported in-dispatch trainer for Mens fine-tuning is **`CandleQlora`**. Burn remains relevant for **legacy checkpoints**, **`vox mens merge-weights`**, and **`vox mens serve`** on merged `.bin` — not as the primary QLoRA path. Details: [mens-training.md](../reference/mens-training.md).

---

## Implementation pointers

- **Candle QLoRA / contract / preflight:** `crates/vox-populi/src/mens/tensor/` (`run_mens_training`, `lora_train.rs`, `finetune_contract.rs`, `preflight_train.rs`)  
- **Tokenizer + JSONL loader:** `crates/vox-tensor/src/data.rs`  
- **Burn model / optim (feature-gated):** `crates/vox-tensor/src/vox_nn.rs`, `optim.rs`, `train.rs`  
- **CLI:** `crates/vox-cli` — `vox mens train`, corpus and eval subcommands; `training/native.rs`, `training/datagen.rs` where applicable  

---

## Consequences

**Positive**

- **No Python** required for HF QLoRA fine-tuning in the default product path.  
- Native tokenizer remains fast for VoxTokenizer-shaped JSONL.  
- Single `vox` binary for data gen, corpus, eval, and Mens train.  
- Stronger Windows story than a Python+CUDA training stack.  
- Training data schema enforced in Rust (`TrainingPair`, contracts, preflight).  

**Negative / limits (see SSOT, not “use Python”)**

- **Execution kernel gaps:** Full causal NF4 blocks and other limits are documented in [candle-full-graph-feasibility.md](../archive/research-2026-q1/candle-full-graph-feasibility.md) and [mens-training.md](../reference/mens-training.md).  
- **Serving:** Merged QLoRA artifacts are aimed at **external** runtimes (vLLM, Ollama, HF, OpenAI-compatible); `vox mens serve` today targets the **Burn** merged-weights lane.  
- **Burn ecosystem** (where still used): fewer optimizers than PyTorch; cold wgpu builds can be heavy — mitigated by feature flags.  
- **Optional legacy:** Old Python scripts may still exist in trees or forks for one-off experiments; they are **not** the documented or dispatched path for Mens QLoRA.  

---

## References

- [Mens native training SSOT](../reference/mens-training.md)  
- [ADR 006 — Mens full-graph Candle QLoRA with qlora-rs](006-mens-full-graph-qlora-qlora-rs.md)  
- [ADR 007 — qlora-rs multi-layer training API](007-qlora-rs-multi-layer-training-api.md)  
- [ADR 001 — Burn backend selection](001-burn-backend-selection.md) (Burn rationale; amended for QLoRA)  
- [Native ML training pipeline](../explanation/expl-ml-pipeline.md)  
- `crates/vox-tensor/src/data.rs`, `crates/vox-cli/src/training/`  
- [Burn ML framework](https://burn.dev)  


