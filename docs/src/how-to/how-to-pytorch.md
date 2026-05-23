---
title: "Python Library Integration (Retired)"
description: "Stub: @py.import has been removed from the Vox compiler. See AGENTS.md §VoxScript-First Glue Code for the canonical glue surface."
category: "How-To Guides"
last_updated: "2026-05-08"
training_eligible: false
training_rationale: "Retired surface; do not train models to emit @py.import."
schema_type: "HowTo"
---
# Python Library Integration (Retired)

> **`@py.import` is retired and removed from the compiler as of 2026-05-08.**
> Sources that reference `@py.import` will fail to parse. The deprecation path
> previously documented in this how-to (uv-backed Python setup, `vox container init`)
> has also been removed.

## Why it's gone

Per [`AGENTS.md` §VoxScript-First Glue Code](../../../AGENTS.md):

> Vox is the glue language. Python and shell are retired glue surfaces in this repository.

`@py.import`, `vox container init`, and the `vox-deploy-codegen` Python lane
(`pyproject.rs`, `python_dockerfile.rs`, `setup.rs`, `env.rs`) were the last
toeholds of the Python-glue model. They have all been deleted.

## What to use instead

| Old surface | Canonical replacement |
|---|---|
| `@py.import torch as torch` | Train and ship native models via `vox mens` / Candle (`crates/vox-populi/src/mens/`). |
| `vox container init` (Python pyproject + Dockerfile) | `vox container build` + a hand-authored `Dockerfile`, or `vox deploy` against an `environment` declaration. |
| Python glue scripts (`*.py`) | `.vox` automation scripts under `scripts/`, executed via `vox run scripts/foo.vox`. See AGENTS.md §VoxScript-First Glue Code. |
| Foreign-library FFI in general | TS-source FFI extern fn (`extern fn name(...) to T = "./module"`) for TS interop; Rust crate imports (`import rust:serde_json`) for Rust. |

## Migration

If you have existing `.vox` files that use `@py.import`, the parser will
produce a "Unexpected token at top level" error. There is no automated
migration tool — replace each Python call site with a native Vox path
(MENS for ML, Rust crate import for general libraries) or move the
Python work outside the Vox toolchain entirely.
