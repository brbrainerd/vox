---
title: "Production Failure Mode Catalog with Mitigations"
category: "Architecture SSOTs"
status: "research"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
**7\. Production Failure Mode Catalog with Mitigations**

| Failure Mode | Trigger Mechanism | Architectural Mitigation |
| :---- | :---- | :---- |
| **Context Bleed / Poisoning** | Passing full accumulated conversation history to downstream, specialized sub-agents, bloating their context windows. | **Surgical Context Injection:** Sub-agents must be instantiated as stateless endpoints. Pass only the explicit task definition, a structured snapshot of current world state, and a maximum of 1-3 relevant history turns.3 |
| **Silent Context Truncation** | Token accumulation exceeds hidden buffer limits (e.g., MEMORY.md 200-line cap), dropping oldest constraints without triggering API errors.62 | **Integrity Assertions:** Monitor stop\_reason flags. Implement a discrepancy check between generated text intent and emitted tool payloads. Route histories through hierarchical compaction prior to context insertion.1 |
| **Infinite Handoff Loop ("Mirror Mirror")** | Directive misalignment between two specialized agents (e.g., conflicting formatting rules) bouncing rejections back and forth without overarching authority.36 | **Stateful Task Lifecycles:** Enforce A2A Task objects that track iteration states. Implement hard timeout budgets and a designated "Manager" or "Supervisor" node with overriding arbitration authority.36 |
| **Identity Smuggling** | A remote agent acts on a delegated task using a generic service account, losing the original user's authorization trace and creating compliance blind spots.64 | **OBO (On-Behalf-Of) Token Exchange:** Embed short-lived, user-scoped OAuth or Decentralized Identifier (DID) tokens within the A2A Request Context. Reject any remote invocation lacking cryptographic provenance.34 |
| **Attention Dilution ("Lost in Middle")** | "Always retrieve" policies flooding the context window with tangentially related chunks (hard distractors), drowning out core logic.9 | **Adaptive Retrieval (CRAG/SCIM):** Insert a lightweight evaluator model before retrieval injection to score chunks. Drop 'Ambiguous' or 'Incorrect' chunks to preserve prompt hygiene and trigger web fallbacks when necessary.55 |

## ---


*(Original Source: AI Agent Context and Handoff Research)*

