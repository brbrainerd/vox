---
title: "Reference: standard library index"
description: "Index of std.* surfaces with stability tiers and authority split between builtins and shell-tier stdlib."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "One navigation page for std.fs/process vs compiler builtins."
schema_type: "TechArticle"
---

# Reference: standard library index

| Tier | Scope | Authority |
| --- | --- | --- |
| **A — Compiler builtins** | Core language helpers surfaced as builtin calls / standard types | [`ref-builtins-stdlib.md`](./ref-builtins-stdlib.md), compiler builtin registry |
| **B — Shell-tier stdlib** | `std.fs`, `std.process`, structured formats (`std.csv`, `std.toml`, `std.yaml`, `std.io`) | [`vox-shell-stdlib-ssot-2026.md`](../architecture/vox-shell-stdlib-ssot-2026.md) |
| **C — HTTP / net (decorators)** | `@endpoint`, HTTP client ergonomics | Phase HTTP specs linked from [`research-index.md`](../architecture/research-index.md) |

## Stability

- Until a symbol is marked **stable** in release notes, treat new `std.*` additions as **experimental** and subject to SSOT collapse / codegen changes ([Phase 1 language rules](../architecture/vox-language-rules-phase1-ssot-collapse-2026.md)).

## See also

- [CLI](./cli.md) — `vox check`, `vox fmt`, script execution tiers ([`AGENTS.md`](../../../AGENTS.md))
