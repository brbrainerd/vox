---
title: "Mens Architecture 2026 Synthesis"
description: "Official documentation for Mens Architecture 2026 Synthesis for the Vox language. Detailed technical reference, architecture guides, an"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Mens Architecture 2026 Synthesis

> [!IMPORTANT]
> This document synthesizes the current architectural state of the Mens training pipeline, traces its mathematical foundations, and suggests strategic improvements based on the evolving ML landscape of 2026 (including Qwen3 MoE, QLoRA advancements, and Rust ML ecosystems).

## 1. Structure in Depth: The Current Mens Pipeline

Vox Mens is the unified native Rust AI/ML subsystem that moves Vox beyond legacy Python/PyTorch dependencies to a high-performance, safe, and easily distributable stack. The architecture is broadly segmented into four parts:

1.  **`vox mens corpus` (Data Pipeline)**: Extracts syntactically correct code samples directly from `.vox` files in the repository. It performs a semantic validation through the Vox compiler and tokenizes data via the deterministic, character-level `VoxTokenizer`.
2.  **`vox-tensor` (Core ML Primitives)**: The foundational crate that wraps backend logic. It abstracts tensors and Neural Network (`nn`) modules so they gracefully dispatch to specific device backends (WGPU, CUDA, Metal, NdArray).
3.  **`vox mens train` (Native Orchestrator)**: The heart of the fine-tuning process. The active and supported path is:
    *   **Candle qlora-rs (`--backend qlora`)**: Geared specifically for 16GB VRAM hardware (e.g., RTX 4080) fine-tuning industry models in the **Qwen 3.5** family (SSOT base: `Qwen/Qwen3.5-4B`; see [`mens-training.md`](../reference/mens-training.md)). It applies NF4 (4-bit NormalFloat) quantization to frozen Hugging Face (HF) base model weights while only training localized high-precision LoRA matrices.
    *   **Burn LoRA (`--backend lora`)**: historical path kept for context only; no longer the active training lane in current code.
4.  **`vox mens serve` (Inference Server)**: For QLoRA run directories, delegates to **`vox-schola serve`** (OpenAI-compatible HTTP); legacy Burn merged checkpoints remain a separate lane. See [`mens-serving-ssot.md`](../reference/mens-serving-ssot.md).

## 2. Mathematical Decisions & Foundations

The core mathematical architecture revolves around making Large Language Model (LLM) fine-tuning radically accessible on consumer hardware:

### Quantized Low-Rank Adaptation (QLoRA)
*   **Low-Rank Decomposition**: Instead of updating a massive weight matrix $W$ with a full gradient $\Delta W$, we decompose the updates functionally into $\Delta W = A \times B$, where $A \in \mathbb{R}^{d \times r}$ and $B \in \mathbb{R}^{r \times k}$. The Mens defaults are aggressively tuned for 16GB cards with $rank (r) = 16$ and $\alpha = 32.0$. This mathematically restricts the complexity of parameter updates while retaining expressivity.
*   **NF4 Quantization**: The base weights are frozen into a 4-bit NormalFloat (NF4) data type. NF4 is an information-theoretically optimal distribution for normally distributed neural network weights, guaranteeing uniform quantization bin mapping.
*   **Double Quantization**: In advanced runs, quantization constants themselves are downscaled from 32-bit to 8-bit to save an extra $\approx 0.4$ MB per parameter chunk.

### Loss Scaling and Target Mapping
*   **Burn Objective**: Predicts standard next-token Cross-Entropy (CE) over the complete model graph in `f32`.
*   **Candle Objective (Proxy Graphing)**: To bypass VRAM limitations, the Candle implementation uses `training_step_lm` over a bounded **proxy graph** consisting mostly of the LM head and an optional `o_proj`/`c_proj` stack. The Mens compiler introduces a suffix CE method `--qlora-ce-last-k`, where mathematical next-token Cross-Entropy is explicitly run on the last $K$ indices of a sequence only (acting essentially as instruction-answer sequence optimization), rather than a full causal decoder backprop.

## 3. What We Do Well (As of 2026)

