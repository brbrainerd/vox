---
title: "Context management phase 1 backlog"
description: "Prioritized high-win backlog, rollout strategy, and KPI/SLO targets for the first implementation wave of the Vox context-management program."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-03-30"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Context management phase 1 backlog

## Purpose

This document is the prioritized first implementation wave for the context-management program. It is intentionally front-loaded toward high-win, low-regret changes that improve correctness before deeper optimization.

Companion documents:

- [Context management research findings 2026](context-management-research-findings-2026.md)
- [Context management implementation blueprint](context-management-implementation-blueprint.md)
- [`contracts/communication/context-envelope.schema.json`](../../../contracts/communication/context-envelope.schema.json)
- [`contracts/orchestration/context-work-item.schema.json`](../../../contracts/orchestration/context-work-item.schema.json)

## Prioritization rules

Tasks are ordered by this priority stack:

1. stop context bleed,
2. stop silent under-grounding,
3. make behavior observable,
4. unify local surfaces,
5. harden distributed handoff,
6. then optimize quality and cost.

## Phase 0: Contract and identity foundation

| Priority | ID | Owner | Task | Depends on | Verify |
| --------- | ---- | ------- | ------ | ------------ | -------- |
| P0 | ctx.001 | orchestrator | Add Rust `ContextEnvelope` model mirroring the schema contract | none | unit_test, contract_validation |
| P0 | ctx.002 | mcp | Add adapter from MCP retrieval evidence to `ContextEnvelope` | ctx.001 | unit_test |
| P0 | ctx.003 | orchestrator | Add adapter from `SessionRetrievalEnvelope` to `ContextEnvelope` | ctx.001 | unit_test |
| P0 | ctx.004 | orchestrator | Add adapter from `SocratesTaskContext` to `ContextEnvelope` projection | ctx.001 | unit_test |
| P0 | ctx.005 | populi | Add remote payload wrapper for `ContextEnvelope` JSON in A2A delivery | ctx.001 | integration_test |
| P0 | ctx.006 | mcp | Introduce explicit session identity helper instead of silent `"default"` for new callers | none | unit_test |
| P0 | ctx.007 | orchestrator | Require session lineage on submit paths that expect continuity | ctx.006 | integration_test |
| P0 | ctx.008 | orchestrator | Add thread lineage fields to task and handoff context adapters | ctx.001 | integration_test |
| P0 | ctx.009 | cross_cutting | Emit `context.capture` and `context.select` tracing events in shadow mode | ctx.001 | telemetry_review |
| P0 | ctx.010 | tests | Add concurrent-session bleed regression fixtures | ctx.006 | integration_test |
| P0 | ctx.011 | docs | Document canonical session and thread invariants in reference docs | ctx.006 | docs_review |
| P0 | ctx.012 | ops | Add feature flags for envelope dual-write and identity enforcement | ctx.001 | manual_trace |

## Phase 1: Local retrieval and gating hardening

| Priority | ID | Owner | Task | Depends on | Verify |
| --------- | ---- | ------- | ------ | ------------ | -------- |
| P1 | ctx.101 | search | Centralize retrieval trigger evaluation into a shared policy module | ctx.001 | unit_test |
| P1 | ctx.102 | mcp | Switch chat preamble retrieval to shared trigger policy | ctx.101 | integration_test |
| P1 | ctx.103 | orchestrator | Switch task-submit retrieval to shared trigger policy | ctx.101 | integration_test |
| P1 | ctx.104 | search | Define common budget knobs for auto preamble, explicit search, and submit-time retrieval | ctx.101 | unit_test |
| P1 | ctx.105 | orchestrator | Distinguish no-retrieval, heuristic, verified, and corrective retrieval tiers in task context | ctx.101 | unit_test |
| P1 | ctx.106 | search | Add retrieval quality evaluator using contradiction, diversity, and citation coverage | ctx.101 | unit_test |
| P1 | ctx.107 | orchestrator | Fail closed on high-risk tasks that remain ungrounded after required retrieval | ctx.105 | integration_test |
| P1 | ctx.108 | mcp | Surface policy version and retrieval decision path in MCP responses | ctx.101 | manual_trace |
| P1 | ctx.109 | tests | Add fixtures for code-navigation, repo-structure, and factual-lookup trigger correctness | ctx.101 | eval_benchmark |
| P1 | ctx.110 | docs | Add search-vs-memory operator guidance | ctx.102 | docs_review |
| P1 | ctx.111 | cross_cutting | Emit `context.retrieve` spans with conversation, agent, and policy metadata | ctx.106 | telemetry_review |
| P1 | ctx.112 | ops | Add rollout toggles for retrieval-policy shadow and enforce modes | ctx.107 | canary_rollout |

