---
title: "Scaling CI enforcement rollout"
description: "Documents toestub-scoped modes (legacy, audit, enforce-warn, enforce-strict), phased rollout from warnings to stricter gates, vox ci scaling-audit commands, PR CI JSON caps, and SSOT pointers to scaling policy and contracts."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Scaling CI enforcement rollout

## Modes

`toestub` / `vox ci toestub-scoped`:

| `--mode` | Exit behavior |
|----------|----------------|
| `legacy` (default) | Fail if any finding ≥ `Error` (unchanged historical behavior) |
| `audit` | Never fail; report `Info`+ (use with `--format json` for snapshots) |
| `enforce-warn` | Fail if any `Critical` (not default CI mode) |
| `enforce-strict` | Fail if any `Warning`+ |

## Recommended rollout

1. **Now:** `toestub-scoped` stays `legacy`; scaling findings are mostly `Warning`/`Info` so they surface without failing CI.
2. **After backlog burn-down:** run scoped paths with `enforce-strict` in optional workflows.
3. **Critical-only gate:** introduce targeted `Critical` rules (e.g. confirmed blocking HTTP without timeouts) and use `enforce-warn` only on explicitly approved hot paths.

## Commands

- `vox ci scaling-audit verify` — schema + embedded policy parse.
- `vox ci scaling-audit emit-reports` — per-crate markdown + rollup + TOESTUB JSON snapshot under `contracts/reports/scaling-audit/`. Honors **`VOX_TOESTUB_MAX_RUST_PARSE_FAILURES`** on the JSON envelope’s `rust_parse_failures` field (see [env-vars SSOT](../reference/env-vars.md)).

**PR CI** additionally runs a full `toestub --format json` scan on `crates/` with the same env cap so `syn::parse_file` regressions fail before merge.

## SSOT

- Policy: `contracts/scaling/policy.yaml`
- Task templates: `contracts/scaling/task-templates.yaml`
- Contract index: `contracts/index.yaml` (`scaling-policy`, `scaling-policy-schema`)

