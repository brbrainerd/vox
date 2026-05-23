---
title: "Review Regression Challenges Contract"
description: "Stability-first contract for review_regression_memory rows."
category: "Language Reference"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---

# Review Regression Challenges Contract

Canonical contract for `review_regression_challenges` rows.

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
- `sample_kind` (string): must be `review_regression_challenges`

## Optional Fields

- `file_path` (string|null)
- `line_start` (integer|null)

## Integrity Rules

- Regression challenge rows should come from warning/error findings.
- Empty `prompt` or `response` rows are invalid and must be rejected.
