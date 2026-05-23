---
title: "Internal Architecture Repository"
description: "Entry point for Vox internal architecture SSOTs, research findings, and planning documents."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-06"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Architecture Index

The files in the `/architecture` directory serve as single sources of truth (SSOTs) and working memory for the Antigravity system and human contributors. 

**Note for End-Users**: This section is internal documentation. For public language and toolchain documentation, see the [Reference Guide](../reference/ref-syntax.md) or [How-to Guides](../how-to/how-to-islands-and-pages.md).

## Core Architecture Documents

- [Language Surface SSOT](language-surface-ssot.md)
- [CLI Design Rules SSOT](cli-design-rules-ssot.md)
- [Trust & Reliability Layer (SSOT)](trust-reliability-layer.md)
- [Codex vNext — Schema Domains](codex-vnext-schema.md)
- [Telemetry Trust Boundary](telemetry-trust-ssot.md)
- [Outbound HTTP Policy](outbound-http-policy.md)

## Master Roadmaps and Backlogs

- [Master Planning Index](planning-meta/01-master-planning-index.md)
- [Vox Bell-Curve Strategy](vox-bell-curve-strategy.md)
- [SSOT & DRY Convergence Roadmap](ssot-convergence-roadmap.md)
- [TanStack Web Roadmap](tanstack-web-roadmap.md)

## AI Generation and Orchestration

- [Agentic Loop & MENS Pipeline Blueprint](vox_agentic_loop_and_mens_plan.md)
- [Completion Policy SSOT (Anticipatory Stopping)](completion-policy-ssot.md)
- [Socrates Anti-Hallucination Protocol](../adr/005-socrates-anti-hallucination-ssot.md)
- [MCP Exposure SSOT](mcp-vox-language-exposure.md)

## RAG, Retrieval, and Autonomous Research

- [**RAG and Research Architecture 2026 (SSOT)**](rag-and-research-architecture-2026.md) — Full pipeline SSOT: corpora, CRAG loop, Socrates gate, Tavily integration, A2A handoff, query pre-processing
- [Research Trust & Reliability Signals](research-trust-reliability-signals-2026.md) — EWMA failure modes, Coverage Paradox, Bayesian routing recommendations
- [A2A Evidence Sharing](research-agent-handoff-a2a-evidence-sharing-2026.md) — Inline embedding vs. durable artifact references, A2A protocol analysis
- [Prompt Engineering & Scientia Research](prompt-engineering-document-skills-scientia-research-2026.md)

## MENS Training Research

- [MENS Cloud GPU Training Strategy](../reference/mens-cloud-gpu.md)
- [Hardware Discovery SSOT (Native Registry)](gpu-discovery-ssot-findings-2026.md)
- [MENS Composer vs. Kimi Findings](mens-composer-kimi-findings-2026.md)
- [Candle Full-Graph Feasibility](candle-full-graph-feasibility.md)

*(For a full auto-generated list of existing architectural blueprints and planning memos, see the underlying `/architecture` directory in your workspace or the file tree.)*

