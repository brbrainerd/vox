---
title: "Svelte 5/6 vs React Meta-Frameworks — Comparative Research and Mineable Ideas for Vox (2026)"
description: "Comparative analysis of Svelte 5/6, Next.js 15/16, and TanStack Start as competitors and idea sources for Vox's GUI authoring layer. Frames Vox's React-emit position, identifies Svelte-specific syntax and compiler primitives worth mining for AI-generated GUI code, and explains why bidirectional Svelte interop is not recommended even though specific Svelte ideas are."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-02"
training_eligible: true
training_rationale: "Competitive ecosystem analysis informing the GUI roadmap and Phase 5 interop scope. Identifies concrete syntax/compiler features to mine from Svelte without changing Vox's React-target stance."
---

# Svelte 5/6 vs React Meta-Frameworks — Comparative Research and Mineable Ideas for Vox (2026)

## Summary

The Vox GUI emitter targets React/TS today and stays that way. The research below compares Svelte 5/6 (with runes), Next.js 15/16 (RSC + cache directives), and TanStack Start (Vite-native, type-safe React) on the dimensions that matter for Vox: **compile-time GUI debugging, AI-generated code reliability, bundle/runtime characteristics, and ecosystem reach**. The conclusion is two-part:

1. **Don't add Svelte as a Vox emit target.** Ecosystem mass, the [Phase 5 React interop spec](../archive/phase5-react-interop-spec-2026.md), and the npm-publishable component story already cover Svelte consumers indirectly (Svelte projects can import React via established wrappers; the cost of a second emit target is high and the marginal user reach is low).
2. **Mine Svelte's syntax and compiler primitives aggressively.** Several things Svelte 5/6 does are *strictly better for LLM codegen* than what Vox has today, and adopting them inside Vox source (lowering to React in emit) would improve compile-time GUI debugging without changing Vox's React positioning. The high-value imports are **runes-style explicit reactive primitives**, **typed snippet-like polymorphic blocks**, **template-binding directives that the compiler can validate**, **universal reactivity in non-component modules**, and **the MCP/llms.txt + benchmark loop that makes "the compiler is the source of truth for AI-written code" real**.

The rest of this document is the evidence and the per-feature breakdown.

## Scope and method

- **Comparators:** Svelte 5 (Oct 2024) and Svelte 6 (late 2025); Next.js 15 (stable) and Next.js 16 (cache components); TanStack Start v1.0 (2026 stable). The framing question is "what does each give an AI writing GUI code that Vox does not give already?"
- **Out of scope:** Solid, Qwik, Astro (already noted elsewhere), Angular, Vue, generic SSR vs SSG debates, marketing benchmarks.
- **Vox baseline used here:** the inventory in the next section, sourced by code search, not by claims in older docs.

## Vox baseline — what the GUI stack already has

Grounded in the current codebase, not aspirational:

