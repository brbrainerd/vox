---
title: "Forward-only migration charter"
description: "Official documentation for Forward-only migration charter for the Vox language. Detailed technical reference, architecture guides, and im"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Forward-only migration charter

## Policy

1. **No restore-based workflows** — Do not rely on Git history replay, `git restore`, or archaeology to recover correct behavior. The current tree and documented contracts are authoritative.
2. **Docs before breaking code** — Update ADRs, architecture pages, and `ref-cli.md` before or alongside behavior changes that affect users or agents.
3. **Explicit retire / port / keep** — Every orphan or duplicate surface is classified in [orphan surface inventory](orphan-surface-inventory.md) with owner, severity, and target milestone.
4. **Single implementation** — One canonical module per domain operation (e.g. database CLI helpers live in `crates/vox-cli/src/commands/db.rs`; `commands/ops/db` re-exports that module).
5. **Arca/Codex DDL** — One manifest in `vox-db` (`crates/vox-db/src/schema/manifest.rs`, `SCHEMA_FRAGMENTS` → `baseline_sql`). The live `schema_version` row matches `BASELINE_VERSION` in that manifest (see `contracts/db/baseline-version-policy.yaml`). Legacy multi-row chains use export/import, not ad-hoc undocumented version integers in docs.
6. **Workspace excludes** — Crates listed under `[workspace].exclude` (e.g. `vox-orchestrator`, `vox-py`, `vox-wasm`) are intentionally outside the default workspace until they are CI-stable. **`vox-codegen-html` is retired** (no in-tree crate); use **`vox-ssg`** per [ADR 010](../adr/010-tanstack-web-spine.md). Workspace members must not add `path = "../…"` dependencies to excluded crates without first removing them from `exclude` and fixing the build graph.

## Enforcement

- **`vox ci check-docs-ssot`** (CI/bootstrap: `cargo run -p vox-cli --quiet -- ci check-docs-ssot`; thin shell: `scripts/check_docs_ssot.sh`) validates inventory structure, referenced paths, workspace crate coverage, and **stale doc/workflow references** to retired Python or shell gates.
- **`vox ci check-codex-ssot`** (same bootstrap pattern; thin shell: `scripts/check_codex_ssot.sh`) ensures core Codex SSOT files exist, `contracts/index.yaml` + baseline policy align with `vox-db` manifest snippets, and OpenAPI path guards hold.

## Related

- [CLI scope policy](cli-scope-policy.md)
- [Compatibility and deprecation windows](compatibility-deprecation-windows.md)
- [Rust modernization baseline (Wave 0)](../ci/rust-modernization-baseline.md)
- [Crate hardening matrix](../ci/crate-hardening-matrix.md)


