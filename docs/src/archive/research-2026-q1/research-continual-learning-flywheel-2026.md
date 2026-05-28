---
title: "Continual Learning Flywheel Risks"
description: "Cluster overview: empirical risk assessment for the Vox MENS dogfood training flywheel, covering model collapse, oracle failures, catastrophic forgetting, slop contamination, and corpus thresholds."
category: "Architecture SSOTs"
status: "research"
research_source: "gemini_deep_research"
research_date: "2026-04-08"
training_eligible: false
last_updated: "2026-04-09"
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Continual Learning Flywheel Risks

## Executive Summary

Deploying an autonomous dogfood or self-play training flywheel—in which a model continuously fine-tunes itself on its own generated outputs—carries a **critical baseline risk of systemic degradation**. Three interacting failure modes threaten the Vox MENS architecture:

1. Recursive ingestion of synthetic data drives Model Autophagy Disorder (MAD), leading to irreversible variance loss and mode collapse.
2. Reliance on a binary compile-pass oracle without semantic execution checks exposes the system to reward hacking and severe semantic drift.
3. Repeated QLoRA fine-tuning cycles on limited data volumes induce catastrophic forgetting, mechanically overwriting the base model's generalized reasoning and natural language capabilities.

Contemporary research offers empirically validated countermeasures: transitioning from a "replace" to an "accumulate" synthetic data strategy; integrating execution-based verification or oracle-less proxy metrics; and deploying advanced PEFT stabilization techniques such as CURLoRA, O-LoRA, or FAPM. Agent-generated prose (Schola/Scientia) remains the most volatile element and requires stringent external filtering.

## Detailed Research Pages

- [Quality and Mode Collapse in Self-Play LLM Loops](research-cl-mad-mode-collapse-2026.md)
- [The Compile-Pass Oracle and Semantic Degradation](research-cl-oracle-semantic-drift-2026.md)
- [Catastrophic Forgetting in QLoRA Fine-Tuning](research-cl-qlora-catastrophic-forgetting-2026.md)
- [The Risks of Agent-Generated Prose (Schola & Scientia)](research-cl-slop-typicality-bias-2026.md)
- [Minimum Viable Corpus Size for QLoRA Domain Adaptation](research-cl-qlora-minimum-corpus-2026.md)
- [Utilizing Parse Failures as Negative Examples](research-cl-nat-dpo-2026.md)
- [Risk Taxonomy, Monitoring Design, and Open Research Questions](research-cl-risk-taxonomy-telemetry-2026.md)
- [Works Cited: Continual Learning Flywheel Risks](research-cl-works-cited-2026.md)


