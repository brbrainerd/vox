---
title: Vox Feature Builds & Capabilities
category: "Language Reference"

schema_type: "TechArticle"
---
# Vox Feature Builds & Capabilities

Vox uses Cargo features to manage build times, binary size, and hardware dependencies (e.g., CUDA, Metal). This document outlines the canonical build profiles and how the system dynamically handles capability discovery.

## Capability Discovery & Drift Guard

As of v0.1.0, the **Vox Build Meta** architecture ensures the binary tracks its own compilation features.
When a user attempts to run a feature-gated command (like `vox mens train` or `vox oratio`) on a binary that lacks the required feature, the CLI intercepts the command and provides an actionable rebuild instruction instead of failing with a generic error.

Features are captured in `FEATURES_JSON` via `vox-build-meta` at compile time and validated dynamically at runtime.

### The Drift Guard (TOESTUB)

The workspace enforces dependency drift protection via the `WorkspaceDriftDetector` in `vox-code-audit`:
- **Orphan Crates:** Crates located in `crates/` but missing from the root `Cargo.toml` `[workspace.dependencies]` are flagged.
- **Inheritance:** The use of inline `path =` dependencies instead of `workspace = true` is forbidden to ensure workspace configuration hygiene.

## Feature Profiles

### 1. Minimal / Core (Default)
**Build Command:** `cargo build -p vox-cli`
- Supports the core language compiler, LSPs, package management, and system tasks.
- Excludes heavy ML dependencies, scripting engines, and gamification logic.

### 2. Script Execution
**Build Command:** `cargo build -p vox-cli --features script-execution`
- Adds the `vox script` lane for fast execution of `.vox` files in a native runner cache.

### 3. Speech-to-Text (Oratio)
**Build Command:** `cargo build -p vox-cli --features oratio`
- Enables `vox oratio` (transcriptions) and microphone capture support (`oratio-mic` where supported).
- Connects the Whisper / Candle ASR backend.

### 4. GPU / Model Training (Mens)
**Build Command:** `cargo build -p vox-cli --features gpu`
- Highly recommended for developers with an RTX 4080+ or equivalent.
- Unlocks local QLoRA training (`vox mens train`), dogfood evaluation, and local serving (`vox mens serve`).

### 5. DEI / Agent Pipelines
**Build Command:** `cargo build -p vox-cli --features mens-dei`
- Contains dependencies for workflow processing, code-review lanes (`vox review`), and AI agents.

## Handling Missing Features

If you hit an unimplemented branch error like this:
```text
[capabilities] Feature 'gpu' is required for this command.
Rebuild the CLI using:
    cargo build -p vox-cli --features gpu
```
Simply copy and run the suggested `cargo build` command in the workspace root to unlock the feature.
