---
title: "Workspace dependency audit (2026-05)"
description: "Evidence-driven dependency normalization: workspace pins, duplicate majors, and follow-up stacks."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true

schema_type: "TechArticle"
---

# Workspace dependency audit (2026-05)

Rolling closure from the **workspace dependency audit v2** plan:

## Completed in-tree

- **`regress`**: kept as a **real dependency** of `vox-research-events`: typify-generated `schema_types.generated.rs` references `::regress::Regex` for pattern-backed string types (grep there before treating it as removable).
- **Workspace inheritance**: external crates in high-churn manifests use `{ workspace = true }` with versions centralized in root `Cargo.toml`.
- **`schemars` 0.8 vs 1**: `vox-scientia-jsonschema-codegen` depends on workspace alias **`schemars08`** (`package = "schemars", version = "0.8"`) while the rest of the workspace stays on `schemars` 1.x / `typify` 0.6.x until a typify upgrade is scoped.
- **LSP stack**: consolidated on **`tower-lsp-server`** / `ls-types` (`Uri` instead of legacy `Url`).
- **Heavy retrieval**: `vox-search` defaults omit lexical/scrape; consumers opt in via features (`tantivy-lexical`, `web-scrape`).
- **Pins**: workspace carries current lines for `thiserror` 2, `wasmtime` / `wasmtime-wasi` 44, `self_update` 0.44, `tokio-tungstenite` 0.29, etc., per lockfile verification (`cargo check --workspace`).

## Intentional duplicates / deferred

- **Candle / QLoRA / zip**: multiple `zip` majors remain via Candle vs other consumers; upgrade needs GPU CI and **ADR 034**.
- **Rand 0.8 vs 0.9**: workspace uses **`rand09`** alias where Oratio STT needs 0.9 APIs; broader unification waits on Wasmtime/Tantivy upstreams.
- **`syn` 1.x**: remains via `abi_stable_derive` (plugin ABI risk).
- **tiktoken-rs**: **not** replaced by heuristics — `cl100k_base` is used for accurate budgeting (`vox-orchestrator`).

## Local evidence artifacts

Optional snapshots (gitignored): `.tmp_audit/duplicates.after.txt`, `.tmp_audit/shear.after.json` — regenerate with:

```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" tree --workspace --duplicates --depth 0 > .tmp_audit/duplicates.after.txt
& "$env:USERPROFILE\.cargo\bin\cargo.exe" shear --format json > .tmp_audit/shear.after.json
```

## Policy

See [Dependency policy](../contributors/dependency-policy.md).

## ADRs

- [ADR 034 — Candle / QLoRA stack upgrades](../adr/034-candle-qlora-stack-upgrades.md)
- [ADR 035 — SWC vs alternatives for JS tooling](../adr/035-swc-parser-alternatives-eval.md)
