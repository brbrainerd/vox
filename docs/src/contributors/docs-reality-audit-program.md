---
title: "Documentation Reality Audit Program"
description: "Sustaining doc/code/contract reality checks: taxonomy, machine-readable backlog, scoring, and CI entry points."
category: "Contributors"
status: "current"
sort_order: 11
last_updated: "2026-05-11"
training_eligible: true
schema_type: "TechArticle"
---

# Documentation Reality Audit Program

This program tracks **aspiration vs fulfillment** and **documentation vs code** truth using a single machine-readable backlog. It complements (does not replace) `vox ci` guards such as `command-compliance`, `retired-symbol-check`, and `ssot-drift`.

## Authoritative artifacts

| Artifact | Purpose |
| --- | --- |
| [`contracts/documentation/docs-reality-audit.program.v1.yaml`](../../../contracts/documentation/docs-reality-audit.program.v1.yaml) | Taxonomy, scoring formula, priority bands, cadence |
| [`contracts/reports/docs-reality-audit/inventory.v1.json`](../../../contracts/reports/docs-reality-audit/inventory.v1.json) | Claim inventory (high-authority docs/contracts + path hints) |
| [`contracts/reports/docs-reality-audit/findings.v1.json`](../../../contracts/reports/docs-reality-audit/findings.v1.json) | Triaged mismatches with dual-track classification |
| [`contracts/reports/docs-reality-audit/metrics.v1.json`](../../../contracts/reports/docs-reality-audit/metrics.v1.json) | Rollout / queue health snapshot (regenerated; safe to commit) |
| `*.schema.json` (same directory) | JSON Schema enforced by `vox ci docs-reality-audit verify` |

## CLI

```bash
# Validate schemas, score invariants, and that inventory path hints resolve
cargo run -q -p vox-cli -- ci docs-reality-audit verify

# Recompute metrics (stdout; add --write to refresh metrics.v1.json)
cargo run -q -p vox-cli -- ci docs-reality-audit metrics --write
```

`vox ci ssot-drift` runs **`docs-reality-audit verify`** after `contracts-index`.

## Classification (dual-track triage)

Every finding uses **one** primary class:

- **CodeDeficit** — spec/doc intent not fully implemented
- **DocDeficit** — code behavior not reflected in docs
- **IntentionalHistorical** — dated or superseded narrative; not current normative behavior
- **AmbiguousNeedsDecision** — needs an explicit architecture/product call before changing either side

Add secondary tags (e.g. `naming-drift`, `security-policy`) in the finding row when useful.

## Priority score

`PriorityScore = Impact×2 + BlastRadius×2 + Staleness + EnforcementGap + Tractability` (see program YAML for band thresholds).

## Operating cadence

- **Weekly:** extend inventory / findings for files touched in the branch; re-run `verify`
- **Monthly:** full pass over `docs/src/` claims and hygiene of closed vs open findings
- **Release:** focus on `docs/src/reference/cli.md`, env vars, and operations catalog parity

## Automation

- Optional orchestration: `vox run scripts/docs-reality-audit-cycle.vox` (verify + metrics write)
- CI catalog entry: `contracts/ci/check-targets.v1.yaml` id `docs-reality-audit` (standalone job; pre-push still gets coverage via `ssot-drift`)

## Related

- [Documentation governance](documentation-governance.md) — authority tiers and `status` vocabulary
- [Command compliance reference](../reference/command-compliance.md) — registry and parity SSOT
- [Where things live](../architecture/where-things-live.md) — `vox ci` implementation paths
