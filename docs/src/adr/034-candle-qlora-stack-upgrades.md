---
title: "ADR 034 — Candle / QLoRA stack upgrades"
description: "Decision record: defer Candle/peft/qlora/version-unification to a dedicated upgrade train with GPU CI."
category: "Architecture Decisions (ADRs)"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "ADR text encodes stack-upgrade policy and risk gates for MENS/Populi GPU paths; useful for model grounding on dependency discipline."

schema_type: "TechArticle"
---

# ADR 034 — Candle / QLoRA stack upgrades (deferred batch)

## Context

- Mens / Populi training paths depend on **Candle 0.9.x**, **qlora-rs** (vendored patch), **peft-rs**, and transitive stacks (`zip`, CUDA kernels).
- The workspace dependency audit shows **duplicate majors** (e.g. `zip`) that cannot be collapsed without coordinated Candle + HF ecosystem bumps.
- GPU builds (`mens-candle-cuda`, NVCC, MSVC toolchain) require explicit CI coverage.

## Decision

- **No ad-hoc Candle major bump** inside manifest-normalization PRs.
- Track **one upgrade initiative** with: MSRV check, `cargo vox-cuda-release` smoke, MENS eval matrix slice, and lockfile diff review for `zip` / `rand` / `half` transitive shifts.
- Keep using workspace pins + patches documented in root `Cargo.toml` until the upgrade PR lands.

## Status

**Proposed** — execution gated on GPU CI sign-off and a green `cargo check --workspace` + targeted training smoke.

## Consequences

- Duplicate transitive versions may persist until the upgrade lands; document them in [workspace dependency audit](../architecture/workspace-dependency-audit-2026.md).
