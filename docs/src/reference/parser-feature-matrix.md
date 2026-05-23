---
title: "Parser feature matrix"
description: "Current parser coverage matrix for Vox declarations and expressions."
category: "Language Reference"
last_updated: "2026-05-09"
training_eligible: true

schema_type: "TechArticle"
---

# Parser feature matrix

## Source of truth
- Parser module scope notes: `crates/vox-compiler/src/parser/mod.rs`
- Parser descent implementation: `crates/vox-compiler/src/parser/descent/`

## Covered in canonical parser
- `fn`, `pub fn`
- `type`, `pub type`
- `import`
- `component` (UI component declaration; emits React/TSX)
- `@endpoint(kind: server|query|mutation)`
- `@loading`
- `@table`, `@index`
- `@mcp.tool`
- `@test`
- `@v0`
- `actor`, `workflow`, `activity`
- HTTP route declarations (`http get/post/put/delete`)
- `routes { }` (router shim generation)
- JSX tags and expressions
- Expression operators including pipeline (`|>`)

## Retired (hard parser error)
- `@island` — retired 2026-05-03; use `component` instead
- `@server`, `@query`, `@mutation` — retired; use `@endpoint(kind: server|query|mutation)` instead

## Explicitly out of parser scope (current)
- `@page`
- `@partial`
- `@theme`
- `@layout`
- `@i18n`
- `@schema`
- `@action`

## Implications
- Out-of-scope declarations increase lowering/codegen coupling and can create parser/docs drift.
- Roadmap target is to pull these into canonical parser/typed-HIR coverage to reduce cross-stage boilerplate.

## Near-term verification
- Keep parser tests aligned with this matrix.
- Fail CI when docs and parser scope diverge for declared feature support.


