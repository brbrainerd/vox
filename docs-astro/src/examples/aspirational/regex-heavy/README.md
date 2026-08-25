---
title: "Aspirational: regex-heavy scripts"
description: "Scripts that need regex APIs Vox does not yet expose (find with capture groups, group(i), etc.)"
category: "examples"
status: "aspirational"
last_updated: "2026-05-23"
training_eligible: false
training_rationale: "Aspirational corpus — these scripts depend on regex APIs that don't exist yet. Do not learn syntax from them."
---

# Aspirational: regex-heavy scripts

These scripts depend on regex APIs Vox doesn't yet provide:

- `re.find(line) -> Option[Match]` where `Match.group(i) -> str` (capture-group access)
- top-level statements (no `fn main()`) outside the script-mode pipeline
- `std.string.sort` / `std.string.unique` (collection helpers)

## Files

- `extract_table_names.vox` — extracts unique table names from a CREATE
  TABLE inventory file. Uses `regex.compile` + `find` + `group(1)` to
  grab the captured table name. The compile/find shape exists, but
  capture-group access is not exposed via the eval-side dispatch yet.

## What needs to land first

1. **`Match` value with `.group(i: int) -> Option[str]`** — already represented
   in the actor-runtime as `VoxMatch` (see `crates/vox-actor-runtime/src/builtins/mod.rs`),
   but not surfaced through eval dispatch.
2. **Collection helpers** (`sort`, `unique`, `dedup`) on `list[T]` — currently
   only `list.push` / `.pop` / `.len` / `.get` / `.contains` / `.join`
   are dispatched. Add the rest in a single Phase G+ pass.

Until both land, this script can't actually do its job — keeping it
under `aspirational/` makes the migration target visible without
polluting `scripts/`.
