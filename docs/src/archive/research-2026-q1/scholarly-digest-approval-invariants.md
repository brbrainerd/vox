---
title: "Scholarly publication: digest-bound approval invariants"
description: "Invariants for CLI, MCP, and publisher worker paths: dual approvers tied to manifest digest, job row digest consistency with publication_manifests, adapter error handling, and preflight ledger semantics."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Scholarly publication: digest-bound approval invariants

These rules apply to **CLI** (`vox db publication-submit-local`, `publication-external-jobs-tick`), **MCP** (`vox_scientia_publication_submit_local`, `vox_scientia_publication_external_jobs_tick`), and the shared worker in [`vox_publisher::scholarly_external_jobs`](../../../crates/vox-publisher/src/scholarly/external_jobs.rs).

## Dual approval

- Before any outbound scholarly **submit** or **retry**, the store must record **two distinct approvers** bound to the **current manifest digest** (`publication_manifests.content_sha3_256`).
- Enforcement: [`VoxDb::has_dual_publication_approval_for_digest`](../../../crates/vox-db/src/lib.rs) (and equivalent checks in operator paths).
- If approval is missing, the operation fails fast (CLI error, MCP tool error, or tick `preflight_rejected` with a retryable / permanent classification per message content).

## Digest consistency

- `external_submission_jobs.content_sha3_256` must match the live row in `publication_manifests` for the same `publication_id`. If the manifest changes, operators must create a new job or re-run submit so the job row aligns with the new digest.

## Adapter routes

- New HTTP-backed adapters must {
  - Respect [`VOX_SCHOLARLY_DISABLE*`](../reference/env-vars.md) (see [`scholarly::flags`](../../../crates/vox-publisher/src/scholarly/flags.rs)).
  - Return failures as [`ScholarlyError`](../../../crates/vox-publisher/src/scholarly/error.rs) so `error_class`, `retryable`, and `scholarly_http_status_code` populate `external_submission_attempts` consistently.
  - Use [`classify_scholarly_http`](../../../crates/vox-publisher/src/scholarly/error.rs) for HTTP error mapping unless the adapter needs venue-specific classification (then extend the shared helper rather than forking logic).

## Ledger pseudo-classes

- Job-only `last_error_class` value **`preflight`** is written when operator gates fail **before** adapter I/O. It is not a `ScholarlyError` variant.