| Capability | Where it lives | Status |
|---|---|---|
| `component Name() { }` keyword (per [AGENTS.md §Grammar Unification](../../../AGENTS.md)) | [crates/vox-compiler/src/ast/decl/fundecl.rs](../../../crates/vox-compiler/src/ast/decl/fundecl.rs) | Implemented; lowers to React TSX in [codegen_ts/component.rs](../../../crates/vox-codegen/src/codegen_ts/component.rs) |
| **Reactive component members `state`, `derived`, `effect`, `on mount`, `on cleanup`, `view:`** | AST: [ast/decl/ui.rs:152–238](../../../crates/vox-compiler/src/ast/decl/ui.rs); parser: [parser/descent/decl/head.rs:253–334](../../../crates/vox-compiler/src/parser/descent/decl/head.rs); codegen: [codegen_ts/reactive.rs:740–815](../../../crates/vox-codegen/src/codegen_ts/reactive.rs); golden: [examples/golden/reactive_counter.vox](../../../examples/golden/reactive_counter.vox) | **Implemented inside a `component { }` block.** Lowers to `useState` / `useMemo` / `useEffect`; auto-emits React deps lists |
| Auto-dep inference (compile-time read-tracking for `derived` and `effect`) | [codegen_ts/hir_emit/state_deps.rs](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs) | Partial: AST walk that finds identifiers matching reactive-binding names; descends into `Lambda` bodies; does **not** cross function-decl boundaries (over-tracks within scope, under-tracks across calls) |
| **Directive-style attributes `on:click`, `on:change`, `on:input`, `on:submit`, `on:keydown` …** | Parser view-call lowering: [parser/descent/expr/pratt_match.rs](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs); mapping: [codegen_ts/hir_emit/compat.rs:27–42](../../../crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs) | **Implemented for the `on:*` family only.** Both `name:suffix` (colon) and `name-suffix` (hyphen) attribute-name syntax are accepted by the parser |
| React-hook bridge (`use_state`, `use_effect`, `use_memo`, `use_ref`, `use_callback`, `use_layout_effect`) | [crates/vox-compiler/src/react_bridge.rs](../../../crates/vox-compiler/src/react_bridge.rs) | 1:1 mapping to React hooks (escape hatch beneath the reactive-member surface) |
| Web IR (intermediate before TSX) | [crates/vox-codegen/src/web_ir/mod.rs](../../../crates/vox-codegen/src/web_ir/mod.rs) | Implemented; G4 parity tests against legacy emit path |
| Compile-time a11y validators (img alt, button accessible name, keyboard handler on `role="button"`) | [crates/vox-codegen/src/web_ir/validate_a11y.rs](../../../crates/vox-codegen/src/web_ir/validate_a11y.rs) | Implemented; structured diagnostic codes `web_ir_validate.a11y.*` |
| Web IR validators (DOM bounds, route IDs, behavior optionality, island prop keys, broken `href`/`to` against route patterns) | [crates/vox-codegen/src/web_ir/validate.rs](../../../crates/vox-codegen/src/web_ir/validate.rs) | Implemented |
| `routes { }` block, type-safe nested entries with `path` / `component_name` / `loader_name` / `pending_component_name` / `redirect` / wildcard | [crates/vox-compiler/src/ast/decl/ui.rs](../../../crates/vox-compiler/src/ast/decl/ui.rs) + [codegen_ts/route_manifest.rs](../../../crates/vox-codegen/src/codegen_ts/route_manifest.rs) | Implemented; conflict detection is exact-string-match only ([routes.rs:87](../../../crates/vox-codegen/src/codegen_ts/routes.rs)) — does not detect `/users/:id` vs `/users/me` overlap |
| `state_machine` keyword, typed states + transitions, discriminated-union types **and reducer function stub** | [crates/vox-compiler/src/ast/decl/state_machine.rs](../../../crates/vox-compiler/src/ast/decl/state_machine.rs) + [state_machine_emit.rs:1–110](../../../crates/vox-codegen/src/codegen_ts/state_machine_emit.rs) | Implemented as types + reducer skeleton; no instantiable runtime object or live state field |
| Design tokens (JSON → CSS custom properties + typed `voxTokens` constant) | [codegen_ts/tokens_emit.rs](../../../crates/vox-codegen/src/codegen_ts/tokens_emit.rs) | Implemented |
| Component-scoped CSS via `style { }` blocks, `raw_css { }` escape hatch | codegen_ts | Implemented |
| Diff-based autofix suggestions on diagnostics | [crates/vox-compiler/src/typeck/autofix.rs](../../../crates/vox-compiler/src/typeck/autofix.rs) | Implemented; rule-driven baseline |
| `vox dev` hot loop (compilerd + JSON-RPC, watches and rebuilds) | [crates/vox-cli/src/commands/dev.rs](../../../crates/vox-cli/src/commands/dev.rs) | Implemented |
| **MCP compile-and-validate tool: `vox_validate_file`** ("Validate a .vox file using the full compiler pipeline") **plus `vox_compiler::ast_inspect`** | Registry: [contracts/mcp/tool-registry.canonical.yaml:1211](../../../contracts/mcp/tool-registry.canonical.yaml); dispatch: [crates/vox-orchestrator-mcp/src/dispatch.rs:237](../../../crates/vox-orchestrator-mcp/src/dispatch.rs); MCP crates: `vox-mcp-meta`, `vox-mcp-registry`; stdio server: [crates/vox-cli/src/commands/mcp_server/mod.rs](../../../crates/vox-cli/src/commands/mcp_server/mod.rs) | Implemented; surfaced through the existing stdio MCP server |
| External-frontend interop pipeline (target split, OpenAPI/JSON Schema emit, bidirectional component interop) | [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md), [phase5-react-interop-spec-2026.md](../archive/phase5-react-interop-spec-2026.md) | Plan / specs; Phase 1 in flight |

**What was wrong in earlier drafts of this section:** an earlier inventory claimed Vox had "no Vox-native reactive primitives" and "no compiler-checked binding directives" and "no MCP server." All three were wrong. Vox has `state` / `derived` / `effect` / `on mount` / `on cleanup` reactive members inside `component { }` blocks, has `on:*` directive-style event attributes with both colon- and hyphen-prefix support in the parser, and has a `vox_validate_file` MCP tool registered and dispatched. The implementation plan ([svelte-mineable-features-implementation-plan-2026.md](svelte-mineable-features-implementation-plan-2026.md)) is built on what is *actually* missing once these are accounted for.

**Real gaps relevant to this comparison:**

- Reactive members `state` / `derived` / `effect` work **only inside `component { }`** — there is no `.vox.ui` module-level analog and no shared-state-without-a-component story.
- Auto-dep inference walks identifiers in the same block but does not cross function-decl boundaries; closure-captured reactive bindings inside lambdas are tracked, but a `derived x = compute(count)` where `compute` is a free function does not see the read.
- Directive families beyond `on:*` are absent: no `bind:value`, no `class:active={cond}`, no `style:--var={token}`. Form binding is hand-written controlled-input pairs.
- No typed parametric template fragments (no `slot`, `fragment`, `block`, or snippet keyword). Function components are the only parameterized markup primitive.
- `vox_validate_file` exists but its diagnostic-result shape, autofix-suggestion surfacing, and "how to point my coding agent at this" docs are thin to nonexistent — the *capability* is shipped, the *AI-usable contract* is not.
- Route conflict detection is exact-string-match; no segment-aware overlap check. No typed `href` helper API (validation happens but is diagnostics-only).
- State machines emit types + a reducer stub but no runtime instance; no `state`-rune-style live field; no helper for instantiation in stdlib.

