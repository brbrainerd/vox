---
title: "Table metadata SSOT (Arca ↔ @table convergence)"
description: "Compares Arca domain DDL, spec append, orchestrator digest, and compiler @table emission; documents current parity/wiring tests and a target single logical table spec driving generators and CI."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Table metadata SSOT (Arca ↔ `@table` convergence)

This document sketches the **shared table-spec pathway** called for in the DB parity program.
It is **not** the full live SSOT yet; shared relational DDL still spans a few Rust locations:

| Source | Role |
|--------|------|
| **Arca** (`crates/vox-db/src/schema/domains/*.rs`) | Canonical SQL DDL per domain fragment; ordered in `manifest.rs` |
| **Arca spec append** (`crates/vox-db/src/schema/spec/mod.rs`) | Cross-cutting DDL (e.g. `populi_training_run`, `codex_capability_map`) concatenated into `baseline_sql()` in `manifest.rs` |
| **Orchestrator digest** (`orchestrator_schema_digest` in the same `spec` module) | `SchemaDigest` for `sync_schema_from_digest` — document **collections** (`_id`/`_data`), not duplicate flat tables for `provider_usage` \| **`vox-orchestrator`** re-exports via `orchestrator_schema()` |
| **Vox `@table`** → HIR → `emit_table_ddl` (`crates/vox-compiler/src/codegen_rust/emit/tables.rs`) | Generated app-local DDL (`_id` autoincrement PK) + typed accessors; parity tests where shapes match |

## Near-term (current)

- Pin **explicit** parity fixtures { see `crates/vox-db/tests/arca_compiler_table_parity.rs` (column signatures + `_id`/`id` mapping where `@table` and Arca both use integer surrogate PK).
- Wire guards: `crates/vox-db/tests/spec_baseline_wiring.rs` asserts spec DDL is embedded in `baseline_sql()` and orchestrator digest invariants.
- Tables with **natural TEXT PK** (e.g. `populi_training_run.run_id`) stay Arca/spec-only until the compiler supports declarative PK shapes in parity tests.
- Normalize comparisons: strip benign `DEFAULT` clauses, compare logical nullability + SQLite affinity, not raw formatting.

## Target architecture

1. **Single logical spec** (YAML/JSON or Rust `const` module) describing:
   - logical table name (Arca snake_case + Vox PascalCase),
   - columns: logical name, storage SQL type, `NOT NULL`, primary key / auto-increment, optional FK.
2. **Generators** (or shared readers):
   - emit Arca domain SQL fragments,
   - emit compiler `HirTable` fixtures or drive `emit_table_ddl` tests,
   - optional: generate `.vox` `@table` stubs for greenfield apps.
3. **CI**: `arca_compiler_table_parity` (and cousins) iterate the spec instead of hand-duplicating DDL strings.

## Related

- `docs/agents/sql-connection-api-allowlist.txt` — consumer crates must not embed ad-hoc SQL; use `VoxDb` ops.
- `docs/src/explanation/expl-architecture.md` — compiler pipeline overview.

