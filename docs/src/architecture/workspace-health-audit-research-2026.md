---
title: "workspace-health-audit-research-2026"
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---
# Research: Workspace Health & Dependency Governance (2026)

## Overview
This document synthesizes improvements made to the Vox architectural enforcement pipeline in April 2026. The goal is to move from fragile, hardcoded heuristics to a machine-verifiable, policy-driven system that surfaces architectural drift and performance regressions automatically in CI/CD.

## Identified Fragilities (Critique)
1.  **Manual Heuristics**: Initial audit scripts relied on string-parsing `Cargo.toml`, which is prone to failure on multi-line entries, workspace inheritance, and complex feature flags.
2.  **Hardcoded Thresholds**: Values like dependency limits (25, 80, 100) were embedded in code, making them difficult to tune across different environments.
3.  **Static Regression Analysis**: Build-time regressions were limited to a fixed 1.5x ratio in the database layer, lacking the flexibility to adjust sensitivity for critical lanes.
4.  **Implicit Tiers**: Crate categorization (Foundational vs Leaf) was based on simple string matching of crate names, which doesn't scale as the repository grows.

## Improvements Implemented

### 1. Centralized Policy SSOT
Moved all thresholds and categorization logic to `docs/ci/workspace-health-policy.json`. This allows the same policy to be shared across the audit script, CLI tools, and documentation.

### 2. Standardized Dependency Auditing
Refactored `audit-workspace-health.vox` to utilize `cargo metadata --no-deps`. This provides:
- **Accuracy**: Correctly identifies direct dependencies even with workspace inheritance.
- **Auto-Categorization**: Crates are automatically assigned to tiers (Foundational, Runtime, Leaf) based on configurable patterns.
- **ML Leak Detection**: Automatically flags if a Foundational crate pulls in heavy ML libraries (Burn/Candle/Mens).

### 3. Dynamic Performance Monitoring
Updated `vox db build-regressions` and the underlying `VoxDb` query to accept a dynamic threshold parameter.
- **Sensitivity**: CI can now run strict checks (e.g., 1.1x) for core crates while allowing more variance (e.g., 1.5x) for experimental feature lanes.
- **Telemetry-Driven**: The audit script now fetches these results directly from the production build-telemetry store.

## CI/CD Integration Plan
The `audit-workspace-health.vox` script is now a mandatory blocking gate in the Vox pipeline.

| Stage | Action | Policy |
|---|---|---|
| **Pre-commit** | `vox run scripts/quality/audit-workspace-health.vox` | Warn on sprawl |
| **Pull Request** | `vox ci audit` (Proposed) | Block on regressions > 1.25x |
| **Release** | `vox db build-health --repo vox-core` | Archive architectural snapshot |

## Future Work (Unsurfaced Issues)
1.  **Layer Violation Detection**: Use the dependency graph from `cargo metadata` to enforce that Foundational crates never depend on Runtime or Leaf crates (Circular dependency check).
2.  **Feature Sprawl**: Detect when a single crate has too many optional features, suggesting it should be split.
3.  **Doc-Code Parity**: Automatically verify that every new crate added to `crates/` has a corresponding entry in `docs/src/architecture/`.

---
*Created by Antigravity (April 2026)*
*See: AGENTS.md §Structural Limits & Code Quality*

