---
title: "vox-tensor-pytorch-gap-audit.md"
description: "Documentation for vox-tensor-pytorch-gap-audit.md."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
training_rationale: "Project architecture context."
archived_date: 2026-04-18
---
# Vox Tensor Gap Audit (April 2026)

With the enforcement of the **Zero Syntactic Configurability** limit and the banning of the "External Library Purgatory" anti-pattern, `vox-tensor` is designated as the sole native bridging layer for deep execution loop math (PyTorch parity).

This document serves as an inventory of current `vox-tensor` PyO3/Burn bindings against standard PyTorch primitives, identifying what is covered and the gaps that need prioritization for ML workflows.

## 1. Supported Neural Network Modules (`vox_nn.rs`)

`vox-tensor` currently wraps the following from the Burn backend, mirroring the PyTorch `nn` API:

*   **Linear/Dense:** `Linear`, `LoraLinear` (native LoRA representation)
*   **Convolutions:** `Conv1d`, `Conv2d` (missing `Conv3d`, `ConvTranspose2d`)
*   **Recurrent:** `Lstm` (guarded by `#[cfg(feature = "lstm")]`) (missing `GRU`, `RNN`)
*   **Attention & Transformers:** `MultiHeadAttention`, `TransformerEncoder` (missing `TransformerDecoder`)
*   **Normalization:** `BatchNorm` (covers 1D/2D implicitly based on input), `LayerNorm` (missing `GroupNorm`, `InstanceNorm`)
*   **Regularization:** `Dropout`
*   **Utility:** `Embedding`
*   **Containers:** `Sequential` (missing `ModuleList`, `ParameterDict`)

## 2. Supported Loss Functions

*   `cross_entropy_loss` (specifically `cross_entropy_with_logits`)
*   `mse_loss`
*   `bce_loss` (Binary Cross Entropy with Logits)

*Gaps:* `KLDivLoss`, `NLLLoss`, `HuberLoss`, `L1Loss`.

## 3. Supported Activations (`activations.rs`)

*   `ReLU`, `GELU`, `Sigmoid`, `Softmax`, `SiLU` (`Swish`), `LeakyReLU`, `Tanh`.

## 4. Tensor Operations & Manipulation (`tensor/`)

*   **Constructors (`ctor.rs`):** zeros, ones, random normal/uniform.
*   **Slicing & Reduction (`slice_reduce.rs`):** mean, sum, indexing.
*   **Reshaping (`cat_reshape.rs`):** concatenate, reshape, transpose.
*   **Element-wise (`elemwise.rs`):** arithmetic, pow, exp, log.

## 5. Architectural Gaps for "PyTorch Parity"

If an LLM attempts to construct a complex vision or generative model purely with `vox-tensor`, it will struggle due to the following gaps:

1.  **Missing Generative Decoders:** The lack of a `TransformerDecoder` module means full encoder-decoder architectures (e.g., T5) or autoregressive decoders (e.g., GPT) require manual assembly in `.vox` using `Linear` and `MultiHeadAttention` blocks, which negates the K-Complexity benefits. 
2.  **Missing Layer Manipulations:** `vox-tensor` heavily abstracts parameter wrapping. Standard PyTorch techniques like gradient clipping (`clip_grad_norm_`) or manually injecting state dicts (`load_state_dict`) lack clear interfaces in the existing struct definitions.

## Next Steps

All foundational ML structures, including multi-tensor forward pipelines for Sequence-to-Sequence generation, are now actively integrated and accessible through standard `.vox` ML execution without requiring syntactic workarounds. The bridge holds parity with Burn 0.14 native configurations.

