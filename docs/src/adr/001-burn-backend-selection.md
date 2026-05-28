---
title: "ADR 001 — Burn Backend Selection for vox-tensor"
description: "Official documentation for ADR 001 — Burn Backend Selection for vox-tensor for the Vox language. Detailed technical reference, architectu"
category: "Architecture Decisions (ADRs)"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 001 — Burn Backend Selection for vox-tensor

**Status**: Accepted (note 2026-04-06: Mens **QLoRA** on HF weights uses **Candle + qlora-rs** in `vox-populi`, not this Burn stack — see [ADR 003](003-native-training-over-python.md), [ADR 006](006-mens-full-graph-qlora-qlora-rs.md), [mens-training.md](../reference/mens-training.md))  
**Date**: 2026-03-02  
**Author**: Bert Brainerd

---

## Context

We needed a native Rust ML training framework for the Mens model. The options were:

1. **PyTorch via PyO3** — keep Python, use Rust bindings
2. **Candle (Hugging Face)** — Rust ML framework, CUDA-first
3. **Burn 0.19** — pure-Rust framework with pluggable backends
4. **ONNX Runtime** — inference-only, not useful for training

The goal: train Mens without requiring Python at all, allow CPU and GPU training, and compile on all major platforms including Windows.

---

## Decision

**Use Burn 0.19 with Wgpu backend (primary) and NdArray backend (CPU fallback).**

```rust
// Feature-gated in vox-tensor/Cargo.toml
[features]
default = []
gpu = ["burn/wgpu", "burn/ndarray"]
```

The `gpu` feature gates all Burn code, keeping `cargo check --workspace` fast (no GPU deps compiled in CI check).

---

## Consequences

**Positive:**
- Zero Python dependency for the training loop
- Runs on any hardware: CPU (NdArray), AMD/Intel/Metal/Wgpu (GPU)
- Clean Rust type system for tensor shapes prevents shape bugs at compile time
- `cargo build -p vox-cli --features native-train` gives a self-contained training binary

**Negative:**
- Burn 0.19 API breaks frequently between minor releases (must pin exact versions)
- The **Burn `VoxTransformer` scratch path** does not load full HF base weights the way the Candle QLoRA pipeline does (HF hub + safetensors for Mens is **`vox mens train --backend qlora`**, not Burn)
- First cold build takes 10-15 min due to Wgpu and SPIR-V compilation

**Mitigations:**
- Pin `burn = "0.19"` everywhere; add `[workspace.dependencies]` entry
- **Large-model QLoRA:** use native **Candle + qlora-rs** via **`vox mens train`** ([ADR 006](006-mens-full-graph-qlora-qlora-rs.md), [mens-training.md](../reference/mens-training.md)); use **Burn** for smaller scratch LoRA / legacy merge-weights + `vox mens serve` flows where still applicable
- Move Wgpu to feature flag so CI check builds skip it

---

## Alternatives Considered

### Candle (evaluation at the time of picking **Burn for vox-tensor**)

We chose Burn for the **small scratch transformer + wgpu** loop in `vox-tensor`. Candle was not selected for that slice.

- **Then:** Pro — Hugging Face–maintained, strong CUDA story; Con — we prioritized **wgpu** portability and kept Candle out of the initial `vox-tensor` trainer.
- **Now:** Candle is the **Mens HF QLoRA** execution kernel (`vox-populi`, qlora-rs, optional **`mens-candle-cuda`** / **`mens-candle-metal`**). MSVC/CUDA build notes live in workspace build policy (`.cursor/rules`, `AGENTS.md`). This ADR’s “alternatives” section records the **original** decision, not the full 2026 Mens stack.

### PyTorch via tch-rs
- Pro: Mature ecosystem, full model zoo access
- Con: Requires LibTorch binary (400MB+), defeats "zero Python" goal

### ONNX Runtime
- Pro: Inference is fast
- Con: No training support

---

## References
- [Burn framework](https://burn.dev)
- `crates/vox-tensor/src/vox_nn.rs` — VoxTransformer implementation (`gpu` feature)
- `crates/vox-cli/src/training/native.rs` — Training loop


