---
title: "Tutorial: first .vox app (checkpoints)"
description: "Checkpoints for a minimal compile/run path"
category: "Tutorials"
sort_order: 3
last_updated: "2026-03-25"

schema_type: "HowTo"
---

# First `.vox` app — checkpoints

Use this alongside [First full-stack app](../how-to/first-full-stack-app.md) and [golden examples](../../../examples/golden/hello.vox).

## Checkpoint A — parse

- [ ] Create `app.vox` with a top-level `fn` or use `examples/golden/hello.vox`.
- [ ] `vox check app.vox` exits **0** (or fix parse diagnostics).

## Checkpoint B — typecheck + HIR

- [ ] `vox check app.vox` shows no type errors.
- [ ] Optional JSON: `vox check app.vox --json` and confirm diagnostics carry `category` when emitted from the shared pipeline.

## Checkpoint C — build / run (when applicable)

- [ ] `vox build app.vox` or your project’s documented build entry.
- [ ] `vox run …` for script mode only when built with **`script-execution`** (see [CLI reference](../reference/cli.md)).

## Checkpoint D — mens (optional)

- [ ] With **`populi`** feature: `vox populi serve` local smoke; see [Populi SSOT](../reference/populi.md).

When stuck, capture **full** diagnostic output and cross-check [parser inventory](../reference/parser-ambiguity-inventory.md) and the [CLI reference](../reference/cli.md).


