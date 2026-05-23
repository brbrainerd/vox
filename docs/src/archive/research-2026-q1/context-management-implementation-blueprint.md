---
title: "Context management implementation blueprint"
description: "Epic structure, capability decomposition, delivery schema, and implementation mechanics for the Vox context-management program."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-03-30"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Context management implementation blueprint

## Purpose

This document translates the research dossier into an implementation program that can expand into hundreds of work items without turning into an unstructured backlog.

Primary companion documents:

- [Context management research findings 2026](context-management-research-findings-2026.md)
- [Context management phase 1 backlog](context-management-phase1-backlog.md)
- [`contracts/orchestration/context-work-item.schema.json`](../../../contracts/orchestration/context-work-item.schema.json)

## Delivery model

### Work-item hierarchy

The program should use three levels only:

| Level | Meaning | Typical size |
| ------ | --------- | -------------- |
| Epic | a user-visible or architecture-visible pillar | 6-12 capabilities |
| Capability | a coherent slice of behavior or infrastructure | 3-8 tasks |
| Task | one implementable change or testable rollout step | 1 PR or small series |

### Required fields for every work item

Every epic, capability, and task should conform to:

- [`contracts/orchestration/context-work-item.schema.json`](../../../contracts/orchestration/context-work-item.schema.json)

Required operational fields:

- stable ID,
- owner type,
- risk tier,
- dependencies,
- acceptance criteria,
- verification method,
- files hint,
- KPI targets where applicable.

### Example work item

```json
{
  "schema_version": 1,
  "program_id": "context_management_sota_2026",
  "work_item_type": "task",
  "id": "ctx.session.reject-default-for-remote",
  "parent_id": "ctx.session.identity-contract",
  "title": "Reject implicit default session on remote task handoff",
  "description": "Require explicit session lineage when a task crosses agent or node boundaries.",
  "owner_type": "orchestrator",
  "deliverable_type": "code",
  "risk_tier": "high",
  "effort_band": "m",
  "status": "planned",
  "depends_on": ["ctx.contract.context-envelope-v1"],
  "files_hint": [
    "crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs",
    "crates/vox-orchestrator/src/a2a/envelope.rs"
  ],
  "acceptance_criteria": [
    "remote-bound tasks include explicit session lineage",
    "missing lineage causes structured fallback or rejection",
    "telemetry identifies the rejection reason"
  ],
  "verification_methods": [
    "integration_test",
    "manual_trace",
    "telemetry_review"
  ]
}
```

## Program epics

### Epic 1: Canonical context contract

**Goal:** make all context-bearing payloads adapt to one envelope.

Capabilities:

1. `ContextEnvelope` v1 schema and examples.
2. Adapters for MCP retrieval, session summary, task context, and remote handoff.
3. Dual-write and canonical-write migration support.

How to implement:

- Add envelope structs and serde adapters in Rust.
- Normalize legacy payloads at ingress boundaries.
- Emit versioned contract-validation tests for known payload fixtures.

### Epic 2: Session and thread identity

**Goal:** eliminate accidental context bleed.

Capabilities:

1. Canonical session/thread/workspace identity contract.
2. Default-session hardening rules.
3. Session lineage on task submit, handoff, and remote execution.

How to implement:

- Introduce session identity helpers in MCP and orchestrator.
- Reject or relabel implicit defaults on remote/handoff paths.
- Add invariants and regression tests for concurrent sessions.

### Epic 3: Compaction and note-taking

**Goal:** preserve long-horizon coherence without bloating prompts.

Capabilities:

1. Envelope-based compaction outputs.
2. Structured notes and session summaries.
3. Compaction lineage and regeneration policy.

How to implement:

- Create summary and note envelope variants.
- Persist compaction generation and parent lineage.
- Add selection policy that prefers summaries plus recent working set over raw history.

### Epic 4: Retrieval policy engine

**Goal:** make search-vs-memory decisions explicit and consistent.

Capabilities:

1. Shared trigger evaluation across MCP and orchestrator.
2. Risk-tier to retrieval-policy mapping.
3. Budget-aware injection and refresh rules.

How to implement:

- Centralize trigger logic in a policy module rather than duplicating it in tool handlers.
- Thread policy version through retrieval diagnostics and envelopes.
- Emit traces for every retrieval decision.

