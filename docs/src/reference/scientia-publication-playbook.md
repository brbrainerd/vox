---
title: "Scientia publication failure playbook"
description: "Deterministic remediation for common publication gate and syndication failures."
category: "Language Reference"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Scientia publication failure playbook

Symptoms link to **stable gate reason codes** from `vox_publisher::gate` and structured tool/CLI errors.

## Gate: `live publish blocked by gate`

JSON includes `blocking_reasons[].code`:

| Code | Meaning | Fast fix |
|------|---------|----------|
| `missing_db` | Live publish without VoxDb | Connect Codex / use `vox db` with a real store; dry-run remains allowed |
| `missing_dual_approval` | Fewer than two distinct approvers for this digest | Run `publication-approve` twice with different approver ids |
| `publish_not_armed` | Armed flag false | Set `VOX_NEWS_PUBLISH_ARMED=1` and/or `[orchestrator.news].publish_armed = true` |
| *(implicit)* | Combined dry-run | Tool `dry_run`, orchestrator `[news].dry_run`, or `syndication.dry_run` — any true keeps fan-out non-live |

## Retry: `malformed syndication outcome_json for digest …`

Latest attempt row for the manifest digest contains JSON that is not a `SyndicationResult`. **Fix:** inspect `publication_attempts.outcome_json` in `publication-status`; delete bad rows or re-run a clean `publication-publish` / `publication-route-simulate` after repair.

## Retry: `no syndication attempt outcome for current manifest digest`

No attempt recorded for the **current** manifest hash (content changed after last run). **Fix:** run `publication-publish` (or orchestrator tick) once to create an attempt row for the new digest.

## Scholarly: `unsupported VOX_SCHOLARLY_ADAPTER`

Supported adapters include `local_ledger` (default), `echo_ledger`, `zenodo`, `openreview`, and other names wired in `vox_publisher::scholarly`. **Fix:** unset `VOX_SCHOLARLY_ADAPTER` for the default, or set a supported value; unknown names error (no silent stub). Kill-switches: `VOX_SCHOLARLY_DISABLE`, `VOX_SCHOLARLY_DISABLE_LIVE`, `VOX_SCHOLARLY_DISABLE_ZENODO`, `VOX_SCHOLARLY_DISABLE_OPENREVIEW` (see [env-vars](env-vars.md)).

## Scholarly external jobs: preflight / retry / `error_class`

- **Dual approval:** submit and job ticks require two digest-bound approvers; missing approval yields CLI/MCP errors or tick outcome `preflight_rejected` with message `dual digest-bound approvals…`. See [scholarly-digest-approval-invariants](../archive/research-2026-q1/scholarly-digest-approval-invariants.md).
- **Digest mismatch:** job `content_sha3_256` must match the live manifest row; otherwise preflight fails (often permanent). Re-create the job or re-run submit from the CLI/MCP after updating the manifest.
- **`external_submission_attempts` {** `error_class` follows `ScholarlyError` (`disabled`, `config`, `auth`, `rate_limit`, `transient`, `fatal`) or raw HTTP-derived classes on the `Http` variant; `http_status` is populated for auth (401/403), rate limits (429), 5xx-mapped transients, and other `Http` failures. Job-only **`preflight`** is not a `ScholarlyError`.
- **Operator tick:** `vox db publication-external-jobs-tick` / MCP `vox_scientia_publication_external_jobs_tick` leases due rows and calls `submit_with_adapter`; inspect JSON `results[].outcome` (`succeeded`, `submit_failed`, `preflight_rejected`, `claim_lost`, etc.).
- **Preflight `metadata_complete`:** CLI `--preflight-profile metadata-complete` / MCP `preflight_profile: "metadata_complete"` requires `scientific_publication` in `metadata_json`, at least one author, `license_spdx`, and non-empty `abstract_text`. Use before Zenodo/Crossref-sidecar workflows.

## Live publish: `live publish blocked by worthiness`

JSON usually includes `worthiness_score` and `floor`. **[news]** / env: `worthiness_enforce` + `worthiness_score_min`, or `VOX_SOCIAL_WORTHINESS_ENFORCE` and `VOX_SOCIAL_WORTHINESS_SCORE_MIN`. Applies on **CLI**, **MCP**, and **orchestrator** when live fan-out would run (not dry-run). **Fix:** raise manifest/preflight signals, lower the floor in config, or disable enforcement for that environment.

## Credentials

Syndication tokens resolve through **Secrets** (`vox_secrets::resolve_secret`) for `VOX_NEWS_*` / `VOX_SOCIAL_*` specs. **Fix:** `vox secrets doctor`, set canonical or alias env vars, or auth JSON per [Secrets SSOT](secrets-ssot.md).

## crates.io channel

If `crates_io` appears in routing, expect **explicit** non-success outcomes until a real adapter exists—never assume a crate was published.