## Phase 2: Corrective retrieval and compaction

| Priority | ID | Owner | Task | Depends on | Verify |
| --------- | ---- | ------- | ------ | ------------ | -------- |
| P2 | ctx.201 | search | Add corrective retrieval planner for weak or contradictory evidence | ctx.106 | unit_test |
| P2 | ctx.202 | search | Implement query rewrite and corpus-broaden hooks for second-pass retrieval | ctx.201 | unit_test |
| P2 | ctx.203 | orchestrator | Thread corrective-retrieval result into Socrates task context | ctx.201 | integration_test |
| P2 | ctx.204 | mcp | Preserve corrective retrieval metadata in MCP evidence envelopes | ctx.201 | unit_test |
| P2 | ctx.205 | mcp | Add envelope-based compaction output for long chat sessions | ctx.001 | integration_test |
| P2 | ctx.206 | orchestrator | Allow task submit to consume compacted session summaries | ctx.205 | integration_test |
| P2 | ctx.207 | mcp | Add note-taking envelope writer for durable task/session notes | ctx.001 | integration_test |
| P2 | ctx.208 | search | Add stale-context refresh rule using TTL and freshness metadata | ctx.001 | unit_test |
| P2 | ctx.209 | tests | Create contradiction-resolution benchmark set | ctx.201 | eval_benchmark |
| P2 | ctx.210 | cross_cutting | Emit `context.compact` and `context.resolve` spans | ctx.205 | telemetry_review |
| P2 | ctx.211 | docs | Document corrective retrieval and compaction lifecycle | ctx.205 | docs_review |
| P2 | ctx.212 | ops | Enable corrective retrieval in shadow mode for selected surfaces | ctx.201 | canary_rollout |

## Phase 3: Handoff and distributed context integrity

| Priority | ID | Owner | Task | Depends on | Verify |
| --------- | ---- | ------- | ------ | ------------ | -------- |
| P3 | ctx.301 | orchestrator | Add `ContextEnvelope` wrapper to local handoff payloads | ctx.001 | integration_test |
| P3 | ctx.302 | orchestrator | Preserve session/thread lineage through `accept_handoff` | ctx.301 | integration_test |
| P3 | ctx.303 | populi | Extend remote task envelope population with context lineage and artifact refs | ctx.005 | integration_test |
| P3 | ctx.304 | search | Implement production handling for `A2ARetrievalRequest` and `A2ARetrievalResponse` | ctx.005 | integration_test |
| P3 | ctx.305 | populi | Add remote retrieval worker flow using shared `vox-search` | ctx.304 | integration_test |
| P3 | ctx.306 | orchestrator | Reconcile remote result lineage with task, lease, and session authority | ctx.303 | integration_test |
| P3 | ctx.307 | populi | Add lease-aware failure states for remote context loss and retry | ctx.303 | integration_test |
| P3 | ctx.308 | cross_cutting | Emit `context.handoff` spans with sender, receiver, node, and lease identifiers | ctx.301 | telemetry_review |
| P3 | ctx.309 | tests | Add remote-handoff integrity evals for session continuity and authority ownership | ctx.303 | eval_benchmark |
| P3 | ctx.310 | docs | Document remote context contract for MENs and Populi | ctx.303 | docs_review |
| P3 | ctx.311 | ops | Add kill-switches for remote envelope enforcement and remote retrieval delegation | ctx.303 | canary_rollout |
| P3 | ctx.312 | orchestrator | Reject remote execution paths that lack explicit lineage when enforcement is on | ctx.311 | integration_test |

