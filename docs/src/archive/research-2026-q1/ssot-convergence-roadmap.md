---
title: "SSOT / DRY convergence roadmap"
description: "Rev C convergence scope: authoritative contract and registry artifacts, machine-readable drift evidence, and planned waves extending existing guards (ssot-drift, persistence normalization) rather than parallel checkers."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# SSOT / DRY convergence roadmap

This document tracks the **Rev C** convergence program: contracts, VoxDb persistence ownership, MCP/CLI parity, and CI gates (`vox ci ssot-drift`).

## Canonical authority registry

Use [`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml) as the single registry for:

- machine spec paths (`A-spec`)
- one canonical human page (`B-canon`)
- generated docs (`C-generated`)
- aliases/pointer stubs (`D-index`)

`vox ci check-docs-ssot` now includes canonical-map validation (uniqueness of `id`/`canon_doc`, alias link/legacy rules, and path existence).

## Authoritative artifacts (current)

- CLI surface — `contracts/cli/command-registry.yaml` + `vox ci command-compliance`
- Contracts index — `contracts/index.yaml` + `vox ci contracts-index`
- Codex HTTP + schema — `contracts/codex-api.openapi.yaml`, `crates/vox-db/src/schema/manifest.rs`, `vox ci check-codex-ssot`
- Baseline / digest policy — `contracts/db/baseline-version-policy.yaml`
- MCP tool names — `contracts/mcp/tool-registry.canonical.yaml` → `vox-mcp-registry` (Rust `TOOL_REGISTRY`)
- Unified operations catalog (authoritative edit plane) — `contracts/operations/catalog.v1.yaml` (`vox ci operations-verify`, `vox ci operations-sync --target catalog|mcp|cli|capability|all`)
- DeI wire types — `vox-protocol` (`DispatchRequest` / `DispatchResponse`), schema `contracts/dei/rpc-methods.schema.json`
- Communication taxonomy — `contracts/communication/protocol-catalog.yaml`, prose [Communication protocols](../reference/communication-protocols.md); advisory synthesis [Protocol convergence research 2026](protocol-convergence-research-2026.md)

## Evidence snapshot

Machine-readable drift notes: `contracts/reports/evidence-snapshot-rev-c.json`. SQL ownership audit (incremental): `contracts/reports/sql-write-ownership-rev-c.json`.

## Next waves

Remaining work follows the internal 292-operation checklist (persistence CRUD normalization, env registry YAML, workflow gate matrix). Prefer **extending** existing guards over parallel checkers.

