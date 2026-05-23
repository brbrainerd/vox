---
title: "Direct `turso::` usage allowlist"
description: "Official documentation for Direct `turso::` usage allowlist for the Vox language. Detailed technical reference, architecture guides, and "
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Direct `turso::` usage allowlist

ADR 004 discourages direct `turso::` usage outside the data-plane crates. In practice, the workspace still contains direct calls in CLI helpers, tests, and integration code. For the full API/env contract, see [Codex / Arca compatibility boundaries](codex-arca-compatibility-boundaries.md).

## Allowed (by design)

| Area | Rationale |
|------|-----------|
| **`vox-pm`** | Owns `CodeStore` and SQL connection lifecycle. |
| **`vox-db`** | Facade over `CodeStore`; may use Turso types in public helpers. |
| **`vox-cli`** | Sample/diagnostic SQL and params (`turso::params!`, `Value`) against the user DB. |
| **Tests / `vox-integration-tests`** | Fixture and contract tests. |

## Goal

Reduce new direct `turso::` surface: application features should call **`VoxDb`** / **`CodeStore`** APIs. When adding a new direct call, document the exception in this file or add a narrow helper on `vox-db` / `vox-pm`.

## Verification

Periodically run `rg "turso::" crates/` and reconcile with this policy.

**Related:** `vox ci sql-surface-guard` enforces `.connection().query|execute(` outside an allowlist. **`vox ci query-all-guard`** (and `ssot-drift`) enforce the `query_all` call-site pattern outside [`docs/agents/query-all-allowlist.txt`](../../agents/query-all-allowlist.txt) plus `crates/vox-db/`. **`vox ci turso-import-guard`** enforces the Turso crate path prefix outside [`docs/agents/turso-import-allowlist.txt`](../../agents/turso-import-allowlist.txt) plus built-in `vox-db` / `vox-pm` / `vox-compiler` prefixes.


