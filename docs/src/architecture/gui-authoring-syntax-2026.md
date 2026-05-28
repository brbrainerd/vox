---
title: "GUI Authoring Syntax (2026): Vox UI as Values (VUV)"
description: "Replaces JSX-shaped view bodies and Tailwind class strings with typed function-call views. No tags, no class strings, no CSS files in user code. Lowers to React/TSX/Tailwind unchanged."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-05-02"
training_eligible: true
training_rationale: "Records the canonical GUI authoring shape MENS should learn to emit. Replaces all JSX + Tailwind-string idioms in the corpus."
---

# GUI Authoring Syntax (2026): Vox UI as Values (VUV)

**Status:** **VUV-1 through VUV-6 implemented** (2026-05-08). VUV-7 in progress; VUV-8 in this commit. See [§Implementation Status](#implementation-status-2026-05-08) for per-phase landing details.
**Scope:** authoring surface only. Web IR ([`crates/vox-codegen/src/web_ir/mod.rs`](../../../crates/vox-codegen/src/web_ir/mod.rs)) and the TSX backend ([`crates/vox-codegen/src/web_ir/emit_tsx.rs`](../../../crates/vox-codegen/src/web_ir/emit_tsx.rs)) keep their contracts. This note changes how source lowers *into* `DomNode` and how style is expressed.

## Motivation

Vox is an **output language for large language models**. Every syntactic family the model has to learn — and every string-typed sub-language hidden inside the syntax — is a surface where the model gets things wrong. JSX in current `.vox` is the worst offender, but the angle brackets are not the deepest cost. The deepest cost is the *hosted sub-languages* JSX ferries:

| Hosted sub-language | Where it appears today | Validated by Vox? |
|---|---|---|
| Tailwind utility names | `class="text-xs font-bold …"` | No |
| Tailwind responsive/state prefixes | `md:`, `hover:`, `focus-within:` | No |
| CSS color literals | inline `style="color: #aaa"` | Phase 5 partial |
| Event-name conventions | `on:click` vs `onClick` vs `on_click` | Partial |
| `{expr}` mode-switch | every dynamic value in a tag | Yes (parser) |
| HTML tag names | `<row>`, `<panel>` | Phase 6 partial |
| ARIA / a11y attribute strings | `aria-label="…"` | Phase 6 partial |

A single `text` element in [`speak.vox:28`](../../../crates/vox-dashboard/app/src/surfaces/speak.vox) carries six independent Tailwind tokens jammed into one opaque string. Removing only the angle brackets would not address any of this.

VUV addresses the surface as a whole: **one syntax (function calls), one type system (Vox tokens), one validator (the compiler). No string-typed sub-languages.**

## The proposal in three rules

### Rule 1 — A view is an expression

Built with **ordinary function calls**. Named arguments are props. A trailing `{ … }` block is the call's children list, where each statement-position expression is one child. Same shape inside a `component`, in a top-level `let`, returned from a function, or passed as an argument. There is no "view mode."

```vox
// vox:skip
button(variant: primary, on_click: submit) {
    text("Send")
}
```

A call with no children is just a call. A call with no props is `name() { … }` — the parens stay required so the parser never has to guess whether a bare identifier followed by `{` is a call or a block.

### Rule 2 — No class strings. Style is typed named args drawing from the token registry

The Phase 4.4 token system (`contracts/tokens/tokens.v1.json`, validated by [`web_ir/validate.rs`](../../../crates/vox-codegen/src/web_ir/validate.rs)) already has typed colors, spacing, and surfaces. Today they are invisible at the authoring layer because users write Tailwind class strings *that happen to map to tokens*. VUV inverts this: **users write tokens directly; the compiler emits Tailwind / CSS / inline styles**.

Style axes become enumerated kwargs: `font`, `weight`, `case`, `color`, `bg`, `pad`, `pad_x`, `pad_y`, `gap`, `align`, `justify`, `radius`, `border`, `surface`, `max_w`, `min_h`, `leading`, `tracking`, `mb`, `mt`, …

Responsive and state variants are *also* typed kwargs, not string prefixes:

```vox
// vox:skip
text("Send", color: zinc.50, color_hover: blue.500, color_md: zinc.100)
```

The compiler decides whether the lowered output is a Tailwind class, a CSS variable, or an inline style. The user never types a class name.

**Escape hatch:** `raw_class("custom-thing")` and `raw_css { … }` exist for genuinely necessary escapes and emit a compiler warning. Same policy as the existing `raw_css` in the `style { }` block.

### Rule 3 — Behavior is typed kwargs

```vox
// vox:skip
button(on_click: submit, disabled: is_submitting) { text("Send") }
```

`on_click` is a `fn() -> Action`. `disabled` is a `bool`. Same naming convention everywhere; the compiler picks the React event name (`onClick`). No `on:click` / `onClick` / `on_click` decision for the author or the LLM.

## Before / after

Source: [`crates/vox-dashboard/app/src/tabs/speak.vox`](../../../crates/vox-dashboard/app/src/surfaces/speak.vox), `ChatMessage`.

**Today (JSX + Tailwind strings):**

```vox
// vox:skip
component ChatMessage(role: str, content: str) {
    view: (
        <row class={if role is "user" { "justify-end px-4 py-2" } else { "justify-start px-4 py-2" }}>
            <panel class={if role is "user" { "max-w-xl bg-blue-600/20 border border-blue-500/30 rounded-2xl rounded-br-sm px-4 py-3" } else { "max-w-2xl bg-white/5 border border-white/10 rounded-2xl rounded-bl-sm px-4 py-3" }}>
                <text class="text-xs font-bold text-zinc-400 uppercase tracking-widest mb-2">{role}</text>
                <text class="text-sm text-white/80 leading-relaxed">{content}</text>
            </panel>
        </row>
    )
}
```

9 string literals containing 31 Tailwind tokens the LLM must spell correctly.

**Proposed (VUV):**

```vox
// vox:skip
component ChatMessage(role: str, content: str) {
    let mine = role == "user"
    view: row(justify: if mine { end } else { start }, pad_x: 4, pad_y: 2) {
        panel(surface: if mine { chat.user } else { chat.assistant },
              max_w: xl, radius: 2xl, pad_x: 4, pad_y: 3) {
            text(role,    font: xs, weight: bold, color: zinc.400, case: upper, mb: 2)
            text(content, font: sm, color: white.80, leading: relaxed)
        }
    }
}
```

0 styling strings. Every axis is type-checked, contrast-validated, and refactorable. The user-content `role` and `content` remain the only strings.

## K-complexity ledger

Counting independent grammar rules and string-typed sub-languages a model must learn to write a view:

| Surface | JSX + Tailwind | VUV |
|---|---|---|
| Open/close matched tag pairs | yes | — |
| Self-closing tag slash (`/>`) | yes | — |
| Fragment shorthand (`<>…</>`) | yes | — |
| Tag-mode vs expression-mode switch | yes | — |
| Attribute vs prop name aliasing (`class`/`className`) | yes | — |
| `{expr}` child escape | yes | — |
| Tailwind utility name vocabulary | yes (~1000 tokens) | — |
| Tailwind responsive/state prefix grammar | yes | — |
| Event-name convention picking | yes | — |
| Inline CSS literals | yes | — |
| Named-argument call | already in Vox | already in Vox |
| Trailing `{…}` block as children list | — | **new** |
| Typed token vocabulary | partial (validators) | promoted to authoring layer |
| **Net new rules** | — | **1** |
| **Sub-languages retired** | — | **9** |

## Why this is React-friendlier, not React-hostile

- **`emit_tsx` is unchanged.** It walks `DomNode` and emits TSX. The lowering step gains a token-resolution phase that turns `color: zinc.400` into either `className="text-zinc-400"` or `style={{color: 'var(--zinc-400)'}}` — a local decision in [`web_ir/lower.rs`](../../../crates/vox-codegen/src/web_ir/lower.rs).
- **Calling existing React components stays a normal function call.** `react(SomeReactComponent, prop1: x, prop2: y) { children }` lowers to `<SomeReactComponent prop1={x} prop2={y}>{children}</SomeReactComponent>`. No special syntax.
- **Tailwind becomes a backend, not a surface.** You can swap to vanilla CSS, CSS modules, styled-components, or zero-runtime CSS-in-JS by changing the lowering, not the source.

## Implementation phasing

This is the phasing the codebase change must follow. Each phase is independently shippable, lands behind a flag where useful, and ends in a green test suite.

| Phase | Work | Surfaces touched | Approx. size | Gate |
|---|---|---|---|---|
| **VUV-1** Token vocabulary expansion | Add font sizes, weights, leading, tracking, justification, alignment, max-width scale, padding scale, radius scale, border presets, state-variant scaffolding to `contracts/tokens/tokens.v1.json` and `tokens/mod.rs`. Validator stays passing on existing inputs. | `contracts/tokens/`, `crates/vox-compiler/src/tokens/`, tests | medium | Existing dashboard still builds |
| **VUV-2** Trailing-block parser + AST | Add optional `children: Vec<Expr>` to `Expr::Call`. Parse trailing `{…}` after a call. Behind `VOX_VUV=1` until VUV-3 lands. | `crates/vox-compiler/src/parser/`, AST, parser tests | medium | New tests green; old grammar untouched when flag off |
| **VUV-3** Lowering: trailing-block-call → `DomNode::Element` | When call resolves to a UI primitive or `component`, lower to `DomNode::Element { tag, attrs, children }`. JSX path retained in parallel. | `crates/vox-codegen/src/web_ir/lower.rs`, integration tests | medium | One hand-written `.vox` view round-trips JSX→VUV with byte-identical TSX output |
| **VUV-4** Typed style kwargs | Recognize style axes (`font`, `color`, `pad`, …) on UI primitive calls; resolve to tokens; emit Tailwind classes via `tokens_emit`. Reject unknown style kwargs. `raw_class()` escape hatch. | `crates/vox-codegen/src/web_ir/lower.rs`, `crates/vox-codegen/src/codegen_ts/tokens_emit.rs`, validators | large | Hand-written sample component compiles to identical TSX as today |
| **VUV-5** Typed event handler kwargs | Normalize `on_click`, `on_change`, `on_submit`, … to React event names in lowering. Retire `on:click` JSX form. | lower.rs, emit_tsx, react_bridge | small | Dashboard event handlers all on the new shape |
| **VUV-6** Dashboard migration (cutover) | Rewrite `app.vox`, `tabs/forge.vox`, `tabs/speak.vox`, `tabs/command.vox`, `tabs/network.vox` to VUV. Delete the JSX path from the parser. Remove `VOX_VUV` flag. | dashboard `.vox`, parser cleanup | large | Dashboard renders identically; visual diff = 0 |
| **VUV-7** Golden corpus + MENS retraining | Rewrite `examples/golden/*.vox` and `crates/vox-compiler/tests/llm_fixtures/*.vox` UI fixtures. Retrain MENS on VUV-only corpus. | corpus, MENS pipeline | large | Eval scores ≥ pre-cutover baseline |
| **VUV-8** Doc + tutorial sweep | Update `gui-native-roadmap-status-2026.md` Phase 6 description, contributor docs, any `.vox` blocks in markdown. Run `vox-doc-pipeline`. | `docs/src/`, generated indices | small | `vox-doc-pipeline --check` green |

**Atomicity:** the JSX form and VUV form must not coexist in the corpus long-term — that confuses MENS. VUV-1 through VUV-5 land additively; VUV-6 is the atomic cutover; VUV-7/8 are mop-up.

## Implementation status (2026-05-08)

| Phase | Status | Notes |
|---|---|---|
| **VUV-1** Token vocabulary | ✅ Done | Phase 4.4 + Phase 6 (TASK-6.1/6.3) of the GUI-native roadmap. `web_ir/primitives/mod.rs` ships 14 primitives + `UNIVERSAL_STYLE_KWARGS`. |
| **VUV-2** Trailing-block parser | ✅ Done | [parser/descent/expr/pratt_match.rs](../../../crates/vox-compiler/src/parser/descent/expr/pratt_match.rs) lines ~262–308: `Ident(args) { children }` lowers to `Expr::Jsx`. Trigger: capitalized callee, recognized primitive, or HTML allowlist. |
| **VUV-3** Lowering trailing-block → DomNode | ✅ Done | View-call form lowers through `Expr::Jsx` → `web_ir::DomNode`. Same Web IR contract; emit_tsx unchanged. |
| **VUV-4** Typed style kwargs | ✅ Done | [web_ir/primitives/mod.rs](../../../crates/vox-codegen/src/web_ir/primitives/mod.rs) `UNIVERSAL_STYLE_KWARGS`. Style axes (`color`, `pad`, `gap`, …) lower to Tailwind via `tokens_emit`. `raw_class()` escape hatch preserved. |
| **VUV-5** Typed event handler kwargs | ✅ Done | [codegen_ts/hir_emit/compat.rs](../../../crates/vox-codegen/src/codegen_ts/hir_emit/compat.rs) `map_jsx_attr_name` normalizes `on_click`/`on:click` → `onClick`, etc. **No `.vox` source uses the colon form** — `on_click` is canonical; the `on:` aliases remain as compatibility for future Svelte-mineable directive families (`bind:`, `class:`, `style:`). |
| **VUV-6** Dashboard cutover | ✅ Done | Angle-bracket JSX parser entry retired (parser/descent/expr/mod.rs comment: "pratt_jsx retired"). Dashboard `.vox` files (`app.vox`, all 4 tabs) authored on the view-call form (TASK-7.1/7.2). `Expr::Jsx` AST node retained as internal sugar from view-calls — no longer parsed from `<>`. |
| **VUV-7** Golden corpus + MENS | 🟡 Partial | Corpus already migrated (TASK-8.1, commit `135b7591`). MENS retraining run pending operator action (TASK-8.2). |
| **VUV-8** Docs sweep | ✅ Done | This block. |
| **VUV-9** Naming policy + codemod | ✅ Done | Policy at [vuv-naming-policy-2026.md](vuv-naming-policy-2026.md); registry at `contracts/naming/renames.v1.json` (empty until first rename); `vox migrate names` codemod (token-based; preserves whitespace/comments/string-literal contents); `vox-arch-check` enforces that registry `from` entries are not still canonical primitives. |

**Companion cleanup (commit on the same branch):** removed 11 dead `Decl` variants (`Context`, `Hook`, `Provider`, `Layout`, `ErrorBoundary`, `NotFound`, `Trait`, `Impl`, `Mock`, `Fixture`, `Keyframes`) that the parser never produced. The retired-React-shapes group (`Context`/`Hook`/`Provider`/`Layout`/`ErrorBoundary`/`NotFound`) was the React-context surface VUV-6 supersedes. The non-UI group (`Trait`/`Impl`/`Mock`/`Fixture`/`Keyframes`) was vestigial AST sprawl from earlier prototypes; their structs and ~50 match arms across the workspace are gone.

## Open questions

These are the only design questions left before VUV-2 starts.

1. **Bare string `"hello"` in child position** — desugar to `text("hello")` or require explicit? **Recommendation:** require explicit. One rule, no sugar, LLM-friendlier.
2. **`if` / `for` / `match` in child position** — unrestricted, or only as expressions whose type is a child node? **Recommendation:** unrestricted, with the type checker enforcing child-type at the call boundary.
3. **Single-child sugar like `button(label: "Send")` ↔ `button() { text("Send") }`** — keep as parallel form? **Recommendation:** no. Consistency over brevity.
4. **Token namespacing** — flat (`zinc.400`, `xs`, `bold`) or grouped (`color: zinc.400`, `font.size: xs`, `font.weight: bold`)? **Recommendation:** flat per kwarg name, grouped under a kwarg only when the axis is genuinely 2-D (e.g. `pad: (x: 4, y: 2)`).

## Out of scope for this note

- Bidirectional React interop (covered by [`external-frontend-interop-plan-2026.md`](external-frontend-interop-plan-2026.md)).
- Whether `style { }` blocks change shape — they become token-registration sites, not rule-writing sites; tracked under VUV-1.
- Modifier-chain ergonomics (Compose-style `Modifier`) — explicitly rejected during the 2026-05-02 review.
