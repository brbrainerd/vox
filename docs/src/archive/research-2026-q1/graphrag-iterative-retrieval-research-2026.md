---
title: "GraphRAG Iterative Retrieval Research 2026"
description: "Deep dive into multi-hop retrieval loops, C2RAG sufficiency checks, and HippoRAG-style knowledge expansion."
category: "Architecture SSOTs"
status: "research"

last_updated: "2026-04-12"
training_eligible: false
archived_date: 2026-04-18
---

# GraphRAG Iterative Retrieval Research (2026)

## 1. The Multi-Hop Retrieval Problem

Single-pass RAG frequently fails on complex queries where evidence for the answer is not directly in the query but is connected through intermediate entities (A → B → C).

## 2. The `Retrieve-Reason-Retrieve` Loop

Vox adopts an iterative loop for high-complexity queries:
1. **Initial Retrieval**: Standard hybrid search over Tier 1/2.
2. **Partial Synthesis**: Socrates (or Lane G) identifies missing constraints.
3. **Query Expansion**: `vox-search` generates refined sub-queries based on partial evidence.
4. **Re-Retrieval**: Fetches new evidence without duplicating existing fetches.
5. **Final Synthesis**: Unified Socrates gate pass.

## 3. Key Heuristics

### 3.1 Stopping Conditions
- `evidence_quality ≥ 0.85`.
- Max hops reached (default: 3).
- Zero unique URLs returned in the latest hop.

### 3.2 Constraint-Checked Retrieval (C2RAG)
Decomposes the query into atomic constraints. Before synthesis, the system verifies that each constraint has at least one supporting chunk in the corpus. Missing constraints trigger a targeted research hop.

## 4. Performance Impacts
Iterative loops increase total research latency by 2x-3x. This is gated by the **Orient Phase**; only tasks in the `HighRisk` or `MultiHop` complexity band trigger expansion.

## 5. References
- *HippoRAG: Knowledge Graphs for Collaborative Reasoning* (2024)
- *GraphRAG-rs Technical Spec* (2026)

