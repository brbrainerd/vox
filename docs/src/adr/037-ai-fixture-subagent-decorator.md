---
title: "ADR 037 — AI fixture `@subagent` decorator"
description: "Proposes a decorator-based author surface for subagent dispatch policy without introducing a new bare keyword."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
---

# ADR 037: AI fixture `@subagent` decorator

## Status

Accepted (2026-05-11). Implemented in compiler lexer/parser/HIR lowering and Rust codegen (`vox-codegen`), staged behind `ai-fixtures-v1` where applicable.

## Context

Subagent routing already ships in orchestrator policy (`DispatchRouter::route`), but there is no first-class language fixture for authors to declare dispatch intent at function scope.

The grammar policy requires decorator composition and forbids introducing new bare keywords for this behavior.

## Decision

Introduce `@subagent(...)` on `fn` declarations with initial payload:

- `policy` (`cap_chain | inline_only | parallel`)
- `max_depth` (`u32`)
- optional `budget_usd`, `description`, `parallel` compatibility key

Lowering target is `HirAiFixture::Subagent` and codegen/orchestrator adaptation through `DispatchSignal` + `DispatchRouter::route`.

## Consequences

- Parser/lowering adds new fixture payload fields.
- Type checking can reject obviously invalid depth/budget combinations at compile time.
- Runtime can emit canonical `orch-subagent-dispatch` telemetry.
- Contract alignment stays in existing ACI + MCP surfaces.

## Closed-keyword-table justification

This extends behavior with a decorator on `fn`, preserving the existing bare-keyword table and avoiding a new `subagent` declaration form.

## Diagnostic IDs to register

- `vox/subagent/chain-depth-exceeded`
- `vox/subagent/budget-exhausted-inline`
