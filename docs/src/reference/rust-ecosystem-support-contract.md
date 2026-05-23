---
title: "Rust ecosystem support contract"
description: "Machine-readable contract paths and semantics for Vox Rust crate-family support metadata."
category: "Language Reference"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---

Machine-readable Rust crate-family support metadata for Vox lives in:

- [`contracts/rust/ecosystem-support.yaml`](../../../contracts/rust/ecosystem-support.yaml)
- schema: [`contracts/rust/ecosystem-support.schema.json`](../../../contracts/rust/ecosystem-support.schema.json)

This registry tracks `product_lane`, support tier, boundary owner,
semantics state, capability value, debt cost, target support, and
decision class (`first_class`, `internal_runtime_only`,
`escape_hatch_only`, `deferred`).

It also includes `template_managed_dependencies`
(`app`, `script_native`, `script_wasi`) used by the compiler build-time
generator to derive template-owned dependency sets from contract data.
It additionally defines `wasi_unsupported_rust_imports`, the explicit
WASI deny set consumed by compiler policy generation.

Runtime defaults and policy behavior:

- If a crate is absent from `support_entries`, classifier fallback is
  `escape_hatch_only`.
- Semantics fallback for crates absent from `support_entries` is
  `partially_implemented`.
- Crates listed in `template_managed_dependencies` should also appear by
  Cargo name in at least one `support_entries.crate_family` so generated
  classifier and template ownership cannot drift.

Executable SSOT wiring:

- `crates/vox-compiler/build.rs` reads
  `contracts/rust/ecosystem-support.yaml` and generates
  `rust_interop_policy.rs` into `OUT_DIR`.
- `crates/vox-compiler/src/rust_interop_support.rs` includes that
  generated table (`GENERATED_RUST_INTEROP_POLICY`) for classifier and
  target/semantics lookup.

Architecture rationale and scoring policy:

- [`docs/src/architecture/rust-ecosystem-support-ssot.md`](../archive/research-2026-q1/rust-ecosystem-support-ssot.md)
- [`docs/src/architecture/interop-tier-policy.md`](../archive/research-2026-q1/interop-tier-policy.md)
- [`docs/src/architecture/vox-bell-curve-strategy.md`](../archive/research-2026-q1/vox-bell-curve-strategy.md)

Local verification:

- `vox ci policy-smoke` (orchestrator check + command-compliance + rust ecosystem parity test)
- `vox ci rust-ecosystem-policy`
- `cargo run -p vox-cli --quiet -- ci rust-ecosystem-policy`
- `cargo test -p vox-compiler --test rust_ecosystem_support_parity`


