---
title: "Additive schema plan: scholarly external jobs and snapshots"
description: "Maps current publish_cloud tables for outbound jobs, attempts, snapshots, and receipts; proposes optional future tables and columns; requires additive migrations shipped with store ops, tests, and documented error/status strings."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Additive schema plan: scholarly external jobs and snapshots

Operational tables live in the **publish_cloud** domain ([`publish_cloud.rs`](../../../crates/vox-db/src/schema/domains/publish_cloud.rs)). Migrations should remain **additive** (new tables/columns/indexes) unless a breaking cutover is explicitly scheduled.

## Current artifacts (reference)

| Concern | Table(s) | Notes |
|---------|-----------|-------|
| Outbound work queue | `external_submission_jobs` | Status, lease columns, idempotency key, attempt_count |
| Per-try audit | `external_submission_attempts` | HTTP status, error_class, retryable, fingerprints |
| Remote truth cache | `external_status_snapshots` | Adapter + external id keyed snapshots |
| Local receipt | `scholarly_submissions` | Digest-bound submission rows |

## Future additions (when needed)

1. **Revision mapping** — If adapters expose multiple revisions per submission, add `scholarly_revision_map` (names indicative) keyed by `(publication_id, content_sha3_256, adapter, external_submission_id, revision_id)` with `created_at_ms`; keep `scholarly_submissions` as the primary “head” receipt.
2. **Dead-letter** — Optional `external_submission_jobs_dead` or `status = dead_lettered` + `dead_lettered_at_ms` on the job row once replay UX exists.
3. **Idempotency index** — Ensure unique index on `(adapter, idempotency_key)` remains enforced when adding partial unique variants per environment.

## Migration discipline

- Ship DDL in the same PR as store ops + tests (`vox-db` integration tests under `tests/publication_flow_tests.rs` or new files).
- Document new `error_class` / job status strings in [`scholarly-digest-approval-invariants.md`](scholarly-digest-approval-invariants.md) or [`scholarly/error.rs`](../../../crates/vox-publisher/src/scholarly/error.rs) module docs.

