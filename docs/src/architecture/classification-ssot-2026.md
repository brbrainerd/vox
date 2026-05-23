---
title: "Classification Taxonomy SSoT"
description: "Single Source of Truth for Vox component classification and naming conventions."
category: "Architecture SSOTs"
sort_order: 15
status: "current"
---

# Classification SSoT (2026)

This document serves as the canonical map for the structural classification of Vox components, as referenced by `AGENTS.md`.

## Core Subsystems

| Domain | Crate Prefix | Responsibility |
|---|---|---|
| **Core Compiler** | `vox-compiler` | Lexing, parsing, HIR lowering, type checking, and IR emission. (Replaces the legacy split lexer/parser crates.) |
| **Orchestration** | `vox-orchestrator` | Agent execution loop, multi-agent coordination, task dispatch/session orchestration (historical `vox-dei` retired — see root `AGENTS.md` retired surfaces). |
| **Capabilities** | `vox-skills` | Isolated agent capabilities (e.g., file system access, network requests). |
| **Game/Combat** | `vox-gamify` | Dystopia MUD modernization, Monte Carlo simulations. |
| **Security/Secrets**| `vox-secrets` | Secret management, API key resolution. |
| **Cryptography** | `vox-crypto` | Pure-Rust cryptographic operations (AEGIS banned). |

## Nomenclature Invariants

- **VoxScript (`.vox`)**: The sole glue language for the repository.
- **HIR**: High-level Intermediate Representation. The target of all structural simplifications.

For deeper architectural constraints, see `docs/agents/governance.md`.