## Phase 4: Conflict governance and enforceable release gates

| Priority | ID | Owner | Task | Depends on | Verify |
| --------- | ---- | ------- | ------ | ------------ | -------- |
| P4 | ctx.401 | orchestrator | Implement conflict classifier for temporal, semantic, authority, source-trust, and policy conflicts | ctx.001 | unit_test |
| P4 | ctx.402 | orchestrator | Implement precedence and merge strategy engine | ctx.401 | unit_test |
| P4 | ctx.403 | search | Bind overwrite behavior to evidence and trust thresholds | ctx.401 | unit_test |
| P4 | ctx.404 | mcp | Mark stale or low-trust context as reference-only instead of inline | ctx.402 | integration_test |
| P4 | ctx.405 | orchestrator | Persist conflict-resolution events for review and metrics | ctx.401 | integration_test |
| P4 | ctx.406 | tests | Add merge-policy regression suite | ctx.402 | eval_benchmark |
| P4 | ctx.407 | cross_cutting | Create scorecard query surfaces for conflict rate and resolution outcomes | ctx.405 | telemetry_review |
| P4 | ctx.408 | ops | Promote high-risk task retrieval enforcement from shadow to opt-in enforce | ctx.107 | canary_rollout |
| P4 | ctx.409 | ops | Promote remote lineage enforcement from shadow to opt-in enforce | ctx.312 | canary_rollout |
| P4 | ctx.410 | ops | Add context-system release checklist and rollback matrix | ctx.407 | docs_review |
| P4 | ctx.411 | docs | Publish conflict-governance SSOT and deprecation criteria for legacy payloads | ctx.402 | docs_review |
| P4 | ctx.412 | cross_cutting | Freeze v1 KPI/SLO gates for CI and staged rollout dashboards | ctx.407 | telemetry_review |

## Detailed operation expansion

The tables above are the phase-level seed. The following sections expand the complex work into operation-level tasks so the program does not claim progress too early on large multi-surface features.

### Phase 0 detailed operations: contract and identity

| ID | Owner | Operation | Depends on | Verify |
| ---- | ------- | --------- | ------------ | -------- |
| ctx.013 | orchestrator | Define envelope fixture for `chat_turn` | ctx.001 | contract_validation |
| ctx.014 | orchestrator | Define envelope fixture for `retrieval_evidence` | ctx.001 | contract_validation |
| ctx.015 | orchestrator | Define envelope fixture for `task_context` | ctx.001 | contract_validation |
| ctx.016 | orchestrator | Define envelope fixture for `handoff_context` | ctx.001 | contract_validation |
| ctx.017 | orchestrator | Define envelope fixture for `execution_context` | ctx.001 | contract_validation |
| ctx.018 | mcp | Map chat history entries into envelope projections | ctx.013 | unit_test |
| ctx.019 | mcp | Add session-ID normalization helper with explicit warning path | ctx.006 | unit_test |
| ctx.020 | mcp | Audit every `session_id` default path under MCP chat and task surfaces | ctx.019 | manual_trace |
| ctx.021 | orchestrator | Add thread-id plumbing for task submit metadata | ctx.008 | integration_test |
| ctx.022 | orchestrator | Add session/thread fields to handoff metadata builder | ctx.008 | unit_test |
| ctx.023 | orchestrator | Add structured warn-only rejection path for missing remote lineage | ctx.007 | integration_test |
| ctx.024 | tests | Add fixture pair proving two concurrent sessions do not share retrieval envelope keys | ctx.010 | integration_test |
| ctx.025 | tests | Add fixture proving remote-bound work cannot silently use implicit default session lineage | ctx.023 | integration_test |
| ctx.026 | cross_cutting | Emit envelope-id generation and propagation traces | ctx.009 | telemetry_review |
| ctx.027 | docs | Document “default session” compatibility and deprecation posture | ctx.020 | docs_review |
| ctx.028 | ops | Add config matrix documenting warn-only vs enforce behavior for missing lineage | ctx.012 | docs_review |

