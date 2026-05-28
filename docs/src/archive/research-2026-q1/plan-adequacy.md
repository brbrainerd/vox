---
title: "Plan adequacy — thin plans, external limits, and Vox mitigation"
description: "Why IDE and API planners produce underspecified plans, how Vox detects and expands them safely, and telemetry for rollout."
category: "Architecture SSOTs"
last_updated: "2026-03-29"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Plan adequacy — research synthesis and Vox behavior

## Why “add more detail” often fails

Planner outputs are constrained by **multiple stacked layers**, not only model capability:

1. **Output token caps** — APIs expose `max_output_tokens`, `max_completion_tokens`, etc.; vendors also tune for cost and latency, which favors shorter completions. See OpenAI’s guidance on controlling response length ([Controlling the length of OpenAI model responses](https://help.openai.com/en/articles/5072518-controlling-the-length-of-openai-model-responses)).
2. **Verbosity and reasoning budgets** — On GPT‑5-class routes, `verbosity` steers detail; `reasoning.effort` consumes part of the completion budget before visible text. A fixed cap can leave little room for a long visible plan (same OpenAI article).
3. **Lossy context compaction** — Long agent sessions summarize or drop old context; Cursor documents that summarization is **lossy** and can degrade task knowledge ([Dynamic context discovery](https://cursor.sh/blog/dynamic-context-discovery)). Training for “self‑summarization” optimizes **dense short** carry‑forward state (~1k tokens vs multi‑k baselines) ([Training Composer for longer horizons](https://cursor.com/blog/self-summarization)).
4. **Dynamic context harnesses** — Agents are steered to pull context on demand rather than materializing one huge plan up front (same dynamic context post). That improves tokens and sometimes quality but **undershoots** users who want one detailed static plan.
5. **Infrastructure** — Truncation, JSON parse failures on long structured outputs, timeouts, and rate limits all present as “the plan stopped early” or “it rewrote without adding substance.”

**Implication:** Safe mitigation is **not** “prompt harder once”; it is **measure thinness**, **expand in bounded steps**, **persist plans outside chat**, and **telemetry** to verify improvement.

## Vox planning surfaces (where adequacy applies)

| Surface | Role | Adequacy integration |
|--------|------|----------------------|
| MCP `vox_plan` | LLM JSON task list + optional refinement | `PlanRefinementReport`: gap heuristics + plan-level adequacy; expansion-first refinement; optional `plan_depth` for token/detail targets |
| Orchestrator goal → `synthesize_plan_nodes` | Rule-based `PlanNode` DAG | Same report shape via `plan_nodes_to_adequacy_tasks`; adequacy JSON on `plan_session_created` lineage; optional `tracing` when thin |
| `quality_gate` | Blocks vague/destructive **nodes** | Uses `orchestrator_node_text_findings` plus `file_manifest` checks (`tbd` path / filename, empty path → `tbd_placeholder` / `manifest_empty_path`); adequacy is **plan-level** and complementary |
| Codex `plan_sessions.iterative_loop_metadata_json` | MCP iterative telemetry | Merge adequacy + refinement **metadata** for analytics |

## Deterministic signals (tier‑1)

Implemented in `vox-orchestrator` [`planning/plan_adequacy.rs`](../../../crates/vox-orchestrator/src/planning/plan_adequacy.rs):

- Per-task: short text, vague phrases, TBD placeholders, destructive cues, dependency integrity, heavy tasks without test hints (aligned with legacy MCP gap behavior).
- Plan-level: **minimum task count** vs estimated goal complexity; **missing verification** for implementation-flavored goals; **flat DAG** (many tasks, no deps); **goal path tokens** without task `files`; **mega-task** clusters (several very high complexity tasks).
- Structural noise: many tasks but **low surface** (short descriptions, few file linkages); **repeated task openings** (copy-paste “detail” without distinct steps).
- Refinement regression (MCP): when a **prior** task list is supplied after a refine pass, signals include **task-count compression**, **lost file linkage**, and **shrunk total description mass**—guarding against “rewrite” that drops substance.

`is_too_thin` combines low adequacy **score** with structural reason codes so refinement triggers even when per-task keyword risk is moderate.

## Safe expansion policy

1. **Expand, don’t wholesale rewrite** — Refinement prompts require preserving existing task IDs and intent unless a gap code demands a fix; new work is **additional** tasks with new IDs.
2. **Bound rounds and token budget** — Reuses `max_refine_rounds`, `refine_budget_tokens`, `gap_risk_threshold`; Auto mode refines when aggregate gap risk **or** `is_too_thin`.
3. **Optional auto-expansion when `loop_mode` is off** — `auto_expand_thin_plan` (default on): run a **small** refinement pass when the draft is thin, so clients that never set `loop_mode` still benefit.
4. **Orchestrator shadow** — `plan_adequacy_shadow` (default `true`): enqueue behavior unchanged; lineage + logs carry adequacy for dashboards before any enforcement.
5. **Orchestrator enforce (opt-in)** — `plan_adequacy_enforce` / `VOX_ORCHESTRATOR_PLAN_ADEQUACY_ENFORCE`: native synthesized plans that remain thin after synthesis are rejected with `ScopeDenied` (after `quality_gate`); the same flag makes MCP `vox_plan` fail when the refined JSON plan is still thin.

## Telemetry and rollout

### Fields to record (conceptual)

Codex / JSON metadata SHOULD include where possible:

| Field | Purpose |
|-------|---------|
| `adequacy_score` | 0..1 structural adequacy |
| `is_too_thin` | Boolean trigger |
| `adequacy_reason_codes` | `too_few_tasks`, `missing_plan_verification`, etc. |
| `detail_target_min_tasks` | Expected floor for complexity |
| `estimated_goal_complexity` | Router/word heuristic |
| `aggregate_unresolved_risk` | Legacy gap rollup |
| `refinement_rounds`, `loop_stop_reason` | Loop outcome |
| `plan_depth` | `minimal` / `standard` / `deep` |
| `initial_plan_max_output_tokens` | Diagnose truncation (MCP metadata) |
| `adequacy_before` / `adequacy_after` | Tier‑1 snapshots before vs after refinement |
| `task_count_before_refine` / `task_count_after_refine` | Detect collapse vs expansion |
| `adequacy_improved_heuristic` | True if score rose, thin cleared, or aggregate risk dropped |

### Rollout stages

1. **Shadow (default)** — `plan_adequacy_shadow: true`; only metrics + logs.
2. **Auto-expand MCP** — Default on via `auto_expand_thin_plan` and Auto loop OR `is_too_thin`.
3. **Enforce native plans (opt-in)** — `VOX_ORCHESTRATOR_PLAN_ADEQUACY_ENFORCE` blocks goal enqueue when the rule-based synthesized DAG is still thin.
4. **Enforce MCP plans (same flag)** — When the flag is on, `vox_plan` returns a tool error if the plan is still `is_too_thin` **after** refinement (telemetry DB updates are skipped on that path).
5. **Stricter MCP / post-refine policy (future)** — Optional extra gates (e.g. max aggregate gap risk) or questioning-first flows when facts are missing. Governance for when planning MUST ask before generating a plan is specified in [`planning-meta/12-question-gate-standard.md`](planning-meta/12-question-gate-standard.md).

### Example SQL (Codex SQLite)

`plan_sessions.iterative_loop_metadata_json` and orchestration lineage payloads may contain JSON blobs. Example exploration query (adjust DB path):

```sql
-- Recent MCP plan sessions with iterative metadata (if populated)
SELECT plan_session_id,
       iterative_loop_round,
       iterative_stop_reason,
       iterative_loop_metadata_json
FROM plan_sessions
WHERE iterative_loop_metadata_json IS NOT NULL
ORDER BY updated_at DESC
LIMIT 20;
```

Use `json_extract(iterative_loop_metadata_json, '$.adequacy_after.score')` (or `$.adequacy_before.score`) where SQLite JSON1 is enabled.

## Related docs

- [Socrates protocol — SSOT](../reference/socrates-protocol.md) — telemetry surfaces for MCP tools
- [Information-theoretic questioning](../reference/information-theoretic-questioning.md) — when to ask vs expand
- [Anti-foot-gun planning standard](planning-meta/05-anti-foot-gun-planning-standard.md)

## External references

- [OpenAI — Controlling the length of model responses](https://help.openai.com/en/articles/5072518-controlling-the-length-of-openai-model-responses)
- [Cursor — Dynamic context discovery](https://cursor.sh/blog/dynamic-context-discovery)
- [Cursor — Training Composer / self-summarization](https://cursor.com/blog/self-summarization)


