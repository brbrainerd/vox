---
title: "Crate API: vox-mcp"
description: "Internal MCP server crate: chat, codegen, and orchestrator bridges."
category: "API Reference — Crates"
status: deprecated
archived_date: 2026-04-13
training_eligible: false
schema_type: "TechArticle"
---

# Crate API: vox-mcp (Archived)

> [!WARNING]
> **ARCHIVED COMPONENT**: This file was archived on 2026-04-13. It is intentionally excluded from active AI context. It must not be referenced for contemporary development.
> This internal MCP server crate was superseded by the split `vox-mcp-meta` and `vox-mcp-registry` crates.

Embedded MCP (`vox-mcp`) talks to the workspace orchestrator for chat, routing telemetry, and codegen tools. See [Unified orchestration — SSOT](../reference/orchestration-unified.md) for contract boundaries.

<a id="llm-model-routing-modelstoml"></a>

## LLM model routing (`models.toml`)

Model registry and Ludus routing for MCP-backed chat and `vox_generate_code` are configured through the workspace model stack (including `models.toml` where present). Env overrides and cost telemetry hooks are documented in the orchestration SSOT and [env vars SSOT](../reference/env-vars.md).

## Execution Time Budgeting

The MCP server exposes `vox_exec_time_query` and `vox_exec_time_record` to interface with the orchestrator's dynamic budgeting system, replacing static timeouts with data-driven forecasts.

## HITL Doubt Integration

The `vox_doubt_task` tool is exposed to allow agents to formally transition their task into `TaskStatus::Doubted`.
Params matching `crate::params::DoubtTaskParams`:
- `task_id` (string): The UUID of the task.
- `reason` (string): Explanation of the contextual ambiguity or missing permission.
- `recommended_human_action` (string): Specific guidance for the human operator to resolve the doubt.
