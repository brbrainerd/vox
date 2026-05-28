---
title: "ADR 033 — Typed parametric fragment primitive"
description: "Introduces a `fragment` bare-keyword scope for typed parametric markup blocks that are passable as props, callable multiple times with different arguments, and validated against consumer prop interfaces at compile time. Drafted ahead of Phase F implementation; deferred until the Phase 6 (TASK-6.1) typed semantic primitives stabilize."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-03"
training_eligible: true
schema_type: "TechArticle"
---
# ADR 033: Typed parametric fragment primitive

## Status

Accepted (2026-05-03). Phase F shipped end-to-end across two commits in the same session:

- Lexer `Token::Fragment` + `Display` arm, AST `FragmentDecl` and `Decl::Fragment` variant, parser dispatch in [parser/descent/mod.rs](../../../crates/vox-compiler/src/parser/descent/mod.rs) and `parse_fragment_decl` in [parser/descent/decl/head.rs](../../../crates/vox-compiler/src/parser/descent/decl/head.rs) (commit `6f01b8ae1`).
- HIR `HirFragmentDecl` node + AST→HIR lowering in [hir/lower/mod.rs](../../../crates/vox-compiler/src/hir/lower/mod.rs) and `emit_fragment_decls` in [codegen_ts/fragment_emit.rs](../../../crates/vox-codegen/src/codegen_ts/fragment_emit.rs) producing typed React function components in `fragments.tsx` with `<Name>Args` prop interfaces, wired into `emitter::generate` (commit `2227e3026`).

The Phase 6 (TASK-6.1) typed semantic primitive surface landed on `main` during the same session merge cycle, so the codegen gate originally noted here cleared without a second migration. Open questions in §"Open questions" remain follow-up sub-slices, not blockers.

## Context

Vox's reactive `component { … view: <markup> }` block lets authors parameterize a component's *whole* output via function parameters, but provides no first-class way to factor out a **named, typed, multiply-renderable chunk of markup** that another component receives as a prop. The current workarounds are:

1. **Children as a function prop.** Common React pattern — `(item: Item) => <Row item={item} />` — but the type system sees this as a function returning a generic `Element`, with no compile-time guarantee that the consumer actually rendered it, no validation that the right number / type of arguments are passed, and no error message that tells an LLM-generated consumer what went wrong.
2. **Sub-component per fragment shape.** Define a separate `component Row(item: Item)`. Works, but each row needs its own file, cross-file imports, and obscures the per-call-site tying-together of a list and its row renderer.
3. **Hand-written `unknown` props with runtime type assertions.** Worst case; surfaces in 100% of LLM-generated table/list code today.

Svelte 5's `{#snippet name(arg) … } / {@render name(value)}` ([snippet docs](https://svelte.dev/docs/svelte/snippet)) — covered in the [Svelte vs React Frameworks Research](../architecture/svelte-vs-react-frameworks-research-2026.md) — solves this with a typed parametric markup primitive (`Snippet<[T1, T2, …]>`). The research identified this as the single biggest reduction in plausible-but-wrong LLM-generated list/table/repeater code.

Vox needs an equivalent. Per the [grammar unification policy](../../../AGENTS.md), introducing a fragment is a candidate for a new bare-keyword scope (it opens a scope with rendering semantics distinct from `fn` / `component`), not a decorator.

## Decision

Introduce a **`fragment` bare-keyword scope** for typed parametric markup blocks. The decision splits into syntax, type system, render shape, and lowering policy.

### Syntax

```text
// vox:skip — illustrative; awaiting implementation
fragment Row(item: Item) {
  <tr><td>{item.name}</td><td>{item.price}</td></tr>
}

component Table(data: List[Item], row: Fragment[(Item,)]) {
  view: (
    <table>
      <tbody>
        for item in data {
          <RenderFragment of={row} args={(item,)} />
        }
      </tbody>
    </table>
  )
}

component Page() {
  view: (<Table data={items} row={Row} />)
}
```

Key choices baked in:

- **Keyword: `fragment`.** Alternatives considered + rejected below.
- **Body shape:** a single markup expression (same shape as `view:`); not a statement block. Future work may relax this if users need imperative setup before markup.
- **Capture rules:** fragments declared at module scope close over module-level bindings only (no implicit closure over `state`/`derived` from a containing component — that path is reserved for fragments declared *inside* a component, which is a Phase F+ extension).

### Type system