### Epic 5: Corrective retrieval and evidence repair

**Goal:** recover when first-pass retrieval is weak or contradictory.

Capabilities:

1. Retrieval quality evaluator.
2. Query/corpus rewrite stage.
3. Escalation and replan contract.

How to implement:

- Convert evidence-quality and contradiction metrics into decision thresholds.
- Add a second-pass retrieval mode with rewritten query and recommended corpora.
- Make Socrates and planning consume the correction result explicitly.

### Epic 6: Search-plane unification

**Goal:** expose the same retrieval semantics to all surfaces.

Capabilities:

1. Common budgets for preamble, tool, and task-submit retrieval.
2. Corpus selection policy that covers memory, knowledge, chunks, repo, and future web.
3. Stable retrieval evidence shape for both local and remote use.

How to implement:

- Move per-surface limits into policy config.
- Preserve both lexical and vector diagnostics visibly.
- Add support for a future web-research corpus without changing envelope shape.

### Epic 7: Handoff and A2A context integrity

**Goal:** make agent handoffs stateful, structured, and debuggable.

Capabilities:

1. Handoff payloads carry normalized context lineage.
2. A2A messages include session/thread/task identity.
3. Handoff policy specifies what is copied, summarized, or refreshed.

How to implement:

- Add context-envelope wrappers to handoff and A2A send paths.
- Preserve sender and receiver identity in every handoff span.
- Add tests for local and remote handoff continuity.

### Epic 8: MENs and Populi remote context delivery

**Goal:** make remote execution context-safe and single-owner.

Capabilities:

1. Remote task envelopes carry context lineage and artifact refs.
2. `A2ARetrievalRequest/Response/Refinement` become production flows, not just contracts.
3. Lease-aware remote result reconciliation.

How to implement:

- Extend `RemoteTaskEnvelope` population to include context refs or embedded envelope snapshots.
- Add remote retrieval worker handling using shared `vox-search`.
- Reconcile lease, task, and context lineage at result ingestion.

### Epic 9: Conflict resolution and governance

**Goal:** merge or escalate contradictory context deterministically.

Capabilities:

1. Conflict taxonomy and precedence engine.
2. Evidence-bound overwrite rules.
3. Tombstoning, expiry, dedupe, and stale suppression.

How to implement:

- Implement conflict classifier before merge.
- Apply strategy by conflict class rather than one global merge rule.
- Persist conflict events for debugging and KPI measurement.

### Epic 10: Context observability

**Goal:** make context behavior traceable end to end.

Capabilities:

1. OpenTelemetry-aligned spans and events.
2. Stable context lifecycle event names.
3. Dashboards and query surfaces for debugging.

How to implement:

- Add explicit span hooks at capture, retrieve, compact, select, handoff, resolve, and gate stages.
- Include conversation, task, session, agent, and node identifiers.
- Add operator-facing views for policy version, merge strategy, and retrieval path.

### Epic 11: Evaluation and release gates

**Goal:** block regressions before context bugs reach users.

Capabilities:

1. Deterministic session and retrieval test corpus.
2. Eval harness for handoff and corrective retrieval.
3. Rollout scorecards and CI gates.

How to implement:

- Add fixed fixtures for chat, retrieval, and handoff cases.
- Run per-epic benchmark suites with baseline comparisons.
- Promote gates from shadow to enforce only after metrics stabilize.

### Epic 12: Rollout, migration, and deprecation

**Goal:** ship safely without breaking existing clients or stored data.

Capabilities:

1. Dual-write transition plan.
2. Fallback and kill-switch matrix.
3. Legacy payload retirement criteria.

How to implement:

- Use additive payload fields first.
- Record adoption and failure rates by surface.
- Remove legacy shapes only after coverage and error budgets pass.

## Second-pass critique and corrections

### What the first blueprint got right

- It chose the correct architectural center: a canonical context envelope.
- It identified the right major systems: MCP, orchestrator, search, Socrates, Populi, and MENs.
- It prioritized anti-bleed, retrieval policy, handoff, conflict handling, and telemetry in the right broad order.

### What the first blueprint under-specified

