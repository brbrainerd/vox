---
title: "Feature growth boundaries"
description: "SSOT for where Vox should add app capability first: WebIR, AppContract, RuntimeProjection, builtin registry, and approved bindings."
category: "Architecture SSOTs"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Feature growth boundaries

## Decision

For bell-curve app work, Vox should grow through existing compiler and contract boundaries before adding new syntax.

Preferred order:

1. `WebIR` for UI and frontend semantics
2. `AppContract` for routes, loaders, mutations, server/client shape, and app capability metadata
3. `RuntimeProjection` for task capability hints, routing, and runtime policy snapshots
4. builtin registry plus runtime/codegen wiring for narrow standard-library growth
5. approved bindings and wrapper packages for third-party capability
6. explicit escape hatches for uncommon cases

## Guardrails

- Do not add a parallel first-class frontend runtime before `WebIR` fully owns the current React/TanStack stack.
- Do not imply `import rust:...` exposes arbitrary typed Vox APIs.
- Do not add syntax when a bounded IR, registry, or approved binding can solve the same problem.
- Treat generated and interpreted workflow behavior as different semantics until they actually converge.
- Keep runtime-engine crate choices (`tokio`, `axum`, `tower`) behind projection/contract boundaries instead of exposing them as user-facing Vox APIs.

## “Implemented” vs “planned”

Use these terms precisely:

| Label | Meaning |
| ----- | ------- |
| `implemented semantics` | behavior exists in the shipping compiler/runtime path and is tested |
| `planned semantics` | docs may describe the intended future model, but it is not yet the live guarantee |
| `language intent` | syntax and design direction exist, but runtime behavior may still be partial |
| `escape hatch` | supported non-default path for advanced or uncommon use cases |

## Review questions

Before adding a new bell-curve feature, answer:

1. Which existing boundary should own this?
2. Why is that boundary insufficient today?
3. Can the need be met by a wrapper or contract instead of syntax?
4. What acceptance tests prevent drift between docs, typechecker, codegen, and runtime?

## Canonical projection drift gate

The **WebIR + AppContract + RuntimeProjection** triplet must stay deterministic and versioned. The integration test `projection_triplet_is_deterministic_and_schema_versioned` in `crates/vox-compiler/tests/projection_parity.rs` exercises canonical byte stability for all three projections from one fixture.

**Local / CI reproducer:**

```bash
cargo test -p vox-compiler --test projection_parity
```

`.github/workflows/ci.yml` runs `cargo test -p vox-compiler --test projection_parity` on the main pipeline. Extend this test (not ad-hoc snapshots) when adding new fields to any of the three contract structs so drift is caught in one place.


