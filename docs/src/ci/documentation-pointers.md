---
title: "Documentation authority pointers"
description: "CI-facing pointer index to canonical documentation authority pages. Keep behavior in reference docs; this page exists for stable guard paths."
category: "CI & Quality"
status: "current"
last_updated: "2026-04-06"

schema_type: "TechArticle"
---

# Documentation authority pointers

This page is a CI-facing pointer surface for documentation authority. Canonical behavior lives in reference pages; this file keeps stable links and guard anchors without duplicating policy text.

## Canonical pages

| Domain | Canonical page | Primary machine artifact(s) |
|---|---|---|
| Doc inventory | [`reference/doc-inventory.md`](../reference/doc-inventory.md) | `docs/agents/doc-inventory.json` |
| Command compliance | [`reference/command-compliance.md`](../reference/command-compliance.md) | `contracts/operations/catalog.v1.yaml`, `contracts/cli/command-registry.yaml`, `contracts/capability/capability-registry.yaml` |
| CLI reference surface | [`reference/cli.md`](../reference/cli.md) | `contracts/cli/command-registry.yaml` |
| Environment variables | [`reference/env-vars.md`](../reference/env-vars.md) | crate implementations + CI guards |
| Canonical authority map | [`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml) | `contracts/documentation/canonical-map.v1.schema.json` |

## Guard links

- `vox ci check-docs-ssot`
- `vox ci command-compliance`
- `vox ci ssot-drift` (bundle that includes check-docs-ssot, check-codex-ssot, and command-compliance)
- `vox ci doc-inventory verify`
- `vox ci check-links`

## RCICD audit

- [RCICD coverage and cost matrix (2026)](rcicd-coverage-cost-matrix-2026.md)