A fragment's type is `Fragment[(T1, T2, …)]` — a tuple of argument types. Empty arg list is `Fragment[()]` (callable with no arguments). The compiler:

- Validates that consumer prop signatures match fragment argument types at the call site.
- Reports `fragment.required_prop_missing` when a consumer requires a fragment prop and the caller doesn't pass one.
- Reports `fragment.arg_arity_mismatch` and `fragment.arg_type_mismatch` for misapplications.
- A fallback (`else` clause inside the consumer's `<RenderFragment>`) makes a fragment prop optional when the consumer accepts `Fragment[(…)] | None`.

### Render shape

`<RenderFragment of={fragmentValue} args={(arg1, arg2)} />`. Decision: **JSX-shaped, not decorator-shaped**.

Rationale: the JSX form is consistent with the rest of Vox markup (everything else inside `view:` is JSX-shaped). A `@render Row(item)` decorator-shaped form would introduce a new statement-vs-expression position that doesn't compose with `for` loops / `if` blocks / fragment composition. JSX-shaped composes cleanly.

Future ergonomic sugar (`{Row(item)}` shorthand) is a Phase F+ extension; the explicit `<RenderFragment>` form is the canonical lowering target.

### Lowering

In React/TSX emit:

- A `fragment Row(item: Item) { … }` declaration emits a typed React function component:
  ```ts
  // generated; illustrative
  export function Row({ item }: { item: Item }): React.ReactElement { return …; }
  ```
- A `Fragment[(Item,)]` prop type lowers to `(args: { item: Item }) => React.ReactElement`.
- `<RenderFragment of={row} args={(item,)} />` lowers to `{row({ item })}` in the consumer's TSX.
- The compiler validates required-prop / arity / type at compile time so the runtime needs no fragment-specific machinery.

Other emit targets (a future native target, etc.) can lower differently without changing the source.

### Coexistence with Phase 6 primitives

Fragments are an **authoring** primitive; Phase-6 typed semantic primitives (`stack`, `text`, `button`, `field`, …) are an **emission** primitive. A fragment body uses Phase-6 primitives in its markup; a Phase-6 primitive can accept a `Fragment[…]` prop (e.g., `<Table row={Row} />`). They compose orthogonally.

The deferral decision: do not implement fragments before the Phase 6 primitive surface stabilizes. A fragment authored today against raw JSX (`<tr><td>…</td></tr>`) would migrate twice — once to the Phase 6 primitive shape, once to whatever fragment composition reveals about primitive design. Wait until at least the 10 highest-usage primitives in [crates/vox-codegen/src/web_ir/primitives/mod.rs](../../../crates/vox-codegen/src/web_ir/primitives/mod.rs) have shipped per-primitive files before opening Phase F code work.

## Alternatives considered

1. **Use `fn` declarations returning `Element` plus a `@fragment` decorator.** Rejected: per the grammar policy, decorators *modify* declarations; a fragment is not a modified function — it has different scope rules (capture restrictions), different call ergonomics (`<RenderFragment>` vs function call), and different validation (consumer prop typing). A new bare-keyword scope is the consistent expression.

2. **Use `slot` as the keyword.** Rejected: `slot` carries existing connotations from the Web Components API (HTML `<slot>` element with its own distribution rules). A `slot` keyword would invite confusion about which semantics are in play. Svelte 5 itself moved *away* from `<slot />` to `{#snippet}` for the same clarity reason.

3. **Use `block` as the keyword.** Rejected: `block` is too generic; would collide with future "code block" / "raw block" features and is not self-describing.

4. **Use `template` as the keyword.** Rejected: `template` collides with the HTML `<template>` element semantics and with web-framework-specific connotations (Vue, Angular).

5. **Use a `@render fragment_name(args)` decorator-shaped render directive instead of `<RenderFragment>`.** Rejected: introduces a new statement-position decorator that doesn't compose inside JSX expressions, `for` loops, conditional blocks, or other fragments. The JSX-shaped `<RenderFragment>` composes cleanly everywhere and matches the rest of Vox's markup.

6. **Make fragments first-class function values without a keyword.** Rejected: erases the structural signal that this is markup-shaped, complicates compiler validation (the compiler must guess from return type whether a function is a fragment), and harms LLM codegen (the AI has no syntactic landmark to anchor on).

7. **Borrow Svelte's `{#snippet}` literal syntax verbatim.** Rejected: Vox uses neither `{#…}` block syntax nor `{@…}` directive syntax anywhere else. Adopting them just for fragments would fragment the grammar.

## Consequences

### Positive

- Fixes the most common LLM-generated GUI bug class (render-prop confusion) by giving the compiler concrete typed primitives to validate against.
- Provides the expressivity to build a Vox-native equivalent of Svelte 5 snippets, which the comparative research identified as the most AI-friendly markup primitive in 2026.
- Lets authors factor a `Table` / `List` / `Repeater` consumer apart from its row/item renderer without per-row component files.
- Composes cleanly with Phase 6 primitives (orthogonal axis: authoring vs. emission).

### Negative

- New bare-keyword scope adds parser complexity (lexer token, AST node, HIR node, lowering pass).
- New diagnostic codes (`fragment.required_prop_missing`, `fragment.arg_arity_mismatch`, `fragment.arg_type_mismatch`).
- Decision to gate on Phase 6 primitives stabilizing means Phase F is on the critical path of TASK-6.1.

### Neutral

- Existing `component`/`fn` semantics unchanged — fragments are purely additive.
- React-emit target unchanged — fragments lower to typed React function components.
- No effect on the [Phase 5 React interop spec](../archive/phase5-react-interop-spec-2026.md) — emitted fragments are first-class TS exports consumable by external React apps.

## Open questions (resolve at implementation time)

1. **Local-fragment scope inside `component { }`.** Should `fragment Row(item) { … }` declared inside a component implicitly close over the component's `state` / `derived` bindings? Phase F v1 says **no** (module-scope only); a Phase F+ extension may revisit when a real consumer surfaces.
2. **Recursive fragments.** Svelte's snippets support self-recursion (e.g., for tree rendering). Phase F v1: out of scope; revisit when a tree-rendering consumer surfaces.
3. **`<RenderFragment>` ergonomic shorthand.** A `{Row(item)}` form is more succinct but harder to grep. Phase F v1: ship the explicit `<RenderFragment>` form only; revisit shorthand after real-world use.
4. **Fragments exported from `.vox.ui` modules.** ADR-032 makes `.vox.ui` modules a thing for module-scope reactive state; whether they can also export fragments is a follow-up. Likely yes (fragments are a packaging concept; same reasoning applies), but defer the decision.

## Implementation references (deferred)

When Phase F code work starts, the touch surface is:

- New lexer token `Token::Fragment` at [crates/vox-compiler/src/lexer/token.rs](../../../crates/vox-compiler/src/lexer/token.rs) (with `Display` arm).
- New AST node `FragmentDecl` at [crates/vox-compiler/src/ast/decl/](../../../crates/vox-compiler/src/ast/decl/).
- Top-level decl dispatch in [crates/vox-compiler/src/parser/descent/mod.rs](../../../crates/vox-compiler/src/parser/descent/mod.rs) — same surface as `component` / `state_machine` (four sites: skip-recovery, async-fn, top-level fn, pub-fn).
- New HIR node `HirFragmentDecl`.
- New codegen at `crates/vox-codegen/src/codegen_ts/fragment_emit.rs`.
- Web IR validation: ensure fragments referenced in `<RenderFragment>` exist; arity / type match.
- Goldens: at minimum `examples/golden/fragment_table_row.vox` (a `<Table>` parameterized by a row fragment).
- Doctest fences in `docs/src/tutorials/` covering `<RenderFragment>` use.

## Related

- [Svelte vs React Frameworks Research (2026)](../architecture/svelte-vs-react-frameworks-research-2026.md) — competitive analysis identifying snippets as the highest-leverage markup primitive to mine.
- [Svelte-Mineable Features Implementation Plan (2026)](../architecture/svelte-mineable-features-implementation-plan-2026.md) — Phase F entry; sequencing notes on the Phase 6 dependency.
- [Vox GUI-Native Language Roadmap (2026)](../architecture/vox-gui-native-roadmap-2026.md) — TASK-6.1 (Phase 6 primitives) is the unblock dependency.
- [ADR 032: `.vox.ui` reactive modules](032-vox-ui-reactive-modules.md) — sibling ADR for the other major Phase D/E surface; same status (accepted + shipped end-to-end in the same session).
- [Phase 5: Bidirectional Vox↔React Interop Spec (2026)](../archive/phase5-react-interop-spec-2026.md) — confirms no `slot`/`fragment` keyword reservation; Phase F is unconstrained.
- [AGENTS.md §Grammar Unification](../../../AGENTS.md) — policy that `fragment` qualifies as a new bare-keyword scope.
