---
title: "Rust ecosystem support SSOT"
description: "Canonical support matrix and debt model for Rust crate families used by Vox lanes."
category: "Architecture SSOTs"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Rust ecosystem support SSOT

This page defines the single source of truth for which Rust crate families Vox supports, how they are exposed (or hidden), and how support decisions are measured against maintenance debt.

## Scope

The support model follows the bell-curve design center and interop constraints:

- prefer `tier0` builtins and narrow `tier1` wrappers for common app software
- keep `tier3` escape hatch (`import rust:...`) available for uncommon needs
- avoid representing arbitrary crate APIs as first-class typed Vox language surfaces

Canonical machine-readable data:

- [`contracts/rust/ecosystem-support.yaml`](../../../contracts/rust/ecosystem-support.yaml)
- [`contracts/rust/ecosystem-support.schema.json`](../../../contracts/rust/ecosystem-support.schema.json)

## Data contract fields

Each support entry records:

- `crate_family`: logical crate group (single crate or paired family)
- `product_lane`: one of `app`, `workflow`, `ai`, `interop`, `data`, `platform`
- `support_tier`: `tier0` / `tier1` / `tier2` / `tier3`
- `boundary_owner`: `WebIR`, `AppContract`, `RuntimeProjection`, `builtin_registry`, `approved_binding`, or `escape_hatch`
- `semantics_state`: `implemented`, `partially_implemented`, `planned`, `docs_only`
- `capability_value`: 0-100 estimate of bell-curve impact
- `debt_cost`: 0-100 estimate of ongoing ownership burden
- `supported_targets`: one or more of `native`, `wasi`, `container`
- `decision`: `first_class`, `internal_runtime_only`, `escape_hatch_only`, or `deferred`
- `notes`: short rationale tied to boundaries and migration risk

## Debt dimensions

`debt_cost` must be justified by this weighted profile:

| Dimension | Weight | Prompt |
|---|---:|---|
| API breadth | 20 | How wide is the Vox-facing wrapper surface we must stabilize? |
| Runtime coupling | 20 | How tightly does this crate couple to runtime internals or async policy? |
| Platform variance | 15 | How much behavior diverges across native, WASI, and container lanes? |
| Security and policy liability | 20 | How much auth, secret, or unsafe network behavior must Vox own? |
| Upstream churn | 15 | How often are breaking changes expected from upstream crates? |
| Docs and test burden | 10 | How many contract tests and docs must stay in parity? |

## Capability model

`capability_value` should be scored against the bell-curve ranking shape:

- user reach in common app software
- LLM leverage (prompt burden removed)
- boundary fit with existing IR/registry/runtime seams
- implementation risk
- drift reduction potential

## Promotion policy

A crate family moves from `tier3`/`deferred` to `tier1` only when all conditions pass:

1. A narrow wrapper namespace is defined (no raw crate mirror).
2. Typecheck and codegen/runtime mappings are deterministic and tested.
3. Docs state implemented/planned semantics precisely.
4. Target support (`native`/`wasi`/`container`) is explicit.
5. The resulting `debt_cost` remains acceptable relative to `capability_value`.
6. Any crate listed under `template_managed_dependencies` must also appear by Cargo name in `support_entries.crate_family`.

## Runtime-internal crates

Some crate families are intentionally "supported but hidden":

- `tokio`
- `axum+tower`

These remain internal runtime engine choices. Vox users should consume stable Vox contracts (`WebIR`, `AppContract`, `RuntimeProjection`, `std.*`) rather than direct crate APIs.

## Data-lane policy

Data support prioritizes `turso+vox-db` before broad SQL ecosystems. `sqlx`, `diesel`, and `sea-orm` remain deferred/escape-hatch until:

- data-lane abstractions are stable,
- representative app/workflow examples prove demand,
- and debt-to-value ratio improves.


