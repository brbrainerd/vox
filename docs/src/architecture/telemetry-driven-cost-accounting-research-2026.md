---
title: "telemetry-driven-cost-accounting-research-2026"
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---
# Telemetry-Driven Cost Accounting Architecture (2026)

## Status: IMPLEMENTED (v59)
**SSOT Reference:** `model-orchestration-ssot-audit-2026.md` (§FIX-75)

## Overview
This document defines the architecture for the Vox telemetry-driven cost accounting system. This system transitions the model orchestrator from static, hardcoded pricing estimates to a **self-correcting, empirical feedback loop** that reflects ground-truth spend reporting across all providers.

## Core Problem
Static model catalogs (like `model-catalog.bootstrap.json`) are brittle. Provider pricing fluctuates (e.g., Gemini promotional periods), and unified billing aggregators (OpenRouter) often have hidden discounts or tiered pricing that static values cannot capture. Relying on static values leads to inaccurate budget reporting and sub-optimal routing.

## Architectural Components

### 1. The Pricing Catalog (Scientia v59)
The `model_pricing_catalog` table in the `scientia` domain serves as the system's permanent memory. It correlates `model_id` and `provider` with observed costs.
- **Observed Blended Rate**: The primary metric (`total_cost / total_tokens`), maximizing compatibility with 100+ providers.
- **Confidence Tiers**: Samples are gated by volume (Low < 20, Medium < 100, High >= 100) to prevent noisy data from poisoning the catalog.

### 2. The Learning Loop
Every API interaction recorded in `llm_interactions` (when provider-reported billing is available) is processed by the **Rollup Engine**.
- **Rollup Engine**: Nightly task executed via `scripts/orchestrator/scoreboard_rollup.vox`.
- **Calibration**: Aggregates `cost_usd`, `input_tokens`, and `output_tokens` into the catalog.

### 3. Dynamic Registry Injection
The `ModelRegistry` performs a calibration step at startup/refresh.
- High-confidence observed prices are injected into `ModelSpec` objects.
- Routing heuristics (Scoring) and budget gates (BudgetManager) automatically utilize these "calibrated" prices.

## Scoring & Optimization
The system achieves **Value-for-Money (VfM) Routing** by correlating:
1. **Observed Cost**: Verifiable ground-truth spend.
2. **Organic Success Rate**: Statistical performance per model/task.

The orchestrator selects models by minimizing `cost_per_success`, ensuring the system always picks the most efficient model for the current task complexity.

## CLI & Operations
- `vox model pricing show`: Provides operator visibility into catalog drift and confidence levels.
- `vox model pricing rollup`: Allows manual trigger of the calibration loop.

## Security & Guardrails
- **Sample Gating**: Prevents a single anomalous interaction from causing a routing spike.
- **Provider Parity**: Normalizes billing across OpenRouter, direct provider APIs, and local `$0` mesh nodes.
- **Audit Consistency**: Verified against `vox ci ssot-audit --features dei`.

