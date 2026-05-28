---
title: "populi-gpu-truth-probe-spec"
category: "Language Reference"
status: "current"
training_eligible: false
---
title: "Populi GPU truth probe specification (Native Layer A)"
description: "How probe-backed native hardware inventory flows into NodeRecord heartbeats and reconciles with ADR 018."
category: "Architecture SSOTs"
last_updated: "2026-04-18"
training_eligible: false
archived_date: 2026-04-18

schema_type: "TechArticle"
---

# Populi GPU truth probe specification (Native Layer A)

This document **implements** the probe slice of [ADR 018: Populi GPU truth layering](../adr/018-populi-gpu-truth-layering.md): **Layer A** fields on `NodeRecord` (`crates/vox-populi/src/node_registry.rs`) populated from the `HardwareRegistry` SSOT.

## Build / runtime

| Surface | Behavior |
| --- | --- |
| Default builds | Uses native `HardwareRegistry` (DXGI/DRM). `vox_populi::mens::hardware::probe` returns authoritative telemetry; join/heartbeat are grounded in facts. |
| `vox-populi` feature `mens-gpu` | Enables the full Mens GPU stack including `HardwareRegistry`. |
| `vox-cli` feature `populi` | Integrates hardware discovery for local node advertisement. |

Typical build:

```bash
cargo build -p vox-cli --features populi,mesh-nvml-probe
```

## Fields populated

When the probe succeeds, `node_record_for_current_process` (`crates/vox-populi/src/lib.rs`) sets:

- `gpu_total_count`, `gpu_healthy_count`, `gpu_allocatable_count` — from native enumeration (v1: healthy/allocatable match detected adapters).
- `gpu_inventory_source` — `"native_registry"`.
- `gpu_truth_layer` — `"layer_a_verified"`.
- `capabilities.min_vram_mb` — minimum **total** VRAM in MiB across devices, only if not already set by config.

## Heartbeat reconciliation

Operators should send the **same** [`NodeRecord`] shape on **join** and **heartbeat** (existing Populi HTTP contract). Rebuilding the record each tick via `node_record_for_current_process` (or equivalent) automatically refreshes Layer A after **GPU hotplug**, driver restart, or VM attach — subject to NVML visibility.

**Layer B** (allocatable after local reservations) and **Layer C** (labels/policy) remain separate; this spec does not merge operator lies with probe facts — ADR 018 precedence still applies when schedulers consume both.

## Related

- [ADR 018](../adr/018-populi-gpu-truth-layering.md)
- [Populi GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md)
- [Mens cloud GPU strategy](../reference/mens-cloud-gpu.md) (boundary vs Populi)




