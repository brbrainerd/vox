---
title: "ADR 040 — AI fixture `@search` decorator"
description: "Proposes retrieval fixture composition across docs, memory, and web search surfaces."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
---

# ADR 040: AI fixture `@search` decorator

## Status

Accepted (2026-05-11). Implemented via `@search` HIR fixture and Rust codegen (runtime wiring hardened in follow-on remediation).

## Context

Vox already ships retrieval/runtime surfaces (`vox-search`, memory manager lookup, research cascade) but they are consumed through programmatic or MCP pathways instead of a language fixture.

## Decision

Introduce `@search(...)` decorator on `fn` declarations with initial payload:

- `corpus` (`docs | code | memory | web`)
- `query` (string)
- `into` (target type)
- optional `top_k`, `policy`

Lowering target is `HirAiFixture::Search`, with codegen dispatch to:

- `execute_search_plan` (docs/code)
- `MemoryManager::lookup_fact_by_key` (memory)
- `cascade_with_optional_manual` (web)

## Consequences

- Unifies retrieval authoring under one language surface.
- Preserves ACI mutation classification for each corpus path.
- Enables compile-time diagnostics for unsupported corpus/type combinations.

## Closed-keyword-table justification

Search intent is modeled as a decorator on existing function declarations, not as a new bare keyword.

## Diagnostic IDs to register

- `vox/search/corpus-denied`
- `vox/search/memory-key-invalid`
- `vox/search/web-policy-denied`
