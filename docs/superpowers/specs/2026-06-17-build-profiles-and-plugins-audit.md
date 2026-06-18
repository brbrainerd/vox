---
title: "Build Profiles & Plugins Audit"
description: "Architectural audit of Vox build distribution targets (mobile, desktop, full, light) and plugin boundaries."
category: "architecture"
status: "current"
---

# Build Profiles & Plugins Audit

This document reviews and defines the canonical build profiles and plugin boundaries for the Vox codebase. It details how the core language compiler remains lightweight and portable while supporting rich desktop, server/CUDA, and mobile applications.

## 1. Overview & Core Principles

To prevent dependency creep and build bloat, the Vox distribution architecture must adhere to two core principles:
1. **The Core Toolchain is Light and Database-Free**: The compiler (`vox-compiler`), code generator (`vox-codegen`), and parser (`vox-ast`) must never depend on database engines (`vox-db`), model orchestration (`vox-orchestrator`), or heavy search indexers (`vox-search`).
2. **Heavy Integrations are Subprocesses or Plugins**: Heavy features like Machine Learning (Candle CUDA/Metal) must be packaged as standalone helper binaries (like `vox-ml-cli`), and heavy I/O systems (like browser automation or RSS/Atom feed parsing) must be packaged as dynamically loaded plugins.

---

## 2. Proposed Build Profiles (The 4 Canonical Targets)

We define four standard distribution builds:

### A. Leaner/Light Build (Compiler-only)
* **Target Binary**: `vox-langtool` (named `vox-min` or `voxc`).
* **Purpose**: A minimal toolchain for syntax checks, linting, code formatting, and fast script execution.
* **Dependencies**: `vox-compiler`, `vox-codegen`, `vox-ast`, `vox-lsp` (compiled without DB features).
* **Footprint**: `< 50 MB` binary, zero external runtime services required.

### B. Mobile Build
* **Target Application**: React Native + Expo App using `@vox/runtime-rn`.
* **Purpose**: A mobile app running Vox user interfaces (VUV) and executing local tasks.
* **Architecture**: 
  - The TypeScript codegen outputs native-compatible TSX.
  - Due to OS-level sandboxing (especially iOS App Store rules), **dynamic loading of native libraries (`dlopen`) is prohibited**. Therefore, mobile capabilities cannot use native code plugins. All mobile capability extensions must be statically compiled React Native native modules or implemented entirely in JS/TS.
  - Local database is SQLite local storage (server databases like PostgreSQL/MySQL are not supported or used in Vox).

### C. Desktop Build
* **Target Application**: Tauri desktop app (`vox-gui`) bundling the `vox-cli` binary.
* **Purpose**: The standard developer workstation experience, with a rich UI, command palette, local vector search, and agent console.
* **Dependencies**: `vox-cli` with default features enabled (`keyring-store`, `fuzzy-search`, `database`, `heavy-retrieval`).
* **Plugin Support**: Full support for dynamic code plugins and agent skills.

### D. Full Build (Server / CUDA)
* **Target Binary**: Full `vox-cli` + `vox-ml-cli` with CUDA/Metal support.
* **Purpose**: High-performance ML training/fine-tuning (Candle QLoRA), local model serving, in-process orchestrator mesh nodes, and heavy background automation.
* **Dependencies**: All features enabled (`mens-candle-cuda`, `mcp-server`, `script-wasi`, `workflow-runtime`).

---

## 3. Plugin Boundaries (What is a Plugin vs. Core)

To maintain clean architecture, features are categorized into one of four packaging types:

| Component | Packaging Type | Reason |
|-----------|----------------|--------|
| **Parser & Compiler** | **Core Binary** | Essential language functionality; must compile instantly. |
| **CLI Command Routing** | **Core Binary** | Orchestrates subcommand delegation. |
| **Candle ML Engine** | **Subprocess Binary** | Avoids linking Candle/CUDA into the main CLI (which would add ~500MB+ to every download). |
| **Browser Automation** | **Dynamic Code Plugin** | `chromiumoxide` and browser management are heavy and only needed by web-agent tasks. |
| **Wasmtime Sandbox** | **Dynamic Code Plugin** | WASI skill execution runtime is heavy; can be loaded on-demand. |
| **Social Media / RSS Ingest** | **Dynamic Code Plugin** | Avoids pulling `feed-rs` and API clients into the default CLI build. |
| **Agent Skills (agentskills.io)** | **Dynamic Skill Directory** | Externally maintained Markdown + tools; loaded at runtime via lexical search. |

---

## 4. Recommendations & Code Cleanup

1. **Verify that retired references remain absent**: Keep the `catalog.toml` clean of references to retired binaries (`vox-bootstrap`, `vox-schola`).
2. **Align `vox-mobile` Bundle**: Update the `vox-mobile` bundle definition in `catalog.toml` to specify Expo/React Native as the target platform rather than Tauri Mobile.
3. **Keep `vox-langtool` Lean**: Ensure that future additions to `vox-langtool` do not transitively import database or orchestrator crates.
