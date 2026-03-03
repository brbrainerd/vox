# Vox Architectural Organization & Governance

This document outlines the strict organizational principles for the Vox repository. Adherence is enforced via the `vox architect` command and the `vox-toestub` reasoning engine.

## 1. The Single Source of Truth (`vox-schema.json`)

All architectural rules are codified in `vox-schema.json` at the repository root. This file defines:
- **Crate Responsibilities**: Every crate in `crates/` must have a defined role.
- **Path Patterns**: Enforces where source files for each crate are allowed to exist.
- **Complexity Thresholds**: Global limits for file length and method density.

## 2. Core Constraints

### God Object Prevention
- **Max File Lines**: 500 lines. Files exceeding this must be decomposed.
- **Max Methods/Entities**: 12 per struct or file. Use trait objects or sub-modules to delegate responsibilities.
- **Trait Decomposition**: Prefer defining behavior in traits and implementing them in separate files (e.g., `feature/logic.rs` + `feature/traits.rs`).

### Sprawl Mitigation
- **Nesting Depth**: Maximum 5 levels deep.
- **Directory Density**: Maximum 20 files per directory. Group related logic into feature sub-directories with `mod.rs`.
- **Forbidden Names**: Generic filenames like `utils.rs`, `helpers.ts`, `misc.py`, or `common.vox` are strictly prohibited. Use descriptive, domain-aligned names.

## 3. The Staging Policy

New or experimental features should be placed in `src/staging/`.
- **Promotion Requirement**: To move from staging to a core crate, a module must pass a `vox review` and be architectural-compliance-clean.

## 4. Automation & Enforcement

### `vox architect check`
Validates that all crates are in their schema-defined locations. Run this before any major commit.

### `vox architect fix-sprawl --apply`
Automatically relocates crates that have drifted from the schema.

### `vox architect analyze <path>`
Performs a deep scan for God Objects and complexity anti-patterns.

### `vox check --strict`
Combines standard language checks (typeck, borrowck) with TOESTUB architectural validation.

## 5. Agent Guidelines

Agents are strictly forbidden from:
1. Creating files that violate the path patterns in `vox-schema.json`.
2. Adding logic to God Objects without first refactoring/decoupling.
3. Using forbidden generic names.

Violations will trigger a `ScopeViolation` or an `ArchitecturalFailure` event in the orchestrator.
