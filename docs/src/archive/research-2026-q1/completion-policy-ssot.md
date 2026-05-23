---
title: "Completion policy SSOT (LLM premature-completion)"
description: "Single source of truth for LLM completion policy enforcement, TOESTUB integration, and CI gating."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-05"
training_eligible: false
training_rationale: "Key architecture constraints and definitions required for agent context"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Completion policy SSOT (LLM premature-completion)

**Policy contract:** `contracts/operations/completion-policy.v1.yaml` (validated by `vox ci command-compliance` against `contracts/operations/completion-policy.v1.schema.json`).

**CI surfaces**

- `vox ci completion-audit` — scans the workspace and writes `contracts/reports/completion-audit.v1.json`.
- `vox ci completion-gates` — Tier **A** hard fail; Tier **B** numeric regression vs `contracts/reports/completion-baseline.v1.json` (`tier_b_max_by_detector`).
- `vox ci completion-ingest` — optional persistence into VoxDB `ci_completion_*` tables (local/default DB).

**Telemetry schemas:** `contracts/telemetry/completion-*.v1.schema.json` (indexed in `contracts/index.yaml`).

**Boundaries**

- **Retention / sensitivity:** `ci_completion_*` is workspace-adjacent (S2); TTL and prune behavior are defined in [telemetry-retention-sensitivity-ssot](telemetry-retention-sensitivity-ssot.md) and [`contracts/db/retention-policy.yaml`](../../../contracts/db/retention-policy.yaml) (`vox db prune-plan` / `prune-apply`).
- Deterministic detectors and policy tiers live in the completion policy contract; `vox-toestub` remains the structural/TOESTUB truth surface.
- Orchestrator placeholder/completion behavior: `crates/vox-orchestrator/src/services/policy.rs` and `orchestrator/task_dispatch/complete.rs`.
- Mens scorecard summaries include an optional `completion_policy` crosswalk (`contracts/eval/mens-scorecard-summary.schema.json`) linking anti-stub metrics to this chain.

**Baseline migration:** raise Tier B caps in `completion-baseline.v1.json` only with deliberate debt acceptance; Tier A findings must be fixed or exempted in the policy `audit_exemptions` block.

**Precision governance:** promote detectors Tier B→A only with fixtures + rolling false-positive evidence; demote on precision regression (see tier notes in the policy YAML). `vox ci completion-ingest` + `ci_completion_detector_snapshot` support trend queries.

**Generated `.vox` / compiler output:** post-codegen static scans are a follow-up (align with `vox-toestub` and `vox ci completion-audit` heuristics); no separate compiler hook ships yet.

**Explicit remediation task IDs:** `contracts/reports/completion-task-ledger.v1.json` (768 entries: `T-WS###-01` … `T-WS###-12` over WS001–WS064). Link ledger items to `contracts/operations/catalog.v1.yaml` operations where applicable.

**TOESTUB in CI:** build `vox-cli` with `--features completion-toestub` so `completion-audit` merges `victory-claim` findings (Tier **C** in policy) from `vox-toestub` without duplicating regex logic in `vox-cli`.

**Extra scan roots:** `vox ci completion-audit --scan-extra path/to/generated-crate` (repeatable). Each directory is canonicalized and must lie under the repo root; default remains `crates/`.


