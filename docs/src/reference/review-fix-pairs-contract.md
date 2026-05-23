---
title: "Review Fix Pairs Contract"
description: "Stability-first contract for review_fix_pairs_memory rows."
category: "Language Reference"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---

# Review Fix Pairs Contract

Canonical dataset contract for `review_fix_pairs` rows exported from VoxDB external review findings.

## Required Fields

- `prompt` (string): user-visible review instruction context.
- `response` (string): suggested fix or finding rationale.
- `category` (string): normalized category from ingest.
- `severity` (string): normalized severity.
- `placement_kind` (string): `inline`, `review_summary`, `issue_comment`, or `reply`.
- `source_id` (string): stable finding identity.
- `repository_id` (string): `owner/repo`.
- `pr_number` (integer): source pull request number.
- `correctness_state` (string): truth state used for weighting.
- `sample_kind` (string): must be `review_fix_pairs`.

## Optional Fields

- `file_path` (string|null): source file path when line-anchored.
- `line_start` (integer|null): source line number.

## Versioning

- Backward-compatible additions are allowed.
- Removing or renaming fields requires a version bump and migration notice.
