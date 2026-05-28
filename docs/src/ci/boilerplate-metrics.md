---
title: "Boilerplate metrics and KPI framework"
description: "KPI framework for tracking accidental-complexity and boilerplate reduction across Vox roadmap waves."
category: "CI & Quality"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# Boilerplate metrics and KPI framework

## Primary KPIs
- `files_touched_per_feature`: median files changed for a representative full-stack feature.
- `handwritten_glue_loc`: lines of manually maintained route/client/validation glue.
- `drift_incidents_per_month`: docs/code/registry contract parity failures in CI.
- `autofix_coverage_ratio`: proportion of diagnostics with safe autofix suggestions.
- `time_to_first_fullstack_feature`: wall-clock setup-to-first-feature benchmark.

## Baseline collection
- Capture pre-wave baseline from current mainline examples and CI runs.
- Store wave snapshots in `contracts/reports/` for reproducibility.
- Track values per wave (`wave1`, `wave2`, `wave3`) and overall trend.

## Suggested data sources
- CLI CI jobs (`vox ci ...`) for drift and parity counts.
- Golden examples and integration tests for feature-level touch counts.
- Diagnostic logs for autofix coverage and error-class frequency.

## Guardrails
- KPI movement must be interpreted with correctness gates; lower boilerplate cannot reduce safety.
- Regressions in compile-time error quality block ergonomics rollout.
- Any metric gain from hidden complexity is invalid.

## Reporting cadence
- Per PR for touched streams.
- Weekly rollup during active roadmap execution.
- End-of-wave signed checkpoint with comparison against baseline.


