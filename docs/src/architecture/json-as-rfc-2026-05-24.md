---
title: "RFC: @json_as(MyType) — typed JSON deserialization decorator"
description: "Schema-typed JSON parsing built atop the strict-Option Json surface; the 80→99% ergonomic close for known-schema JSON in Vox."
last_updated: "2026-05-24"
category: "Architecture SSOTs"
status: approved
---

# RFC: `@json_as(MyType)` — typed JSON deserialization decorator

**Status:** ratified 2026-05-24.

## 1. Motivation

The strict-Option Json API
([json-ergonomics-rfc-2026-05-23.md](./json-ergonomics-rfc-2026-05-23.md))
solves traversal of *unknown-schema* JSON well: `data.pointer("/x/y")
.and_then(fn(j: Json) to Option[str] { j.as_str() })`. For unknown
shapes (arbitrary LLM responses, CLI tool outputs) it's the right tool.

But ~80% of Vox JSON workloads have a **known schema**: HTTP API
responses, MCP JSON-RPC envelopes, configs, training fixtures. For
these, the manual `pointer + and_then + as_X` ladder is ceremony — the
schema is fixed, the field names are known at write time, and missing
fields are typically errors (not "use a default").

`@json_as(MyType)` closes the gap. One decorator on a typed struct
binds Vox to the `serde` deserialization machinery: missing fields,
type errors, and unknown variants all surface as one structured
`Result[MyType]` instead of dozens of `Option` returns.

## 2. What we audited about serde derive

Before recommending an API, we surveyed how Rust's `#[derive(Deserialize)]`
shapes its trade-offs and which apply to Vox.

| serde decision | Why | Apply to Vox? |
|---|---|---|
| Generate a per-type `Deserialize` impl at compile time | Zero-cost dispatch; no runtime schema lookup | **Yes** — decorator runs at compile, not parse |
| Missing required field → `Err("missing field 'foo'")` | Loud failure for required data | **Yes** — matches Vox's no-silent-null principle |
| `Option<T>` field → field can be absent OR JSON null | Distinguishes optional from required | **Yes** — keep |
| `#[serde(default)]` for "missing → default value" | Backwards-compatible config evolution | **Yes** — `@json_as(MyType, defaults: true)` |
| `#[serde(rename = "...")]` for JSON name ≠ Rust name | snake_case vs camelCase mismatch | **Yes** — `@field_name("foo_bar")` attribute on the type def |
| `#[serde(rename_all = "camelCase")]` whole-type rename | Common when consuming JS APIs | **Yes** — `@json_as(MyType, naming: "camelCase")` |
| `#[serde(tag = "type")]` for tagged enums | Discriminated unions in JSON | **Yes** — required for MCP / JSON-RPC envelopes |
| `#[serde(deny_unknown_fields)]` | Strict mode for config files | **Yes** — `@json_as(MyType, strict: true)` |
| Custom `#[serde(deserialize_with = "func")]` | Escape hatch | **Defer** — would require Vox-level fn pointers; revisit in v1.0+ |
| Borrowed deserialization (`&'a str`) | Zero-copy from input buffer | **No** — Vox doesn't have lifetimes; always owned String |

**The principle to take:** compile-time codegen, structured `Result`,
support the common attributes (`default`, `rename`, `rename_all`,
`tag`, `strict`). Skip the lifetime-/closure-heavy parts.

## 3. Realistic Vox JSON workload (audit reuse from json-ergonomics §3)

| Source | Schema known? | Shape | `@json_as` fit |
|---|---|---|---|
| LLM tool-use response (`@ai`) | yes (per-model schema) | nested with optional `action` / `tool_calls` | **strong** — tagged-enum `tool_calls` is the killer use case |
| `cargo metadata` (audit scripts) | yes | deeply nested arrays of records | **strong** — replace the Bucket-B `.get(k).and_then(...)` ladders |
| HTTP / MCP JSON-RPC | yes (RFC-defined) | request/response envelopes | **strong** — every `@endpoint` server response should derive |
| Configs | yes | mostly static | **strong** — `@json_as(MyConfig, strict: true)` |
| `process.run` JSON output | varies | depends on tool | **weak** — use strict-Option API |
| LLM corpus harvest (mens) | yes | flat | **strong** — typed records save dozens of lines per row |

## 4. The API

### 4.1 Type definition

```vox
// vox:skip — @json_as decorator is the subject of this RFC; not yet implemented.
@json_as(Product)
type Product {
    id: int,
    name: str,
    price: float,
    @field_name("in_stock") in_stock: bool,
    tags: list[str],
    description: Option[str],
    metadata: Option[Json],
}
```

The decorator at the type definition generates a `Product::from_json(j: Json) → Result[Product]`
free function (and a sibling `Product::to_json(p: Product) → Json` for symmetry).

### 4.2 Decorator parameters

| Parameter | Type | Default | Meaning |
|---|---|---|---|
| (positional) | type name | — | The Vox type to derive against. Required. |
| `naming` | str | `"snake_case"` | `"snake_case"` / `"camelCase"` / `"kebab-case"` / `"PascalCase"` for the type-wide convention. |
| `strict` | bool | `false` | When true, unknown JSON fields fail. When false (default), they're ignored. |
| `defaults` | bool | `false` | When true, missing fields fall back to `T::default()`. When false (default), missing required fields fail. |

### 4.3 Per-field attributes

