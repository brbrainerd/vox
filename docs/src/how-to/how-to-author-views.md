---
title: "How to author Vox views (VUV)"
description: "Practical guide to writing UI in Vox using the view-call (VUV) authoring syntax. Covers primitives, typed style kwargs, conditionals, components, and escape hatches."
category: "How-To Guides"
status: "current"
last_updated: "2026-05-03"
training_eligible: true
training_rationale: "Canonical authoring guide for VUV — what MENS should learn to emit."
---

# How to author Vox views (VUV)

Vox UI is built with **ordinary function calls**. There is no JSX, no class strings, and no separate styling DSL — every piece of a view is a typed Vox expression.

This guide is the practical companion to [`gui-authoring-syntax-2026.md`](../architecture/gui-authoring-syntax-2026.md), which is the design rationale.

## The three rules

1. **A view is an expression.** Built with function calls. Same syntax inside a `component`, in a top-level `let`, returned from a function.
2. **A trailing `{ … }` block is the call's children.** Each statement-position expression in the block is one child.
3. **Style is typed named arguments**, not class strings. The compiler emits Tailwind/CSS.

## Your first view

```vox
// vox:skip
component Greeting(name: str) {
    view: column(pad=4, gap=2) {
        text(size="lg", weight="bold") { "Hello, " }
        text() { name }
    }
}
```

What's happening:

- `column(pad=4, gap=2)` is a primitive. It lowers to `<div class="flex flex-col p-4 gap-2">`.
- Children inside the `{ … }` block: two `text` elements.
- `text(size="lg", weight="bold")` adds `text-lg font-bold` to the emitted class list.
- `name` (a bare identifier) is one child — the runtime value of the parameter.

## Self-closing form

A view-call without children doesn't need an empty block. Three trigger forms all sugar to a self-closing element:

```vox
// vox:skip
ComposerPanel()                      // capitalized — always a view-call
panel(w=2, h=2, bg="zinc.600")       // recognized primitive — always a view-call
input(attr_type="checkbox")          // lowercase + named-only args — view-call
my_func()                            // bare lowercase, no args — regular function call
Some(x)                              // capitalized + positional arg — enum constructor (regular call)
```

## The primitive set

These are recognized as UI primitives by the lowering layer. They emit a fixed HTML tag plus a base Tailwind class list, then accept your typed kwargs.

| Primitive | HTML | Notes |
|---|---|---|
| `stack`, `column` | `<div>` | flex, flex-col |
| `row` | `<div>` | flex, flex-row |
| `wrap` | `<div>` | flex, flex-wrap |
| `text` | `<p>` | accepts `size`, `weight` |
| `heading` | `<h1>`–`<h6>` | accepts `level` (1–6), `size`, `weight` |
| `link` | `<a>` | underline-on-hover styling |
| `image` | `<img>` | `src` and `alt` pass through as HTML attrs |
| `button` | `<button>` | accepts `variant: "default"|"outline"|"ghost"|"destructive"`, `size: "sm"|"lg"|"icon"` |
| `panel`, `card` | `<div>` | accept `surface` for token pairs |
| `list`, `list_item` | `<ul>` / `<li>` | |
| `route_outlet` | `<div>` | |
| `overlay`, `toast`, `drawer`, `modal` | `<div>` | accept `position`, `z` |

For anything not in the primitive set (raw HTML elements like `input`, `select`, `textarea`), the lowercase + named-args rule kicks in and the tag passes through verbatim.

## Universal style kwargs

Any primitive accepts these kwargs. Values resolve to Tailwind classes via a typed table; conflicts with primitive defaults on the same axis are auto-suppressed.

| Kwarg(s) | Tailwind |
|---|---|
| `pad`, `pad_x`, `pad_y`, `pad_t`, `pad_b`, `pad_l`, `pad_r` | `p-N`, `px-N`, … |
| `mb`, `mt`, `ml`, `mr`, `mx`, `my` | `mb-N`, … |
| `w`, `h`, `min_w`, `min_h`, `max_w`, `max_h` | `w-N`, `max-w-N`, … |
| `bg`, `color` | `bg-…`, `text-…` (token-shaped values like `zinc.400` auto-dash to `zinc-400`) |
| `border`, `border_x`/`y`/`t`/`b`/`l`/`r`, `border_color` | `border`, `border-x-N`, `border-{color}` |
| `radius`, `radius_t`/`b`/`l`/`r`/`tl`/`tr`/`bl`/`br` | `rounded`, `rounded-tl-N`, … |
| `overflow`, `overflow_x`, `overflow_y` | `overflow-…` |
| `flex`, `shrink`, `grow` | `flex-1`, `shrink-0`, … |
| `justify`, `items`, `gap`, `gap_x`, `gap_y` | `justify-X`, `items-X`, `gap-N` |
| `tracking`, `leading`, `case`, `italic`, `font_family` | `tracking-X`, `leading-X`, `uppercase`/`lowercase`, `italic`/`not-italic`, `font-mono`/`sans`/`serif` |
| `position`, `inset`, `top`, `bottom`, `left`, `right` | `absolute`/`relative`/`fixed`, `top-N`, … |
| `shadow`, `opacity` | `shadow`, `shadow-md`, `opacity-50` |
| `raw_class` | (escape hatch — value passes through verbatim) |

