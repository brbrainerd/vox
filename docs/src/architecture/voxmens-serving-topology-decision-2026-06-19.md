---
title: "VoxMens Serving Topology Decision"
description: "Decision record for the multi-LoRA spoke serving architecture and base model selection."
category: "architecture"
status: "current"
---

# VoxMens Serving Topology Decision

This document records the architectural decision for serving trained adapters and selecting base models across different spokes in VoxMens.

## Decision Summary

1. **Local/Offline Base Selection**: Base model selection (capability tag to VRAM-fit base ID mapping) is resolved locally/offline at training time. There is no active network coupling or central inference serving registry required during fine-tuning.
2. **Adapter Multiplexing via DomainRouter**: Inference-time multiplexing of trained spoke adapters is performed by hot-swapping domain adapters on top of a shared running base model. This leverages the existing [DomainRouter](file:///c:/Users/Owner/vox/crates/vox-populi/src/mens/tensor/domain_router.rs) architecture.
3. **No S-LoRA Dependency**: S-LoRA (or similar dynamic multi-adapter serving frameworks) is not introduced into the training or inference pipeline. The system maps and loads adapters using direct, simple filesystem structures.
4. **Heterogeneous Base Constraint**: If different spokes target different base models (e.g. 3B vs 7B models), they cannot be loaded concurrently in a single running instance of a GPU runner without reloading the base model or running multiple backend serving processes.

## Cross-References

- For the boundary rules and runtime dispatch contract, see the [model selection convergence design](file:///c:/Users/Owner/vox/docs/superpowers/specs/2026-06-19-voxmens-model-selection-convergence-design.md).

## Preset Name Canonicalization

To align the preset schema across different hardware specifications and configurations:
- **Canonical Preset**: `qwen_4080_16g` is the canonical ID representing the RTX 4080 class (16GB VRAM) training profile.
- **Legacy Alias**: `prosumer_16g` is kept as a documented alias mapping directly to `qwen_4080_16g` in the registry (`preset_schema.rs`).
This ensures consistency between `domain-profiles.yaml` and client autodetect logic while maintaining backward compatibility with cloud sizing estimations.

