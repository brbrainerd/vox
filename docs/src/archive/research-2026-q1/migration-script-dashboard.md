---
title: "Migration metrics (script → `vox ci`)"
description: "Official documentation for Migration metrics (script → `vox ci`) for the Vox language. Detailed technical reference, architecture guides,"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Migration metrics (script → `vox ci`)

| Metric | Baseline (2026-03-21) | Current (2026-03-21 QA recovery) |
|--------|------------------------|---------|
| GitHub `ci.yml` bash `scripts/*` invocations | 9 | 0 (Rust `vox ci` / `cargo run -p vox-cli -- ci …`) |
| Python doc-inventory in CI | 1 | 0 |
| Mens matrix steps (sequential) | 18 | 1 (`ci mens-gate --profile ci_full`) |
| `vox-cli` CI feature matrix includes `script-execution` | 0 | 1 (plain + `stub-check` mix) |
| `vox-compilerd` `run` RPC carries `RunMode` | no | yes (`mode` JSON field) |
| Stale ref scan (retired Python / shell gates in `docs/src` + workflows) | no | yes (`check-docs-ssot`) |
| Dogfood Mens orchestration in PS1 | ~60 lines | thin delegate → `vox mens pipeline` |
| ML workflow (`ml_data_extraction.yml`) Python one-liner for eval summary | 1 | 0 (`vox corpus eval --print-summary`) |
| GitLab inline `grep`/`find` repo guards | 3 blocks | `vox ci repo-guards` (in `vox-ci-guards` job) |

Source: [`docs/agents/baseline-script-metrics.json`](../../agents/baseline-script-metrics.json), [`docs/agents/script-registry.json`](../../agents/script-registry.json).


