---
title: "Socrates protocol — single source of truth"
description: "Official documentation for Socrates protocol — single source of truth for the Vox language. Detailed technical reference, architecture gu"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Socrates protocol — single source of truth

The **Socrates** protocol is Vox’s unified anti-hallucination pipeline: retrieve evidence, verify claims, calibrate confidence, gate outputs, and persist telemetry. Implementation spans `vox-socrates-policy`, `vox-orchestrator`, `vox-code-audit` (review), `vox-mcp`, and Codex schema extensions.

Questioning strategy (when to ask, what question type to ask, and when to stop) is specified in the companion SSOT:

- [Information-theoretic questioning protocol](information-theoretic-questioning.md)

## Protocol states

1. **Retrieve** — Hybrid lexical + vector retrieval; every factual claim should bind to `EvidenceItem` records. Pure fusion helpers in `crates/vox-db/src/retrieval.rs` (`RetrievalResult`, `fuse_hybrid_results`) preserve **`evidence_source`**, timestamps, optional **`query_id`**, **`supporting_claim_ids`**, and **`contradiction_hints`** across modality merge. In-process memory search uses `HybridSearchHit` (`potential_contradiction`) in `vox-orchestrator`.
2. **Verify** — Claims checked against evidence; contradictions increase `contradiction_ratio`.
3. **Calibrate** — Produce `ConfidenceSignal` (score, coverage, contradiction ratio).
4. **Gate** — `RiskDecision`: `Answer`, `Ask`, or `Abstain` via `ConfidencePolicy::evaluate_risk_decision` in crate `vox-socrates-policy`.
5. **Persist** — Log outcomes to `research_metrics` / `eval_runs` / reliability tables; update routing weights.

## Telemetry and hallucination-risk proxies

- **MCP tools** (`vox_chat_message`, `vox_plan`, `vox_replan`, `vox_plan_status`, `vox_inline_edit`, `vox_ghost_text`): when Codex is attached, each successful turn appends `research_metrics` with `metric_type = socrates_surface`, `session_id = mcp:<repository_id>`, `metric_value = hallucination_risk_proxy(...)`, and JSON metadata `SocratesSurfaceTelemetry` in `crates/vox-db/src/socrates_telemetry.rs` (re-exported from `vox_db`). Logs also emit target `vox_socrates_telemetry`. Effective thresholds follow `OrchestratorConfig::effective_socrates_policy()` (merges `vox-socrates-policy` with optional config overrides).
  - **`vox_plan` adequacy (Codex)**: when `plan_telemetry_session_id` is set, `plan_sessions.iterative_loop_metadata_json` may include `adequacy_before`, `adequacy_after` (and/or legacy `adequacy`), `adequacy_improved_heuristic`, `task_count_before_refine` / `task_count_after_refine`, `aggregate_unresolved_risk`, `plan_depth`, and `initial_plan_max_output_tokens`. The tool response adds `plan_adequacy_score`, `plan_too_thin`, `adequacy_reason_codes`, and `plan_depth_effective`. See [plan adequacy](../archive/research-2026-q1/plan-adequacy.md).
- **Hybrid memory retrieval** (`vox_search::MemorySearchEngine::hybrid_search`): used by MCP unified retrieval triggers (`vox_chat_message` autonomous preamble and `vox_memory_search`) via [`vox_search`](../../../crates/vox-search/src/lib.rs), appends `memory_hybrid_fusion` under session `socrates:retrieval` with contradiction-rate metadata.
- **Rollups** — `VoxDb::aggregate_socrates_surface_metrics`, `VoxDb::record_socrates_eval_summary` (writes `eval_runs` with answer/abstain rates and a quality proxy derived from mean risk proxy).
- **CLI** — `vox codex socrates-metrics` prints the aggregate JSON; `vox codex socrates-eval-snapshot --eval-id <stable-id>` appends an `eval_runs` row (same DB resolution as other `vox codex` commands). **Fails** if there are zero `socrates_surface` rows in the scan window (prevents bogus “perfect” scores). For a nightly job: set `VOX_DB_*` (or local path), then e.g. `vox codex socrates-eval-snapshot --eval-id nightly-$(date +%F)` (POSIX) or a CI step with a unique `eval_id` per run.

## Canonical JSON shapes (orchestrator / MCP)

**Input (task or turn context)**

```json
{
  "risk_budget": "normal",
  "factual_mode": true,
  "required_citations": 1
}
```

**Output envelope (optional `socrates` on MCP chat / plan / inline / ghost tools)**

```json
{
  "risk_decision": "answer",
  "confidence_estimate": 0.82,
  "contradiction_ratio": 0.05
}
```

(`risk_decision` is serialized from `vox_socrates_policy::RiskDecision`.)

**Handoff extension** (`HandoffPayload`)

- `confidence_signal`, `unresolved_claims`, `required_checks` — see `crates/vox-orchestrator/src/handoff.rs` in the repo.

## Invariants

- No **high-confidence** factual assertion without linked evidence when `factual_mode` is true.
- **Abstain** when normalized confidence is below `ConfidencePolicy::abstain_threshold` or contradiction ratio exceeds `max_contradiction_ratio_for_answer`.
- **Unresolved contradictions** block `Answer`; gate returns `Abstain` or `Ask` per policy.
- `Ask` decisions should follow information-theoretic question selection and stop rules from the questioning SSOT.

## Shared policy crate

Numeric defaults and risk classification live in **`vox-socrates-policy`** — do not duplicate magic thresholds in prompts or filters; import or configure via `ConfidencePolicy` and `ConfidencePolicyOverride` merge in the orchestrator. **Reputation routing:** blend weight for Socrates reputation signals is configurable via `OrchestratorConfig::socrates_reputation_weight` and env **`VOX_ORCHESTRATOR_SOCRATES_REPUTATION_WEIGHT`** (see `vox-orchestrator` `config.rs`).

## Rollout

- **Shadow** — `OrchestratorConfig.socrates_gate_shadow`: compute and log `SocratesOutcome` without blocking completion.
- **Enforce** — `OrchestratorConfig.socrates_gate_enforce`: failed gate requeues task with structured remediation (when task carries `SocratesTaskContext`).

## Related ADR

- [ADR 005: Socrates anti-hallucination SSOT](../adr/005-socrates-anti-hallucination-ssot.md)