### Phase 1 detailed operations: retrieval policy parity

| ID | Owner | Operation | Depends on | Verify |
| ---- | ------- | --------- | ------------ | -------- |
| ctx.113 | search | Define shared retrieval-policy decision result shape | ctx.101 | unit_test |
| ctx.114 | search | Classify query families into low-risk, normal, and high-risk buckets | ctx.101 | unit_test |
| ctx.115 | search | Define forced-search categories for codebase and environment claims | ctx.114 | docs_review |
| ctx.116 | mcp | Replace local trigger heuristics in chat preamble path with shared policy adapter | ctx.102 | integration_test |
| ctx.117 | mcp | Replace explicit search-tool trigger reporting with shared policy adapter | ctx.102 | integration_test |
| ctx.118 | orchestrator | Add policy-evaluation call before `attach_goal_search_context_with_retrieval` | ctx.103 | integration_test |
| ctx.119 | orchestrator | Preserve policy-evaluation rationale in task trace metadata | ctx.118 | telemetry_review |
| ctx.120 | search | Add per-surface retrieval budget knobs and defaults | ctx.104 | unit_test |
| ctx.121 | search | Add parity tests ensuring MCP and orchestrator classify the same query identically | ctx.113 | unit_test |
| ctx.122 | tests | Add code-navigation trigger fixture set | ctx.109 | eval_benchmark |
| ctx.123 | tests | Add repo-structure trigger fixture set | ctx.109 | eval_benchmark |
| ctx.124 | tests | Add factual-lookup trigger fixture set | ctx.109 | eval_benchmark |
| ctx.125 | tests | Add “should skip retrieval” low-risk fixture set | ctx.109 | eval_benchmark |
| ctx.126 | orchestrator | Add high-risk deny-complete gate when retrieval was required but absent | ctx.107 | integration_test |
| ctx.127 | cross_cutting | Emit trace field for retrieval-skip reason | ctx.111 | telemetry_review |
| ctx.128 | cross_cutting | Emit trace field for retrieval-policy version and risk tier | ctx.111 | telemetry_review |
| ctx.129 | docs | Publish policy table describing search-required vs memory-allowed behavior | ctx.110 | docs_review |
| ctx.130 | ops | Add shadow scorecard comparing pre-policy and post-policy retrieval decisions | ctx.112 | telemetry_review |
| ctx.131 | ops | Add rollback threshold for search-policy false positives | ctx.112 | docs_review |
| ctx.132 | ops | Add rollback threshold for search-policy false negatives | ctx.112 | docs_review |

### Phase 2 detailed operations: corrective retrieval and compaction

