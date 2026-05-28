---
title: "GPU Discovery & C++ Retirement Strategy (2026)"
description: "Authoritative reference for hardware discovery logic and C++ decommissioning status."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-18"
training_eligible: false
training_rationale: "Defines the authoritative strategy for hardware detection and build hygiene."
archived_date: 2026-04-18
---

# GPU Discovery & C++ Retirement Strategy (2026)

## Overview
As of April 2026, Vox has converged on a native, tiered hardware discovery registry within `vox-populi`. This system eliminates reliance on brittle shell-parsing (WMIC, nvidia-smi) for core sizing and telemetry, ensuring accurate VRAM and compute backend detection across Windows, Linux, and macOS.

## 1. Authoritative Implementation (SSOT)
The Single Source of Truth for hardware discovery is the `HardwareRegistry` singleton in `crates/vox-populi/src/mens/hardware/`.

### Probing Hierarchy
1.  **OS-Native Probes**:
    *   **Windows**: `IDXGIAdapter::GetDesc()` (via `windows` crate). Precise model names and VRAM.
    *   **Linux**: `/proc/driver/nvidia` (procfs) and `/sys/class/drm` (sysfs).
    *   **macOS**: Metal (stubbed).
2.  **Cross-Platform Fallback**:
    *   **WGPU**: `wgpu::Instance::enumerate_adapters`. Used when native probes fail or for generic vendor ID mapping.
3.  **Telemetry Fallback**:
    *   **NVML**: `nvml-wrapper` with dynamic loading. Used for high-precision utilization and temperature monitoring (NVIDIA only).

## 2. Hardware Taxonomy
We standardize on the following PCI Vendor ID mapping:

| Vendor | ID (Hex) | Compute Backend |
|---|---|---|
| **NVIDIA** | `0x10DE` | `Cuda` (via Burn/Candle) |
| **AMD** | `0x1002` | `Wgpu` (Vulkan/Metal) |
| **Intel** | `0x8086` | `Wgpu` (Vulkan/DX12) |
| **Apple** | `0x106B` | `Wgpu` (Metal) |

## 3. C/C++ Retirement Status
The project is actively transitioning to eliminate build-time C/C++ dependencies (`nvcc`, `cl.exe`).

### Milestone: Pre-compiled PTX Shimming (Complete)
*   **Current State**: `vox-populi` embeds pre-compiled `.ptx` kernels for all core NVIDIA operations (affine, conv, quantized, etc.).
*   **Outcome**: The requirement for `nvcc` and the full CUDA Toolkit during `cargo build` has been eliminated. Driver-only runtime execution is now standard.

## 4. Operational Invariants
*   **VRAM Reporting**: Always reported in Megabytes (MB) using the `BYTES_TO_MB` constant (1024^2).
*   **Headless Support**: Native probes are verified to work in headless/SSH/RDP sessions where display drivers are correctly initialized.

## 6. VoxScript-First Glue Directive
To prevent diagnostic fragmentation, all project automation and hardware-specific setup routines must be written as `.vox` scripts.
*   **Retired**: `.ps1`, `.sh`, and `.py` glue scripts are deprecated for project-internal automation.
*   **Canonical**: `vox run scripts/*.vox` for CI, corpus preparation, and hardware-specific shimming.
*   **Benefit**: Scripts are type-checked by the Vox compiler, cross-platform by default, and provide native telemetry.

## Cross-References
- [Vox Mens Qwen Migration](mens-qwen-family-migration-research-2026.md)
- [Populi GPU mesh implementation plan](../architecture/populi-gpu-mesh-implementation-plan-2026.md)
- [ADR 018: Populi GPU Truth Layering](../adr/018-populi-gpu-truth-layering.md)