| Weak spot in v1 | Why it is a problem | Correction in this revision |
| --------------- | ------------------- | --------------------------- |
| “centralize policy” was too vague | current code has multiple trigger enums and call-site ownership boundaries | use a shared policy contract and parity tests before extracting shared code |
| compaction was listed too casually | there is no obvious single compaction runtime owner yet | add a compaction-ownership design slice before implementation |
| handoff work was too small | current handoff payloads and accept path do not preserve session/thread context | break handoff into identity, payload, context-store bridge, and verification tasks |
| remote context delivery was too compressed | remote relay ordering and payload shape are both incomplete | split remote work into ordering fix, payload expansion, worker intake, and result reconciliation |
| conflict handling was scheduled too late | trust/precedence fields influence adapter design immediately | define minimal conflict vocabulary at contract stage and delay full enforcement only |
| task counts were too low for distributed work | A2A, MENs, and corrective retrieval each require many integration and rollout steps | expand complex epics into explicit operation packs |

### Corrected sequencing

The safer program order is:

1. contract and identity,
2. current-path telemetry,
3. ordering fixes on submit and handoff paths,
4. retrieval policy parity,
5. corrective retrieval,
6. compaction ownership and implementation,
7. remote context payload expansion,
8. remote retrieval delegation,
9. conflict engine shadow mode,
10. enforce only after eval and canary evidence.

## Explicit operation packs by epic

This section expands each epic into concrete operations. These are intentionally explicit so that complex work does not collapse into underspecified “implementation” tasks.

### Epic 1 operations: canonical context contract

1. Define the Rust `ContextEnvelope` type and serde helpers.
2. Create fixture examples for each envelope variant.
3. Add validation tests against `contracts/communication/context-envelope.schema.json`.
4. Define a backward-compatible “legacy projection” API for legacy payloads.
5. Add versioned parsing behavior: strict for tests, permissive for runtime additive fields.
6. Add tracing helpers that log envelope IDs without dumping sensitive payloads.
7. Document allowed producers and consumers for each variant.
8. Add a migration note for legacy shapes that cannot losslessly round-trip.

Entry points:

- `crates/vox-orchestrator/src/mcp_tools/memory/retrieval.rs`
- `crates/vox-orchestrator/src/socrates.rs`
- `crates/vox-orchestrator/src/handoff.rs`
- `crates/vox-orchestrator/src/a2a/envelope.rs`

### Epic 2 operations: session and thread identity

1. Define canonical identity fields and defaulting rules.
2. Add MCP helper for explicit session allocation and validation.
3. Audit all current uses of default `"default"` session behavior.
4. Tag remote or handoff-bound work as requiring explicit lineage.
5. Thread session and thread IDs through task submit and planning paths.
6. Add session lineage fields to handoff payloads.
7. Add rejection or warn-only modes for missing lineage.
8. Add concurrent-session tests for bleed prevention.
9. Add migration behavior for existing clients that omit session IDs.
10. Emit telemetry whenever fallback defaulting still occurs.

Entry points:

- `crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/chat/message.rs`
- `crates/vox-orchestrator/src/mcp_tools/tools/task_tools.rs`
- `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs`
- `crates/vox-orchestrator/src/handoff.rs`
- `crates/vox-orchestrator/src/orchestrator/agent_lifecycle.rs`

### Epic 3 operations: compaction and note-taking

1. Decide compaction owner: MCP turn loop, orchestrator, or dedicated helper surface.
2. Define compaction input and output envelope shapes.
3. Define what raw history is preserved, summarized, or dropped.
4. Define compaction lineage fields and generation increments.
5. Add summary storage and retrieval rules.
6. Add note-taking envelope shape distinct from compaction summaries.
7. Define reinjection priority between raw history, summaries, and notes.
8. Add compaction-trigger thresholds and disable flags.
9. Add tests for factual continuity after compaction.
10. Add tests for not re-injecting stale or superseded summaries.

Important critique:

The first blueprint assumed compaction could be scheduled immediately. The codebase currently has memory and transcript surfaces but not a single obvious compaction runtime owner, so this epic must start with design and ownership, not code-first implementation.

### Epic 4 operations: retrieval policy engine

