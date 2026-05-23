---
title: "MENS Research Track Blueprint 2026"
description: "Architectural specification for MENS Lane G (research-expert) adapter and post-training protocol."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-12"
training_eligible: false
archived_date: 2026-04-18
---

# MENS Research Track Blueprint (2026)

## 1. Lane G: `research-expert` Specification

The `research-expert` lane is a dedicated training track focused on evidence synthesis, multi-hop reasoning, and contradiction resolution.

### 1.1 Objective
Unlike Lane A (code generation), Lane G is optimized for:
- **Evidence Synthesis**: Merging RRF hit lists into coherent reasoning.
- **Multi-hop Logic**: Chaining facts A + B to answer query C.
- **Abstention Calibration**: Refusing to answer when evidence quality is below 0.3 or contradictory.

## 2. Training Paradigm

### 2.1 Base Model
- **Base**: `Qwen/Qwen3.5-4B`.
- **Target**: 16GB VRAM (Consumer GPU invariant).

### 2.2 Stage 1: SFT
- **Data**: 10,000 synthetic multi-hop chains from `vox-corpus research-gen`.
- **Format**: Instruction-pair with structured synthesis.

### 2.3 Stage 2: GRPO Fine-Tuning
Utilizes Group Relative Policy Optimization (GRPO) with Reinforcement Learning with Verifiable Rewards (RLVR).

| Reward | Signal | Failure Penalty |
|---|---|---|
| **Citation Groundedness** |Cited URL exists in input | -1.0 |
| **Synthesis Completeness**| All sub-questions answered | 0.0 |
| **Format Adherence** | Valid JSON/Structure | -0.5 |
| **Contradiction Res** | Downstream gate consistency | 0.0 |

## 3. Synthetic Data Strategy

To avoid data exhaustion and privacy leakage, we use rule-based synthetic generation of fictional knowledge graphs. This forces the model to learn the *logic* of composition rather than memorizing facts.

```json
{
  "lane": "vox_research_expert",
  "task_family": "retrieve_and_synthesize",
  "hop_count": 3
}
```

## 4. Integration into Socrates

Local synthesis results are injected into the `SocratesTaskContext`. When `research_model_enabled` is true, the orchestrator delegates to this specific adapter rather than using the generic code model for research summaries.

