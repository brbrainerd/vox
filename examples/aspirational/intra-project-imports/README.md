---
title: "Aspirational: intra-project imports"
description: "Helper-file pattern awaiting `import './foo.vox'` + `pub fn` support."
category: "examples"
status: "aspirational"
last_updated: "2026-05-23"
training_eligible: false
training_rationale: "Aspirational corpus — these scripts target a future Vox language feature that has not landed. Do not learn syntax from them."
---

# Aspirational: intra-project imports

These scripts target the v0.7+ "intra-project imports" feature: file
A in a project can `import "./other.vox"` and use `pub fn`s declared
there. The lexer/AST machinery is in place (`Token::Pub`, `ImportDecl`)
but the lowering + resolution pipeline isn't wired through.

For the audit/decision behind this deferral, see
[`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md`](../../../docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md)
§Imports/Modules/FFI audit (2026-05-23 health-corrections session).

## What these files want to be

- `walk_docs.vox` / `walk_sources.vox` — file-tree walkers consumed by
  `scripts/mens-corpus/harvest.vox`. Today they use CommonJS
  `module.exports = walk_docs` syntax, which Vox does not have. The
  intended Vox shape:

  ```vox
  // helpers/walk_docs.vox
  pub fn walk_docs(root: str) to list[str] { ... }

  // harvest.vox
  import "./helpers/walk_docs.vox"
  let files = walk_docs(repo_root)
  ```

- `emit_diagnostics.vox` — diagnostic extractor consumed by the same.
- `jsonl_writer.vox` — JSONL output helper.

## Migration plan

Once intra-project imports land (planned v0.7 per audit doc §12 Phase G+):
1. Replace `module.exports = name;` with `pub fn name(...) { ... }`.
2. Have `harvest.vox` `import "./mens-corpus/walk_docs.vox"` and call directly.
3. Move these files back to `scripts/mens-corpus/`.

## Do not learn syntax from these

The `module.exports = x;` and `-> Type` (function-return-arrow inside
expressions) patterns in these files are NOT valid Vox. They're
preserved as-is to make the diff at the migration step small.