| Attribute | Effect |
|---|---|
| `@field_name("foo")` | Override per-field JSON name (escapes the type-wide `naming`). |
| `@default(expr)` | Per-field default expression (overrides `defaults: true`). |
| `@skip_if_none` | When serializing, omit `Option::None` fields rather than emit `null`. |

### 4.4 Tagged enums

```vox
// vox:skip — @json_as + struct-shape variants are RFC-only.
@json_as(ToolCall, tag: "kind")
type ToolCall =
    | Search { query: str }
    | Compute { expr: str, precision: int }
    | Done
```

JSON input `{"kind": "Search", "query": "vox lexer"}` → `ToolCall::Search { query: "vox lexer" }`.
The `tag` parameter picks the discriminator field. Untagged enums (where the variant is inferred
from structure) are a v2 concern; require explicit `tag:` for v1.

### 4.5 Call sites

```vox
// vox:skip — Uses Product::from_json synthesized by the RFC; not yet emitted.
fn handle_response(body: str) to Result[Product] {
    let json_res = json.parse(body)
    if json_res.is_err() { return Err("bad JSON: " + json_res.unwrap_err()) }
    return Product::from_json(json_res.unwrap())
}
```

## 5. Error shape

The generated `from_json` returns `Result[T]` with a structured error message including a JSON Pointer path:

```
json_as Product: missing required field 'price' at path "/"
json_as Product: type mismatch at "/tags": expected list[str], got string
json_as Product (strict): unknown field "color" at path "/"
json_as ToolCall: unknown tag value "Unknown" at "/kind"; expected one of: Search, Compute, Done
```

## 6. Compile-time codegen

The `@json_as` decorator runs at HIR-lowering time. For each tagged type, the lowering pass:

1. Inspects the type's field list (or variant list for enums).
2. Resolves each field's Vox type to its Json shape (`int` → `Json::as_int`, `Option[T]` → presence check via `.has(k)` then recurse, user-defined `@json_as` type → recursive call to its `from_json`, etc.).
3. Generates a `<TypeName>_from_json(j: Json) -> Result[<TypeName>]` HirFn body.
4. Generates a sibling `<TypeName>_to_json(v: <TypeName>) -> Json`.
5. Registers the two functions in the importer's scope.

No runtime schema: all dispatch is statically compiled.

## 7. Why this honors Vox principles

- **No silent failures:** missing fields fail loudly with a JSON Pointer path. `defaults: true` is explicit opt-in.
- **One canonical pattern:** `@json_as(T)` annotation + `T::from_json(j)` call.
- **AI-friendly:** matches Rust `serde` mental model 1:1.
- **K-complexity low:** four optional decorator parameters; three per-field attributes; no new keywords.
- **Layered with strict-Option Json:** mixed walks work via `metadata: Option[Json]` escape-hatch fields.

## 8. What we explicitly defer

- Custom `deserialize_with` / `serialize_with` — requires user-level fn pointers; revisit.
- Untagged enums — defer to v2.
- Borrowed / zero-copy deserialization — Vox has no lifetimes.
- JSON Schema export — out of scope.
- Streaming / partial parse — out of scope.
- Multiple-name aliasing — defer; users wrap manually.

## 9. Implementation plan

Tracked as Phase M of the audit doc. Six steps, ~3-5 days total:

1. **AST parser** — `@json_as(TypeName, params...)` decorator on `type` declarations; per-field attributes.
2. **HIR lowering** — emit two synthetic `HirFn`s per annotated type.
3. **Typeck** — register `<TypeName>::from_json` / `<TypeName>::to_json` in the importer's `TypeEnv`.
4. **Eval** — runtime support is regular Vox function dispatch; no special eval path.
5. **Codegen** — `--mode script` Rust dispatch parallels the synthesized HIR.
6. **Tests + golden example** — `tests/json_as_test.rs` + `examples/golden/json_as_typed.vox`.

## 10. Migration impact

`generate-matrix-doc.vox` / `audit-workspace-health.vox` / `audit-dependency-layers.vox`
collapse from ~40 lines of `pointer + and_then + as_X` to ~10 lines of struct definition
+ one `T::from_json(data)` call. Post-Phase-M; doesn't block landing.

## 11. Risks / open questions

| Risk | Mitigation |
|---|---|
| Lowering complexity for nested generics | Recurse with a depth budget matching typeck's existing limit. |
| `Option[T]` vs missing-vs-null ambiguity | Inherits from json-ergonomics §9. Explicit `Json` typed field for the rare PATCH case. |
| Tagged-enum payload shapes | Limit v1 to externally-tagged. Internally-tagged + adjacently-tagged are v2. |
| Mutual recursion | Two-pass lowering: collect all `@json_as` types first, then emit bodies. |
| Decorator + alias-form intra-project imports | `import "./types.vox" as t` → `t::Product::from_json(j)` — works via the existing namespace-method dispatch from intra-project-imports RFC §11. Verify in integration test. |

## 12. Related

- [json-ergonomics-rfc-2026-05-23.md](./json-ergonomics-rfc-2026-05-23.md) — the dynamic Json surface this builds atop.
- [intra-project-imports-rfc-2026-05-23.md](./intra-project-imports-rfc-2026-05-23.md) — supplies the `T::from_json(j)` namespace-method dispatch.
- [closures-rfc-2026-05-23.md](./closures-rfc-2026-05-23.md) — chaining mechanism for mixed dynamic+typed cases.
- [vox-stdlib-gap-audit-2026-05-23.md](./vox-stdlib-gap-audit-2026-05-23.md) §11.7 — original "defer @json_as" decision; this RFC ratifies the design without changing that timeline.