| ID | Owner | Operation | Depends on | Verify |
| ---- | ------- | --------- | ------------ | -------- |
| ctx.213 | search | Define corrective-retrieval trigger thresholds in config | ctx.201 | unit_test |
| ctx.214 | search | Add reason taxonomy for weak evidence, contradictions, and stale evidence | ctx.201 | unit_test |
| ctx.215 | search | Implement query-broaden rewrite helper | ctx.202 | unit_test |
| ctx.216 | search | Implement query-narrow rewrite helper | ctx.202 | unit_test |
| ctx.217 | search | Implement corpus recommendation output for correction stage | ctx.202 | unit_test |
| ctx.218 | orchestrator | Preserve correction-stage diagnostics inside Socrates task context | ctx.203 | integration_test |
| ctx.219 | mcp | Preserve correction-stage diagnostics inside MCP retrieval envelope | ctx.204 | unit_test |
| ctx.220 | mcp | Decide compaction owner and create design note in code/docs | ctx.205 | docs_review |
| ctx.221 | mcp | Define compaction input window selection rules | ctx.220 | docs_review |
| ctx.222 | mcp | Define compaction output envelope shape and lineage fields | ctx.205 | contract_validation |
| ctx.223 | mcp | Implement summary persistence path for compacted sessions | ctx.222 | integration_test |
| ctx.224 | orchestrator | Add read path for compacted session summary during submit | ctx.206 | integration_test |
| ctx.225 | mcp | Implement note-taking envelope write path distinct from compaction | ctx.207 | integration_test |
| ctx.226 | search | Add freshness-aware rejection or refresh rule for stale context | ctx.208 | unit_test |
| ctx.227 | tests | Add benchmark where corrective retrieval improves weak first-pass evidence | ctx.209 | eval_benchmark |
| ctx.228 | tests | Add benchmark where contradiction should escalate rather than continue retrieving | ctx.209 | eval_benchmark |
| ctx.229 | tests | Add session-compaction continuity benchmark | ctx.223 | eval_benchmark |
| ctx.230 | tests | Add stale-summary suppression benchmark | ctx.223 | eval_benchmark |
| ctx.231 | cross_cutting | Emit compaction generation and parent-envelope lineage traces | ctx.210 | telemetry_review |
| ctx.232 | ops | Add corrective-retrieval loop budget and stop-limit rollout controls | ctx.212 | canary_rollout |

### Phase 3 detailed operations: handoff and remote context

| ID | Owner | Operation | Depends on | Verify |
| ---- | ------- | --------- | ------------ | -------- |
| ctx.313 | orchestrator | Extend `HandoffPayload` with session identity fields | ctx.301 | unit_test |
| ctx.314 | orchestrator | Extend `HandoffPayload` with thread identity fields | ctx.301 | unit_test |
| ctx.315 | orchestrator | Extend `HandoffPayload` with retrieval-envelope reference fields | ctx.301 | unit_test |
| ctx.316 | orchestrator | Add invariant requiring session/thread continuity on resumable handoff | ctx.302 | integration_test |
| ctx.317 | orchestrator | Add warn-only mode for missing handoff lineage | ctx.302 | integration_test |
| ctx.318 | orchestrator | Bridge handoff payloads to context-store retrieval references when available | ctx.315 | integration_test |
| ctx.319 | tests | Add local handoff continuity benchmark with session and thread preservation | ctx.316 | eval_benchmark |
| ctx.320 | tests | Add stale-handoff rejection benchmark for missing lineage | ctx.316 | eval_benchmark |
| ctx.321 | orchestrator | Move retrieval attachment earlier in submit path before remote relay build | ctx.303 | integration_test |
| ctx.322 | orchestrator | Add task-trace marker proving context assembly completed before remote relay | ctx.321 | telemetry_review |
| ctx.323 | populi | Extend remote envelope population with session identity | ctx.303 | integration_test |
| ctx.324 | populi | Extend remote envelope population with thread identity | ctx.303 | integration_test |
| ctx.325 | populi | Extend remote envelope population with artifact references | ctx.303 | integration_test |
| ctx.326 | populi | Extend remote envelope population with context-envelope reference or embedded snapshot | ctx.303 | integration_test |
| ctx.327 | populi | Add remote worker parser for richer remote envelope fields | ctx.303 | integration_test |
| ctx.328 | search | Implement requester-side send path for `A2ARetrievalRequest` | ctx.304 | integration_test |
| ctx.329 | populi | Implement worker-side retrieval handler using shared `vox-search` | ctx.305 | integration_test |
| ctx.330 | search | Implement response normalization from `A2ARetrievalResponse` into envelope form | ctx.304 | integration_test |
| ctx.331 | search | Implement refinement resend path using `A2ARetrievalRefinement` | ctx.304 | integration_test |
| ctx.332 | orchestrator | Reconcile remote result against lease lineage and session identity | ctx.306 | integration_test |
| ctx.333 | orchestrator | Add fallback path when remote result lacks required lineage | ctx.306 | integration_test |
| ctx.334 | tests | Add remote retrieval delegation benchmark | ctx.329 | eval_benchmark |
| ctx.335 | tests | Add remote result reconciliation benchmark | ctx.332 | eval_benchmark |
| ctx.336 | ops | Add canary matrix for remote envelope enforcement, remote retrieval delegation, and fallback modes | ctx.311 | canary_rollout |

