---
title: "ADR 039 — AI fixture `@hole` decorator"
description: "Proposes explicit deferred-fill fixtures with compile-time enforcement and reviewer accountability."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
---

# ADR 039: AI fixture `@hole` decorator

## Status

Accepted (2026-05-11). Implemented with typeck `vox/fixture/unfilled-hole` and optional `vox/fixture/stale-hole` ledger guard.

## Context

Teams need a structured way to defer implementation with AI assistance while preserving compile-time safety and reviewability.

Unstructured placeholders are not acceptable for Vox language quality and training-data integrity.

## Decision

Introduce `@hole(...)` decorator on function bodies with required payload:

- `spec` (intent string)
- `reviewer` (`human | ci`)
- `cache_key` (stable deterministic key)
- optional `constraints` list

No runtime lowering in the initial implementation. Type checking emits errors until the hole is resolved or explicitly suppressed under policy.

## Consequences

- Compile safety remains strict; unfinished holes cannot silently ship.
- CI can add stale-hole detection keyed by `cache_key`.
- Provides an explicit substrate for future fill tooling without enabling hidden execution paths.

## Closed-keyword-table justification

A decorator-based fixture modifies `fn` behavior and avoids introducing a new bare `hole` keyword.

## Diagnostic IDs to register

- `vox/fixture/unfilled-hole`
- `vox/fixture/stale-hole`
- `vox/fixture/expression-hole-unsupported`
