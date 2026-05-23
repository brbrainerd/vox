---
title: "Language Syntax Reference"
description: "A comprehensive, scannable syntax quick-reference page."
category: "Language Reference"
status: "current"
last_updated: "2026-04-26"
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
// vox:skip
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
// vox:skip
// ANCHOR: functions
fn add(a: int, b: int) to int {
    return a + b
}

component Button(label: str) {
    view: <button>{label}</button>
}
// ANCHOR_END: functions
```

```vox
// vox:skip
// From examples/golden/ref_orchestrator.vox
@mcp.tool "search: Search the knowledge base"
fn search(query: str) to List[str] {
    return ["result 1", "result 2"]
}
```

Lexical constraints and properties can be modeled strictly using Abstract Data Types (ADTs) and Table definitions.

```vox
// vox:skip
type Shape =
    | Circle(radius: float)
    | Rect(w: float, h: float)
```

```vox
// vox:skip
@table type Task {
    title: str
    done: bool
    owner: str
}
```

### Branching
```vox
// vox:skip
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
// vox:skip
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
// vox:skip
let value = " 123 " |> trim |> parse_int |> double
// Compiles to: double(parse_int(trim(" 123 ")))
```

### Loops
```vox
// vox:skip
loop {
    if should_exit() { break }
    continue
}
```

### Comments
Comments use `//`. Block comments and `#` comments are not supported.
```vox
// vox:skip
// This is a comment
let x = 1
```

### Error Propagation (`?`)
The `?` suffix unpacks an `Ok` result, returning early if the result is an `Error(e)`.

```vox
// vox:skip
fn build_report() to Result[str] {
    let raw_data = get_data()?
    return Ok("Report { " + raw_data)
}
```

Actors operate isolated asynchronous loops responding to discrete event handler payloads via `on`. 

```vox
// vox:skip
fn Counter_increment(count: int, n: int) to int {
    return count + n
}

fn Counter_get(count: int) to int {
    return count
}
```

```vox
// vox:skip
let c = spawn Counter_increment(0, 5)
let val = Counter_get(c)
```

## Agents

Agents define LLM-backed roles with systematic instructions and toolsets.

```vox
// vox:skip
@llm(model="claude-3-opus")
fn summarize(text: str) to str
```

Use `workflow` to group state machine processes that survive process restarts. Use `activity` to dictate atomic, retry-able execution sequences.

```vox
// vox:skip
@endpoint(kind: query) fn get_notes() to List[Note] {
    return db.Note.all()
}

@endpoint(kind: mutation) fn create_note(title: str, content: str) to Result[Id[Note]] {
    let id = db.Note.insert({ title: title, content: content })?
    return Ok(id)
}
```

## Component and UI Syntax

UI is declared with `component`. The codegen emits a plain React/TSX component for the
external frontend to import. (`@island` was retired 2026-05-03 — see
[architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).)

```vox
// vox:skip
component TaskList(tasks: list[Task]) {
    view: <ul></ul>
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
// vox:skip
fn double(x: int) to int { return x * 2 }
fn square(x: int) to int { return x * x }
```

Vox imports use fully qualified paths. Use `import rust:<crate>` for native interop.

```vox
// vox:skip
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
