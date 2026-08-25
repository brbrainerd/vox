---
title: "Language Syntax Reference"
description: "A comprehensive, scannable syntax quick-reference page."
category: "Language Reference"
status: "current"
training_eligible: true

schema_type: "TechArticle"
keywords: ["Vox syntax reference", "Vox language keywords", "Vox grammar specification", "full-stack syntax guide"]
---

# Reference: Language Syntax

This page provides the canonical structural layout for Vox language syntax aligned with the workspace compiler version ([Versioning Policy (SSOT)](../../../AGENTS.md#versioning-policy-ssot); currently **0.5.x**). All code samples are grounded in the confirmed `examples/golden/` files where noted.

## Primitive Types

| Type | Example | Description |
| :--- | :--- | :--- |
| `str` | `"hello world"` | Text string (UTF-8) |
| `int` | `42` | Signed 64-bit integer |
| `float` | `3.14159` | 64-bit floating point number |
| `bool` | `true`, `false` | Boolean value |
| `Unit` | `()` | Equivalent to `void` |

Variable assignments are immutable by default in Vox. Prefix with `mut` for mutability.

```vox
// ANCHOR: variables
fn demo_vars() {
    let x = 10
    let mut y = 20
    y = 30
}
// ANCHOR_END: variables
```

Functions mapping natively to networking, storage, or internal agentic constraints.

```vox
// ANCHOR: functions
fn add(a: int, b: int) to int {
    return a + b
}

component Button(label: str) {
    view: button() { label }
}
// ANCHOR_END: functions
```

```vox
// From examples/golden/ref_orchestrator.vox
tool "search: Search the knowledge base" search(query: str) to List[str] {
    return ["result 1", "result 2"]
}
```

Lexical constraints and properties can be modeled strictly using Abstract Data Types (ADTs) and Table definitions.

```vox
type Shape =
    | Circle(radius: float)
    | Rect(w: float, h: float)
```

```vox
table Task {
    title: str
    done: bool
    owner: str
}
```

### Branching
```vox
fn check(n: int) to str {
    if n > 0 {
        return "positive"
    } else {
        return "other"
    }
}
```

### Pattern Matching (`match`)
```vox
fn area(s: Shape) to float {
    match s {
        Circle(r) => 3.14 * r * r
        Rect(w, h) => w * h
    }
}
```

### Pipe Operator (`|>`)
The `|>` operator passes the expression on the left as the first argument to the function on the right. Works with any function.
```vox
fn trim_str(s: str) to str { return s.trim() }
fn parse_int(s: str) to int { return int(s) }
fn doubled(n: int) to int { return n * 2 }

fn demo_pipe() to int {
    return " 123 " |> trim_str |> parse_int |> doubled
    // Compiles to: doubled(parse_int(trim_str(" 123 ")))
}
```

### Loops
```vox
fn should_exit() to bool { return true }

fn demo_loop() {
    loop {
        if should_exit() { break }
        continue
    }
}
```

### Comments
Comments use `//`. Block comments and `#` comments are not supported.
```vox
fn demo_comment() {
    // This is a comment
    let x = 1
}
```

### Error Propagation (`?`)
The `?` suffix unpacks an `Ok` result, returning early if the result is an `Error(e)`.

```vox
fn get_data() to Result[str] {
    return Ok("data")
}

fn build_report() to Result[str] {
    let raw_data = get_data()?
    return Ok("Report { " + raw_data)
}
```

Actors operate isolated asynchronous loops responding to discrete event handler payloads via `on`. 

```vox
fn Counter_increment(count: int, n: int) to int {
    return count + n
}

fn Counter_get(count: int) to int {
    return count
}
```

```vox
// vox:skip -- native spawn/send actor grammar not yet restored (see examples/golden/ref_actors.vox)
let c = spawn Counter_increment(0, 5)
let val = Counter_get(c)
```

## Agents

Agents define LLM-backed roles with systematic instructions and toolsets.

```vox
// vox:skip -- illustrative future syntax; the @llm agent decorator is not implemented yet
@llm(model="claude-3-opus")
fn summarize(text: str) to str
```

Use `workflow` to group state machine processes that survive process restarts. Use `activity` to dictate atomic, retry-able execution sequences.

```vox
table Note {
    title: str
    content: str
}

query get_notes() to Result[List[Note]] {
    return Ok(db.Note.all()?)
}

mutation create_note(title: str, content: str) to Result[int] {
    let id = db.Note.insert({ title: title, content: content })?
    return Ok(id)
}
```

## Component and UI Syntax

UI is declared with `component`. The codegen emits a plain React/TSX component for the
external frontend to import. (`@island` was retired 2026-05-03 — see
[architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).)

```vox
component TaskList(tasks: list[Task]) {
    view: column() {
        text() { "tasks" }
    }
}

component AboutPage() {
    view: text() { "About" }
}

// Web Routing Layout Mapping
routes {
    "/"         to TaskList
    "/about"    to AboutPage
}
```

### Return Keyword
`return` is the canonical way to return a value from a function.

```vox
fn double(x: int) to int { return x * 2 }
fn square(x: int) to int { return x * x }
```

Vox imports use fully qualified paths. Use `import rust:<crate>` for native interop.

```vox
// vox:skip -- import rust: always emits an unavoidable doctest-failing metadata-pin warning
import react.use_state
import rust:serde_json as json
```

## See also

- [Operator precedence](./ref-operator-precedence.md)
- [Literals](./ref-literals.md)
- [Type system](./ref-type-system.md)
- [Async and concurrency](./ref-async-concurrency.md)
- [FFI](./ref-ffi.md)
- [Standard library index](./ref-stdlib-index.md)
