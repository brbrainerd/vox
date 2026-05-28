---
title: "Rust pattern modernization — Wave 0 baseline"
description: "Official documentation for Rust pattern modernization — Wave 0 baseline for the Vox language. Detailed technical reference, architecture "
category: "CI & Quality"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Rust pattern modernization — Wave 0 baseline

Rolling snapshot for [`.cursor/plans/rust-pattern-modernization-master_d4c4c376.plan.md`](../../../AGENTS.md). Re-record counts when starting a new wave.

## Workspace lint manifest (authoritative)

From root [`Cargo.toml`](../../../Cargo.toml) `[workspace.lints]`:

| Lint group | Level |
|------------|-------|
| `rust::unsafe_code` | `warn` |
| `clippy::all` | `warn` |

Stricter policy described in governance docs is **not** yet fully mirrored here (see plan § Wave 6).

## Edition / toolchain

- Workspace `edition = "2024"`, `rust-version` in root `Cargo.toml` (align with CI `dtolnay/rust-toolchain@stable`).

## High-risk pilot files (Wave 1+)

Priority set from the master plan (error handling / async / tracing / process):

- `crates/vox-orchestrator/src/mcp_tools/tools/codex_tools.rs`
- `crates/vox-cli/src/dispatch_protocol.rs`
- `crates/vox-actor-runtime/src/llm_result.rs`
- `crates/vox-orchestrator/src/models.rs`
- `crates/vox-codegen-rust/src/emit.rs`

## TOESTUB

- Crate: `vox-code-audit`; CLI entry: `vox` diagnostics / stub-check (see plan § Wave 5–6).
- CI: default job uses `ci toestub-scoped --mode legacy` (see `.github/workflows/ci.yml`). **Tightening:** switch to stricter modes only after backlog burn-down and cross-provider parity review.

## Verification commands

```bash
cargo check --workspace
cargo clippy --workspace -- -W clippy::all
cargo doc --workspace --no-deps
cargo test -p vox-code-audit
```

Use [crate hardening matrix](crate-hardening-matrix.md) for per-crate feature flags.

## Related

- [Forward migration charter](../archive/research-2026-q1/forward-migration-charter.md)
- [Workflow enumeration](workflow-enumeration.md)
- [Crate hardening matrix](crate-hardening-matrix.md)

