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

- [ ] **33 of 97 top-level `vox` commands (34%) are absent from [`cli.md`](../reference/cli.md)** — measured 2026-08-22 against the clap `Cli` enum in `crates/vox-cli/src/lib.rs`. Nothing is documented-but-nonexistent; the page is stale by omission only. Undocumented:

  `attention` `audit` `axis` `bundle-app` `catalog` `chat` `component` `config`
  `container` `create` `dispatch` `drift-check` `ext` `grammar` `gui` `harness`
  `kubernetes` `llm` `model` `new` `plan` `play` `plugin` `policy` `repair`
  `repl` `rollback` `safety` `snapshot` `ssh` `stop` `term` `wasm`

  Includes first-class user lanes (`gui`, `chat`, `plugin`, `new`, `model`, `plan`, `config`).

  **Do not close this by hand-writing 33 sections** — that recreates the drift. `crates/vox-cli/src/command_catalog.rs` already reflects the full clap tree via `build_catalog()`, and `contracts/operations/catalog.v1.yaml` (which feeds the generated `cli-command-surface.generated.md`) is itself missing 31 of the same commands. Retarget `command_sync.rs` at `build_catalog()` so the generated inventory comes from clap rather than from a YAML that shadows it; the narrative and tombstones in `cli.md` stay hand-written.

## Medium Priority
- [ ] Explain the underlying generic instantiation (`<T>`) algorithm used by HIR logic
- [ ] Detail all `mcp.tool` options regarding rate limits and user confirmation schemas
- [ ] Add explicit HTTP request payload mapping examples for `server` endpoints

## Completed 
- [x] Standard library built-ins (completed 2026-04-06)
- [x] Correct `component` declaration syntax (completed 2026-04-06; `@island` retired 2026-05-03)
- [x] Example pipeline validation documentation (completed 2026-04-06)


