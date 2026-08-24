---
title: "Tutorial: Building UI with VUV view-calls"
description: "Learn how to build modern, reactive UIs in Vox using the view-call (VUV) authoring syntax — function calls + trailing-block children + typed style kwargs."
category: "Tutorials"
status: "current"
sort_order: 5
training_eligible: true

schema_type: "HowTo"
---
# Tutorial: Building UI with VUV view-calls

Learn how to build modern, reactive user interfaces with Vox. This tutorial covers the `component` declaration, the VUV view-call authoring syntax (function calls + trailing-block children + typed style kwargs), and binding UI state to backend logic.

> [!NOTE]
> Angle-bracket JSX (`<row>...</row>`) was retired in 2026-Q2. All views use the VUV view-call form. See [`how-to-author-views.md`](../how-to/how-to-author-views.md) for the full reference.

## 1. The `component` declaration

Vox UI components are declared with the `component` keyword. The body holds state, derived values, effects, and a `view:` expression that produces the rendered tree.

```vox
component Profile(name: str, bio: str) {
    view: panel(pad=6, bg="white", shadow=true, radius="lg") {
        heading(level=2, size="xl", weight="bold") { name }
        text(color="gray.600") { bio }
    }
}
```

The view is a single expression: a `panel` view-call with two children. Style is a series of typed named arguments — `pad=6`, `bg="white"`, `radius="lg"`. The compiler resolves these to Tailwind classes.

## 2. Composing components

Components compose by calling each other. Capitalized callees with named-only args become self-closing automatically; add a trailing block for children.

```vox
component UserProfile() {
    view: column() {
        heading(level=1) { "User Profile" }
        Profile(name="Alice", bio="Developer")
    }
}

routes {
    "/profile" to UserProfile
}
```

## 3. Iteration and conditionals

A trailing block can contain `if`/`else`, `match`, and `.map(...)` expressions. Each must produce view-shaped values.

```vox
component UserList(users: list[str]) {
    view: list() {
        users.map(fn(user) {
            list_item(pad_y=2) { user }
        })
    }
}
```

Conditional render:

```vox
component MaybeBanner(show: bool) {
    view: column() {
        if show {
            panel(bg="amber.100", pad=3) {
                text() { "Heads up!" }
            }
        } else {
            text() { "" }
        }
    }
}
```

## 4. Binding to backend logic

The true power of Vox is technical unification. UI event handlers can call bare `mutation` or `server` functions directly. Use snake_case event kwargs (`on_click`, `on_change`, `on_submit`); the compiler renames to React's camelCase at emit.

```vox
table Task {
    task_id: Id[Task]
    title:   str
    done:    bool
}

mutation complete_task(task_id: Id[Task]) to Result[str] {
    db.Task.delete(task_id)?
    return Ok("completed")
}

component TaskRow(t: Task) {
    view: row(items="center", gap=2) {
        button(
            attr_type="checkbox",
            checked=t.done,
            on_change={fn() complete_task(t.task_id)}
        )
        text() { t.title }
    }
}
```

`table` always adds its own surrogate `_id` primary key, but it isn't exposed as a Vox-source
field access (`t._id` doesn't typecheck) — so a component that needs to reference a row's
identity, like the delete-button handler here, declares its own `Id[Task]` field. It just can't
be literally named `id`: that collides with the auto-added `_id` and triggers a compiler
warning (`crates/vox-compiler/src/typeck/ast_decl_lints.rs`).

The `attr_` prefix on `attr_type` strips at parse time so the emitted HTML attribute is `type="checkbox"` — needed because `type` is a Vox keyword.

## 5. Routing

Map URLs to top-level components in the `routes { }` block:

```vox
component NewsletterForm() {
    view: text() { "Subscribe" }
}

routes {
    "/" to NewsletterForm
    "/profile" to UserProfile
}
```

## 6. Typed style kwargs (the big one)

You will not write Tailwind class strings in `.vox` source. Style is a series of named arguments. Common ones:

| Kwarg | Effect |
|---|---|
| `pad`, `pad_x`, `pad_y` | padding |
| `bg`, `color` | background, text color (`zinc.400` auto-dashes to `zinc-400`) |
| `radius` | corner radius (`xl`, `2xl`, `full`) |
| `border`, `border_color` | border + color |
| `gap` | flex gap |
| `flex`, `shrink` | flex-1, shrink-0 |
| `case` | `upper`, `lower`, `normal` |
| `weight` | `normal`, `medium`, `semibold`, `bold` |

Dynamic values (`if` expressions) work directly:

```vox
fn submit() {
    print("submitted")
}

component SubmitButton(is_primary: bool) {
    view: button(
        bg=if is_primary { "blue.600" } else { "white/5" },
        color=if is_primary { "white" } else { "zinc.400" },
        on_click={submit()}
    ) { "Send" }
}
```

For escapes, use `raw_class="…"` to drop in Tailwind utilities verbatim. Use sparingly — `raw_class` content is invisible to the compiler.

---

**Next Steps**:
- [How to author Vox views (full reference)](../how-to/how-to-author-views.md)
- [GUI Authoring Syntax 2026 (design rationale)](../architecture/gui-authoring-syntax-2026.md)
- [First App](tut-first-app.md) — Apply these UI patterns to a collaborative task list.
