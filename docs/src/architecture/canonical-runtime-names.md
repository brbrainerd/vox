---
title: "Canonical runtime names (daemon, MCP, env)"
description: "Frozen canonical identifiers vs deprecated aliases — prevents split-brain between CLI, MCP, docs, and contracts."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Reduces agent/human drift when renaming crates, binaries, tools, or env families."
---

# Canonical runtime names

**Machine registry for env keys:** [`contracts/config/env-vars.v1.yaml`](../../../contracts/config/env-vars.v1.yaml).  
**Human prose tables:** [`docs/src/reference/env-vars.md`](../reference/env-vars.md).  
CI enforces that prose cites only registered names (`vox ci command-compliance`).

## Orchestrator daemon binary

| Canonical | Role |
|-----------|------|
| `vox-orchestrator-d` | Shipped daemon binary (workspace crate `vox-orchestrator-d`). |

**Deprecated path names** (`vox-dei-d`, prose-only “DEI daemon”) may appear in logs or legacy docs — treat them as **aliases** of the same IPC peer; new code should prefer `vox-orchestrator-d` / orchestrator wording.

## Gamify / Ludus

| Surface | Canonical | Deprecated alias (compat) |
|---------|-----------|-----------------------------|
| Crate | `vox-gamify` | `vox-ludus` (retired crate name) |
| MCP tool prefix | `vox_gamify_*` | `vox_ludus_*` (alias layer in MCP dispatch) |
| Env family | Prefer documented `VOX_GAMIFY_*` where introduced | `VOX_LUDUS_*` still read for backward compatibility in `vox-gamify` |

Do **not** introduce new `VOX_LUDUS_*` keys without an explicit migration note in `env-vars.v1.yaml`.

## Database / secrets

| Canonical | Deprecated (compat only) |
|-----------|---------------------------|
| `VOX_DB_URL`, `VOX_DB_TOKEN`, `VOX_DB_PATH` | `VOX_TURSO_*`, `TURSO_*` |

See AGENTS.md retired surfaces table and `vox-db` `DbConfig` resolution.

## ARS / OpenClaw

| Retired | Canonical |
|---------|-----------|
| `vox-ars` crate | `vox-openclaw-runtime` |

## Scientifica research event types

JSON Schema SSOT lives under [`contracts/scientia/*.schema.json`](../../../contracts/scientia/). Rust mirrors in `vox-research-events` must stay aligned (prefer codegen when touching schemas).
