---
title: "Known Documentation Gaps & Backlog"
description: "Living checklist of documentation gaps, backlog items, and recently completed doc work for contributors."
category: "API Reference — Crates"
status: current

schema_type: "TechArticle"
---

# Known Documentation Gaps & Backlog

This is a living checklist for the Vox open source community and core contributors to track undocumented or under-documented language features.

## High Priority
- [x] Add deep dive for `workflow` and `activity` compilation phases — **closed 2026-08-23**, see [expl-actors-workflows.md §Compilation Phases](../explanation/expl-actors-workflows.md). Traced parse -> HIR lowering -> determinism lint -> codegen with source citations at each step. The load-bearing finding: `workflow` bodies are not compiled to Rust at all -- codegen emits a call into `interpret_workflow_durable` that re-interprets the embedded HIR at runtime and journals each step, which is *why* the determinism lint exists. `activity` bodies compile to real Rust, journaled as an opaque step. Also corrected the page's own Durability Taxonomy: it described a second, separately-compiled "strictly Rust async" workflow path lacking step-level durability as a live "future parity" gap (citing ADR-021) -- no such second path exists in the current emitter; there is exactly one workflow codegen path today, and it's the durable one.
- [x] Document difference between `query` and `mutation` transactional boundaries natively — **closed 2026-08-23**, see [ref-db-surface.md §Transactional boundaries](../reference/ref-db-surface.md#transactional-boundaries-query-vs-mutation-vs-server). Verified against `crates/vox-codegen/src/codegen_rust/emit/http.rs` and `app_contract.rs`: `query` never runs in a transaction; `mutation` does whenever the module has any `table` (a module-level flag, not per-function); `server` shares mutation's emitter but is always called with the transaction flag hard-coded `false`.
- [x] Expand the `Codex` abstraction API reference — **closed 2026-08-23**, see [codex-http-api.md §Rust API surface](../reference/codex-http-api.md). The existing page covered only the HTTP/OpenAPI layer. `Codex` itself is `pub type Codex = VoxDb` (a type alias, not a distinct struct); documented `connect`/`transaction`/`connection()` (a mutex-guarded `turso::Connection` wrapper, with a real documented race-condition caveat on `last_insert_rowid()`), the generated per-table CRUD methods, and all 12 `facade/*.rs` extension modules with what each adds.
- [x] List all compiler auto-injected properties for `table` types (`id`, `created_at`, `updated_at`) — **closed 2026-08-23; corrected 2026-08-23 after shipping a wrong answer for ~10 hours (session commit `9019f6612`).**

  **What was wrong.** The original entry claimed "`table` gets zero auto-injected fields" and cited `crates/vox-db/src/ddl/emit.rs`. That path is right but the claim describing it was wrong -- I had actually read a *different* file with the *same function name*, `crates/vox-codegen/src/codegen_rust/emit/tables/codegen.rs`, and never reopened `vox-db`'s own `table_to_ddl`. Both files export a function called `table_to_ddl`; only one was checked.

  **What's actually true, re-verified 2026-08-23 by reading `vox-db/src/ddl/emit.rs::table_to_ddl` directly (not the codegen one) and confirming empirically:**
  - `table` **does** get one auto-injected column: `_id INTEGER PRIMARY KEY AUTOINCREMENT`, unconditionally prepended before the declared fields (`vox-db/src/ddl/emit.rs:36`) -- this is the real schema-migration DDL, consumed by `auto_migrate.rs`'s `db.auto_migrator()`, the production path that reconciles a live SQLite schema against `@table` declarations.
  - The generated Rust struct mirrors this: `crates/vox-codegen/src/codegen_rust/emit/tables/codegen.rs::emit_table_struct` unconditionally emits `pub _id: Option<i64>,` before the user's declared fields (line ~283) -- three lines above the `insert`/`get`/`delete` methods I *did* read carefully while implementing `update()` the same day, and somehow still missed the struct field two lines up.
  - `created_at`/`updated_at` are **not** auto-injected -- that part of the original answer holds. Confirmed: `table_to_ddl` has no such columns; only `collection_to_ddl` (a different, unrelated construct) emits `_created_at`/`_updated_at`, alongside `_data`.
  - A user declaring their own `id: int` field collides with this and gets a real compiler warning: `crates/vox-compiler/src/typeck/ast_decl_lints.rs:447` ("Table 'X' declares a column `id`; Vox `@table` already adds a surrogate `_id` primary key.") -- which is itself evidence the original "zero auto-injected fields" claim was wrong; the warning only makes sense if `_id` is real.
  - The E1041/E1042 framing was also wrong: those validate the *named* `pk:` argument to `@table(pk: ...)` when one is given; they say nothing about whether a table lacking one gets a default. It does -- `_id` is that default, not a rejection.

  Lesson for future verification passes in this repo: **grep for the exact defining `fn`, not just a plausible-looking file path** -- two crates independently defined a `table_to_ddl` with different bodies, and citing the right *name* while reading the wrong *file* produced a confident, wrong, and unnoticed-for-hours answer.

## CLI surface coverage

- [x] **26 of 76 top-level `vox` commands were absent from [`cli.md`](../reference/cli.md)** — closed 2026-08-23 by adding a *Command index* section listing each with the description clap already carries. Counted from `vox commands --format json`, i.e. `build_catalog()` walking the real clap tree. (An earlier note here said "33 of 97"; that came from regex-parsing the `Cli` enum in `lib.rs` and over-counted cfg-gated stub variants and acronym spellings. The 34% rate was right, the counts were not — prefer `build_catalog()` over source parsing.)

  The index is hand-written and will drift. The durable fix is generating it from `build_catalog()` and gating it in `ssot-drift`; the table carries a `ponytail:` marker saying so.

- [x] **20 of 76 top-level commands were missing from [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml)** — closed 2026-08-23 by adding a `- id: <name>` row to `contracts/operations/catalog.v1.yaml` for each of `bundle-app`, `chat`, `component`, `config`, `container`, `dispatch`, `drift-check`, `emit`, `ext`, `grammar`, `harness`, `llm`, `new`, `play`, `plugin`, `policy`, `repair`, `rollback`, `term`, `wasm`, modeled on the existing pure-CLI entries (`mcp: null`, populated `cli:` block with `handler_rust` traced to the actual dispatch match arm). `vox ci operations-verify`, `operations-sync --target cli --write`, and `operations-sync --target capability --write` regenerated `command-registry.yaml` and `capability-registry.yaml`; `command-sync --write` regenerated `cli-command-surface.generated.md`. All 20 now appear in both generated artifacts.

  That registry (projected from `contracts/operations/catalog.v1.yaml`) is what `vox ci command-sync` renders into `cli-command-surface.generated.md`. It was gated and freshly generated, but the generator was pointed at a hand-maintained catalog that shadowed the clap tree for a quarter of the top-level surface — the CLI itself always had these 20 commands, `catalog.v1.yaml` just never had rows for them.

  Nothing currently checks registry coverage against `build_catalog()`: `command_compliance` validates MCP tool wiring and capability rows, not command presence. Adding that check remains open — it would have caught this gap immediately instead of needing a manual audit.

## Retired-syntax prose lint (measured, not yet closed)

- [ ] **`contracts/documentation/retired-symbols.v1.yaml` has no pattern for any at-prefixed data-layer form** (`@table`/`@query`/`@mutation`/`@server`/`@tool`/`@resource`/`@form`/`@index`/`@endpoint`) — blocked at **180 residual false positives** after heading-scoping (down from 195), needing a cue-word filter plus a carve-out for `docs/agents/vox-language-surface.v1.json` (a data file, not prose) before it can ship. Full measurement log, false starts, and next steps: [archived investigation](../archive/research-2026-q1/retired-syntax-prose-lint-investigation-2026-08-23.md).

## Medium Priority
- [x] Explain the underlying generic instantiation (`<T>`) algorithm used by HIR logic — **closed 2026-08-23**, see [ref-type-system.md §7](../reference/ref-type-system.md). Traced `instantiate`/`fresh_var`/`unify`/`resolve` in `crates/vox-compiler/src/typeck/unify.rs`: a generic (`Ty::GenericParam`) is replaced with a fresh `Ty::TypeVar` at every use site (so separate call sites never cross-contaminate their `T`), then pinned down by unification with an occurs check against infinite types.
- [x] Detail all `mcp.tool` options regarding rate limits and user confirmation schemas — **closed 2026-08-23**, see [ref-decorators.md §`tool`](../reference/ref-decorators.md). Mostly a false premise: the `tool` keyword itself takes no options beyond its description string. Writing `@rate_limit`/`@cors`/`@pii`/`@webhook`/`@layer` on a `tool` or `resource` was found silently inert (parsed, never consumed by codegen) and is now a compile error instead — see `bcb8eb433`, which generalized the existing bare-`fn` version of this same check. Confirmation is real but lives entirely outside the language, as runtime policy in `contracts/orchestration/permission-modes.v1.yaml` + `dispatch.rs`'s dangerous-tool gate: risk classification, three `PermissionMode`s, a per-repo persisted allowlist, and a documented (partially unwired) 5-tier precedence order.
- [x] Add explicit HTTP request payload mapping examples for `server` endpoints — **closed 2026-08-23**, see [wire-format-v1-ssot.md §2.2](../architecture/wire-format-v1-ssot.md). `mutation` and `server` share the same param-extraction emitter (`request["<name>"].clone()`, flat top-level JSON lookup); the only wire differences are the path root and that `server` never gets transaction wrapping.

## Completed 
- [x] Standard library built-ins (completed 2026-04-06)
- [x] Correct `component` declaration syntax (completed 2026-04-06; `@island` retired 2026-05-03)
- [x] Example pipeline validation documentation (completed 2026-04-06)


