---
title: "Context isolation — SSOT"
description: "Policy summary for multi-repo and agent context boundaries; links to archived deep research."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Surfaces non-archive B-canon for context isolation; trains agents on boundary rules."
schema_type: "TechArticle"
---

# Context isolation — SSOT

## Operator policy

- **Treat workspace roots as trust boundaries.** Agents and automation must not silently blend corpora, secrets, or instruction stacks across unrelated repositories without an explicit operator-approved bridge (documented env + policy).
- **Prefer repo-local SSOT.** What ships in `contracts/` and `docs/src/` for this repository wins over informal cross-repo notes.
- **Historical research stays archived.** Long-form investigations live under `docs/src/archive/` for human reference only — do not treat archived filenames as live navigation targets in new work.

## Archived reference

Deep-dive background (not ingested by automation per `AGENTS.md` archival protocol) remains at:

`docs/src/archive/research-2026-q1/multi-repo-context-isolation-research-2026.md`

Use this SSOT page for **current policy framing**; open the archive path only when a human explicitly needs the historical narrative.
