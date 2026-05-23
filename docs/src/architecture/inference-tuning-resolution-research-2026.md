---
title: "Inference Tuning Resolution Research 2026"
description: "Research into precedence-based parameter resolution for LLM inference."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Documents the April 2026 shift to granular tuning overrides."
---

# Inference Tuning Resolution Research 2026

## Objective
Establish a robust, secure, and predictable mechanism for propagating inference parameters (`temperature`, `top_p`) from diverse sources (MCP clients, environment variables, and tool defaults) to LLM providers.

## The Problem
Prior to April 2026, inference tuning was fragmented. Some tools hardcoded `0.1` for stability, others used `0.7` for variety, and there was no way for a user to override these values without modifying the source code. This prevented power users from fine-tuning reasoning models or relaxing the temperature for creative tasks.

## Resolution Model: The 3-Tier Precedence
We have adopted a strict 3-tier resolution model for all inference calls:

| Priority | Source | Description |
| --- | --- | --- |
| **1 (Highest)** | **Request Override** | Parameters passed directly in the MCP tool call (e.g., `temperature` field in `ChatMessageParams`). |
| **2** | **Registry Secret** | Provider-specific environment variables resolved via `vox-secrets` (e.g., `GEMINI_TUNING_TEMPERATURE`). |
| **3 (Baseline)** | **Tool Default** | The "safe" baseline defined by the tool author (e.g., `0.1` for `inline_edit`). |

## Implementation Strategy

### 1. Registry Integration
We extended the Secret Registry to include tuning overrides for major providers. This ensures that a single environment variable can shift the baseline for all tools using that provider.

- `GEMINI_TUNING_*`
- `OLLAMA_TUNING_*`
- `OPENAI_TUNING_*`
- `ANTHROPIC_TUNING_*`

### 2. Signature Standardization
The core `mcp_infer_tool_completion` was refactored to take `base_temperature: f32` and `Option<f32>` overrides. This forces every call site to consciously provide a functional baseline while allowing the bridge to handle the complex resolution logic.

### 3. Telemetry and Observability
To prevent "invisible tuning" (where a user is unsure which value was actually sent), we instrumented the `vox.mcp.llm.tuning` channel. This target logs the final resolved parameters for every request.

## Findings
- **Anthropic Constraints**: Anthropic's API is sensitive to `top_p` values when they are not explicitly needed. Our implementation uses `Option<f32>` to ensure we only serialize these fields when they have been explicitly overridden.
- **Provider Parity**: Mapping `OpenRouter` and `Custom` variants to the `OPENAI_TUNING_*` secrets provides a consistent experience for cloud proxies.

## Future Work
- **Per-Model Tuning**: Currently, tuning is provider-wide. Future iterations may allow `GEMINI_1_5_PRO_TUNING_TEMPERATURE` for more granular control.
- **User Preference Sync**: Integration with a central user preference store to persist tuning settings across sessions.
