---
title: "Orchestrator multi-agent groundwork (2026)"
description: "Code-grounded baseline for delegation topology, unified routing, provenance ledger, Vox orchestration surface, and OpenRouter enrichment."
category: "Architecture SSOTs"
last_updated: "2026-03-28"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

## Orchestrator multi-agent groundwork (2026)

This document records groundwork implemented in code for the orchestrator audit:

- canonical topology snapshot shape with delegation edges
- model-routing convergence across MCP surfaces
- durable operation-log persistence into Codex
- minimal `.vox` orchestration surface definition (phaseable)
- dynamic OpenRouter enrichment strategy grounded in current code

It is intentionally implementation-oriented and does not replace a full rollout plan.

## 1) Canonical execution object model

Target model used for future decomposition and verification:

`Campaign -> PlanSession -> RoleNode -> TaskAttempt -> ToolAction -> Artifact -> VerificationResult -> TrustUpdate`

Current code now includes a first-class topology snapshot shape in `vox-orchestrator`:

- `AgentTopologySnapshot`
- `AgentTopologyNode`
- `DelegationEdge`
- `AgentDelegationBinding`
- `TopologyGap`

These are exposed via orchestrator accessors and included in MCP `vox_orchestrator_status`.

## 2) Agent topology and parent/child delegation

Groundwork implemented:

- orchestrator now tracks `child -> parent` delegation bindings (`agent_delegations`)
- dynamic spawns can optionally carry parent, source task id, and reason metadata
- topology snapshots include:
  - node role hints (`planner`, `executor`, `verifier`, `researcher`, `synthesizer`)
  - parent/child edges
  - explicit known-gaps metadata for operators

This gives durable shape for future policy engines without changing existing queue-first semantics.

## 3) Unified model-routing contract (current convergence)

Current model selection still has multiple paths, but one high-impact divergence is now closed:

- `vox_suggest_model` now uses the same MCP model resolver/scoring path as live MCP chat (`resolve_mcp_chat_model_sync`) rather than a separate `best_for` heuristic.

This creates one practical scoring contract for interactive MCP model picks while preserving task-runtime behavior in `vox-orchestrator`.

## 4) Durable provenance backbone (current convergence)

Groundwork implemented:

- `Orchestrator::record_operation(...)` now persists operation entries to Codex (`agent_oplog`) using circuit-breaker guarded append paths after writing in-memory `OpLog`.

Effect:

- in-memory undo/redo behavior remains unchanged while `undone` state is synchronized to Codex
- long-term audit rows now receive operation records from the main operation path
- MCP/state outputs can evolve toward DB-backed replay without changing the core operation callsites again

Scope note:

- this durability path now covers both `record_operation(...)` and `record_ai_usage(...)` (`record_ai_call` oplog entries are persisted via the same `persist_oplog_entry(...)` path).

## 5) `.vox` orchestration surface (minimal, safe, phaseable)

The canonical `.vox` surface remains metadata-first today (`.scope(...)`, retrieval hints).
Minimal phaseable orchestration surface for future parser/runtime work:

```vox
// vox:skip
@orchestrate fn taskName(input: Input) -> Output {
  role planner
  role executor
  role verifier
  delegate planner -> executor
  verify verifier before publish
}
```

Safety constraints for this surface:

- no direct arbitrary process spawn from language code
- role declarations compile to orchestrator capability/delegation metadata
- side-effecting actions remain gated at MCP/tool policy boundaries
- verification edges become explicit plan-node contracts, not prompt-only conventions

## 6) OpenRouter dynamic enrichment (implemented + next)

Implemented in catalog refresh:

- parse and preserve `supported_parameters`
- parse architecture modalities (input/output) when present
- set capability hints (`supports_json`, `supports_vision`)
- infer initial `strengths` heuristically from model id/description/parameters
- bound `max_tokens` from provider completion limits when exposed
- apply refresh cadence controls via `VOX_OPENROUTER_CATALOG_MIN_REFRESH_INTERVAL_SECS` and `VOX_OPENROUTER_CATALOG_REFRESH_JITTER_MS`

Rationale:

- newly discovered models are no longer `strengths = []` by default
- dynamic models can participate in task-fit routing with better priors

Next enrichment pass (not yet implemented):

- periodic refresh with TTL + jitter
- trust-weighted admission policy for new models
- shadow-routing and score capture before full production eligibility
- provider constraints (`allow/ignore/order/sort`) mapped into Vox routing policy config

## 7) Remaining hard gaps

- no first-class verifier consensus cohort yet
- no single MAT-style (message-action trace) table family that unifies trust, lineage, tool actions, and generations
- runtime task execution and runtime provider-lane routing are still separate policy surfaces
- `.vox` orchestration grammar above is documented target surface, not yet parser/runtime behavior