*   **Python Elimination**: Bypassing the Global Interpreter Lock (GIL), Python environment hell, and runtime overheads. Integrating training directly into the CLI via `vox mens train` allows users to deploy reproducible compilation-and-training loops safely.
*   **Contract-first native path**: Vox uses a contract/planner-preflight flow with Candle QLoRA as the active execution kernel while preserving historical Burn context for migration clarity.
*   **Industry Class UX**: Mens's telemetry features an Exponential Moving Average (EMA) for reliable training times and true "Sample-based Counting" allowing stable loss scaling regardless of `grad_accum` sizes.

## 4. Gaps and Future Directions (Improvements for late 2026)

As we analyze the trends from late 2025 and 2026 (e.g., the introduction of Qwen3-Coder's MoE architectures and advanced Burn/Candle developments), several critical gaps in Mens emerge:

### A. Full-Graph NF4 + PEFT Parity in Candle
**The Gap:** Currently, Mens's Candle QLoRA backend uses a *bounded proxy graph*. It does not train the full causal NF4 decoder loop via qlora-rs because of missing capabilities in deep attention/FFN residuals. Loss curves between Burn and Candle cannot be compared apples-to-apples. 
**The Fix:** We must transition Phase 2c to a full causal NF4 + PEFT implementation, allowing us to accurately backpropagate through attention layers without exploding VRAM, eventually matching upstream Python `peft` capabilities. 

### B. Mixture of Experts (MoE) Architecture Adoption
**The Gap:** Qwen3-Coder (mid-2025) and Qwen3-Coder-Next (2026) achieve their state-of-the-art inference efficiency using expansive MoE architectures (e.g., activating only 35B parameters out of a 480B pool). Our native `LoraVoxTransformer` in Burn remains a classic dense transformer.
**The Fix:** Introduce native primitive layers for MoE routing within `vox-tensor`. Implementing "Hybrid Thinking Modes" natively inside the Burn graph would drastically cut computational budgets for code-generation verification loops while exponentially increasing agentic context length scaling up to 256K tokens natively.

### C. Legacy Burn `LoraAttention::merge` RoPE support
**The Gap:** Our current `LoraAttention::merge` path inside Burn mandates `use_rope == false` (GPT-2 logical style). Rotary Position Embeddings (RoPE) are mathematically essential for modern contexts (used by Qwen and Llama), but our RoPE stacks remain unmerged in Burn.
**The Fix:** Complete the mathematical formulation for merging LoRA layers across RoPE-injected vectors to allow `--backend lora` to fully support modern Qwen/Llama architectures natively inside Vox.

### D. Export Pipelines for External Runtimes
**The Gap:** Mens's `merge-qlora` command outputs raw `.safetensors`, but we cannot serve nested qlora adapters within our own `vox mens serve`. Users are forced to eject the pipeline into an external runtime (Ollama, vLLM).
**The Fix:** Expand our native Candle execution server or extend Burn's inference loaders to interpret `QloraAdapterMetaV2` and `v3` schemas, creating a seamless "Train-in-Candle, Serve-in-Vox" pipeline for large open-weight models.

### E. Dedicated Research Reasoning Adapter (Lane G)
**The Gap:** Research synthesis is currently performed by code-generation models, leading to low-quality evidence summaries and poor contradiction resolution.
**The Fix:** Train Lane G (research-expert) via GRPO+RLVR to specialize in evidence synthesis and multi-hop reasoning.

## 5. Provenance and attribution as first-class training metadata

MENS must treat model lineage as part of the run contract, not as an afterthought in release notes.
This is especially important when using open-weight upstream bases and applying downstream continued
pretraining and RL. Training artifacts should carry:

- upstream family and model id,
- license classification and attribution expectations,
- whether attribution is required for a promoted artifact.

This keeps compliance visible to operators and avoids ambiguity during model promotion and external
distribution. Supporting evidence and confidence labels for the 2026 Composer/Kimi discussion are
tracked in [`mens-composer-kimi-findings-2026.md`](mens-composer-kimi-findings-2026.md).


