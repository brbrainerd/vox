---
title: "Reference: FFI and interop"
description: "Rust crate imports, extern declarations, and frontend bridge boundaries."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Single entrypoint for native and JS interop docs."
schema_type: "TechArticle"
---

# Reference: FFI and interop

## Rust crates (`import rust:`)

- Vox sources may import Rust crates using `import rust:<crate>` (see [syntax](./ref-syntax.md)).
- Lockfile and dependency roles are described in [CLI reference](./cli.md) and [package manager](./ref-package-manager.md).

## `extern fn`

- `extern` is a lexer keyword for foreign-call surfaces; detailed semantics follow codegen and capability rules — track [`gui-native-roadmap-status-2026.md`](../architecture/gui-native-roadmap-status-2026.md) and frontend interop plans.

## JavaScript / TypeScript emit

- Component and route surfaces emit to the React baseline per [`external-frontend-interop-plan-2026.md`](../architecture/external-frontend-interop-plan-2026.md).

## See also

- [`where-things-live.md`](../architecture/where-things-live.md) — crate map (`vox-compiler`, `vox-codegen`, …)
