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
- [ ] Document difference between `query` and `mutation` transactional boundaries natively
- [ ] Expand the `Codex` abstraction API reference 
- [ ] List all compiler auto-injected properties for `table` types (`id`, `created_at`, `updated_at`)

## CLI surface coverage

- [x] **26 of 76 top-level `vox` commands were absent from [`cli.md`](../reference/cli.md)** — closed 2026-08-23 by adding a *Command index* section listing each with the description clap already carries. Counted from `vox commands --format json`, i.e. `build_catalog()` walking the real clap tree. (An earlier note here said "33 of 97"; that came from regex-parsing the `Cli` enum in `lib.rs` and over-counted cfg-gated stub variants and acronym spellings. The 34% rate was right, the counts were not — prefer `build_catalog()` over source parsing.)

  The index is hand-written and will drift. The durable fix is generating it from `build_catalog()` and gating it in `ssot-drift`; the table carries a `ponytail:` marker saying so.

- [ ] **20 of 76 top-level commands are missing from [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml)** — `bundle-app`, `chat`, `component`, `config`, `container`, `dispatch`, `drift-check`, `emit`, `ext`, `grammar`, `harness`, `llm`, `new`, `play`, `plugin`, `policy`, `repair`, `rollback`, `term`, `wasm`.

  That registry (projected from `contracts/operations/catalog.v1.yaml`) is what `vox ci command-sync` renders into `cli-command-surface.generated.md`. So that file is gated, freshly generated, and missing a quarter of the top-level surface — a generator pointed at a hand-maintained list that shadows the clap tree.

  Nothing currently checks registry coverage against `build_catalog()`: `command_compliance` validates MCP tool wiring and capability rows, not command presence. Adding that check is the fix, but it fails on the 20 rows above until they are written, and each needs curated metadata (capability id, category) that clap cannot supply — so the rows come first, then the gate.

## Retired-syntax prose lint (measured, not yet closed)

- [ ] **`contracts/documentation/retired-symbols.v1.yaml` has no pattern for any at-prefixed data-layer form** — no `@table`, `@query`, `@mutation`, `@server`, `@tool`, `@resource`, `@form`, `@index`, or `@endpoint`. It carries the retired `@component`-plus-`fn` form but not the eight forms that became hard parse errors on 2026-06-30 (`cd7cc96874`). So `vox ci retired-symbol-check` could not fire on the defect it exists to prevent, and `ref-decorators.md` and `migration-0.5-to-0.6.md` told readers to migrate *onto* dead syntax for months with every gate green.

  **Adding the nine patterns is not the fix — measured 2026-08-23.** They work (verified against an injected probe) but produce **195 violations on a clean tree**: 65 `@endpoint`, 26 `@table`, 22 `@query`, 20 each `@tool`/`@mutation`, 19 `@form`, 16 `@server`, 3 each `@resource`/`@index`. Concentrated in `boilerplate-reduction-gap-analysis-2026.md` (27), `vox-gui-native-roadmap-2026.md` (21), `migration-0.5-to-0.6.md` (14), `ref-decorators.md` (10). The last two are *correct* — the retired forms live in their "Retired" columns and migration tables, which is what those pages are for.

  A line-level migration-cue allowlist (`retired|removed|deprecated|instead|→|no longer|superseded|…`) was measured against all 194 decorator hits: it clears **62**, leaving **132**. Insufficient — the remainder are legitimate mentions inside historical and design docs whose individual lines carry no cue.

  What this actually needs is **block-level context**: skip a fenced block or a section under a "Retired"/"Historical" heading, the way `is_historical_or_audit_doc` already approximates at whole-file granularity. Until that exists, the patterns stay out; a gate the tree fails on 195 legitimate lines is not a guard.

  Related measurement: of 629 stale-syntax occurrences repo-wide, only **6.2%** sit inside ```vox fences (all correctly `vox:skip`-marked), **60.9%** are prose and **25.1%** table cells. The doctest gate can never reach the latter two.

## Medium Priority
- [ ] Explain the underlying generic instantiation (`<T>`) algorithm used by HIR logic
- [ ] Detail all `mcp.tool` options regarding rate limits and user confirmation schemas
- [ ] Add explicit HTTP request payload mapping examples for `server` endpoints

## Completed 
- [x] Standard library built-ins (completed 2026-04-06)
- [x] Correct `component` declaration syntax (completed 2026-04-06; `@island` retired 2026-05-03)
- [x] Example pipeline validation documentation (completed 2026-04-06)


