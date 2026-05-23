---
title: "Feature growth and boundaries SSOT (2026)"
description: "Governance for the Vox feature surface, crate sprawl limits, and the deterministic projection parity gate."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-21"
training_eligible: true
training_rationale: "Defines the limits of the Vox feature set and the parity gates used to enforce them."
schema_type: "TechArticle"
---

# Feature growth and boundaries SSOT (2026)

This document defines the authoritative boundaries for Vox feature growth to prevent "God Object" sprawl and crate bloat. It also documents the machine-verified gates that ensure language surface stability across the compiler, runtime, and app contract.

## The Drift Gate (Projection Parity)

To ensure that the WebIR, AppContract, and RuntimeProjection do not drift, we enforce a deterministic projection mapping. Any changes to the core language surface that impact the target application projection must be reflected in the test suite.

The canonical gate is:
- **Test:** `projection_parity`
- **Validation:** `projection_triplet_is_deterministic`
- **Reproduction:** `cargo test -p vox-compiler --test projection_parity`

Failure of this test indicates a non-deterministic or unmapped change to the HIR/IR transformation that breaks the AppContract stability guarantee.

## Sprawl Limits

- **Maximum Workspace Crates:** 40 (Audit April 2026 identified 63; remediation active).
- **Core Compiler LOC Limit:** 50 kLOC.
- **Orchestrator Boundary:** DEI daemon must remain protocol-agnostic.

## Crate Modularization

Every new feature requiring a dependency with >10 downstream crates MUST be placed in its own workspace crate (e.g., `vox-oratio` for speech-to-code) to keep the core `vox` binary and `vox-compiler` library lean.

## Governance

Changes to the feature set defined here require an ADR (Architectural Decision Record) and a corresponding update to the operations catalog.

