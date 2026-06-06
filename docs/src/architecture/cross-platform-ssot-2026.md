---
title: "Cross-Platform Compatibility SSOT (2026)"
description: "Canonical source of truth for Vox cross-platform compatibility invariants across macOS, Windows, and Linux."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-13"
training_eligible: true
---

# Cross-Platform Compatibility SSOT (2026)

This document defines the normative requirements for maintaining Vox as a first-class citizen on macOS, Windows, and Linux.

## 1. Supported Platforms Matrix

| OS | Arch | Target Triple | Tier |
|----|------|---------------|------|
| **Linux** | x64 | `x86_64-unknown-linux-gnu` | Tier 1 (Primary CI/CD) |
| **Windows** | x64 | `x86_64-pc-windows-msvc` | Tier 1 (Development) |
| **macOS** | arm64 | `aarch64-apple-darwin` | Tier 1 (Development) |
| **macOS** | x64 | `x86_64-apple-darwin` | Tier 2 (Release only) |

## 2. Toolchain Invariants (SSOT)

The workspace versions are locked in `contracts/toolchain/workspace-toolchain.v1.yaml`.

- **Rust:** 1.95.0 (standardized in `rust-toolchain.toml`)
- **Node.js:** 24.x (standardized in all CI workflows)
- **pnpm:** 9.x
- **CUDA:** 12.1 (runtime driver dependency, not build-time toolchain)

## 3. Platform-Specific Enforcement (Sandbox)

| OS | Mechanism | Enforcement Status |
|----|-----------|--------------------|
| **Linux** | Landlock LSM | Full (Filesystem Read/Write) |
| **Windows** | Job Objects | Partial (Memory/Kill-on-close) |
| **macOS** | Fallback | Warning only (Entitlement-based in GUI) |

## 4. Hardware Probing (GPU/ML)

- **Linux:** Probes via DRM (`/dev/dri/`).
- **Windows:** Probes via DXGI / `windows_fallback.rs`.
- **macOS:** Probes via Metal (`macos_metal.rs`).
- **Nvidia (Cross):** Probes via `vox-plugin-nvml-probe` when installed.

## 5. Development Constraints

### Filesystem Paths
- **Rule:** Never use `/` or `\` in string literals for internal paths.
- **Implementation:** Use `std::path::PathBuf`.
- **Enforcement:** `layers.toml` rule `no-unix-path-literals`.

### Shell Execution
- **Rule:** Automation is `.vox` only (executed via `vox run`).
- **Implementation:** `AGENTS.md §VoxScript-First Glue Code`.
- **Enforcement:** `layers.toml` rule `no-platform-exec-outside-cfg`.

### Line Endings
- **Rule:** LF for all source and markdown. CRLF for `.ps1` only.
- **Enforcement:** `vox ci line-endings`.

## 6. Tauri Mobile Matrix

| Feature | Android | iOS |
|---------|---------|-----|
| Codegen | Cross-platform | Cross-platform |
| Build | macOS / Linux | macOS Only |
| ASR (Sherpa) | Native Plugin | Native Plugin |

## 7. CI Verification Policy

- **Linux (Self-hosted):** Full workspace test + LLVM Coverage on every PR.
- **Windows/macOS (GitHub-hosted):** `cargo check --workspace` + platform-sensitive tests on PR (crates: `vox-config`, `vox-cli-core`).
- **Release Matrix:** Full binary + GUI bundle verification on tag.
