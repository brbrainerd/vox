---
title: "Operations catalog SSOT"
description: "Unified operation catalog for MCP + CLI + planner parity."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-02"
training_eligible: false
training_rationale: "Key architecture constraints and definitions required for agent context"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Operations catalog SSOT

The canonical edit surface for first-party operation identity is:

- [`contracts/operations/catalog.v1.yaml`](../../../contracts/operations/catalog.v1.yaml)

Schema:

- [`contracts/operations/catalog.v1.schema.json`](../../../contracts/operations/catalog.v1.schema.json)

**Human-edited (first-party operations):** only this catalog YAML (including the nested `capability:` block for runtime builtin maps + capability exemptions). **Generated — do not hand-edit:**

- MCP registry [`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml)
- CLI registry [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml) (non-CLI surfaces + `script_duals` / `env_var_ssot_index` are carried forward on sync)
- Capability registry [`contracts/capability/capability-registry.yaml`](../../../contracts/capability/capability-registry.yaml)

`vox ci operations-verify` refuses drift: it compares those three files to fresh projections from the catalog (in addition to parity checks and MCP dispatch + input-schema + read-role governance coverage).

## CI commands

- `vox ci operations-verify` — validates catalog parity against committed MCP/CLI/capability registries, MCP dispatch + `input_schemas.rs` coverage, read-role governance profile vs catalog, derived-artifact strict match, and refreshes [`contracts/reports/operations-catalog-inventory.v1.json`](../../../contracts/reports/operations-catalog-inventory.v1.json)
- `vox ci operations-sync --target catalog --write` — regenerates **operation rows** from live registries while preserving the catalog `capability` + `exemptions` roots (requires an existing catalog)
- `vox ci operations-sync --target mcp --write` — writes MCP registry from catalog
- `vox ci operations-sync --target cli --write` — writes **vox-cli** rows in the command registry from catalog
- `vox ci operations-sync --target capability --write` — writes capability registry from catalog (`capability:` block + projected curated rows)
- `vox ci operations-sync --target all --write` — runs `mcp`, then `cli`, then `capability`

## Scope boundary

User `@mcp.tool` and `@mcp.resource` generated app surfaces remain outside this first-party catalog. They are represented by per-app contracts emitted by the compiler and may be federated later.

## Related telemetry work

Implementation and producer-audit backlog (including catalog ↔ guard alignment): [`telemetry-implementation-backlog-2026.md`](./telemetry-implementation-backlog-2026.md).

Optional operator upload queue is catalogued as **`telemetry`** / **`telemetry.*`** in the same YAML; see [ADR 023](../adr/023-optional-telemetry-remote-upload.md), [telemetry-remote-sink-spec](telemetry-remote-sink-spec.md), and **`vox telemetry`** in [`cli.md`](../reference/cli.md).


