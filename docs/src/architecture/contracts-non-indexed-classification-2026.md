---
title: "Contracts outside contracts/index.yaml — classification (2026)"
description: "How to treat YAML/JSON under contracts/ that are not listed in contracts/index.yaml."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Reduces duplicate ownership guesses for schema sprawl."
---

# Contracts outside `contracts/index.yaml`

[`contracts/index.yaml`](../../../contracts/index.yaml) is the **preferred** machine inventory for maintained artifacts (`owner`, `enforced_by`, `kind`).

Many paths under `contracts/` are intentionally **not** indexed:

| Class | Examples | Policy |
|-------|----------|--------|
| **Generated / snapshot reports** | `contracts/reports/**` JSON | Do not treat as hand-edited SSOT; prune only when no CI consumer reads them |
| **Derived registries** | `contracts/cli/command-registry.yaml`, `contracts/mcp/tool-registry.canonical.yaml`, `contracts/capability/capability-registry.yaml` | Edit **`contracts/operations/catalog.v1.yaml`** then `vox ci operations-sync` |
| **Nested schema siblings** | JSON Schema next to a parent YAML named in the index | Owned by the same `id` as the parent row where obvious |
| **Fixture / eval payloads** | Large matrices under `contracts/eval/**` | Add an index row when a CI job treats them as authoritative |

## When to add an index row

Add a row when **any** of these hold:

1. A `vox ci *` guard reads the file path literally.
2. Human docs call the file “SSOT” without pointing at a parent catalog.
3. The file defines new `VOX_*` env vars or persistent Tier A shapes.

## Version header vs filename

Some files use `*.v1.yaml` with **`x-vox-version` > 1**. Treat **`x-vox-version`** as semantic contract version; filename suffix is legacy. Renames are backlog items — see [legacy remediation ledger](./legacy-tombstone-remediation-ledger-2026.md).
