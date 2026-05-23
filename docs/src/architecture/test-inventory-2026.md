---
title: "Workspace test inventory (2026)"
description: "Regenerable counts of Rust tests, ignores, golden Vox, and app E2E paths across the workspace (fully regenerated; refresh dates via git history)."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Workspace test inventory

Regenerate this page (full Rust-side metrics, ignores, harness patterns, and sorted JSON) with:

`cargo run -p vox-cli -- ci test-inventory --markdown docs/src/architecture/test-inventory-2026.md`

Machine-readable JSON:

`cargo run -p vox-cli -- ci test-inventory --json`

Regenerate the committed snapshot used by CI drift checks:

`cargo run -p vox-cli -- ci test-inventory --output contracts/reports/test-inventory.v1.json`

Verify a committed JSON snapshot (parses both sides and compares structured report data, not raw text):

`cargo run -p vox-cli -- ci test-inventory --check contracts/reports/test-inventory.v1.json`

## Runtime report from JUnit (slow tests / retries)

After CI produces nextest JUnit (see [`runner-contract`](../ci/runner-contract.md)), summarize timings and retry heuristics:

`cargo run -p vox-cli -- ci test-runtime-report --junit target/nextest/ci/junit.xml --json`

(`--markdown <path>` writes a short advisory Markdown; optional `--fail-over-ms` / `--fail-retry-count` warn only.)

## Summary counts

The authoritative numbers are emitted by `vox ci test-inventory`. Until you run it on a clean build, treat the rows below as **illustrative probes** from the repo layout (they align with what the scanner walks but do not replace JSON).

| Metric | Probe / note |
| --- | ---: |
| Workspace crates (`crates/*/Cargo.toml`) | 107 |
| Rust files under `crates/**/*.rs` (recursive; includes fixtures) | See JSON from generator |
| Golden `.vox` files (`examples/golden/**/*.vox`) | 54 |
| Lines containing `@test` in golden Vox (substring probe; generator counts line-leading `@test`) | 15 |
| App E2E-style files (`apps/**/*.test.*` / `*.spec.*`) | 4 |

After `cargo run … --markdown …`, this section should be overwritten with the generator table (unit vs integration vs bench vs ignored, doctest candidates, harness pattern totals).

## Caveats

- **WebIR / internal pipelines:** Ignored tests that mention WebIR (path or ignore reason) are treated as **active internal pipeline tests** unless the ignore reason clearly indicates tombstone, retired, or dropped parity language.
- **Nextest vs doctests:** `cargo nextest` runs compiled test binaries for crates but does **not** replace `cargo test` doctests. This inventory lists doctest **candidates** separately (rust/no_run doc fences in `src` trees).

## Zero-test crates

(Run the generator for the live list.)

## Top ignored files

(Run the generator for the ranked table.)

## Rust files by kind

(Run the generator for `unit_src` / `integration_tests` / `benches` / `other`.)
