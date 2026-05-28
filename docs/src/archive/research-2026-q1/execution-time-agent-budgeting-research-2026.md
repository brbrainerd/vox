---
title: "Execution Time Budgeting and Agent Learning Research 2026"
description: "Research and feasibility analysis for dynamic execution time budgeting, wait forecasting, and autonomous latency adaptation within Vox DEI and Arca architectures."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-10"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Execution Time Budgeting and Agent Learning Research 2026

## Executive Summary

As Vox transitions to advanced autonomous agents operating over unpredictable processes (including closed-source UI automation and complex compiler toolchains), relying on static wall-clock timeouts or "Intention Budgets" alone is insufficient. This document synthesizes recent 2026 industry research on dynamic timeout adaptation and outlines how to integrate these concepts into the existing Vox architecture. 

The core thesis: **Yes, based on the current Vox Orchestrator (DEI) and Arca storage layer, we can implement persistent execution time learning.** The agent can maintain an "Inter-Episode History" of tool execution durations and use it to calibrate its own delays, preventing endless loops or brittle, hard-coded sleeps without requiring human intervention.

## 1. Research Findings: The State of the Art (2026)

Extensive web research across modern LLM agent patterns yields four pillars of resilient temporal budgeting:

1. **Behavior-Aware Governance (Embedded Budgets):** Financial and intentional budgets must be translated into explicit execution constraints at inference time. Advanced systems use Budget-Aware Test-time Scaling (BATS), treating compute time as a constrained resource available in the agent's context.
2. **"Cognitive Timeline" Alignment (ICL for Time):** Avoid static `sleep()` calls. Agents use In-Context Learning (ICL) by receiving the actual execution time of past identical steps, calculating variance, and dynamically forecasting the safest wait constraint for the current step.
3. **Condition-Based Synchronicity:** For closed-source system interactions where completion events are hidden, agents transition to *Observe-Think-Act* loops. They execute a continuous, low-latency "is-ready" heuristic instead of monolithic, blocking waits.
4. **Adaptive Calibration (Inter-Episode History):** Rather than arbitrary guesses, agents record success, failure, and timeouts into persistence. A timeout is logged as a specific failure mode ("insufficient wait time"), triggering a decay/scaling factor applied to the agent's future wait-parameter estimates for that specific workflow.

## 2. Capability Assessment against Vox Architecture

Can Vox currently support Persistent Execution Time Learning? **Yes. The primitives exist.**

### Existing Telemetry & Persistence (Arca)
- **Status:** Vox possesses a robust, SQLite-backed telemetry layer (`research_metrics`, `chat_and_agent_tables`). 
- **Application:** We can store the start, completion, and tool footprint of external actions in Arca. The Arca schema (`telemetry-implementation-blueprint-2026.md`) provides the foundation.

### Exposing Temporal State to vox-dei (Orchestrator)
- **Status:** `vox-dei` dictates workflow routing and session management (`plan_sessions`).
- **Application:** Prior to invoking an inherently slow tool (e.g., launching a heavy application, training a net), the orchestration layer can query Arca for the P90 latency profile of that specific tool invocation. This historical data is injected into the agent's prompt/context frame ("*Historical average execution time: 45s. Timeout threshold set to 90s*"). 
- **Learning:** If a timeout triggers, the Orchestrator records a `timeout_exceeded` event in Arca. Subsequent agent runs naturally fetch a revised P90 latency or a heuristic scale factor, inherently dodging the endless loop.

## 3. Recommended Implementation Roadmap

To fully realize temporal resilience without degrading the prompt context limits:

1. **Phase 1: Tool Invocation Telemetry (Instrumentation)**
   - Wrap all state-mutating and asynchronous agent tool calls inside a `TimedExecution` context.
   - Flush execution durations grouped by tool name/fingerprint into an Arca table (e.g., `agent_exec_history`).
  
2. **Phase 2: Budget-Injection via Orchestrator Context**
   - Provide a new contextual read endpoint for the agent: `vox db query_tool_latency`.
   - Update `Contracts/ExecPolicy` to allow the DEI engine to preemptively enforce dynamic timeouts by pulling historical `avg_duration_ms` + a safety multiplier (e.g., 2.0x).

3. **Phase 3: Timeout Reflection (Self-Correction)**
   - When an agent process yields a timeout error, inject the error into the "Think" loop instead of hard-failing the session. Let the agent formulate a recovery protocol (e.g., "The software load timed out after 30 seconds. Based on history, I should retry with a 60-second observation boundary.").

## 4. Documentation Organization Review

An audit of the `docs/src/architecture/` boundary indicates that the project documentation is **properly organized in a highly structured, front-facing manner**. 
- The extensive use of **Single Source of Truth (SSOT)** documents (e.g., `telemetry-trust-ssot.md`, `operations-catalog-ssot.md`) isolates authoritative policy from transient tutorials. 
- Prefix and suffix conventions (`research-*`, `*-blueprint`, `-ssot`) systematically categorize intents. 
- The `architecture-index.md` acts as a cohesive landing page for navigation. 
The database of architectural knowledge scales very well for autonomous ingestion, precisely because it adheres to strict file naming and categorical domain segregation.


