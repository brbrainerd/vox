---
title: "HIR legacy AST wrappers (inventory)"
description: "AST-retained HIR nodes and migration targets (Path C)"
category: "Language Reference"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# HIR legacy inventory

[`HirModule`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) holds first-class vectors for codegen (`functions`, `tables`, …) plus:

- **`legacy_ast_nodes`** — declarations with no dedicated `Hir*` bucket yet (see lowering default arm in [`lower/mod.rs`](../../../crates/vox-compiler/src/hir/lower/mod.rs)).
- **AST-retained wrappers** — `HirComponent`, `HirPage`, `HirIsland`, … wrapping raw AST decls until TS/Rust codegen is fully HIR-native.

## Recently lowered (database)

| AST variant | HIR target |
|-------------|------------|
| `Decl::Collection` | [`HirCollection`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) |
| `Decl::VectorIndex` | [`HirVectorIndex`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) |
| `Decl::SearchIndex` | [`HirSearchIndex`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) |

## Wrapper types (migrate to typed HIR bodies)

| Type | Notes |
|------|--------|
| `HirComponent` | Component AST retained |
| `HirV0Component` | v0 stub |
| `HirRoutes` / `HirIsland` / `HirLayout` / `HirPage` | Router / TanStack migration |
| `HirContext` / `HirHook` / `HirErrorBoundary` / `HirLoading` / `HirNotFound` | UI shells |

## Baseline gate

Unit test **`hir_lowering_maps_collection_vector_search_out_of_legacy`** ensures collection / vector / search indices do not land in `legacy_ast_nodes`. Extend with new constructs as they graduate from the default lowering arm.

## Related

- [Compiler lowering explanation](../explanation/expl-compiler-lowering.md)
- [Parser ambiguity inventory](parser-ambiguity-inventory.md)


