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
- [ ] Add deep dive for `workflow` and `activity` compilation phases — **reinstated 2026-08-22.** This was struck as "removed from the public grammar", which is false: both are live `#[token]`s in `crates/vox-compiler/src/lexer/token.rs`, both appear in `DECLARATION_KEYWORDS` and `LSP_KEYWORD_SNIPPETS`, and both are exercised by compiled golden files (`examples/golden/checkout_workflow.vox`, `durable_workflow_real.vox`). AGENTS.md §Grammar Unification and [ADR-041](../adr/041-durable-functions-completion-2026.md) both record them as stable.
- [x] Document difference between `query` and `mutation` transactional boundaries natively — **closed 2026-08-23**, see [ref-db-surface.md §Transactional boundaries](../reference/ref-db-surface.md#transactional-boundaries-query-vs-mutation-vs-server). Verified against `crates/vox-codegen/src/codegen_rust/emit/http.rs` and `app_contract.rs`: `query` never runs in a transaction; `mutation` does whenever the module has any `table` (a module-level flag, not per-function); `server` shares mutation's emitter but is always called with the transaction flag hard-coded `false`.
- [ ] Expand the `Codex` abstraction API reference 
- [x] List all compiler auto-injected properties for `table` types (`id`, `created_at`, `updated_at`) — **false premise, closed 2026-08-23.** `table` gets zero auto-injected fields. `table_to_ddl` in `crates/vox-db/src/ddl/emit.rs` emits exactly `table.fields` as declared, plus a `UNIQUE(pk)` constraint if `@table(pk: ...)` names one — no `id`/`created_at`/`updated_at` synthesis anywhere in `typeck/registration.rs`, `hir/lower/*.rs`, or the Rust codegen tables emitter. That's why `@table(pk: ...)` and the bare-form's implicit `id` requirement exist as *typecheck errors* (E1041/E1042) rather than defaults: a `table` without a declared primary-key-eligible field is rejected, not backfilled. The auto-injected `_id INTEGER PRIMARY KEY AUTOINCREMENT` / `_data TEXT` / `_created_at` / `_updated_at` columns (note the underscore prefix) belong to a different, unrelated construct — `collection` (document-store), via `collection_to_ddl` in the same file. Confusing the two would have shipped a doc claiming `table` does something it doesn't.

## CLI surface coverage

- [x] **26 of 76 top-level `vox` commands were absent from [`cli.md`](../reference/cli.md)** — closed 2026-08-23 by adding a *Command index* section listing each with the description clap already carries. Counted from `vox commands --format json`, i.e. `build_catalog()` walking the real clap tree. (An earlier note here said "33 of 97"; that came from regex-parsing the `Cli` enum in `lib.rs` and over-counted cfg-gated stub variants and acronym spellings. The 34% rate was right, the counts were not — prefer `build_catalog()` over source parsing.)

  The index is hand-written and will drift. The durable fix is generating it from `build_catalog()` and gating it in `ssot-drift`; the table carries a `ponytail:` marker saying so.

- [x] **20 of 76 top-level commands were missing from [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml)** — closed 2026-08-23 by adding a `- id: <name>` row to `contracts/operations/catalog.v1.yaml` for each of `bundle-app`, `chat`, `component`, `config`, `container`, `dispatch`, `drift-check`, `emit`, `ext`, `grammar`, `harness`, `llm`, `new`, `play`, `plugin`, `policy`, `repair`, `rollback`, `term`, `wasm`, modeled on the existing pure-CLI entries (`mcp: null`, populated `cli:` block with `handler_rust` traced to the actual dispatch match arm). `vox ci operations-verify`, `operations-sync --target cli --write`, and `operations-sync --target capability --write` regenerated `command-registry.yaml` and `capability-registry.yaml`; `command-sync --write` regenerated `cli-command-surface.generated.md`. All 20 now appear in both generated artifacts.

  That registry (projected from `contracts/operations/catalog.v1.yaml`) is what `vox ci command-sync` renders into `cli-command-surface.generated.md`. It was gated and freshly generated, but the generator was pointed at a hand-maintained catalog that shadowed the clap tree for a quarter of the top-level surface — the CLI itself always had these 20 commands, `catalog.v1.yaml` just never had rows for them.

  Nothing currently checks registry coverage against `build_catalog()`: `command_compliance` validates MCP tool wiring and capability rows, not command presence. Adding that check remains open — it would have caught this gap immediately instead of needing a manual audit.

## Retired-syntax prose lint (measured, not yet closed)

- [ ] **`contracts/documentation/retired-symbols.v1.yaml` has no pattern for any at-prefixed data-layer form** — no `@table`, `@query`, `@mutation`, `@server`, `@tool`, `@resource`, `@form`, `@index`, or `@endpoint`. It carries the retired `@component`-plus-`fn` form but not the eight forms that became hard parse errors on 2026-06-30 (`cd7cc96874`). So `vox ci retired-symbol-check` could not fire on the defect it exists to prevent, and `ref-decorators.md` and `migration-0.5-to-0.6.md` told readers to migrate *onto* dead syntax for months with every gate green.

  **Adding the nine patterns is not the fix — measured 2026-08-23.** They work (verified against an injected probe) but produce **195 violations on a clean tree**: 65 `@endpoint`, 26 `@table`, 22 `@query`, 20 each `@tool`/`@mutation`, 19 `@form`, 16 `@server`, 3 each `@resource`/`@index`. Concentrated in `boilerplate-reduction-gap-analysis-2026.md` (27), `vox-gui-native-roadmap-2026.md` (21), `migration-0.5-to-0.6.md` (14), `ref-decorators.md` (10). The last two are *correct* — the retired forms live in their "Retired" columns and migration tables, which is what those pages are for.

  A line-level migration-cue allowlist (`retired|removed|deprecated|instead|→|no longer|superseded|…`) was measured against all 194 decorator hits: it clears **62**, leaving **132**. Insufficient — the remainder are legitimate mentions inside historical and design docs whose individual lines carry no cue.

  **Block-level context was implemented and measured — still not enough (2026-08-23).** `scan_source_lines` in `retired_symbol_check.rs` now tracks markdown heading depth and skips a section opened by a "## Retired"/"### Historical"/"#### Superseded" heading until the next heading at the same or shallower level (bounded, unlike the whole-file `is_historical_or_audit_doc` carve-out). It is a strict widening with zero regression against the shipped 14 patterns — `retired-symbol-check OK` before and after — so it is kept regardless of the nine at-prefixed patterns' status.

  Re-measured with the nine patterns re-added: **195 → 180 violations.** Two new obstacles, beyond what section-scoping can reach:
  - Single-sentence mentions with no enclosing "Retired" section — e.g. `AGENTS.md:244` and `:510`, plain prose ("Removed in v0.6.0: `@endpoint`") inside otherwise-current sections that aren't themselves headed "Retired". The cue-word filter (62/194) was built for exactly this case and remains the more promising direction than heading-scoping; the two should probably compose.
  - `docs/agents/vox-language-surface.v1.json` is a machine-generated **data file**, not prose — it deliberately documents the retired `@`-forms as part of the language-surface SSOT. `cfg.is_md` gates all of the frontmatter/fence/heading logic, so a `.json` file gets none of it; JSON needs its own carve-out (e.g. skip a `"note"` field whose text contains a migration cue) or exclusion from the scan entirely, since the check's doc comment already says Rust sources are "intentionally out of scope" for a symmetric reason.

  Still not shipping the nine patterns. A gate that fails on legitimate SSOT data and legitimate single-sentence retirement notices is not ready.

  Related measurement: of 629 stale-syntax occurrences repo-wide, only **6.2%** sit inside ```vox fences (all correctly `vox:skip`-marked as of 2026-08-23), **60.9%** are prose and **25.1%** table cells. The doctest gate can never reach the latter two.

## Medium Priority
- [ ] Explain the underlying generic instantiation (`<T>`) algorithm used by HIR logic
- [ ] Detail all `mcp.tool` options regarding rate limits and user confirmation schemas
- [x] Add explicit HTTP request payload mapping examples for `server` endpoints — **closed 2026-08-23**, see [wire-format-v1-ssot.md §2.2](../architecture/wire-format-v1-ssot.md). `mutation` and `server` share the same param-extraction emitter (`request["<name>"].clone()`, flat top-level JSON lookup); the only wire differences are the path root and that `server` never gets transaction wrapping.

## Completed 
- [x] Standard library built-ins (completed 2026-04-06)
- [x] Correct `component` declaration syntax (completed 2026-04-06; `@island` retired 2026-05-03)
- [x] Example pipeline validation documentation (completed 2026-04-06)


