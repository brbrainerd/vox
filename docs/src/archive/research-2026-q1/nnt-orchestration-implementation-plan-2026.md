---
title: "Network Neuroscience Theory Orchestration Implementation Plan (2026)"
description: "Implementation roadmap for integrating small-world topologies and affinity routing into the vox-dei orchestrator."
category: "Architecture SSOTs"
status: "roadmap"
sort_order: 186
last_updated: "2026-04-16"
training_eligible: false
training_rationale: "Defines the exact codebase mutations required to implement dynamic NNT-inspired routing."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Network Neuroscience Theory Orchestration Implementation Plan (2026)

## Overview

This implementation plan builds upon the research established in `network-neuroscience-theory-research-2026.md`. To evolve the Vox orchestrator into an **Agentic Neural Network**, we must deprecate static pipelines in favor of dynamic small-world topology routing.

This plan details the two core execution vectors:
1. **Dynamic Affinity Routing:** Using a distance matrix to group agents into "local clusters" or "long-range hubs".
2. **GRPO Reward Shaping:** Integrating an `r_routing_efficiency` signal into the MENS continuous learning loop.

## 1. Dynamic Boundary Definition (Affinity Matrix)

**Target:** `crates/vox-orchestrator/src/topology.rs`

Currently, `AgentRole` defines strict categorizations (`Generalist`, `Planner`, `Executor`, `Verifier`, etc.), but there is no policy enforcing routing boundaries. We will introduce an `AffinityMatrix` struct that computes semantic distance.

### Implementation Steps:
1. Add the `AffinityMatrix` struct to `topology.rs`.
2. Implement an `AffinityMatrix::distance(a: AgentRole, b: AgentRole) -> u8` method.
   - Distance 1 (Local Cluster): High-bandwidth loops like `Executor ↔ Verifier`.
   - Distance 2 (Intermediate): Medium hops like `Planner ↔ Generalist`.
   - Distance 3 (Long-Range Hub): Cross-domain hops bridging separated tasks.
3. Add a `routing_efficiency_penalty(&AgentTopologySnapshot)` method to calculate the aggregate topological stress of the current agent graph.
4. Update `TopologyGap` tracking from `topology.delegation_role_policy_missing` to reflect the newly implemented affinity.

## 2. Dynamic Handoff Resolution

**Target:** `crates/vox-orchestrator/src/handoff.rs`

Handoff payloads that do not specify a target agent (`to_agent: None`) currently fall back to broadcasting (`AgentId(0)`). We will enhance the handoff protocol so that receiver daemons use the `AffinityMatrix` to pluck pending tasks based on proximity to the sender's role.

### Implementation Steps:
1. Extend `handoff_context_event_metadata` or the `execute_handoff` pipeline to trace the role of the `from_agent`.
2. Document in the `PlanHandoff` event struct that consumers must sort available agent pools by the shortest affinity distance.

## 3. MENS GRPO Reward Function Updates

**Target:** `docs/src/architecture/research-grpo-reward-shaping-2026.md` and related MENS training presets (`contracts/mens/training-presets.v1.yaml`).

The current MENS continuous learning pipeline relies on a code-centric reward scalar:
`0.6 × r_syntax + 0.3 × r_test + 0.1 × r_coverage`

To explicitly train the LLM orchestrator to optimize for small-world topologies, we modify the loss function parameters.

### Implementation Steps:
1. Introduce the `r_routing_efficiency` term (derived from the inverse of the `routing_efficiency_penalty` from `topology.rs`).
2. Adjust the global weight configuration for GRPO:
   - `0.45 × r_syntax`
   - `0.25 × r_test`
   - `0.10 × r_coverage`
   - `0.20 × r_routing_efficiency`
3. This ensures the model receives positive reinforcement when self-organizing tight executor-verifier clusters, and penalties when making redundant, high-distance orchestrator hops.

## Wave Validation
- **Wave 0:** Ensure `cargo test -p vox-orchestrator` passes after injecting `AffinityMatrix`.
- **Wave 1:** Simulate a long-range vs short-range handoff and verify the penalty scores.
- **Wave 2:** Integrate into `vox-populi` continuous learning loss calculations.


