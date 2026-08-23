---
title: "Reference: Type System"
description: "Deep dive into the Vox type system: ADTs, parametric types (list/Option/Result/Id), zero-null discipline, and bidirectional inference."
category: "Language Reference"
training_eligible: true

schema_type: "TechArticle"
keywords: ["Vox type system", "Option type Vox", "Result type Vox", "null-free language types"]
---

# Reference: Type System

Vox features a strongly-typed, expressive type system designed for technical unification between Rust (backend) and TypeScript (frontend). It is designed to be **AI-readable**, meaning the type signatures provide enough context for an LLM to generate correct code without hallucinating field names.

## 1. Core Philosophy: Zero-Null Discipline

In Vox, `null` and `undefined` do not exist. Absence must be modeled explicitly using `Option[T]`, and fallible operations must use `Result[T, E]`.

| Feature | Vox Implementation | Benefit |
|---------|-------------------|---------|
| **Absence** | `Option[T]` | Forced handling of empty states; no "null pointer" crashes. |
| **Failure**| `Result[T, E]` | Errors are part of the type signature; cannot be ignored. |
| **Branching** | Pattern Matching | Compiler ensures all cases (variants) are handled. |

---

## 2. Primitive Types

| Type | Description | Rust Equivalent | TS Equivalent |
|------|-------------|-----------------|---------------|
| `str` | UTF-8 String | `String` | `string` |
| `int` | 64-bit Integer | `i64` | `number` / `BigInt` |
| `float`| 64-bit Float | `f64` | `number` |
| `bool` | Boolean | `bool` | `boolean` |
| `Unit` | Empty placeholder | `()` | `void` |

---

## 3. Algebraic Data Types (ADTs)

### Structs (Product Types)
A named collection of fields.

```vox
table Task {
    id:       Id[Task]
    title:    str
    done:     bool
    priority: int
}
```

### Enums (Sum Types / Tagged Unions)
Types that can be one of several variants, potentially carrying extra data.

```vox
{{#include ../../../examples/golden/ref_types.vox:adt}}
```

---

Vox uses the `match` keyword for exhaustive destructuring of ADTs. The compiler will reject a match expression that does not cover every possible variant.

```vox
{{#include ../../../examples/golden/ref_types.vox:matching}}
```

---

### `Option[T]`
Used for values that might be missing.

```vox
fn find_user(id: int) to Option[str] {
    if id is 1 {
        return Some("alice@example.com")
    }
    return None
}
```

### `Result[T, E]`
Used for operations that can fail.

```vox
server remove_task(id: Id[Task], title: str) to Result[str] {
    if title.len() is 0 {
        return Error("Title cannot be empty")
    }
    db.Task.delete(id)?
    return Ok("removed")
}
```

---

Similar to Rust, the `?` operator can be used to early-return on `None` or `Err`.

```vox
fn get_user_email(id: int) to Option[str] {
    let email = find_user(id)? // If None, returns None early
    return Some(email)
}
```

---

## 6b. Parametric types and generics (current surface)

Vox supports **type constructors** such as `list[T]`, `map[K, V]`, `Option[T]`, `Result[T, E]`, and branded identifiers `Id[Entity]` at the language level. These are the primary **generic-style** forms authors use today.

**Not yet a full Rust-like trait system:** user-defined type-parameter declarations on `fn` / `type` (beyond what the compiler accepts for specific builtins) remain roadmap-governed — see [`gui-native-roadmap-status-2026.md`](../architecture/gui-native-roadmap-status-2026.md) and language enforcement phases. Prefer concrete `type` aliases and ADTs until a trait / constraint story is explicitly shipped.

---

## 7. Bidirectional Type Inference

You rarely need Type annotations for local variables. Vox infers them from the right-hand side or from how the variable is used.

```vox
fn add_task(title: str) to int {
    return 1
}

fn demo_inference() {
    let x = 10                  // inferred as int
    let names = ["Alice", "Bob"] // inferred as list[str]
    let result = add_task("Hi")  // inferred from add_task signature
}
```

Explicit types are **required** on:
1. Function parameters
2. Function return types
3. `table` and `type` definitions

### How generic types get resolved (instantiation)

Verified against `crates/vox-compiler/src/typeck/unify.rs`. A generic type
like `list[T]` is stored internally with a placeholder (`Ty::GenericParam`).
Every time that generic is *used* — a call to a generic builtin, a generic
field access — the checker calls `instantiate()`, which walks the type
structurally and replaces each `GenericParam` with a **fresh type variable**
(`fresh_var()`, a new numbered `Ty::TypeVar`). "Fresh" is the key word: two
separate uses of the same generic function get two independent sets of type
variables, so `list[T].append` called once with `int` and again with `str`
in the same file doesn't cross-contaminate — each call site solves its own
`T` independently. This is why annotating one call site's generic argument
never affects type inference at a different call site.

Those fresh variables get pinned down by **unification** (`unify()`): as the
checker walks the expression, it unifies the type variable against whatever
concrete type shows up (an argument's actual type, a return position's
expected type). Unification includes an **occurs check** — it refuses to
solve a type variable to a type that contains itself, which is what
prevents an infinite type from silently type-checking. Once solved,
`resolve()` follows the substitution chain to get the concrete type back out
for diagnostics and codegen.

The doc comment on `instantiate()` notes it "matches the AST pipeline's
instantiate so builtins like `use_state` and `List::append` unify correctly
in the HIR Checker" — the same algorithm has to agree between two
type-checking entry points (an older AST-level pass and the HIR-level
`Checker`) precisely so a generic builtin resolves the same way regardless
of which pass reaches it first.

---

## 8. Collection Types

### `list[T]`
An ordered sequence of elements.
- **Usage**: `list[int]`
- **Literals**: `[1, 2, 3]`

### `map[K, V]`
A collection of key-value pairs.
- **Usage**: `map[str, int]`
- **Literals**: `{ "key": 10 }`

---

## 9. Next Steps

- **[Language Guide](./ref-syntax.md)** — General syntax overview.
- **[Decorator Registry](./ref-decorators.md)** — How types interact with `table`, `query`, `mutation`, and `server`.
- **[Functions](./ref-syntax.md)** — Detailed function signature reference.
- **[Literals](./ref-literals.md)** — Numeric and string literal rules.
- **[Diagnostic ID policy](./ref-diagnostic-id-policy.md)** — Stable compiler vs audit identifiers.