### Phase 4 detailed operations: conflict governance and release gates

| ID | Owner | Operation | Depends on | Verify |
| ---- | ------- | --------- | ------------ | -------- |
| ctx.413 | orchestrator | Define explicit precedence order across system, policy, user, peer, and derived context | ctx.401 | docs_review |
| ctx.414 | orchestrator | Add freshness-based conflict classifier branch | ctx.401 | unit_test |
| ctx.415 | orchestrator | Add semantic-disagreement classifier branch | ctx.401 | unit_test |
| ctx.416 | orchestrator | Add authority-conflict classifier branch | ctx.401 | unit_test |
| ctx.417 | orchestrator | Add policy-conflict classifier branch | ctx.401 | unit_test |
| ctx.418 | orchestrator | Add dedupe-key and tombstone behavior for superseded envelopes | ctx.402 | unit_test |
| ctx.419 | search | Add evidence-required overwrite rule for high-risk contexts | ctx.403 | unit_test |
| ctx.420 | mcp | Add reference-only injection mode for low-trust or stale envelopes | ctx.404 | integration_test |
| ctx.421 | orchestrator | Persist structured conflict-resolution event rows | ctx.405 | integration_test |
| ctx.422 | tests | Add stale-summary overwrite regression suite | ctx.406 | eval_benchmark |
| ctx.423 | tests | Add authority-override regression suite | ctx.406 | eval_benchmark |
| ctx.424 | tests | Add contradictory-evidence merge regression suite | ctx.406 | eval_benchmark |
| ctx.425 | cross_cutting | Add operator query surfaces for conflict-class counts by surface | ctx.407 | telemetry_review |
| ctx.426 | cross_cutting | Add operator query surfaces for merge-strategy outcomes | ctx.407 | telemetry_review |
| ctx.427 | ops | Add enforce-readiness checklist for local retrieval gate | ctx.408 | docs_review |
| ctx.428 | ops | Add enforce-readiness checklist for remote lineage gate | ctx.409 | docs_review |
| ctx.429 | ops | Add deprecation checklist for legacy payload readers | ctx.410 | docs_review |
| ctx.430 | ops | Add rollback drill for bad envelope parse or bad merge behavior | ctx.410 | canary_rollout |
| ctx.431 | docs | Publish operator SSOT for conflict interpretation and remediation | ctx.411 | docs_review |
| ctx.432 | cross_cutting | Freeze scorecard schema and CI reporting format for context-system gates | ctx.412 | telemetry_review |

## High-win first 15

If only a small first wave can ship immediately, do these first:

1. `ctx.001` canonical Rust envelope model.
2. `ctx.006` explicit session identity helper.
3. `ctx.007` task-submit lineage enforcement.
4. `ctx.010` concurrent-session bleed tests.
5. `ctx.101` shared retrieval trigger policy.
6. `ctx.102` MCP adoption of shared retrieval policy.
7. `ctx.103` orchestrator adoption of shared retrieval policy.
8. `ctx.106` retrieval quality evaluator.
9. `ctx.107` high-risk ungrounded-task fail-closed path.
10. `ctx.111` retrieval lifecycle spans.
11. `ctx.201` corrective retrieval planner.
12. `ctx.205` envelope-based compaction.
13. `ctx.301` local handoff envelope wrapper.
14. `ctx.303` remote task envelope lineage population.
15. `ctx.401` conflict classifier.

