---
title: "Svelte-Mineable Features Implementation Plan (2026)"
description: "Phased implementation plan for the Svelte 5/6-inspired Vox compiler and MCP improvements identified in the comparative research. Concrete file changes, scope estimates, ADR triggers, test strategies, and dependency ordering. Built on what Vox already ships, not greenfield."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-05-02"
training_eligible: true
training_rationale: "Implementation roadmap for compiler-level GUI debugging and AI codegen improvements; references concrete file paths a downstream session must touch."
---

# Svelte-Mineable Features Implementation Plan (2026)

Companion to [Svelte 5/6 vs React Meta-Frameworks Research (2026)](svelte-vs-react-frameworks-research-2026.md). The research identifies seven items (M1–M7) worth porting from Svelte. This document is the concrete plan to land them.

## Critical alignment note (added 2026-05-02 after verification round 2)

**Angle-bracket JSX source is retired in favor of view-call syntax and typed semantic primitives.** Parser entry now lowers `Tag(named=props) { ... }` / `Tag(named=props)` via [pratt_match.rs](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs), while angle-bracket forms are no longer a parser entry. The replacement direction is still **TASK-6.1 — Vox GUI semantic primitive set** in the [GUI-native roadmap](vox-gui-native-roadmap-2026.md): typed primitives (`stack`, `row`, `column`, `text`, `button`, `field`, `panel`, `card`, `list`, `route_outlet`, …) with **fixed prop signatures** that emit canonical HTML + Tailwind. The Web IR primitives module is scaffolded ([web_ir/primitives/mod.rs](../../../crates/vox-codegen/src/web_ir/primitives/mod.rs)) but per-primitive files have not yet been created. Per the doc: "Each primitive has a fixed prop signature (no prop extension, period). Declares which HTML tag it emits. Declares its accessibility affordances. Accepts typed token refs for visual properties."

**Implications for the phases below:**

