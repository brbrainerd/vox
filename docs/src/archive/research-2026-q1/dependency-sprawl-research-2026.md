---
title: "Dependency Sprawl Audit and Resolution (2026)"
category: "Architecture SSOTs"
status: "research"
training_eligible: false
archived_date: 2026-04-18
---

# Dependency Sprawl Audit and Resolution (2026)

## Overview

This document records the audit and subsequent remediation of dependency sprawl within the Vox workspace. As the project scaled, individual crates began declaring explicit versions for external dependencies (e.g., `axum`, `uuid`, `gix`, `jj-lib`) rather than inheriting them from the workspace root. This led to:
1. Increased risk of duplicate compilation (multiple semver-compatible versions in `Cargo.lock`).
2. Fragmented security auditing (difficulty in verifying which version of a library is used globally).
3. Drift in architectural consistency.

## Theoretical Justification

Cargo workspaces allow centralizing version definitions in the root `Cargo.toml` under `[workspace.dependencies]`. Sub-crates then use `{ workspace = true }` to inherit these versions. 

> "Using workspace dependencies ensures that a single version of a crate is used across the entire project, reducing build times and artifact size through deduplication." — (Rust Foundation, 2024).

## Audit Methodology (2026-04-13)

The audit was performed using the following steps:
1. **Discovery**: A workspace-wide scan using `grep` and `cargo metadata` identified all `Cargo.toml` files containing explicit `version = "..."` keys for external crates.
2. **Standardization**: Sprawling versions were collected and moved to the root `Cargo.toml`. Sub-crates were modified to use `workspace = true`.
3. **Internal Path Centralization**: Local path dependencies (e.g., `vox-db = { path = "../vox-db" }`) were also moved to `workspace.dependencies` to allow for central renaming and relocation of crates without breaking dozens of files.

## Resolution Summary

| Crate | Resolved Dependencies | Impact |
|---|---|---|
| `vox-git` | `gix`, `jj-lib` | Standardized VCS bridge versions |
| `vox-populi` | `axum`, `tower-http`, `subtle`, `ctrlc` | Centralized transport layer versions |
| `vox-mcp` | `rmcp`, `wasmtime`, `rmp-serde`, `lru` | Unified agent-to-agent protocol stack |
| `vox-toestub` | `syn`, `quote`, `proc-macro2`, `similar` | Synchronized compiler/AST tooling |

## CI-CD Governance

To prevent future sprawl, the **TOESTUB** engine has been updated with an enforcement rule:

### `arch/workspace_drift` (Severity: Error)
The `WorkspaceDriftDetector` now explicitly blocks:
1. `version = "..."` keys in sub-crates.
2. `path = "..."` keys in sub-crates (except for `workspace-hack`).

This ensures that any new dependency introduction MUST pass through the root `Cargo.toml`, facilitating review by architecture leads.

## Future Considerations

- **Automated Upgrades**: Integrate `cargo-edit` or `cargo-dist` to perform workspace-wide version bumps.
- **Vulnerability Scanning**: Centralized versions simplify the usage of `cargo-audit` to identify CVEs across the entire dependency graph.

## References

1. Rust Foundation. (2024). *Cargo Workspace Documentation*. Retrieved from https://doc.rust-lang.org/cargo/reference/workspaces.html
2. Vox Architecture SSOT. (2026). *AGENTS.md*. (Internal Repository Documentation).

