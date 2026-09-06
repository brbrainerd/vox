---
title: "Legacy / tombstone remediation ledger (2026)"
description: "Machine-readable decisions for retired surfaces, schema lineage, and SSOT drift — actions, owners, sunsets."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Governance artifact for agents reducing maintenance burden on retired symbols."
---

# Legacy / tombstone remediation ledger (2026)

Single rolling ledger for **retired / tombstoned / legacy** surfaces and **contract SSOT** decisions. Update this file when classification changes.

## Legend

| Column | Meaning |
|--------|---------|
| **Dependency** | `wire` (MCP/OpenAPI/client), `runtime` (Rust dispatch), `docs` (narrative only), `data` (persisted keys), `none` |
| **Action** | `canonical`, `alias`, `remove`, `defer`, `registry-fix` |
| **Sunset** | Target removal / rename deadline; `none` if permanent alias |

## MCP gamify tools (`vox_ludus_*` → `vox_gamify_*`)

| Surface | Canonical name | Legacy alias | Dependency | Action | Sunset |
|---------|----------------|--------------|------------|--------|--------|
| MCP tool / catalog | `vox_gamify_*` | `vox_ludus_*` via [`TOOL_WIRE_ALIASES`](../../../crates/vox-orchestrator-mcp/src/tool_aliases.rs) | wire + runtime | canonical + alias | Remove `vox_ludus_*` aliases **≥ 0.6** after extension/dashboard telemetry shows zero use |

## Orchestrator naming (`vox-dei`, `vox-dei-d`)

| Surface | Canonical | Notes | Dependency | Action |
|---------|-----------|-------|------------|--------|
| Crate name | `vox-orchestrator` | AGENTS retired-surfaces | docs | remove stray refs |
| Daemon binary | `vox-orchestrator-d` | User-facing install hints | docs + CLI strings | replace `vox-dei-d` in operator copy |
| Companion DB id | `vox-dei` | Codex `companions` row primary key | data | **defer** rename until migration |

## Compiler monolith (`vox-parser`, `vox-lexer`, …)

| Surface | Canonical | Dependency | Action |
|---------|-----------|------------|--------|
| Crate paths | `vox-compiler` | docs, CI `paths:`, hotspots | registry-fix / remove stale globs |
| Strict-parse test | `cargo test -p vox-compiler --test golden_examples_strict_parse` | docs | remove stale strict-parse command snippets that reference **vox-parser** |

## Schema / contracts

| Topic | Issue | Action |
|-------|-------|--------|
| `filename.v1.*` vs `x-vox-version` | Known drift on several YAML files | Track under data-storage / versioning backlog; do not silent-delete |
| `contracts/index.yaml` gaps | Some YAML/JSON live outside index | See [contracts non-indexed classification](./contracts-non-indexed-classification-2026.md) |

## SSOT drift fixes (this effort)

| Item | Canonical source | Fixed in |
|------|------------------|----------|
| Env vars registry | `contracts/config/env-vars.v1.yaml` | Added missing routing/plugin vars; canonical-map `env-vars` domain |
| MCP dispatch verify paths | `crates/vox-orchestrator-mcp/src/dispatch.rs` | `vox-cli` `operations_catalog.rs` |
| `vox-schema.json` crate map | Real workspace crates | Removed phantom `vox-dei` entry |

## Verification commands

```bash
cargo run -p vox-cli -- ci operations-verify
cargo run -p vox-cli -- ci retired-symbol-check
cargo test -p vox-compiler --test golden_examples_strict_parse
```

## Related

- [Contributor hub](../contributors/contributor-hub.md)
- [Where things live](./where-things-live.md)
- [Contracts non-indexed classification](./contracts-non-indexed-classification-2026.md)
- `contracts/documentation/retired-symbols.v1.yaml`
