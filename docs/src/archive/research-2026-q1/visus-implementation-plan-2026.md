---
title: "Archive"
description: "Archive"
category: "Architecture SSOTs"
status: "legacy"
training_eligible: false
archived_date: 2026-04-18
---
# Vox Visus: GUI Visual Intelligence Implementation Plan (2026)

## Overview
This document outlines the transition roadmap for **Vox Visus**, the GUI Visual Intelligence track within the FableForge ecosystem. With the deterministic CSS pipeline and structural DOM isolation finalized, Visus will provide the necessary VLM-driven guardrails to detect visual hallucinations, hydration flashes, and z-index overlap bugs that static IR analysis cannot catch.

## Current System State
As of April 2026, the underlying persistence and routing models for Visus are already seeded in the architecture:
- `TaskCategory::Visus` routing is fully defined in the orchestrator.
- Database tables and types (`visus_baselines`, `visus_audit_log`, `VisusBaselineRow`, `VisusAuditLogRow`) exist in `vox-db`.
- MCP tools (`vox_visus_audit`, `vox_visus_baseline`) are registered in `tool-registry.yaml`.

**Missing Context (The Gap):**
The MCP tool `vox_visus_audit` is currently missing from the orchestrator's dispatch pipeline (`crates/vox-orchestrator-mcp/src/dispatch.rs`) and lacks the execution logic needed to trigger the browser subagent, capture WebP/screenshots, and pipe them into the Vision-Language Model (VLM).

## Implementation Waves

### Wave 1: Dispatch Integration
1. **Tool Dispatch:** Wire up `vox_visus_audit` and `vox_visus_baseline` inside `dispatch.rs` and `input_schemas.rs`.
2. **Subagent Orchestration:** Implement the scaffolding to trigger a headless browser session (using standard DevTools or Playwright backend) to capture the DOM state.
3. **Payload Construction:** Format the screenshot capture (WebP/PNG) into a base64 payload alongside the AXTree (Accessibility Tree) for context.

### Wave 2: VLM Inference Pipeline
1. **Model Routing:** Ensure that `TaskCategory::Visus` correctly routes to a vision-capable frontier model (`qwen-vl`, `gpt-4o`, etc.) using the inference gateway.
2. **Audit Prompt Design:** Define the canonical prompt for `vox_visus_audit` to specifically detect:
   - Overlapping text / stacking context collisions.
   - Contrast ratio violations against the design token palette.
   - Cascading CSS leakage outside of expected `@layer` boundaries.

### Wave 3: Persistence and Regression Testing
1. **Golden Baselines:** Implement `vox_visus_baseline` to store passing GUI states in the local `vox-db`.
2. **CI Regression Guard:** Integrate `vox ci visus` to enforce that subsequent PRs do not trigger visual anomalies compared to the established baseline.


