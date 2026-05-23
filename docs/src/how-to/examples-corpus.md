---
title: "Golden Examples Corpus"
description: "How to use, maintain, and contribute to the machine-verified Golden Examples documentation corpus."
category: "How-To Guides"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---

# Golden Examples Corpus

The Vox documentation utilizes a "Golden Example" architecture to prevent documentation drift and ensure that all documented code actually compiles against the latest compiler version.

How goldens and docs feed **Mens** training (lexer vs HF tokenizer, corpus roots): [Vox source → Mens pipeline SSOT](../archive/research-2026-q1/vox-source-to-mens-pipeline-ssot.md). Pair layout and hygiene: [Mens training data contract](../reference/mens-training-data-contract.md).

## How Golden Examples Work

Instead of writing raw code blocks directly inside Markdown files, documentation should pull snippets from the `examples/golden/` directory.

CI enforces goldens in two layers: (1) **`vox-compiler`** integration test `all_golden_vox_examples_parse_and_lower` — every `examples/golden/**/*.vox` must parse, lower to HIR, pass WebIR validation, and emit Syntax-K metrics; (2) **mdBook / doc pipeline** — pages that use `{{#include}}` must resolve to real golden `.vox` files (`examples_ssot` test). A full `vox build` per golden may run in additional doc or integration jobs; do not assume “build-only” is the only gate.

## Adding a Golden Example

To document a feature with machine verification:

1. **Create the file**: Create a valid `.vox` file in `examples/golden/`.
2. **Write the code**: Add the required logic to the file. Ensure the file works when compiled.
3. **Define regions**: If your file is large but you only want to document a specific function, wrap the target logic in `[REGION:name]` anchors.
4. **Include it**: In your Markdown document, use the standard `mdbook` include syntax:

```markdown
&#123;&#123;#include ../../../examples/golden/my_example.vox:my_region&#125;&#125;
```

## The `// vox:skip` Directive

Sometimes it is necessary to show brief, inline examples that cannot be fully compiled (e.g., demonstrating a syntax error, or showing an incomplete code snippet for brevity).

In these cases, you must add a `// vox:skip` comment *inside* the code fence. The `vox-doc-pipeline` linter will scan for this directive; if it finds raw code fences without `// vox:skip` and without an `#include` directive, the build will fail.

```vox
// vox:skip
fn incomplete_function() {
    // This inline code will not be strictly verified by the compiler.
}
```

By ensuring every code fence is either an immutable golden reference or explicitly marked as skipped, Vox guarantees absolute trust in its documentation.