- **Phase B (`bind:` / `class:` / `style:` directive families) is partially obsoleted.** `bind:value` should be a prop convention on the `field` primitive (typed against the field's value type), not a JSX directive. `class:NAME` is not coherent against fixed-prop-signature primitives — replace with `surface={cond ? "primary" : "default"}` on the primitive. `style:--TOKEN` is already covered by the primitives' typed token-ref props (`gap`, `size`, `weight`, `surface`). **Recommendation: rescope Phase B as "binding/state-flow conventions on the Phase-6 primitives" and execute it as part of Phase 6, not as standalone JSX work.**
- **Phase F (typed fragments) should wait for Phase 6 primitives to stabilize.** Designing fragment composition against the deprecated JSX authoring surface would create a second migration. Defer until the 10 highest-usage primitives ship.
- **Phases A, C, D, E, G are unaffected** — none touches the JSX-vs-primitives surface.

This note supersedes the earlier "Phase B is small" framing.

## Premise

Vox already ships more of the Svelte-style surface than the original research draft credited:

- `state` / `derived` / `effect` / `on mount` / `on cleanup` / `view:` are working reactive primitives **inside `component { }` blocks** ([reactive_counter.vox](../../../examples/golden/reactive_counter.vox), [codegen_ts/reactive.rs:740–815](../../../crates/vox-codegen/src/codegen_ts/reactive.rs)).
- Auto-dep inference for `derived`/`effect` already runs ([hir_emit/state_deps.rs](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs)) — emits the React deps array automatically. It descends into lambdas; it does not cross function-decl boundaries.
- `on:click`, `on:change`, `on:input`, `on:submit`, `on:keydown` directive-style attributes ship today ([compat.rs:27–42](../../../crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs)). The parser path that lowers view-calls already accepts arbitrary `name:suffix` and `name-suffix` attribute names ([pratt_match.rs](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs)).
- A `vox_validate_file` MCP tool that runs the full compiler pipeline is registered ([tool-registry.canonical.yaml:1211](../../../contracts/mcp/tool-registry.canonical.yaml)) and dispatched ([mcp_tools/dispatch.rs:237](../../../crates/vox-orchestrator-mcp/src/dispatch.rs)).
- A `state_machine` keyword emits typed states + events + a reducer function stub ([state_machine_emit.rs](../../../crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)).
- Web IR validates literal `href`/`to` attributes against declared route patterns ([web_ir/validate.rs](../../../crates/vox-codegen/src/web_ir/validate.rs)).

Each phase below is sized against this real baseline, not a clean slate. Phase work should land via the same TDD/review discipline used for the GUI roadmap ([gui-native-roadmap-status-2026.md](gui-native-roadmap-status-2026.md)).

## Sequencing summary

```
Phase A (MCP polish) ──┐
Phase B (directives)  ──┼──► Phase D (.vox.ui modules) ──► Phase G (reactive-class SM)
Phase C (route flow)  ──┤
Phase E (auto-deps)   ──┘
                          Phase F (typed fragments) — independent, largest scope
```

| Phase | Item | Est. scope | Depends on | Needs ADR? |
|---|---|---|---|---|
| A | M5 — `vox_validate_source` + autofix surfacing | **✅ Code + docs complete** ([8ae08edab](https://github.com/vox-foundation/vox)). `DiagnosticInfo.{code,fixes}` plumbed end-to-end; new `vox_validate_source` tool in registry/dispatch/http_gateway. User-facing how-to at [how-to-mcp-vox-validate.md](../how-to/how-to-mcp-vox-validate.md) ([423a44e7e](https://github.com/vox-foundation/vox)) covers Claude Desktop / Cursor wiring, diagnostic shape, the iterate-against-the-compiler loop, and the diagnostic-code namespaces. | — | No |
| B | M3 — Binding/state-flow conventions on **Phase-6 primitives** (rescoped) | Folds into Phase 6 (`stack`, `field`, `button`, …). Do not pursue as standalone JSX directive work | TASK-6.1 | Coordinated with Phase-6 ADR |
| C | M6 — Route segment-aware overlap + typed `href` | **✅ Foundation + integration shipped** ([e0dd30fb0](https://github.com/vox-foundation/vox), [5be659ed1](https://github.com/vox-foundation/vox)). `RoutePattern::parse` + `Overlap::{None,Shadowed,Ambiguous}` + `validate_express_route_emit_input` upgraded to segment-aware. **✅ Typed `routePath` builder shipped** ([latest](https://github.com/vox-foundation/vox)). `KnownRoute` union plus a `routePath` builder map for parameterized paths (for example `/users/:id`) in `routes.manifest.ts`. **`url`-decl ↔ routes cross-validation deferred** — slice A bypassed the need by generating builders directly from route patterns; explicit `url` decls remain a general-purpose typed-URL construct independent of `routes { }`. | — | No |
| D | M1+M4 — `.vox.ui` modules (reactive members at module scope) | **✅ Shipped end-to-end.** FileKind helper + `parse_with_kind` ([954ad8775](https://github.com/vox-foundation/vox)); `Decl::ReactiveModule` AST + parser ([26c90f9be](https://github.com/vox-foundation/vox)); `HirReactiveModule` + lowering + `reactive_module_emit.rs` codegen emits `<Name>Provider.tsx` (typed `Value` interface + Context + Provider component + `use<Name>()` hook). Module name PascalCases from snake/kebab; falls back to `ReactiveModule<index>` when parser leaves it empty. CLI wire-up to pass file basename through is the remaining sub-slice. | — | **✅ ADR-032 accepted** |
| E | M1 — Cross-call auto-dep inference + `@reactive` decorator | **✅ Shipped end-to-end** ([5db51fc0a](https://github.com/vox-foundation/vox), [a1c6f46ec](https://github.com/vox-foundation/vox), [6691bf831](https://github.com/vox-foundation/vox)). `@reactive` decorator parses + lowers; analyzer recurses one level into `@reactive` callees with a self-reference guard; reactive emit uses the analyzer; tier-2 `dep_inference.over_track` hint surface emits a `// dep_inference.over_track` comment in TSX naming unannotated callees. | — | No |
| F | M2 — Typed parametric fragment primitive | **✅ Shipped end-to-end.** Lexer `Token::Fragment` + AST + parser dispatch ([6f01b8ae1](https://github.com/vox-foundation/vox)); HIR `HirFragmentDecl` + lowering + `fragment_emit.rs` codegen ships `fragments.tsx` with typed React function components and `Args` prop interfaces. Phase 6 primitive surface unblocked the codegen mid-cycle. | TASK-6.1 + ADR-033 | **✅ ADR-033 accepted** |
| G | M7 — Reactive-class state-machine instances | **✅ Shipped end-to-end.** Runtime `ReactiveStateMachine<S, E>` helper in `vox-actor-runtime/src/state_machine.rs` (Rust-side instance; 5 unit tests). Codegen `state_machine_emit.rs` now emits a `use<Name>StateMachine(initial)` React hook alongside the reducer, returning `{ state, send }`. The hook owns `useState` of the current state and a memoized `send(event)` running the reducer. | D | No |

**Shipped this session (2026-05-02 → 2026-05-03):** Phases A, C foundation + integration + typed-builder, E foundation + wiring + tier-2. Plus housekeeping: ADR-032 drafted ([0fcb7340d](https://github.com/vox-foundation/vox), [4df880e7c](https://github.com/vox-foundation/vox), [e45629d8f](https://github.com/vox-foundation/vox), [277942a63](https://github.com/vox-foundation/vox), [f0a3d1b75](https://github.com/vox-foundation/vox)) — orphan-snapshot cleanup, two stale-test fixes (ADR-028 / TASK-2.6 alignment), inbound-link goldens fix, `view_roots`-setup gap fix, contributor doc note on snapshot-baseline workflow. Full vox-compiler test suite went from 7 known failures → 0.

Phases A, B, C, D, E, F are independent and can land in parallel by separate sessions. G depends on D.

---

## Phase A — `vox_validate_source` MCP tool + autofix surfacing (M5)

**Why first:** the capability is shipped; the AI-usable contract is not. Three days of work on the response shape and one new sibling tool unlock every other phase's testability via "ask the AI to write Vox; let the AI iterate against the compiler."

**Goal.** Make the compile-and-validate loop usable from any MCP-aware coding agent without the agent first writing files to disk.

### Concrete changes

1. **New tool `vox_validate_source`** that takes `{ source: string, virtual_path?: string }` and returns the same diagnostic shape as `vox_validate_file` without touching the filesystem.
   - Add registry entry: `contracts/mcp/tool-registry.canonical.yaml` (alongside `vox_validate_file` at line 1211).
   - Add dispatch arm: [crates/vox-orchestrator-mcp/src/dispatch.rs:237](../../../crates/vox-orchestrator-mcp/src/dispatch.rs) (sibling to `vox_validate_file`).
   - Add input schema: [crates/vox-orchestrator-mcp/src/input_schemas.rs](../../../crates/vox-orchestrator-mcp/src/input_schemas.rs) (alongside the existing `vox_validate_file` schema).
   - Wire through `http_gateway`: [crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs:57](../../../crates/vox-orchestrator-mcp/src/http_gateway/mod.rs).
   - Implementation: invoke the same observer path used by `vox_validate_file`, but feed it source bytes via a temp `Cursor` or in-memory virtual filesystem rather than `resolve_existing_path_in_repository`.

2. **Structured autofix in the diagnostic response.** Today, `validate_file` ([code_validator.rs:63–88](../../../crates/vox-orchestrator/src/mcp_tools/code_validator.rs)) returns full LSP-style `DiagnosticInfo { severity, message, source, start_line, start_col, end_line, end_col }` from `vox_lsp::validate_document_with_hir`. The struct has **no `fix` field** — the existing autofix `FixSuggestion`s ([typeck/autofix.rs](../../../crates/vox-compiler/src/typeck/autofix.rs)) never reach the MCP boundary. Add a `fixes: Vec<FixInfo>` field to `DiagnosticInfo` (in [params.rs](../../../crates/vox-orchestrator/src/mcp_tools/params.rs)) and thread the autofix list from `vox_lsp` through the conversion. Each `FixInfo` carries `{ message, range: { start, end }, replacement_text, explanation }`. Note: also check `vox_check` ([code_validator.rs:92](../../../crates/vox-orchestrator/src/mcp_tools/code_validator.rs:92)) — it's the second validation entry point and needs the same field added for consistency.

3. **Documentation.** New how-to at `docs/src/how-to/how-to-mcp-vox-validate.md`:
   - "Point your coding agent at the Vox MCP server."
   - Concrete `claude_desktop_config.json` and Cursor MCP config snippets that wire `vox` → stdio MCP server → `vox_validate_source` / `vox_validate_file` / `vox_compiler::ast_inspect`.
   - Worked example: agent writes a buggy Vox component, calls `vox_validate_source`, receives a diagnostic with autofix, applies it, re-validates.

4. **Update `llms.txt`.** Add a section in [docs/src/.well-known/llms.txt](../.well-known/llms.txt) pointing AI clients at the MCP-server how-to.

### Verification

- New unit test in [mcp_tools/dispatch.rs](../../../crates/vox-orchestrator-mcp/src/dispatch.rs) tests for `vox_validate_source`: pass a source string with a known-error idiom (e.g., `<img>` without `alt`), assert the response includes the `web_ir_validate.a11y.img.missing_alt` diagnostic and at least one autofix suggestion.
- Integration test mirrors a real MCP client roundtrip via the stdio server.
- Add to [http_gateway_tests.rs](../../../crates/vox-orchestrator-mcp/src/http_gateway_tests.rs).

### Out of scope for this phase

- vox-bench corpus / scoring harness. Deferred per research §M5.
- Web UI for inspecting diagnostics. Use the existing dashboard if needed.

---

## Phase B — `bind:`, `class:`, `style:` directive families (M3)

**Why second:** the syntactic infrastructure is in place. The parser already accepts `name:suffix` attribute names. The mapping table and lowering logic at [compat.rs:27–42](../../../crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs) is the only place new families need to be wired. No grammar change.

**Decision recorded:** **separator is colon `:`**, identical to the established `on:*` family. Consistent, parser-free, no syntactic carve-out needed.

### Concrete changes

1. **Extend [`hir_emit/compat.rs`](../../../crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs) `map_jsx_attr_name`** with new directive families:
   - `bind:value` — lowers to `value={x}` + auto-emitted `onChange={e => set_x(e.target.value)}` when `x` is a reactive `state` binding. The setter name `set_x` is already the convention emitted by [reactive.rs:761](../../../crates/vox-codegen/src/codegen_ts/reactive.rs:761) (`const [{}, set_{}] = useState({});`).
   - `bind:checked` — same pattern with `e.target.checked`.
   - `bind:group` — emits a coordinated set of controlled inputs (radio group). Lower priority; gate behind a feature flag if it adds scope.
   - `class:NAME` — the directive after the colon is the class name; lowers to `className={clsx(existing, { NAME: value })}` (or simpler ternary if no other `className`). Requires a small clsx-style runtime helper or inline ternary composition.
   - `style:--TOKEN` and `style:PROP` — lowers to `style={{ '--TOKEN': value }}` / `style={{ propCamelCase: value }}` merged with any existing `style` attribute.

2. **New diagnostic codes** for misuse:
   - `directive.bind.target_not_state` — `bind:value={x}` where `x` is not a reactive `state` binding. Autofix: suggest converting to a one-way `value={x}` with separate handler, or wrapping `x` as state.
   - `directive.bind.type_mismatch` — `bind:value` on `<input type="number">` requires a numeric `state`. Reuse the existing typeck infrastructure.
   - `directive.class.empty_name` — `class:={…}` (missing class name).
   - `directive.style.unknown_property` — `style:bogus={…}` not a known CSS property and not a CSS custom property.

3. **HIR pass.** When the JSX attribute name has a recognized directive prefix, lower it to a structured `HirDirective { family, name, value }` rather than passing the raw string through. This unblocks the typeck checks above.

4. **Web IR alignment.** [web_ir/lower.rs](../../../crates/vox-codegen/src/web_ir/) maps attributes to behavior nodes; ensure directive-attributed elements land in the right behavior bucket.

5. **Update [examples/golden/reactive_counter.vox](../../../examples/golden/reactive_counter.vox)** to demonstrate `bind:value` for a controlled input alongside the existing `on:click` use, and add a new golden `examples/golden/form_directives.vox` covering all five families.

6. **Doctest fences** in `docs/src/tutorials/` covering each directive (auto-validated by the doc pipeline; no `// vox:skip`).

### Verification

- Compiler unit tests in `crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs#tests` for each new directive's TSX output (snapshot tests).
- Web IR validation tests for the new diagnostic codes.
- Reactive smoke test ([crates/vox-compiler/tests/reactive_smoke_test.rs](../../../crates/vox-compiler/tests/reactive_smoke_test.rs)) extended to cover `bind:value` round-trip.
- New golden file passes `vox check` and produces stable TSX.

### Out of scope

- A `use:action` directive (Svelte's "actions"). Park; no clear use case yet.
- Two-way binding for component-prop pass-through (`bind:prop` on a component). Decide separately when needed.

---

## Phase C — Route segment-aware overlap detection + typed `href` helper (M6)

**Why parallel-able:** independent of the reactive surface. Touches only `routes`-related code paths.

### Concrete changes

1. **Replace exact-string-match conflict detection** at [codegen_ts/routes.rs:87](../../../crates/vox-codegen/src/codegen_ts/routes.rs):
   - Introduce a `RoutePattern` type that parses `/users/:id` into segment kinds (`Literal("users")`, `Param("id")`, `Wildcard`).
   - Conflict rule: two routes of the same method conflict if their segment lists unify under any concrete substitution (literal-vs-literal must match; literal-vs-param wins specificity to literal; param-vs-param ambiguous → conflict diagnostic with documented precedence rule).
   - Emit diagnostic code `routes.overlap.unresolvable_precedence` for ambiguous overlap (e.g., two `/:a/:b` routes); allow `routes.overlap.shadowed` (info-level) when one literal route shadows a param route.

2. **Typed `href` helper.** Generate a `routes` module in TSX emit that exports a typed builder:
   ```text
   // emitted TSX (illustrative)
   export const routes = {
     users: { show: (id: string) => `/users/${id}` },
     // …
   } as const;
   ```
   Authors can then write `<a href={routes.users.show(userId)}>` and get full type-flow. Requires extending [route_manifest.rs](../../../crates/vox-codegen/src/codegen_ts/route_manifest.rs) emit.

3. **Loosen the broken-link validator** at [web_ir/validate.rs](../../../crates/vox-codegen/src/web_ir/validate.rs) to accept dynamic expressions whose static type resolves to a known route-builder return value. Hand-written literal `href="/whatever"` continues to be checked exactly.

4. **Reuse the route-pattern parser** for the path-param decorators in [phase3-http-ergonomics-spec-2026.md](phase3-http-ergonomics-spec-2026.md) — there is shared scaffolding to extract here.

### Verification

- Unit tests for `RoutePattern::parse`, `RoutePattern::overlaps_with`.
- Goldens covering the three overlap cases (no overlap, shadowed, ambiguous).
- TSX emit snapshot for the generated `routes` builder.

### Out of scope

- Search-param typing (TanStack Router's `validateSearch`). Decide separately.
- Loader-result type-flow into component props (Phase 4 tail).

---

## Phase D — `.vox.ui` reactive modules (M1 module-scope half + M4)

**Why this needs an ADR.** A new file-suffix convention is grammar-adjacent policy. Decide once, document the SSOT, and don't churn it. ADR scope: the `.vox.ui` suffix, what's allowed at module scope, how cross-module reactive imports lower in TSX emit.

### Concrete changes

1. **ADR draft** at `docs/src/adr/032-vox-ui-reactive-modules.md`:
   - Suffix: `.vox.ui` (matches the precedent of suffixed file conventions established by `.generated.md` and `.voxignore`-derived files in [AGENTS.md §Auto-generated docs](../../../AGENTS.md)).
   - Allowed top-level decls in a `.vox.ui` file: regular Vox decls (`type`, `fn`, `component`) **plus** module-scope `state` / `derived` / `effect` / `on mount` / `on cleanup`.
   - Disallowed in a regular `.vox` file: module-scope reactive members (existing behavior, made explicit).
   - Lowering: each `.vox.ui` file emits a TSX module exporting a React context + provider + `use<Name>()` hook.

2. **Parser change** at [parser/descent/mod.rs](../../../crates/vox-compiler/src/parser/descent/mod.rs): when the input file's path ends in `.vox.ui`, allow the existing `Token::State`/`Token::Derived`/`Token::Effect`/`Token::On + Mount/Cleanup` branches at the top-level decl matcher (currently only legal inside `finish_reactive_component_after_name`).

3. **New AST/HIR node `ReactiveModule`** at [ast/decl/ui.rs](../../../crates/vox-compiler/src/ast/decl/ui.rs) — wraps the same `Vec<ReactiveMemberDecl>` already used by `ReactiveComponentDecl`.

4. **Codegen** at [codegen_ts/reactive.rs](../../../crates/vox-codegen/src/codegen_ts/reactive.rs): mirror the `ReactiveComponentDecl` lowering but emit a context+provider+hook scaffold instead of a function component.

5. **Cross-module reactive imports.** A regular `.vox` `component { }` that imports `count` from `./counter.vox.ui` should have the import auto-rewritten in TSX emit to a `useCounterStore()` call. Read-tracking analysis (Phase E below) needs to know that the imported binding is reactive.

6. **Two new goldens:** `examples/golden/counter.vox.ui` (the store) and `examples/golden/counter_consumer.vox` (the component). Add to the doc-pipeline doctest suite.

### Verification

- Parser tests for both legal (in `.vox.ui`) and illegal (in `.vox`) module-scope reactive members.
- Codegen snapshot tests for the emitted provider/hook shape.
- End-to-end golden round-trip.

### Out of scope

- Persistence / hydration (don't smuggle in a state-management library). Pure in-memory reactive store only.
- Server-side rendering coordination across multiple `.vox.ui` modules. Decide separately if/when SSR comes up for the reactive surface.

---

## Phase E — Cross-call auto-dep inference (M1 closure-tracking half)

**User direction:** track reads through closures, but cost-bound the analysis (no whole-program escape analysis). Conservative over-tracking is acceptable when proving reachability is expensive.

### Cost model

Three tiers, each with a bounded analysis budget:

| Tier | Tracks | Analysis cost | Action when over budget |
|---|---|---|---|
| 1 | Direct identifier reads in the same expression tree | O(nodes) | (always run) |
| 2 | Reads inside `HirExpr::Lambda` bodies whose closure escapes to an effect/derived | O(nodes × lambda depth) | already implemented at [state_deps.rs:95](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs:95); extend to track closure-captured reactive bindings |
| 3 | Reads through `HirExpr::Call` to a free function declared in the same module | O(callees × analysis budget) | gate on `@reactive`-annotated callees only; otherwise emit `dep_inference.over_track` info diagnostic and add the conservative "everything in scope" dep set |

Whole-program / cross-crate / dynamic-dispatch analysis: explicitly **not** implemented. Authors get a clean opt-in (`@reactive fn compute(x: int) to int { … }`) for free functions that should participate in dep tracking; without the annotation, the call site over-tracks (correct but pessimistic).

### Concrete changes

1. **Extend [`hir_emit/state_deps.rs`](../../../crates/vox-codegen/src/codegen_ts/hir_emit/state_deps.rs)** with a `ReadAnalyzer` struct that:
   - Carries a budget (default 100 expression nodes per analysis call; configurable via env or `Vox.toml`).
   - Tracks visited callees in a `HashSet<DeclId>` to bound recursion.
   - When budget is exhausted, returns a `DepSet::Conservative` variant that emits the "over-tracked" diagnostic and falls back to "every reactive binding in scope."

2. **`@reactive` decorator on free `fn` declarations.** Per [AGENTS.md §Grammar Unification](../../../AGENTS.md), this is a decorator on `fn` — fits the existing decorator pattern, no new bare keyword. Marks the function body as eligible for cross-call dep inference; the analyzer descends into the function's HIR body.

3. **Lowering preserved.** The emitted React `useMemo([…])` / `useEffect([…])` deps array remains the existing one when analysis succeeds. When `DepSet::Conservative` triggers, emit a comment in the TSX explaining the over-track (helps debuggers understand why the dep array is large).

4. **Diagnostic codes:**
   - `dep_inference.over_track` (info) — analysis exhausted budget; conservative dep set used.
   - `dep_inference.unannotated_call` (info, opt-in via lint level) — `derived`/`effect` calls a non-`@reactive` free function; the read may be missed. Suggest annotating the callee.

### Verification

- Unit tests for each tier in `state_deps.rs#tests`.
- Snapshot test: a `derived label = format(count)` where `format` is `@reactive fn format(c: int) to str { "v=" + str(c) }` produces deps `[count]`.
- Snapshot test: same code with no `@reactive` annotation produces `DepSet::Conservative` and the info diagnostic.

### Out of scope (the "may-being" the user asked us to be conservative on)

- **Speculative escape analysis** (proving a closure does or does not escape). Don't.
- **Effect-typed analysis** (Koka/Eff-style row-polymorphic effect tracking). Don't.
- **Cross-crate tracking.** Don't.
- **Dynamic-dispatch resolution.** Don't.

If a future need surfaces (e.g., MENS-spoke wants per-call effect inference for a specific reason), revisit then with a concrete consumer.

---

## Phase F — Typed parametric fragment primitive (M2)

**Largest scope.** New bare-keyword scope per the grammar policy → new lexer token, new parser production, new AST/HIR node, new codegen, new validation. Needs an ADR.

### ADR scope

`docs/src/adr/033-typed-fragment-primitive.md`:
- Keyword choice (recommend `fragment`, alternatives `block` / `slot` / `template` discussed and rejected with rationale).
- Type system: `Fragment[(T1, T2, …)]` as the type of a parametric fragment. Empty arg list valid.
- Render syntax: decide between `<RenderFragment of={Row} args={[item]} />` (JSX-shaped) or `@render Row(item)` (decorator-shaped). Recommend the former for consistency with existing JSX, but note both have proponents.
- Lowering policy: emit fragments as typed React function components with a `Fragment.children` prop convention.

### Concrete changes (post-ADR)

1. New lexer token `Token::Fragment` at [lexer/token.rs](../../../crates/vox-compiler/src/lexer/token.rs).
2. New AST node `FragmentDecl` at [ast/decl/](../../../crates/vox-compiler/src/ast/decl/).
3. New parser production at [parser/descent/decl/](../../../crates/vox-compiler/src/parser/descent/decl/).
4. New HIR node `HirFragmentDecl`.
5. New codegen at `crates/vox-codegen/src/codegen_ts/fragment_emit.rs`.
6. Web IR validation: ensure fragments referenced in JSX exist; ensure the right number/types of args are passed.
7. Goldens: at minimum `examples/golden/fragment_table_row.vox` (a `<Table>` parameterized by a row fragment).

### Verification

- ADR review and merge first.
- Each lexer/parser/AST/HIR/codegen layer gets unit tests.
- End-to-end golden round-trip with TSX snapshot.

### Out of scope

- Recursive fragments calling themselves (the Svelte case). Decide separately.
- Fragments exported from `.vox.ui` modules — Phase F lands fragments at module/component scope of regular `.vox` files; cross-module fragment export is a follow-up.

---

## Phase G — Reactive-class state-machine instances (M7)

**Depends on Phase D** if instances should be usable from `.vox.ui` modules without a component wrapper.

### Concrete changes

1. **Extend [state_machine_emit.rs](../../../crates/vox-codegen/src/codegen_ts/state_machine_emit.rs)** to additionally emit:
   - A `useFooStateMachine(initial: FooState): { state: FooState, send: (e: FooEvent) => void }` hook for in-component use — internally a `useState` + the existing reducer.
   - An exported reactive class `class Foo { state = $state(initial); send(e: FooEvent) { this.state = fooReducer(this.state, e); } }` for `.vox.ui`-module use (depends on Phase D).
2. **Stdlib helper** in `vox-stdlib` (or `vox-actor-runtime` if stdlib doesn't exist yet — verify against repo state at implementation time): a generic `<S, E>` reactive-state-machine wrapper that the codegen-emitted class extends. Avoids per-state-machine boilerplate.
3. **Update existing state-machine goldens** to use the new instance API. Keep the discriminated-union types and pure reducer function as the testable primitives.

### Verification

- Unit tests for the emitted hook (renders, dispatches an event, observes state change).
- Reactive-class goldens validated by the doc pipeline.

---

## Cross-cutting concerns

- **Doc-pipeline doctests.** All `.vox` snippets in this plan's downstream docs must compile cleanly per [AGENTS.md §Markdown Hygiene](../../../AGENTS.md). Use `// vox:skip` only when illustrating a deliberate error.
- **Goldens.** Each phase adds its own golden(s) under `examples/golden/`. Update the existing parity tests (Web IR vs legacy emit, G4) to cover the new emission paths.
- **Telemetry.** Emit `vox.compiler.directive.*`, `vox.compiler.dep_inference.*`, `vox.mcp.validate_source.*` events for observability of how the new surfaces are being used.
- **No new bare keywords without an ADR.** Phases D and F trigger ADRs (file-suffix convention; new bare-keyword scope). Phases A/B/C/E/G do not need ADRs because they extend existing surfaces.
- **Backward compatibility.** Every phase preserves the existing reactive-component, JSX-attribute, route-block, and state-machine surfaces unchanged. Only additive changes to the lowering layer.
- **Migration.** No migration commands needed — none of these are removals. The existing React-hook bridge ([react_bridge.rs](../../../crates/vox-compiler/src/react_bridge.rs)) stays as the escape hatch.

## Explicitly deferred

- **vox-bench / SvelteBench analog.** No defined consumer for the metric. Revisit when a specific gating decision (e.g., "should we upgrade the MENS spoke to model X") would be informed by the score.
- **`use:action` directive.** No clear use case. Revisit if a third-party Vox component library asks for it.
- **Two-way `bind:` on component props** (cross-component data flow). Out of Phase B; decide when a real consumer surfaces.
- **Whole-program / cross-crate dep inference.** Cost is not justified by current evidence.
- **Search-param typing in routes.** Out of Phase C; decide separately.
- **A first-party `import svelte …` form.** Per the [research doc §Interop position](svelte-vs-react-frameworks-research-2026.md), no.

## Cross-references

- [Svelte 5/6 vs React Meta-Frameworks Research (2026)](svelte-vs-react-frameworks-research-2026.md) — the source research and rationale.
- [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md) — the React-interop pipeline this plan does not touch.
- [Phase 3: HTTP Ergonomics Decorators Spec (2026)](phase3-http-ergonomics-spec-2026.md) — overlaps with Phase C on path-param typing.
- [Phase 5: Bidirectional Vox↔React Interop Spec (2026)](../archive/phase5-react-interop-spec-2026.md) — Phase F (fragments) interacts with React-component import surface; check before implementing.
- [Vox GUI-Native Language Roadmap (2026)](vox-gui-native-roadmap-2026.md) — the umbrella roadmap; slot the phases above into the appropriate roadmap phase.
- [GUI-Native Roadmap Execution Status (2026)](gui-native-roadmap-status-2026.md) — track per-phase commit references here.
- [AGENTS.md §Grammar Unification](../../../AGENTS.md) — grammar policy that gates Phases D and F.
- [Internal Web IR Implementation Blueprint](internal-web-ir-implementation-blueprint.md) — the IR layer that receives directive lowerings (Phase B) and fragment lowerings (Phase F).
