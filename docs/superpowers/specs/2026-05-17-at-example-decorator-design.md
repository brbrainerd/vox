# `@example` Decorator (Language Surface)

**Date:** 2026-05-17
**Closes:** the language-surface prerequisite for corpus-eligible reference solutions per [`vox-as-llm-target-audit-and-plan-2026.md`](docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md) §6 ordering item #4 ("Once `@example` is a doctest+corpus entry, the benchmark assembles mechanically").

## Goal

Make `@example fn name() to Unit { ... }` a first-class declaration. Tooling (HumanEval-Vox mining, doctest harvester) can then enumerate corpus-eligible reference solutions via `HirModule::examples` without grepping the regression-test set.

## Why a separate decorator (not `@test`)

`@test` is for regression — failure means a real defect. `@example` is for *intent* — these are authored solutions meant for harvesting and demonstration. Mixing them would pollute the test runner with examples and pollute mining with regression cases.

## Shape

```vox
@example
fn ex_greet_user() to Unit {
    assert(greet("Ada") is "Hello Ada!")
}

@example("triple a value")
fn ex_triple_works() to Unit {
    assert((3 * 3) is 9)
}
```

Mirrors `@test`: optional `("label")` then a `fn name() to Unit { body }`. Body is typically an `assert(...)` over a function defined earlier in the same file.

## Implementation seams

- `crates/vox-compiler/src/lexer/token.rs` — `#[token("@example")] AtExample` + Display arm.
- `crates/vox-compiler/src/ast/decl/fundecl.rs` — `ExampleDecl { label, func }` struct.
- `crates/vox-compiler/src/ast/decl/types.rs` — `Decl::Example(ExampleDecl)` variant + `span()` arm.
- `crates/vox-compiler/src/ast/decl/callable.rs` — `Decl::Example` arm for deprecated/traced propagation.
- `crates/vox-compiler/src/fmt/printer.rs` — `Decl::Example => print_fn(&e.func, "@example ")`.
- `crates/vox-compiler/src/typeck/ast_decl_lints.rs` — `Decl::Example` visit arm.
- `crates/vox-compiler/src/parser/descent/mod.rs` — `Token::AtExample` in two declaration-position sets + parse_decl dispatch.
- `crates/vox-compiler/src/parser/descent/decl/head.rs` — `parse_example()` mirroring `parse_test()`.
- `crates/vox-compiler/src/hir/nodes/decl.rs` — `pub examples: Vec<HirFn>` field on `HirModule` (serde-default for forward-compat).
- `crates/vox-compiler/src/hir/lower/mod.rs` — `Decl::Example => hir.examples.push(...)` arm.
- Downstream exhaustive matches updated: `vox-corpus/src/corpus/extract_vox/part_ast.rs` (`"example"` kind tag), `vox-ml-cli/src/training/core.rs` (`"example"` construct tag).

## Tests

- `crates/vox-compiler/tests/example_decorator_test.rs` — 3 tests:
  - `example_decl_parses_typechecks_and_lowers_into_examples_vec` — two `@example` blocks plus one `@test`; verifies HIR partitioning is correct.
  - `example_without_label_parses` — bare `@example` (no label) works.
  - `example_pre_fix_would_have_failed_to_parse` — regression guard: silent removal of `AtExample` token would re-introduce the "Unexpected token at top level: xample" pre-fix error.

## Why this lift is real, not stub

- The grammar genuinely did not accept `@example` before this change — the lexer rejected it (`@e` substring matched no token). Adding the token makes the surface real.
- HIR carries the examples; tooling can enumerate them today without further plumbing.
- The corpus extractor and MENS training tagger both classify `@example` as its own construct kind, so harvested examples won't be misattributed.

## Out of scope

- Mining `examples/golden/**` reference solutions into HumanEval-Vox via `@example` — that is the consumer-side tooling lift (P3.1 in the implementation plan); the language change here is its prerequisite.
- A doctest-pipeline mode that runs `@example` bodies. The body must still type-check today (it does), but execution remains routed through `vox test` / the daemon RPC.
- `@example` cross-file visibility (modules, imports of named examples). Single-file scope is sufficient for the mining use-case.
