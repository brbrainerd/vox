---
title: "Mens Cloud GPU Training Strategy"
description: "Official documentation for Mens Cloud GPU Training Strategy for the Vox language. Detailed technical reference, architecture guides, an"
category: "Language Reference"
last_updated: "2026-03-29"
training_eligible: true

schema_type: "TechArticle"
---

# Mens Cloud GPU Training Strategy

This page documents what is implemented now in cloud-profile selection and what remains experimental.

## Implemented behavior (code-aligned)

- Local 4080-class training remains the baseline: `vox mens train --backend qlora --preset 4080`.
- `DEFAULT_PRESET` is `4080` in `preset_schema`.
- `4080` is an alias of `qwen_4080_16g` in in-code preset shaping.
- `--preset auto` resolves from `mens/config/gpu-specs.yaml` (`presets` table) by VRAM fit.
- CUDA VRAM hinting may also select QLoRA presets through `vram_autodetect` helper output.

## Canonical preset sources

- Runtime preset defaults and aliases: `crates/vox-populi/src/mens/tensor/preset_schema.rs`.
- Runtime VRAM autodetect helper: `crates/vox-populi/src/mens/tensor/vram_autodetect.rs`.
- SSOT GPU/preset data for local + cloud estimators: `mens/config/gpu-specs.yaml`.

## Profile compatibility matrix (practical)

| Surface | Supported now | Notes |
| --- | --- | --- |
| Local workstation (4080 class) | Yes | Primary baseline; recommended default path. |
| Local higher VRAM (24G/48G/80G) | Yes | Use explicit preset or `--preset auto`. |
| `vox mens train --cloud ...` dispatch | Feature-gated | Requires `vox-cli` built with `cloud`; provider dispatch path exists but should be treated as additive. |
| Remote execution via Populi routing hints | Read-only scheduling signal | Hints enrich placement choices; execution remains local-safe unless explicitly extended. |

## Boundary vs Populi mesh

These surfaces should not be conflated:

- **Local MENS training:** the primary and best-supported path today.
- **Cloud provider dispatch:** a separate, feature-gated path for provisioning or sending work to external providers.
- **Future Populi-managed GPU mesh:** a research target for user-owned local or overlay-connected clusters, **not** current shipped behavior.

Important current boundary:

- Populi node visibility and routing hints do **not** yet form an authoritative GPU scheduler.
- `vox mens train --cloud` and Populi mesh are **different execution surfaces** with different trust, networking, and lifecycle assumptions.
- Remote execution through Populi remains experimental and local-safe unless a future design adds explicit ownership, checkpointing, and recovery semantics.

See [Populi GPU network research 2026](../archive/research-2026-q1/populi-gpu-network-research-2026.md) for the gap analysis and external guidance that should inform the later implementation plan.

**Placement boundaries:** [work-type placement policy matrix](populi-work-type-placement-matrix.md); **execution ownership (design intent):** [ADR 017](../adr/017-populi-lease-remote-execution.md); **GPU inventory layering:** [ADR 018](../adr/018-populi-gpu-truth-layering.md).

## Non-goals (current wave)

- No promise of full provider-native lifecycle automation parity across all clouds.
- No replacement of local-first runbook with cloud-only assumptions.
- No second preset stack: cloud path reuses the same preset machinery as local.
- No claim that cloud dispatch and Populi mesh already form one unified GPU fabric.

## Operational guidance

- Keep `4080` as first-pass default for regression and acceptance gating.
- Use cloud dispatch when you need faster iteration or larger VRAM, not as a dependency for baseline dev flow.
- For interruptible cloud hosts, persist `--output-dir` to durable storage and avoid `--force-restart` unless intentionally resetting.