## Dynamic values

Style kwargs can take `if`/`else` expressions. The compiler lowers them to runtime ternaries inside `className`:

```vox
// vox:skip
component Bubble(role: str) {
    view: panel(
        bg=if role is "user" { "blue.600/20" } else { "white/5" },
        radius_br=if role is "user" { "sm" } else { "2xl" }
    ) {
        text() { role }
    }
}
```

Lowers to (effectively):

```text
<div className={[
    (role === "user" ? "bg-blue-600/20" : "bg-white/5"),
    (role === "user" ? "rounded-br-sm" : "rounded-br-2xl"),
    /* primitive base classes */
].filter(Boolean).join(" ")}>
    <p>{role}</p>
</div>
```

Other expression shapes (function calls, complex computations) on a typed kwarg fall back to passthrough — fix the source so the value is a literal or `if`/`else`.

## Event handlers

Event kwargs use snake_case. The compiler renames to React-style camelCase at emit:

```vox
// vox:skip
button(on_click={count = count + 1}) { "Increment" }
input(on_change={fn(e) handle(e)}, attr_type="text")
```

Supported events: `on_click`, `on_change`, `on_input`, `on_submit`, `on_keydown`, `on_keyup`, `on_mouseenter`, `on_mouseleave`. Add more in [`crates/vox-codegen-ts/src/hir_emit/compat.rs`](../../../crates/vox-codegen-ts/src/hir_emit/compat.rs).

## Reserved-keyword attribute names

HTML attributes whose names collide with Vox keywords (`type`, `for`, …) use the `attr_` prefix:

```vox
// vox:skip
input(attr_type="checkbox", checked=t.done)
label(attr_for="email") { "Email" }
```

The parser strips `attr_` so the emitted HTML uses the bare attribute name.

## Children

Each statement-position expression inside a trailing block is one child. Expressions can be:

- **String literals** — `"Hello"`.
- **Bare identifiers / field access** — `name`, `t.title`.
- **Other view calls** — nested.
- **`if`/`else` expressions returning views** — branches must be view-shaped.
- **`match` expressions returning views** — each arm is a view.
- **`tasks.map(fn(t) { … })`** — list comprehensions.

```vox
// vox:skip
column() {
    if logged_in {
        Dashboard(user=current_user)
    } else {
        LoginForm()
    }
    tasks.map(fn(t) {
        TaskRow(task=t)
    })
}
```

## Calling React components

Capitalized callees + named-only args sugar to JSX self-closing. To call an existing React component, just call it:

```vox
// vox:skip
ComposerPanel()
DataChart(data=[1, 2, 3])
ChatMessage(role="user", content=msg) {
    // children inside another view-call's block work too
    text() { "Reply" }
}
```

## Escape hatches

When typed kwargs don't cover what you need:

- **`raw_class="…"`** — Tailwind utilities verbatim. Use sparingly; everything in `raw_class` is invisible to the compiler.
- **Unknown kwargs** — pass through as raw HTML attributes (after kwarg-to-className resolution finds nothing).

## What's NOT supported

- Angle-bracket JSX (`<row>...</row>`) — retired. `<` in expression position is a parse error.
- Class strings as the styling primitive — replaced by typed kwargs.
- Positional args in view calls — view calls are keyword-only by design.
- HTML attributes whose name is a Vox keyword *without* the `attr_` prefix.

## Common patterns

### Conditional rendering

```vox
// vox:skip
column() {
    if items.is_empty() {
        text(color="zinc.500") { "Nothing here yet." }
    } else {
        list() {
            items.map(fn(i) { ItemRow(item=i) })
        }
    }
}
```

### Active-tab styling

```vox
// vox:skip
button(
    raw_class="tab-btn",
    bg=if active_tab is "speak" { "blue.600" } else { "transparent" },
    on_click={active_tab = "speak"}
) { "LOQUELA" }
```

### Surfaces (foreground/background pair)

```vox
// vox:skip
panel(surface=if mine { "chat.user" } else { "chat.assistant" }) {
    text() { content }
}
```

The `surface` kwarg pulls a registered foreground/background pair from the token registry; the validator then checks WCAG contrast at compile time.

## Where to read more

- [GUI Authoring Syntax 2026 (design rationale)](../architecture/gui-authoring-syntax-2026.md)
- [Phase 6 primitive set + Web IR primitives](../architecture/gui-native-roadmap-status-2026.md)
- [Token registry and contrast validation](../reference/token-system.md)
- [Web IR ADR (`crate::web_ir`)](../../../crates/vox-codegen/src/web_ir/mod.rs)
