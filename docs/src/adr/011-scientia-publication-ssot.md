---
title: "ADR 011: Scientia publication manifest SSOT"
description: "Unifies Scientia, news, and scholarly submission around one publication manifest and digest-bound approvals."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 011: Scientia publication manifest SSOT

## Status

Accepted.

## Context

The repository has two adjacent but separate publication surfaces:

- `vox scientia` / `vox db` research ingestion and capability mapping.
- news syndication (`vox-publisher`, orchestrator `NewsService`, MCP `vox_news_*` tools).

The news path already enforces strong controls (digest-bound approvals and publish gates), but the scientific publication path had no first-class manifest lifecycle for journal-style interoperability.

## Decision

Adopt a single publication domain model centered on a canonical manifest persisted in Codex:

- New tables in `vox-db` publication domain:
  - `publication_manifests`
  - `publication_approvals`
  - `publication_attempts`
  - `scholarly_submissions`
  - `publication_status_events`
- Digest-bound approvals are the active approval model for publication workflows.
- `vox-publisher::publication::PublicationManifest` is the shared Rust contract type across community and scholarly workflows.
- `vox-publisher::scholarly::ScholarlyAdapter` is the adapter contract; `LocalLedgerAdapter` is the first integration path.
- News publishing writes through the publication manifest/attempt/state ledger while preserving existing community channels.

## Consequences

### Positive

- One lifecycle model for news and scientia publication artifacts.
- Clear provenance: immutable digest, dual approval counts, submission IDs, and status transitions.
- Reusable gate and approval logic across orchestrator, CLI, and MCP.

### Trade-offs

- Temporary overlap with legacy news approval tables during migration windows.
- Additional manifest synchronization responsibilities for callers that prepare content outside existing news files.

## Implementation notes

- DB ownership follows `docs/agents/database-nomenclature.md`.
- `vox scientia` now exposes publication lifecycle commands:
  - `publication-prepare`
  - `publication-approve`
  - `publication-submit-local`
  - `publication-status`
- MCP gains matching scientia publication tools for non-CLI clients.
- Optional structured scholarly metadata (`scientific_publication` inside `metadata_json`) is carried on prepare via `--scholarly-metadata-json` / MCP `scholarly_metadata` (see `vox_publisher::scientific_metadata`).
- Preflight: `publication-prepare --preflight`, `publication-prepare-validated`, `publication-preflight`, MCP `vox_scientia_publication_preflight` + prepare `preflight` flags (`vox_publisher::publication_preflight`).
- Zenodo metadata JSON (no HTTP): `publication-zenodo-metadata` (`vox_publisher::zenodo_metadata`).

## Related publication readiness guidance

- For journal and self-publication interoperability requirements, gap analysis, and phased implementation guidance, see:
  - `docs/src/architecture/scientia-publication-readiness-audit.md`
  - `docs/src/architecture/scientia-publication-automation-ssot.md`
  - `docs/src/reference/scientia-publication-worthiness-rules.md`


