---
title: "Vox Configuration Architecture (SSOT)"
description: "Single Source of Truth for Vox Configuration architecture and the three-tier precedence rules."
category: "Architecture SSOTs"
training_eligible: false
archived_date: 2026-04-18
---

# Vox Configuration Architecture (SSOT)

This document serves as the Single Source of Truth (SSOT) for the Vox Configuration architecture, establishing the three-tier precedence rules and defining the boundary between secrets, tuning parameters, and runtime environments.

## The Three-Tier Precedence

All configurable boundaries in Vox adhere to the following resolution cascade. The highest priority is resolved first.

1.  **Environment Variables (Escape Hatch)**
    The highest precedence. Primarily meant for CI gates, ephemeral overrides, or Docker deployments. If an environment variable is set (e.g., `VOX_DB_CIRCUIT_BREAKER`), it immediately shadows lower tiers.

2.  **Layered Configuration (The Standard)**
    The local TOML configurations at `~/.vox/config.toml`. All standard user and node tuning configurations must be persisted here via the `vox config set` interface.

3.  **Hardcoded Defaults**
    The lowest fallback. Specified inline via the `vox_config::env_parse::resolve_config_*` bindings within the Rust implementation.

## Schema Taxonomy

To prevent conflation, the `OperatorEnvSpec` registry assigns every tuning variable a formal `ConfigClass` taxonomy:
-   **`UserPreference`**: Ergonomic settings (e.g., terminal output colors). Can be written to `~/.vox/config.toml`.
-   **`NodeLocal`**: Node performance parameters (e.g., worker counts, buffer sizes). Can be written to `~/.vox/config.toml`.
-   **`Bootstrap`**: Immutable startup definitions (e.g., repository roots).
-   **`CiGate`**: High-risk environment overrides. **Strictly prohibited** from being loaded through `~/.vox/config.toml`.

## Synchronized Settings across VoxDB

To support sovereign multi-device environments, Vox enables explicit cross-device sync of the local `config.toml` using the `vox config sync` CLI tooling.
This process relies on the underlying `account_config` table in the VoxDB mesh.

**Commands:**
-   `vox config sync --push`: Snapshot all explicitly defined local `config.toml` key-value pairs into the `account_config` VoxDB table.
-   `vox config sync --pull`: Overwrite the local `config.toml` configuration properties with the database canonical versions.

## Code Standards

1.  **Deprecation of `std::env::var`**: Do not use `std::env::var` directly to pull configuration parameters. You **must** utilize the bindings under `vox_config::env_parse::*` (e.g., `resolve_config_bool`, `resolve_config_u64`).
2.  **Secret Isolation**: **Never** store secrets in `~/.vox/config.toml`. All secrets must strictly route through `vox_clavis` and the Zero-Knowledge vault.

**See Also:**
-   Secret Management SSOT: [Clavis SSOT](../reference/clavis-ssot.md)
-   Terminal Policy: [Terminal AST Validation Research](terminal-ast-validation-research-2026.md)

