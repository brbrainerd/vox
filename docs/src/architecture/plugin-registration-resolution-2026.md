---
title: "Plugin Registration Resolution Path (2026)"
description: "Documentation of the resolution path for plugin registration errors, specifically related to the tensor-burn-wgpu regression in MENS."
category: "Language Reference"
status: "current"
training_eligible: false
---
# Plugin Registration Resolution Path (2026)

This document records the resolution path for the `tensor-burn-wgpu` to `mens-candle-cuda` regression in the `vox-ml-cli` pipeline, providing a template to prevent recurring CI/CD failures during the plugin system migration.

## The Problem

In early 2026, the `vox mens train` command failed with the following error:
```
This Vox capability requires the 'gpu' plugin, which is not installed.
To install it, run: vox plugin install tensor-burn-wgpu
```
However, running the suggested command failed, and the plugin `tensor-burn-wgpu` did not exist in the catalog (`crates/vox-plugin-catalog/catalog.toml`).

## Root Cause

During the Sub-Project 3 (SP3) Plugin System Redesign, ML backend functionality was extracted out of `vox-populi` into dedicated plugins. The `tensor-burn-wgpu` plugin was either deprecated or never materialized in favor of `mens-candle-cuda` as the canonical CUDA/QLoRA backend. The fallback error message in `crates/vox-ml-cli/src/commands/mens/populi/dispatch.rs` was not updated to reflect this catalog change, leading to a hallucinated resolution path.

## Resolution Steps

1. **Verify the Canonical Catalog**: Always check `crates/vox-plugin-catalog/catalog.toml` to confirm which plugins are actively supported. In this case, `mens-candle-cuda` was listed as the correct plugin exposing the `MlBackend` extension point.
2. **Update the Dispatch Guard**: The `cfg(not(feature = "gpu"))` guard in `crates/vox-ml-cli/src/commands/mens/populi/dispatch.rs` was updated to suggest installing `mens-candle-cuda`.
3. **Verify Feature Compilation**: The training CLI was verified to compile gracefully and fall back to HuggingFace dispatch or mock when compiled without the `mens-candle-cuda` native link dependencies. 
4. **Environment Isolation**: Native link errors for CUDA (e.g., `cublasLtEmulationDescInit_internal`) when compiling `vox-ml-cli` directly with `--features gpu` indicate that the host environment lacks the correct MSVC/CUDA configuration. The architectural intent is that end users rely on pre-compiled plugin binaries or dynamic loading rather than compiling the `gpu` feature directly on incompatible hosts.

## Future Prevention

- **Catalog SSoT Rule**: All CLI fallback paths suggesting a plugin installation must be integration-tested against the active `vox-plugin-catalog`.
- **Error Messages**: Hardcoded plugin names in `anyhow::bail!` statements are a code smell. Future architecture should dynamically read the suggested plugin for a given capability from the `VoxPluginRoot` or extension-point registry.
