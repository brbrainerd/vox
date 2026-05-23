---
title: "ADR 014: async-openai selective adoption (spike outcome)"
description: "Bounded spike: whether to adopt the async-openai crate after internal OpenAI-compat unification"
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 014: `async-openai` selective adoption (spike)

## Context

Vox now shares **non-streaming** chat JSON types via `vox-openai-wire`, **SSE line assembly and deltas** via `vox-openai-sse`, and **HTTP client defaults** via `vox-reqwest-defaults`. Durable runtime chat/stream/embed paths stay in `vox-runtime` with Clavis-backed key resolution.

## Spike scope

Evaluate [`async-openai`](https://crates.io/crates/async-openai) for **strictly OpenAI-compatible** HTTPS endpoints only (official API shape), *after* the above internal modules exist — so the decision is about dependency surface, not about fixing parsing drift.

## Findings (go / no-go)

**Decision: no-go as a mandatory core dependency for now.**

| Criterion | Outcome |
| --- | --- |
| OpenRouter / HF router / custom `base_url` | Still need bespoke URL + header wiring; `async-openai` targets the official client shape. |
| Streaming | We standardized on `vox-openai-sse` + `reqwest` byte streams; swapping to crate-specific stream types duplicates that layer. |
| Secrets | Clavis resolution must remain at the boundary; wrapping `async-openai` would still tunnel API keys we assemble ourselves. |
| Code reduction post-unification | Marginal for our **multi-provider** matrix; cost is an extra abstraction and version lock on upstream breaking changes. |

## When to revisit

- If a single product path becomes **OpenAI-only** (fixed URL, official SDK semantics) *and* we drop custom SSE for that path.
- If we need **official**-assisted request types beyond our thin `vox-openai-wire` structs and are willing to take version churn.

## Related

- `vox-openai-wire`, `vox-openai-sse`, `vox-reqwest-defaults`, `vox-runtime` LLM modules.
- Maintainability plan Phase 4 / `async-openai` spike item — this ADR records the outcome.