1. Define a policy contract shared by MCP and orchestrator call sites.
2. Normalize trigger names and semantics across surfaces.
3. Define risk-tier classes and mapping to retrieval requirements.
4. Define common budget knobs for preamble, explicit tool, and submit-time retrieval.
5. Add a policy-evaluation result struct with explanation fields.
6. Add parity tests comparing MCP and orchestrator decisions for the same input.
7. Preserve policy version in all retrieval evidence envelopes.
8. Add operator-visible traces for “why retrieval ran” or “why retrieval skipped.”
9. Add deny-list or forced-search rules for high-risk categories.
10. Add canary mode for policy decisions before enforcement.

Important critique:

The first blueprint talked about “centralizing trigger logic,” but the correct first move is to centralize the **contract and semantics**, not necessarily the code module, because current crate ownership is still split.

### Epic 5 operations: corrective retrieval and evidence repair

1. Convert retrieval quality signals into a first-pass evaluator.
2. Define thresholds for contradiction, narrow evidence, stale evidence, and weak coverage.
3. Implement rewrite rules for query broadening and narrowing.
4. Implement corpus override or recommendation hints.
5. Preserve verification reason and verification query consistently.
6. Add retry budget and loop limit controls.
7. Thread corrective results into Socrates context and planning metadata.
8. Add explicit “still insufficient” escalation outputs.
9. Add eval cases where second pass improves outcome.
10. Add eval cases where second pass should stop and ask or abstain.

### Epic 6 operations: search-plane unification

1. Inventory per-surface search limits and modes.
2. Move those settings into policy and env-backed config where appropriate.
3. Define a single evidence envelope surface for local and remote use.
4. Preserve backend provenance across MCP and orchestrator callers.
5. Make RRF and corpus-specific contributions visible in telemetry.
6. Define how Tantivy and Qdrant participation should be surfaced to callers.
7. Add explicit deferred-scope handling for `WebResearch`.
8. Add tests for exact-token, semantic, and hybrid search parity.
9. Add docs describing supported vs deferred corpora.

Important critique:

The first blueprint implied that future web corpus integration was near at hand. The code review shows it should remain explicitly deferred until a real executor and trust model exist.

### Epic 7 operations: handoff and A2A context integrity

1. Extend `HandoffPayload` with session/thread/context-envelope references.
2. Define which fields are embedded vs referenced by durable artifact IDs.
3. Add validation invariants for session/thread continuity.
4. Bridge handoff payloads to context-store retrieval envelopes where appropriate.
5. Add sender/receiver identity traces.
6. Add local A2A message wrappers for envelope-aware handoff.
7. Add context-transfer tests for local handoff.
8. Add stale-handoff tests for missing or expired lineage.
9. Add policy for partial handoff versus hard reset.
10. Add documentation for receiver obligations before resuming work.

### Epic 8 operations: MENs and Populi remote context delivery

1. Fix submit ordering so required context exists before remote relay uses it.
2. Expand `RemoteTaskEnvelope` population with lineage and context references.
3. Decide when context is embedded versus passed as durable artifact refs.
4. Add worker-side intake that can parse the richer envelope.
5. Add remote retrieval request handling using `A2ARetrievalRequest`.
6. Add remote retrieval response handling and requester-side normalization.
7. Add refinement follow-up flow for weak remote evidence.
8. Add result reconciliation against lease, task, and session lineage.
9. Add failure handling for missing artifacts or expired context.
10. Add kill-switches and staged rollout controls.
11. Add remote inbox, relay, and result tests.
12. Add explicit operator docs for context-safe remote execution.

Important critique:

This was the most under-decomposed part of the first blueprint. Distributed context delivery is not one capability. It is a chain of ordering, serialization, transport, worker intake, result reconciliation, and rollback work.

### Epic 9 operations: conflict resolution and governance

1. Define minimal conflict classes in the envelope contract.
2. Add a conflict classifier operating on normalized envelopes.
3. Define precedence order across system, user, policy, peer, and derived context.
4. Add freshness and expiry rules.
5. Add evidence-required overwrite rules for high-risk updates.
6. Add dedupe keys and tombstoning behavior.
7. Add event logging for conflict decisions.
8. Add shadow-mode merge strategy output before enforcement.
9. Add regression tests for semantic disagreement and stale-summary suppression.
10. Add docs for operator interpretation of conflict events.

### Epic 10 operations: context observability

