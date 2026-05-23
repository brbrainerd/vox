---
title: "Language ergonomics principles"
description: "Principles for reducing boilerplate in Vox while preserving readability and explicit intent."
category: "Language Reference"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# Language ergonomics principles

## Goals
- Reduce repetitive syntax that carries no domain meaning.
- Keep control flow and data ownership explicit.
- Prefer transformations that compile to predictable core IR forms.

## Rules for adding sugar
- Add syntax sugar only when it removes repeated patterns seen in real code.
- Every sugar feature must have a direct desugared form in docs and tests.
- Avoid sugar that hides side effects or mutability.
- Favor local inference over whole-program implicit behavior.

## Inference boundaries
- Inference is preferred for local bindings and obvious expression results.
- Explicit annotations remain required when ambiguity impacts readability or diagnostics.
- Public APIs should remain readable without deep type reconstruction.

## Error ergonomics
- Error propagation should minimize ceremony while preserving type-level clarity.
- Early-exit forms must remain obvious in control-flow graphs and diagnostics.
- Compiler diagnostics should suggest desugared equivalents when syntax is unfamiliar.

## Full-stack ergonomics guardrails
- One declaration should define route contract, server behavior, and typed client shape.
- Validation schemas should be shareable across frontend and backend.
- Command and tool metadata should derive from one canonical source where possible.

## Admission checklist for new ergonomics features
- Boilerplate reduction is measurable (lines or repeated edit classes).
- Parsing and lowering rules are deterministic and test-covered.
- Typechecker behavior remains stable and diagnosable.
- Codegen for Rust and TS remains semantically aligned.
- Migration path and lint guidance are provided.


