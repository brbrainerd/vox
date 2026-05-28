---
title: "Review Anti-Pattern Catalog Contract"
description: "Stability-first contract for review_antipattern_memory rows."
category: "Language Reference"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---

# Review Anti-Pattern Catalog Contract

Canonical contract for `review_antipattern_memory` rows.

## Required Fields

- `prompt` (string)
- `response` (string)
- `category` (string)
- `severity` (string)
- `placement_kind` (string)
- `source_id` (string)
- `repository_id` (string)
- `pr_number` (integer)
- `correctness_state` (string)
- `sample_kind` (string): must be `review_antipattern_memory`

## Optional Fields

- `file_path` (string|null)
- `line_start` (integer|null)

## Determinism

- Rows are sorted by `source_id`, then `sample_kind`.
- Export must be stable for repeated runs over the same DB snapshot.
