---
title: "Agent Messaging & Orchestration Roadmap (Aspirational)"
description: "Official documentation for Agent Messaging & Orchestration Roadmap (Aspirational) for the Vox language."
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Agent Messaging & Orchestration Roadmap (Aspirational)

This document outlines the aspirational goals for the Vox Distributed Execution Intelligence (DEI) orchestrator and agent-to-agent (A2A) messaging architecture, tracking toward state-of-the-art 2026 multi-agent patterns.

## 1. Context Management Evolution

**Current State**: Context is primarily bounded by file selections, explicit `@mentions`, and static chat history keys.
**Aspirational Goals**:
- **Continuous Context Engineering**: Move beyond static prompt injection. Introduce automatic real-time context summarization where long-running agent threads compress their episodic memory into semantic checkpoints.
- **Multimodal State Integration**: Support the injection of UI visual snapshots and multimodal telemetry natively in `ChatMessage` constructs, preventing agents from becoming text-blind to DOM or pixel-level changes.
- **Context Routing**: Implement policies that automatically "shed" irrelevant history when an agent shifts execution domains (e.g., from database debugging to UI CSS tweaking) -> save token budgets and prevent hallucination bleed.

## 2. Multi-Agent Topologies & Orchestration

**Current State**: Tasks are routed to the most capable single agent based on affinity (`vox-orchestrator`'s routing service).
**Aspirational Goals**:
- **Specialized "Agent Pods"**: Break down monolith tasks into sub-delegations using a hierarchical task network (HTN). Assign specialized agents (Planner, Executor, Verifier, Researcher) -> specific nodes instead of relying on general-purpose code-gen agents.
- **Dynamic Handoff/Triage (Delegation Pattern)**: An agent can unilaterally pause execution to issue an A2A RPC requesting help from an agent with higher `Trust` or specific `tool` permissions (e.g., a "Security Agent" for signing commits or handling API tokens).
- **Parallel Analysis (Map-Reduce)**: The Orchestrator should support spawning *N* ephemeral agents to analyze independent files concurrently across the mens, gathering the results via an accumulator agent.

## 3. Advanced Memory & Socrates Integration

**Current State**: `vox_chat_message` and `vox_memory_search` share a unified retrieval trigger that prefers hybrid BM25 + vector search and falls back deterministically when embeddings/DB are unavailable. Broader autonomous contradiction-resolution orchestration remains aspirational.
**Aspirational Goals**:
- **Autonomous Subconscious Recall**: All LLM entrypoints should automatically run a low-latency vector-BM25 hybrid query against the `Codex` memory block using the user's prompt as the latent space seed. High-confidence facts (`score > 0.85`) should silently append to the preamble, fulfilling the "agent *knows* when to look" imperative.
- **Contradiction Resolution Agents**: If the `MemorySearchEngine` detects a `potential_contradiction`, the Orchestrator should automatically pause the fast-path pipeline and insert a "Resolution Re-plan" task, spawning an investigative agent to resolve the factual split before the primary agent generates code.

## 4. System Governance as an 'OS' Layer

**Current State**: Orchestrator enforces basic limits (`max_agents`, `stale_threshold_ms`, lock contention).
**Aspirational Goals**:
- **Structured Orchestration Transitions**: Formalize task execution into a state machine: `Understand -> Plan -> Act -> Evaluate`. Currently, agents can loop infinitely unless gated. This OS-level transition forces an episodic commit at each boundary.
- **Standardized A2A Protocol Alignment**: Expose the internal `MessageBus` to conform fully with emerging 2026 standards like Google's Agent-to-Agent (A2A) protocol or Anthropic's Model Context Protocol (MCP) multi-agent routing extensions, allowing Vox mens nodes to interoperate with non-Vox, third-party agents running on external infrastructure.

## Next Steps for Build-out
1. Implement basic session-isolated history in `vox-mcp` (Immediate).
2. Extend chat retrieval into task-level replan orchestration when contradiction hints are detected (Immediate).
3. Draft the HTN topology spec for `vox-orchestrator/src/queue.rs` (Q3 2026).
4. Build the `PodManager` to enforce specialized agent teaming (Q4 2026).


