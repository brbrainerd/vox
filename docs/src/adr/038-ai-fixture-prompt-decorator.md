---
title: "ADR 038 — AI fixture `@prompt` decorator"
description: "Proposes stage-aware prompt fixture lowering onto runtime cascade primitives."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
---

# ADR 038: AI fixture `@prompt` decorator

## Status

Accepted (2026-05-11). Implemented via `@prompt` lowering to `chat_with_cascade` / `ResearchStage` in Rust codegen.

## Context

Runtime already exposes research-stage cascades (`chat_with_cascade`, `cascade_for_research_stage`), but there is no declarative author-time fixture to bind a function to a stage/schema prompt contract.

## Decision

Introduce `@prompt(...)` decorator on `fn` declarations with initial payload:

- `stage` (`Planner | ClaimExtraction | Verification | Synthesis | Judge | SelfVerification`)
- `schema` (type name for structured contract)
- optional `redact` list for sensitive prompt fields

Lowering target is `HirAiFixture::Prompt`, then Rust codegen calls `chat_with_cascade` using stage defaults.

## Consequences

- Brings runtime stage policy into language-level authoring.
- Enables structured diagnostics for invalid stage/sensitive prompt shapes.
- Reuses existing LLM config and cascade machinery rather than introducing parallel prompt engines.

## Closed-keyword-table justification

Prompt behavior is represented as a decorator modifier on `fn`; no new bare `prompt` block is introduced.

## Diagnostic IDs to register

- `vox/prompt/invalid-stage`
- `vox/prompt/secret-leakage`
