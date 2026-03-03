---
title: "ADR 001 — Burn Backend Selection"
category: adr
constructs: []
last_updated: 2026-03-02
training_eligible: false
difficulty: advanced
---

# ADR 001 — Burn Backend Selection for vox-tensor

**Status**: Accepted
**Date**: 2026-03-02
**Author**: Vox Core Team

---

## Context

We needed a native Rust ML training framework for the Populi model. The options were:

1. **PyTorch via PyO3** — keep Python, use Rust bindings
2. **Candle (Hugging Face)** — Rust ML framework, CUDA-first
3. **Burn 0.19** — pure-Rust framework with pluggable backends
4. **ONNX Runtime** — inference-only, not useful for training

The goal: train Populi without requiring Python at all, allow CPU and GPU training, and compile on all major platforms including Windows.

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
- No pre-trained model loading yet (can't download Qwen2.5 weights into Burn)
- First cold build takes 10-15 min due to Wgpu and SPIR-V compilation
- For large-model fine-tuning (QLoRA), we still fall back to Python/Unsloth

**Mitigations:**
- Pin `burn = "0.19"` everywhere; add `[workspace.dependencies]` entry
- Use Python QLoRA path for large-model fine-tuning; native Burn for smaller architecture iterations
- Move Wgpu to feature flag so CI check builds skip it

---

## Alternatives Considered

### Candle
- Pro: HuggingFace maintained, CUDA optimized
- Con: Windows support poor, no Wgpu, requires CUDA at compile time

### PyTorch via tch-rs
- Pro: Mature ecosystem, full model zoo access
- Con: Requires LibTorch binary (400MB+), defeats "zero Python" goal

### ONNX Runtime
- Pro: Inference is fast
- Con: No training support

---

## References
- [Burn framework](https://burn.dev)
- `crates/vox-tensor/src/nn.rs` — VoxTransformer implementation
- `crates/vox-cli/src/training/native.rs` — Training loop