## Svelte 5 / Svelte 6 — what they actually do well

Svelte 5 (Oct 2024) replaced implicit `let`-based reactivity with **runes**: explicit reactive primitives that work in `.svelte` files **and** in plain `.svelte.ts` modules. Svelte 6 (late 2025) matured runes, made stores largely obsolete, made reactive classes the recommended pattern, and integrated an [official MCP server](https://svelte.dev/docs/mcp) for AI assistants.

### Runes (`$state`, `$derived`, `$effect`, `$props`, `$bindable`)

```svelte
<!-- vox:skip — illustrative Svelte source, not Vox -->
<script lang="ts">
  let count = $state(0);
  let doubled = $derived(count * 2);
  $effect(() => { console.log("count is", count); });
</script>

<button onclick={() => count++}>{count} (×2 = {doubled})</button>
```

What's good for AI codegen:

- **Explicit primitives, no rules-of-hooks ritual.** No "rules of runes" page exists because the model is uniform: a rune is a function-shaped declaration that means *exactly* one thing. There is no order-of-call rule; no dependency array; no stale-closure trap; no `useCallback`-vs-`useMemo` decision tree. A 2024–2026 SvelteBench analysis ([khromov/svelte-bench](https://github.com/khromov/svelte-bench)) found that idiomatic-rune output from frontier LLMs (including Claude) is more consistent than React-hook output on the same prompts, primarily because the failure mode "model wrote a hook call inside an `if`" simply doesn't exist in the rune model.
- **Universal reactivity outside the component file.** A `.svelte.ts` module can declare `$state` at module scope and export it, replacing 80% of the use cases that Vue, React, Solid, etc. push into a separate state-management library. For Vox, the analog would be a `.vox.ui` reactive-module convention.
- **Reactive classes.** `class Counter { count = $state(0); double = $derived(this.count * 2); }` — runes inside class fields. This is the Svelte 6 idiom for what used to require Svelte stores. Big win for AI: classes are a familiar, regular shape.

### Snippets — replacing slots

Svelte 4's `<slot />` is gone; `{#snippet name(arg1, arg2)} … {/snippet}` + `{@render name(value)}` replaces it.

```svelte
<!-- vox:skip — illustrative Svelte source -->
{#snippet row(item: Item)}
  <tr><td>{item.name}</td><td>{item.price}</td></tr>
{/snippet}
<Table data={items} {row} />
```

Snippets are typed (`Snippet<[Item]>`), parameterized, can be passed as props, can fall back, can render multiple times with different arguments. They are a strict superset of React's children/render-props pattern with first-class compiler support and TypeScript inference. The pattern that React expresses as `({ item }) => <Row item={item} />` (function-as-children) is `{#snippet row(item)} … {/snippet}` in Svelte and the type checker actually understands it as a fragment of markup, not a function.

### Template directives (`bind:`, `on:`, `class:`, `style:`, `use:`)

Form binding is a directive that the compiler validates against the element's attribute type:

```svelte
<!-- vox:skip — illustrative Svelte source -->
<input bind:value={name} />
<input type="checkbox" bind:checked={agreed} />
<div class:active={isOpen} class:disabled={isDisabled}>…</div>
<div style:--accent-color={token.accent}>…</div>
```

What this gives the compiler: `bind:value` on `<input type="number">` knows the bound variable should be a number, not a string. `class:active={isOpen}` is checked at compile time; `class:active=${isOpen}` (typo) errors. The author cannot forget to wire both `value` and `onChange` because there is no two-half pattern.

For AI codegen this is the single biggest reduction in plausible-but-wrong output: React form code's most common LLM bug is "wrote `value={x}` and forgot `onChange`", or wrote `onChange` but assigned the event object instead of `e.target.value`. Svelte makes the bug unrepresentable.

### Compile-to-DOM, no virtual DOM

Svelte compiles each component to imperative DOM mutation code keyed off the reactive graph. Gzipped runtime is in the 2–5 KB range; React is in the 42 KB range; for Vox the relevant claim is not "Svelte is faster" but "Svelte's compiled output is smaller and more deterministic, which means it's easier for a human or an AI to read the emitted code and understand what changed."

### CSS scoping and `style:` directives

Single-file `<style>` block, automatically scoped via hashed class names; `style:` directive for dynamic per-element CSS variables; `:global()` escape hatch when needed. Vox already has component-scoped CSS via `style { }` blocks, so this is parity, not a gap.

### Official AI tooling (Nov 2025–2026)

- **[Svelte MCP server](https://svelte.dev/docs/mcp)** sits between the AI assistant and Svelte docs, *and statically analyzes generated code against the compiler before suggesting it back*. This is the closest existing prior art for "the compiler is the source of truth for AI-written code."
- **[svelte-llm](https://svelte-llm.stanislav.garden/) llms.txt distilled docs** (~120 KB).
- **[SvelteBench](https://github.com/khromov/svelte-bench)** — HumanEval-style benchmark for AI-generated Svelte 5 components. Concrete reproducible measurement of which LLMs and prompts produce working Svelte.

### Where Svelte hurts you

- **Ecosystem.** Smallest of the three. Fewer component libraries, fewer Stack Overflow answers, fewer hires.
- **Snippet `children` syntax has known confusion** (per a sustained Svelte GitHub discussion thread); the special-meaning `children` variable is not as obvious as `<slot />` was.
- **Migration debt.** Svelte 4 → 5 was real work for existing apps; Svelte 5 → 6 was easier but the rune model is not what most Svelte tutorials still teach.
- **No React Server Components analog.** SvelteKit has its own server-load model that is fine but is not API-compatible with the RSC mental model that a fraction of teams have already adopted.

## Next.js 15/16 — the incumbent

- **Status.** Next.js 15 stable; Next.js 16 introduces Cache Components and the `'use cache'` directive. Largest React meta-framework by a wide margin.
- **Strengths.** Ecosystem reach; mature Vercel deployment story; React Server Components stable; Actions API stable; broad library support; React Compiler v1.0 (Oct 2025) auto-memoizes idiomatic React, reducing manual `useMemo` / `useCallback` toil.
- **Costs.** RSC + Cache Components are a complex mental model with edge cases. The `'use cache'` directive cannot be combined naively with `cookies()`, `headers()`, runtime params, or `next-intl` without architectural workarounds. Cache scopes are isolated from `React.cache`. Edge runtime is partially unsupported. The complexity surface that Cache Components add is exactly the surface where AI codegen produces plausible-looking but broken output.
- **Relevance to Vox.** Vox's `--target=server` mode (per [Phase 1 spec](phase1-build-targets-spec-2026.md)) makes Next.js a *consumer* of Vox APIs via the OpenAPI emit path. We do not need to compete with Next.js's app-router story — we need to be a first-class backend that a Next.js app can talk to.

## TanStack Start v1.0 — the type-safe React alternative

- **Status.** v1.0 production-ready (2026); Vite-native, file-based routing with full type inference, isomorphic loaders, no React Server Components, no `'use cache'` directive.
- **Strengths for AI codegen.** Type-safe routing all the way through: `href="/users/$userId"` is typed, params are typed, loader return types flow into the component automatically. Loaders run before render, so the data shape at component-mount time is unambiguous — this eliminates a large class of "AI wrote a render that assumes data exists when it might not" bugs.
- **Costs.** Smaller ecosystem than Next.js; pre-stable culture in some tooling; no built-in RSC story (which is a feature for some teams, a gap for others).
- **Relevance to Vox.** TanStack Start is the closest existing-React-framework analog to what Vox's emit could look like if the `routes` block tightened up. The TanStack pattern of "loader runs first, types flow through, no string guessing on URLs" is something Vox can learn from directly — see §What to mine.

## Side-by-side feature matrix

| Capability | Vox (today, verified) | Svelte 5/6 + SvelteKit | Next.js 15/16 | TanStack Start v1.0 |
|---|---|---|---|---|
| Reactive primitives in source | **Yes inside `component { }`** (`state` / `derived` / `effect` / `on mount` / `on cleanup`); React-hook bridge as escape hatch | Runes (`$state`/`$derived`/`$effect`), universal | `useState`/`useReducer` + Compiler v1.0 auto-memo | Same as Next.js + first-class router state |
| Reactivity outside components | No | Yes (`.svelte.ts` modules) | Limited (Server Components, contexts) | Limited (router context) |
| Auto-dep inference for derived/effect | Partial (intra-block, lambda-aware, doesn't cross function-decl boundaries) | Yes (compiler-tracked) | Yes (React Compiler v1.0) | Same as Next.js |
| Typed parametric markup blocks | No (function components only) | Yes (snippets, typed `Snippet<[T]>`) | No (children + render props) | No (children + render props) |
| Directive-style attributes | **Partial** (`on:*` event family only) | Yes (`on:`, `bind:`, `class:`, `style:`, `use:`) | No (props by hand) | No (props by hand) |
| Compile-checked form binding | No | Yes (`bind:value`) | No (controlled inputs by hand) | No (controlled inputs by hand) |
| File-based routing with full type-flow | Block-based, partial | Yes (SvelteKit `+page.ts`) | Yes (App Router) | Yes (best-in-class type inference) |
| Route conflict detection | Exact-string-match only ([routes.rs:87](../../../crates/vox-codegen/src/codegen_ts/routes.rs)) | Segment-aware | Partial | Segment-aware |
| Broken-link compile check | Yes ([web_ir/validate.rs](../../../crates/vox-codegen/src/web_ir/validate.rs)) | Via SvelteKit type-check | No | Via typed router |
| Compile-time a11y checks | Yes (img alt, button name, keyboard) | Yes (eslint-plugin-svelte; not compiler-native) | No (eslint-plugin-jsx-a11y, runtime axe) | Same as Next.js |
| Scoped CSS by default | Yes (`style { }`) | Yes (single-file `<style>`) | No (CSS Modules opt-in) | No (CSS Modules opt-in) |
| Design-token primitive | Yes (`voxTokens`, CSS vars) | Via `style:--var` directive | Per-team convention | Per-team convention |
| State-machine primitive | Yes (`state_machine` keyword, types + reducer stub) | No (XState as library) | No (XState as library) | No (XState as library) |
| MCP compile-and-validate tool | **Yes** (`vox_validate_file`, `vox_compiler::ast_inspect`); needs ergonomic polish | Yes (official MCP, with static validation) | Vercel AI SDK; no MCP-with-validation | Community MCPs |
| AI codegen benchmark suite | No (deferred — see M5) | Yes ([svelte-bench](https://github.com/khromov/svelte-bench)) | No standardized one | No |
| Bidirectional component interop with React | Phase 5 plan | Via [react-svelte](https://github.com/Rich-Harris/react-svelte) / [svelte-preprocess-react](https://github.com/bfanger/svelte-preprocess-react) / [Sveltris](https://sveltris.vercel.app/) | Native | Native |
| Bundle size class | Compiles to React (React-class) | 2–5 KB runtime | ~42 KB React + Next runtime | ~42 KB React + Vite |
| Ecosystem size | Vox-native + React via interop | Smaller than React | Largest React subset | Smaller React subset, growing |

## What to mine from Svelte (the actionable section)

Each item below is **mineable into Vox source syntax** and **lowerable into the existing React TSX emit**. None require dropping the React target or adding a Svelte target. The full implementation plan with concrete file changes, test strategy, scope estimates, and dependency ordering lives in the companion document: [Svelte-Mineable Features Implementation Plan (2026)](svelte-mineable-features-implementation-plan-2026.md). What follows is the rationale and design intent for each item.

### M1 — Reactive primitives outside `component { }` plus better auto-dep inference

**What's already done.** `state name: T = init`, `derived name = expr`, `effect: { … }`, `on mount: { … }`, `on cleanup: { … }`, and `view: …` all parse and lower correctly inside a `component { }` block — see [reactive_counter.vox](../../../examples/golden/reactive_counter.vox) and [codegen_ts/reactive.rs:740–815](../../../crates/vox-codegen/src/codegen_ts/reactive.rs). Auto-dep inference for `derived` and `effect` runs via [hir_emit/state_deps.rs](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs) and emits the React deps array automatically.

**Two real gaps.**

1. **Reactivity outside `component { }`.** The reactive members are only legal inside a reactive-component decl. There is no way to declare module-level reactive state and import it from multiple components. (The Svelte 6 `.svelte.ts` analog.)
2. **Auto-dep inference doesn't cross function-decl boundaries.** `extract_state_deps()` walks the same expression tree as the `derived`/`effect` body. It descends into lambdas (`HirExpr::Lambda` → recurse on body, [state_deps.rs:95](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs:95)), but a `derived label = format_label(count)` where `format_label` is a free function does not see the read of `count` through the call. The current behavior under-tracks cross-call reads.

**Why it helps AI:** module-level reactive state matches the natural shape of small AI-generated examples; better auto-deps removes the only remaining "stale closure" failure mode on the reactive surface.

### M2 — Typed parametric fragment blocks

**Status:** confirmed missing. No `slot`, `fragment`, `block`, or snippet keyword exists in the lexer or parser. Function components are the only parameterized markup primitive, and they don't compose well as multiply-renderable fragments passed as props.

**Idea.** A first-class typed parametric markup primitive (working name `fragment` — final name decided in the implementation plan). Bare-keyword scope (consistent with `component`/`routes`/`state_machine` per [AGENTS.md §Grammar Unification](../../../AGENTS.md)), because it opens a scope with rendering semantics:

```text
// vox:skip — design sketch; see implementation plan for final shape
fragment Row(item: Item) { <tr><td>{item.name}</td></tr> }
<Table data={rows} row={Row} />
```

A fragment value is typed, passable as a prop, callable multiple times with different arguments, supports an optional fallback. **Lowering** in React-emit mode is to typed function-as-children that the consuming component invokes; the Vox compiler validates "this fragment is required by `Table`'s prop interface and you didn't pass it" at compile time.

**Why it helps AI:** the single biggest cause of "AI wrote a list/table/repeater that doesn't compose" today is render-prop confusion. Typed fragments are concrete, named, and inspectable.

### M3 — Additional directive families: `bind:`, `class:`, `style:`

**What's already done.** `on:*` event-handler directives work today, parsed in the current view-call parser path at [pratt_match.rs](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs) (which accepts both colon- and hyphen-prefixed attribute names) and mapped to React props at [compat.rs:27–42](../../../crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs:27). The colon separator is the established Vox convention; M3 is *additional families*, not a new syntax.

**Decision (separator):** **colon `:`**. Confirmed by code search — `on:click` already ships and the parser accepts arbitrary `name:suffix` attribute names. Adopting the same separator for `bind:`, `class:`, `style:` is consistent with what's there, with no parser-level work to enable the syntax (only mapping table additions and the lowering logic).

**New directives.**

- `bind:value={name}` — two-way binding on form inputs. `name` must be a reactive `state` binding; `bind:value` on `<input type="number">` requires a numeric state, etc.
- `bind:checked={agreed}` — checkbox/radio binding.
- `bind:group={selected}` — radio-group binding (lowers to controlled inputs sharing a name).
- `class:active={isOpen}` — conditional class application; lowers to clsx-style composition.
- `style:--accent={tokens.accent}` — direct integration with the existing `voxTokens` design-token system; no `style={{ ['--accent' as any]: … }}` cast in TSX.

**Why it helps AI:** eliminates the controlled-input two-half bug class entirely; makes design tokens directly addressable in markup; turns directive-name typos into compile errors.

### M4 — Reactive modules (`.vox.ui` files)

**Idea.** A file convention: any file with a `.vox.ui` suffix may declare module-scope `state` / `derived` / `effect` and export them. This is the Svelte 6 `.svelte.ts` analog. The existing parser already accepts the `state` / `derived` / `effect` tokens; the work is allowing them at module scope (currently they are only legal inside a `ReactiveComponentDecl`'s member list).

**Lowering.** Reactive-module exports compile to a generated React context provider + a `useReactiveStore`-style hook in the emitted TSX. Other emit targets (any future native target) can lower differently without changing the source.

**Why it helps AI:** matches the "small example outside a component" pattern that LLM-generated code naturally produces, and removes the otherwise-needed third-party state-management library.

### M5 — Polish and document the existing `vox_validate_file` MCP tool

**What's already done.** `vox_validate_file` ("Validate a .vox file using the full compiler pipeline (lexer → parser → typeck → HIR)") is registered in [contracts/mcp/tool-registry.canonical.yaml:1211](../../../contracts/mcp/tool-registry.canonical.yaml) and dispatched at [crates/vox-orchestrator-mcp/src/dispatch.rs:237](../../../crates/vox-orchestrator-mcp/src/dispatch.rs). The companion `vox_compiler::ast_inspect` tool ([same file:351](../../../crates/vox-orchestrator-mcp/src/dispatch.rs)) returns the parsed AST as JSON. The stdio MCP server is wired through [crates/vox-cli/src/commands/mcp_server/mod.rs](../../../crates/vox-cli/src/commands/mcp_server/mod.rs).

**Real gaps.**

- The diagnostic-result shape returned by `vox_validate_file` does not currently surface the existing autofix suggestions ([typeck/autofix.rs](../../../crates/vox-compiler/src/typeck/autofix.rs)) in a structured form an AI client can apply.
- There is no documented "point your coding agent at this MCP" how-to.
- There is no in-memory variant of the tool — every call goes through `resolve_existing_path_in_repository`, which means an AI cannot validate a snippet without first writing it to a file. A `vox_validate_source` (text-in, diagnostics-out) tool is the missing piece for the iterative-loop use case.

**Why this is the highest-leverage item.** Vox's structured diagnostic codes (`web_ir_validate.a11y.*`, `web_ir_validate.island.*`, etc.) and the autofix-suggestion framework are *already* designed for machine consumption. They are not currently shaped for AI clients to consume directly. The work is mostly polish on top of code that ships.

**On a vox-bench analog:** dropped from active scope. A SvelteBench-style HumanEval suite for AI Vox codegen has no defined consumer right now (it would not gate releases, would not feed any optimization loop, and would not directly drive grammar decisions). The cost is significant (curated prompt corpus, golden expected behaviors, runner harness, baselines per model). Park it. Revisit when there is a specific decision the metric would inform — e.g., comparing two grammar variants for AI-friendliness, or gating a model upgrade in MENS for "still writes Vox correctly."

### M6 — Route segment-aware conflict detection plus typed `href`

Not strictly Svelte, but in the same family.

- **Segment-aware overlap detection.** Current check at [routes.rs:87](../../../crates/vox-codegen/src/codegen_ts/routes.rs) builds a `HashSet<(Method, String)>` and only catches identical literal strings. Extend to detect `/users/:id` vs `/users/me` overlap (with documented precedence resolution).
- **Typed `href` helper.** [validate.rs](../../../crates/vox-codegen/src/web_ir/validate.rs) already validates literal `href` and `to` attributes against declared route patterns (broken-link diagnostic). Extend to a typed helper API so authors can write `href={route.users.show(id)}` and get full type-flow.
- **Loader return-type → component prop-type flow** (Phase 3 spec covers part of this on the HTTP side).

**Why it helps AI:** removes "AI hallucinated a route URL that doesn't exist" entirely.

### M7 — Reactive-class state-machine ergonomics

The existing `state_machine` keyword emits typed states + events + a reducer function stub at [state_machine_emit.rs](../../../crates/vox-codegen/src/codegen_ts/state_machine_emit.rs). What's missing is a runtime instance pattern. Borrow Svelte 6's reactive-class idiom: make `state_machine Foo` instantiable through a generated `useFoo()` hook (inside a `component { }`) or a plain reactive-class instance (inside a `.vox.ui` module from M4). The state field becomes a reactive `state` binding; `machine.send(event)` is a normal method.

Depends on M4 landing if reactive-class instances should be usable outside components.

## What NOT to mine

- **Single-file component format with magic `<script>` / `<template>` / `<style>` blocks.** Vox is a programming language; its grammar is consistent with itself and with the [grammar unification policy](../../../AGENTS.md). Importing Svelte's file-shape would break that consistency and force a parser dialect.
- **Stores as a primitive.** Svelte 6 itself is moving away from stores toward reactive classes; Vox should skip the intermediate concept entirely.
- **Compile-to-no-runtime.** Vox emits to React, which has a runtime; chasing zero-runtime would mean dropping the React target. The user explicitly does not want that and the [external-frontend-interop plan](external-frontend-interop-plan-2026.md) explicitly preserves it.
- **A separate `.svelte` parser inside Vox.** Out of scope and unjustified.

## Interop position — should Vox import or export Svelte components?

**Recommendation: no, with one caveat.**

- **Vox importing a Svelte component:** Possible via wrappers like [Sveltris](https://sveltris.vercel.app/) or [svelte-preprocess-react](https://github.com/bfanger/svelte-preprocess-react), which already bridge Svelte ↔ React. Since Vox emits React, anyone who needs a Svelte component inside a Vox-emitted UI can use those wrappers in their own React project. Vox does not need a first-party `import svelte …` form analogous to the [Phase 5 `import react …`](../archive/phase5-react-interop-spec-2026.md) form. **Cost/benefit ratio is poor.**
- **A Vox component being consumable by a SvelteKit app:** Already covered indirectly. Per [Phase 5](../archive/phase5-react-interop-spec-2026.md), emitted Vox components are first-class npm-publishable React components; a SvelteKit consumer wraps them with one of the existing react-in-svelte adapters. No Vox-side work required.
- **Caveat — backend-only mode.** For users on `--target=server` (per [Phase 1 spec](phase1-build-targets-spec-2026.md)), the frontend can be SvelteKit just as easily as Next.js; this is already supported by the OpenAPI emit. No Svelte-specific work needed; document it in the "Bring your own React frontend" tutorial planned for Phase 5 (rename to "Bring your own frontend" and add a SvelteKit example).

## Recommendation summary (for sequencing)

Reordered after the codebase verification — many items are smaller than the original draft assumed because Vox already ships the foundations. Detailed phase plan, file changes, and scope estimates are in [Svelte-Mineable Features Implementation Plan (2026)](svelte-mineable-features-implementation-plan-2026.md).

1. **M5 — `vox_validate_source` (text-in MCP variant) + structured autofix surfacing** (highest leverage; the tool exists, the AI-usable contract does not).
2. **M3 — Add `bind:`, `class:`, `style:` directive families** (parser already accepts the syntax shape; need lowering and validation tables only).
3. **M6 — Route segment-aware conflict detection + typed `href` helper** (extends the existing exact-match check and `href` static-validator).
4. **M1 — Reactivity outside `component { }` (`.vox.ui` modules) + cross-call auto-dep inference** (extends `state`/`derived`/`effect` which already work inside components; the closure-tracking analysis builds on `state_deps.rs`).
5. **M4 — `.vox.ui` module convention** (largely covered by M1; tracked separately for spec hygiene).
6. **M2 — Typed parametric fragments** (largest greenfield item — new bare-keyword scope).
7. **M7 — Reactive-class state-machine instances** (depends on M4 if instances should live outside components).

Deferred: **vox-bench** (no defined consumer for the metric — see M5 rationale). Revisit when a specific decision would be informed by the score.

## Cross-references

- [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md) — five-phase interop plan; this research informs Phases 5 and the post-Phase-5 GUI authoring evolution.
- [Phase 5: Bidirectional Vox↔React Interop Spec (2026)](../archive/phase5-react-interop-spec-2026.md) — the React-interop spec this research is *not* trying to displace.
- [Phase 1: Build Target Split Spec (2026)](phase1-build-targets-spec-2026.md) — `--target=server` makes Vox usable by SvelteKit consumers without any Svelte work in Vox itself.
- [Phase 3: HTTP Ergonomics Decorators Spec (2026)](phase3-http-ergonomics-spec-2026.md) — overlaps with M6 for path-param typing on the HTTP side.
- [Vox GUI-Native Language Roadmap (2026)](vox-gui-native-roadmap-2026.md) — the roadmap into which M1–M7 should be slotted.
- [GUI-Native Roadmap Execution Status (2026)](gui-native-roadmap-status-2026.md) — track new mineable-feature work here once scoped.
- [Internal Web IR Implementation Blueprint](internal-web-ir-implementation-blueprint.md) — the IR layer that would receive directive lowerings (M3) and rune lowerings (M1).
- [AGENTS.md §Grammar Unification](../../../AGENTS.md) — the rule that decides whether each new feature is a decorator or a bare-keyword scope.
- [docs/src/.well-known/llms.txt](../.well-known/llms.txt) — the agent-discovery surface a `vox-llm` MCP server (M5) would extend.

## Open questions (resolved)

The earlier draft of this doc carried five open questions. After codebase verification all five are resolved or retired:

1. **Naming for reactive primitives.** ~~`$state` vs `state(...)` vs decorator?~~ Resolved by the codebase: `state name: T = init`, `derived name = expr`, `effect: { … }` are already lexer keywords and parse cleanly inside `ReactiveComponentDecl` ([head.rs:253](../../../crates/vox-compiler/src/parser/descent/decl/head.rs)). The naming question is closed; the open work is allowing them at module scope (M1/M4).
2. **Auto-dep inference cost.** ~~Whole-program analysis required?~~ The implementation plan scopes this explicitly: lexical closure-capture tracking (extends [state_deps.rs](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs)), bounded cross-decl analysis with annotation-based opt-in for free functions, no whole-program escape analysis. Conservatively over-track on the uncertain edges (mark dep set as "depends on everything reachable" when escape is unprovable). Decision: do the closure work; bound the cost.
3. **Directive separator.** ~~`:` vs another separator?~~ Resolved: **colon `:`**, because it is already the established Vox convention for `on:click` ([compat.rs:30](../../../crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs:30)) and the current parser path already accepts arbitrary `name:suffix` attribute names ([pratt_match.rs](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs)). New families (`bind:`, `class:`, `style:`) follow the same shape with no parser change.
4. **MCP scope: doc-retrieval vs compile-and-validate?** Resolved: **compile-and-validate**. The tool is shipped (`vox_validate_file`); the work is exposing in-memory text-source validation (`vox_validate_source`) and surfacing the existing autofix suggestions in the result schema.
5. **vox-bench corpus from SvelteBench?** ~~Fork the MIT prompts?~~ **Deferred.** Without a defined gating decision the metric would inform, building and maintaining a benchmark suite is a science project. Track in case a future need (e.g., gating MENS model upgrades on continued Vox-fluency) surfaces.

## Sources

- [Introducing runes — Svelte blog](https://svelte.dev/blog/runes)
- [`$derived` — Svelte Docs](https://svelte.dev/docs/svelte/$derived)
- [`$effect` — Svelte Docs](https://svelte.dev/docs/svelte/$effect)
- [`{#snippet …}` — Svelte Docs](https://svelte.dev/docs/svelte/snippet)
- [`{@render …}` — Svelte Docs](https://svelte.dev/docs/svelte/@render)
- [`class` directive — Svelte Docs](https://svelte.dev/docs/svelte/class)
- [Svelte MCP overview — Svelte Docs](https://svelte.dev/docs/mcp)
- [Svelte 5 migration guide](https://svelte.dev/docs/svelte/v5-migration-guide)
- [Universal reactivity tutorial](https://svelte.dev/tutorial/svelte/universal-reactivity)
- [What's new in Svelte: May 2026](https://svelte.dev/blog/whats-new-in-svelte-may-2026)
- [Snippets in Svelte 5 — Frontend Masters](https://frontendmasters.com/blog/snippets-in-svelte-5/)
- [Svelte 5 Runes: A Practical Guide 2026 — PkgPulse](https://www.pkgpulse.com/guides/svelte-5-runes-complete-guide-2026)
- [Svelte 5 Runes in 2026: How They Work — PkgPulse](https://www.pkgpulse.com/blog/svelte-5-runes-complete-guide-2026)
- [Svelte Best Practices in 2026 — onehorizon.ai](https://onehorizon.ai/blog/svelte-best-practices-in-2026-scaling-with-runes-snippets-and-pure-reactivity)
- [Svelte 6 in 2026 — Tent of Tech](https://tentoftech.com/blog/svelte-6-in-2026-why-its-the-perfect-ui-framework-for-the-ai-era/)
- [SolidJS vs Svelte 5 vs React: Reactivity 2026 — PkgPulse](https://www.pkgpulse.com/guides/solidjs-vs-svelte-5-vs-react-reactivity-2026)
- [React 19 Compiler vs Svelte 5 — SitePoint](https://www.sitepoint.com/react-19-compiler-vs-svelte-5-virtual-dom-latency-benchmark/)
- [SvelteKit vs Next.js in 2026 — DEV (paulthedev)](https://dev.to/paulthedev/sveltekit-vs-nextjs-in-2026-why-the-underdog-is-winning-a-developers-deep-dive-155b)
- [SvelteKit vs Next.js 2026 — PkgPulse](https://www.pkgpulse.com/blog/sveltekit-vs-nextjs-2026-full-stack-comparison)
- [Next.js 16 vs TanStack Start for E-commerce — Crystallize](https://crystallize.com/blog/next-vs-tanstack-start)
- [TanStack Start vs Next.js — TanStack Docs](https://tanstack.com/start/latest/docs/framework/react/start-vs-nextjs)
- [TanStack Start v1.0 — byteiota](https://byteiota.com/tanstack-start-v1-0-type-safe-react-framework-2026/)
- [TanStack Router vs React Router v7 — PkgPulse](https://www.pkgpulse.com/blog/tanstack-router-vs-react-router-v7-2026)
- [Next.js 16 release notes](https://nextjs.org/blog/next-16)
- [Next.js 16 Cache Components + next-intl — Aurora Scharff](https://aurorascharff.no/posts/implementing-nextjs-16-use-cache-with-next-intl-internationalization/)
- [`use cache` directive — Next.js Docs](https://nextjs.org/docs/app/api-reference/directives/use-cache)
- [Better AI LLM assistance for Svelte 5 — Stanislav Khromov](https://khromov.se/getting-better-ai-llm-assistance-for-svelte-5-and-sveltekit/)
- [svelte-llm — distilled docs](https://svelte-llm.stanislav.garden/)
- [SvelteBench — khromov/svelte-bench](https://github.com/khromov/svelte-bench)
- [Svelte llms.txt](https://svelte.dev/docs/ai/overview/llms.txt)
- [Beyond Functional Correctness: Hallucinations in LLM Code — arXiv 2404.00971](https://arxiv.org/abs/2404.00971)
- [react-svelte — Rich-Harris](https://github.com/Rich-Harris/react-svelte)
- [svelte-preprocess-react — bfanger](https://github.com/bfanger/svelte-preprocess-react)
- [Sveltris](https://sveltris.vercel.app/)
- [Combining React and Svelte 5 — Bob Fanger / Medium](https://bfanger.medium.com/combining-react-and-svelte-in-a-single-app-interop-6f78aed96ce2)