## Rollout strategy

### Stage 1: Shadow only

- Emit envelopes and traces without changing current behavior.
- Preserve current payloads and derive envelope projections from them.
- Record bleed, grounding, and handoff correlation metrics before any enforcement.

### Stage 2: Dual-write

- Write both legacy payloads and normalized envelopes.
- Compare envelope-derived behavior to current production behavior.
- Gate remote and high-risk paths behind kill switches.

### Stage 3: Local enforce

- Enforce explicit session lineage on local handoff and task-submit paths.
- Enforce retrieval requirements on high-risk local tasks.
- Keep remote enforcement in shadow until correlation metrics are healthy.

### Stage 4: Remote enforce

- Require lineage and envelope presence for remote execution and remote retrieval.
- Enable lease-aware remote context reconciliation.
- Keep rollback flags for remote relay and retrieval delegation.

### Stage 5: Legacy retirement

- Remove legacy-only consumers after error budgets hold.
- Keep adapters for historical replay and migration tooling as needed.

### Required rollback guardrails

| Guardrail | Purpose |
| ---------- | --------- |
| envelope dual-write flag | disable canonical-write if adapter regression appears |
| explicit-session enforcement flag | fall back to warn-only when clients lag |
| retrieval-policy enforce flag | return to shadow if false negatives appear |
| corrective-retrieval flag | disable second-pass cost spikes quickly |
| remote-envelope enforcement flag | avoid breaking remote execution during rollout |
| conflict-engine enforce flag | revert to advisory mode if merges are too aggressive |

## KPI and SLO framework

### Core KPIs

| KPI | Definition | Initial target |
| ----- | ------------ | ---------------- |
| context bleed rate | percentage of cross-session contamination incidents in deterministic tests and canaries | 0 in tests, near-zero in canaries |
| unsupported factual claim rate | percentage of high-risk completions lacking required evidence | reduce materially release over release |
| retrieval adequacy rate | percentage of high-risk tasks with acceptable diversity, quality, and citation coverage | > 95% in controlled evals |
| corrective retrieval success rate | percentage of weak first passes improved by second pass | trend upward and stabilize |
| A2A handoff correlation success | percentage of handoffs preserving session/thread/task lineage end-to-end | > 99% in integration tests |
| remote authority mismatch rate | percentage of remote results that fail lease or lineage reconciliation | near-zero |
| token overhead delta | increase in input token cost after envelope adoption | bounded and visible |
| latency overhead delta | increase in end-to-end latency after policy changes | bounded and visible |

### SLO candidates

1. **SLO-context-bleed {** zero deterministic bleed regressions on main.
2. **SLO-high-risk-grounding:** no enforced high-risk path ships with unsupported-claim rate above agreed budget.
3. **SLO-handoff-lineage:** remote and local handoff lineage integrity remains above 99% in gated suites.
4. **SLO-observability:** every enforced policy decision emits a correlated trace or event.

## Acceptance criteria for phase 1 completion

Phase 1 is complete only when all of the following are true:

1. Canonical envelopes exist in code and contract form.
2. Session and thread lineage are explicit on local task-submit and handoff paths.
3. Search trigger policy is shared between MCP and orchestrator.
4. Corrective retrieval is available in shadow mode with telemetry.
5. Remote envelopes can carry structured lineage and artifact references.
6. Conflict classes and observability vocabulary exist, even if full enforcement is still gated.
7. Deterministic eval suites cover bleed, grounding, corrective retrieval, and handoff integrity.

## Suggested next expansion after phase 1

After the first wave, expand the program by generating capability-level tasks under each epic using the work-item schema. This document now seeds 120+ explicit tasks when the detailed operation expansion is included, but the full program should still grow beyond this into the full hundreds-item implementation set described in the blueprint.