1. Define stable span names and event payload fields.
2. Map them to OpenTelemetry conventions where possible.
3. Add envelope, session, task, thread, agent, and node identifiers to traces.
4. Add sampling guidance so context-debugging spans are not dropped during rollout.
5. Add retrieval, handoff, compaction, and conflict dashboards or query specs.
6. Add correlation rules between local and remote events.
7. Add redaction guidance for payload-bearing spans and logs.
8. Add canary review queries and operator runbook snippets.

### Epic 11 operations: evaluation and release gates

1. Define deterministic fixture families by failure mode.
2. Create session bleed test corpus.
3. Create retrieval trigger parity test corpus.
4. Create contradiction and corrective-retrieval test corpus.
5. Create handoff continuity test corpus.
6. Create remote relay and remote result reconciliation test corpus.
7. Define scorecard formats and threshold interpretation.
8. Add shadow-vs-enforce comparison dashboards or reports.
9. Add CI gating order for unit, integration, eval, and canary evidence.

### Epic 12 operations: rollout, migration, and deprecation

1. Define dual-write and dual-read stages by surface.
2. Add per-surface feature flags.
3. Define fallback behavior when envelope parsing fails.
4. Define compatibility behavior for missing lineage fields.
5. Define rollback conditions for each major epic.
6. Define telemetry thresholds required to move from shadow to enforce.
7. Define deprecation criteria for legacy payloads.
8. Define archival or replay strategy for legacy stored payloads.
9. Add operator-facing upgrade and rollback notes.

## Capability generation rules

When splitting an epic into capabilities, every capability must answer:

1. What user-visible or operator-visible problem does it solve?
2. Which code surfaces own the behavior?
3. What evidence proves success?
4. What contexts can it break if incorrectly rolled out?

When splitting a capability into tasks, every task must:

- change one contract, one policy, one test surface, or one rollout control at a time,
- have a rollback path,
- have an observable success signal,
- avoid mixing unrelated surfaces in one PR unless the change is purely mechanical.

For complex distributed or multi-surface capabilities, add one more rule:

- break sequencing-sensitive work into explicit ordering, serialization, transport, intake, reconciliation, and rollback tasks rather than one “wire it up” task.

## Suggested epic-to-owner map

| Epic | Primary owner | Secondary owner |
| ------ | --------------- | ----------------- |
| canonical contract | orchestrator | mcp |
| session identity | mcp | orchestrator |
| compaction | mcp | orchestrator |
| retrieval policy | search | orchestrator |
| corrective retrieval | search | mcp |
| search-plane unification | search | mcp |
| handoff integrity | orchestrator | mcp |
| MENs/Populi context delivery | populi | orchestrator |
| conflict governance | orchestrator | search |
| observability | cross_cutting | ops |
| evaluation | tests | search |
| rollout and deprecation | ops | cross_cutting |

## Sequencing rules

### Order of operations

1. Freeze the canonical contract and session identity model.
2. Instrument the current lifecycle before changing behavior.
3. Unify retrieval policy and corrective retrieval next.
4. Harden handoff and remote execution once envelope semantics are stable.
5. Introduce conflict-resolution enforcement after observability and tests exist.
6. Promote from shadow to enforce only after eval metrics hold.

### What must not happen

- Do not deploy remote context delivery before session lineage is explicit.
- Do not enforce search requirements before the retrieval policy engine is shared.
- Do not merge conflicting context silently once conflict classes are available.
- Do not compact aggressively without compaction lineage and recovery tests.

## Target scale

The following sizing is intentionally large because the system spans multiple crates and rollout phases:

| Epic count | Capabilities per epic | Tasks per capability | Estimated total tasks |
| ----------- | ------------------------ | ---------------------- | ----------------------- |
| 12 | 8-12 | 4-10 | 384-1440 |

This is the correct scale for the program. The system already exists in partial form; the remaining work is integration, hardening, telemetry, and release engineering.

## Verification posture

Each epic should include at least one of:

- unit tests for adapters or policy logic,
- integration tests across MCP/orchestrator/Populi seams,
- deterministic eval fixtures,
- telemetry review queries,
- canary rollout checks.

The preferred rollout path is always:

1. contract added,
2. adapter added,
3. telemetry added,
4. shadow behavior enabled,
5. benchmark reviewed,
6. enforce only when safe.

## Next document

The prioritized first implementation wave lives in:

- [Context management phase 1 backlog](context-management-phase1-backlog.md)


