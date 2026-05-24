---
title: "RFC: JSON ergonomics — strict-Option + pointer"
description: "Vox's typed Json surface, why we diverge from serde_json::Value::Index, and the canonical traversal idioms."
last_updated: "2026-05-23"
status: ratified
---

# RFC: JSON ergonomics — strict-Option + pointer

**Status:** ratified 2026-05-23 (user-approved Decisions 1 & 2; Decision 3
— `@json_as` derive — deferred).

## 1. Motivation

Vox is an AI-destination language. LLMs emit JSON. HTTP APIs return
JSON. CLI tools (`cargo metadata`, `jq`, every modern dev tool) emit
JSON. Configs are JSON-shaped. A Vox program that can't traverse a
3-4 level JSON tree without nested `match` chains is dead on arrival
for real work.

Today (pre-RFC), the typed `Ty::Named("Json")` surface in
[`typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs)
exists but is **unreachable from `json.parse`**: that function's
signature is `Ty::Fn(Str -> GenericParam(0))`, returning a fresh
type variable. Typeck can never dispatch `.get_str` on the result.
Three Bucket-B scripts
(`audit-workspace-health`, `audit-dependency-layers`,
`generate-matrix-doc`) fail `vox check` for exactly this reason.

Even when the typed surface IS reachable, every accessor returns
`Result[T]`, requiring per-hop `match Ok/Error` blocks. The canonical
golden example at [`examples/golden/json_stdlib.vox`](../../../examples/golden/json_stdlib.vox)
needs 8 lines of nested `match` to read a 2-level-deep integer field.

## 2. What we audited about serde_json

Before recommending an API, we studied
[`serde_json::Value`](https://docs.rs/serde_json/latest/serde_json/enum.Value.html)
and the rationale behind its decisions.

| serde decision | Why | Apply to Vox? |
|---|---|---|
| `Value::get(idx) -> Option<&Value>` | Receiver may not be object/array; collapse "wrong receiver" and "key absent" into one `None` | **Yes** — single Option discipline matches Vox's no-silent-null principle |
| Preserve missing-vs-null distinction (`Option<&Value>` vs `Some(&Value::Null)`) | RFC 8259 says they're different; PATCH-style APIs depend on it (`{"name": null}` clears, `{}` leaves alone) | **Yes** — keep the distinction. `Json::Null` is a value; `Option::None` is absence. |
| `Value::as_str() -> Option<&str>` (NOT Result) | "Wrong type" and "is null" both produce None; callers rarely need to distinguish | **Yes** — leaf coercion is Option |
| `impl Index<&str> for Value` returning `&Value::Null` on miss | Rust syntax forces it: `Index::index -> &Self::Output` can't return Option | **No** — Vox has no subscript syntax; we don't share Rust's ergonomic pressure. Silent-Null propagation is the most-criticized part of serde_json's API. |
| `#[derive(Deserialize)]` into typed structs for known schemas | Most production code knows the schema | **Defer** — `@json_as(MyType)` decorator planned for follow-up RFC. Out of scope here. |

**The principle to take from serde:** Option discipline at every
fallible boundary; preserve missing-vs-null; coerce at leaves.

**The accommodation NOT to take:** the `Index` silent-Null trait.

## 3. Realistic Vox JSON workload (corpus + apps grep)

| Source | Shape | Schema known? | Pattern |
|---|---|---|---|
| LLM responses (`@ai` decorator) | structured + optional `action`/`thought` | partial | known-key access + null checks |
| `cargo metadata` (audit scripts) | deeply nested arrays of records | known | walk + filter |
| HTTP/MCP responses | JSON-RPC envelopes | known | typed (defer to `@json_as`) |
| Configs | mostly static | known | typed (defer) |
| `process.run` JSON output | varies by tool | unknown | walk + filter |
| mens-corpus harvest | line-oriented JSONL | known | flat |

**~80% of Vox JSON is known-schema.** The pointer + Option pattern
serves that well; the remaining 20% (truly unknown shapes) is also
served by the same API without compromising the typed case.

## 4. The API

### 4.1 Constructor

| Method | Signature | Notes |
|---|---|---|
| `json.parse(s)` | `Str → Result[Json]` | **Fix:** today returns `GenericParam(0)`; corrected to `Result[Json]`. |
| `json.stringify(v)` | `α → Str` | Unchanged. Serialize any value to its JSON text. |

### 4.2 Navigation (single hop)

All return `Option[Json]`. `None` collapses key-absent, index-out-of-bounds,
AND wrong-receiver-type into one signal — caller distinguishes via
`Json.is_null` (which is a *value* check) and via the fact that they
already know the schema.

| Method | Signature | Behavior |
|---|---|---|
| `Json.get(key: str)` | `→ Option[Json]` | Object key access. None if receiver is not an object or key is absent. |
| `Json.at(i: int)` | `→ Option[Json]` | Array index access. None if receiver is not an array or index is out of bounds. |
| `Json.pointer(path: str)` | `→ Option[Json]` | **Primary deep-access tool.** RFC 6901 JSON Pointer. `data.pointer("/products/0/name")`. |

### 4.3 Leaf coercion

All return `Option[T]`. `None` collapses "wrong type" and "is null"
(matching serde_json's `as_*` semantics).

| Method | Signature |
|---|---|
| `Json.as_str()` | `→ Option[Str]` |
| `Json.as_int()` | `→ Option[Int]` |
| `Json.as_float()` | `→ Option[Float]` |
| `Json.as_bool()` | `→ Option[Bool]` |
| `Json.as_array()` | `→ Option[list[Json]]` |
| `Json.as_object()` | `→ Option[Json]` (stays Json so chaining continues) |

### 4.4 Inspection (no Option)

| Method | Signature |
|---|---|
| `Json.is_null()` | `→ bool` — true iff the value is the JSON `null` literal |
| `Json.has(key: str)` | `→ bool` — convenience for `get(key).is_some()` |
| `Json.to_string()` | `→ str` — render any Json back to text |

### 4.5 Retired methods

The pre-RFC `Result`-returning methods are **removed** (pre-1.0; no
back-compat shims):

- `get_str`, `get_int`, `get_float`, `get_bool` → use `get(k).and_then(fn(j) { j.as_str() })` or `pointer("/k").and_then(fn(j) { j.as_str() })`
- `get_object`, `get_array` → use `get(k).and_then(fn(j) { j.as_object() })` / `.as_array()`
- `length`, `keys` (today Int/list[Str]) → become `Option[Int]` / `Option[list[Str]]` since non-array/non-object receivers have neither

## 5. Canonical idioms

### Shallow, single field
```vox
let kind = data.get("kind").and_then(fn(j: Json) to Option[str] { j.as_str() })
```

### Deep, known path
```vox
let name = data.pointer("/products/0/name")
               .and_then(fn(j: Json) to Option[str] { j.as_str() })
               .unwrap_or("anonymous")
```

### Membership check before navigation
```vox
if data.has("action") {
    let action = data.get("action").unwrap()
    // ...
}
```

### Walking an array
```vox
let products = data.get("products").and_then(fn(j: Json) to Option[list[Json]] { j.as_array() }).unwrap_or([])
for p in products {
    let n = p.get("name").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("?")
    print(n)
}
```

### Exhaustive handling (when "I genuinely care about the error" — e.g. parse)
```vox
match json.parse(input) {
    Ok(data) => process(data),
    Error(msg) => log.error("bad json: " + msg)
}
```

## 6. Why this honors Vox principles

- **No silent failures:** every fallible access returns `Option`. Misses don't silently become `Json::Null`; they become `None`, which the type system forces the caller to handle (via `unwrap_or`, `and_then`, `match`, or explicit `unwrap` with the `_Panic` sentinel that the closures session already wired up).
- **Phonetic operators only:** no `?` / `?.` / `!` operator added. Chaining uses existing `.and_then` + closures (landed earlier this session) and existing `or` keyword via `unwrap_or`.
- **Low K-complexity:** one access pattern per concept. Navigation = Option[Json]. Leaf = Option[T]. Pointer = Option[Json]. No parallel APIs.
- **AI-friendly:** matches the Rust+Scala+Swift+OCaml Option-chaining pattern every LLM is trained on. `pointer` paths are dense and unambiguous.
- **Strict typing:** unlike `dynamic`/`any` types, Json values keep their type identity. The escape only happens at the leaf via `as_X`, which is where the user explicitly asks for unsafe coercion.

## 7. Implementation plan

1. **§4.1 signature fix** — change `json.parse` typeck signature to `Ty::Fn(vec![Ty::Str], Ty::Result(Ty::Named("Json")))`. Single commit, ~5 LoC.
2. **VoxJson methods** — rewrite [`actor-runtime/src/builtins/mod.rs`](../../../crates/vox-actor-runtime/src/builtins/mod.rs) `VoxJson` impl: drop the old `Result`-returning `get_*` methods, add `get/at/pointer` (Option) and `as_str/as_int/as_float/as_bool/as_array/as_object` (Option), add `has`.
3. **Eval dispatch** — update [`eval/builtins.rs`](../../../crates/vox-compiler/src/eval/builtins.rs) Json arm to dispatch new methods, return `VoxValue::Option(...)` everywhere.
4. **Typeck registrations** — update [`typeck/builtins.rs`](../../../crates/vox-compiler/src/typeck/builtins.rs) `Ty::Named("Json")` method table to match.
5. **Integration tests** — new `crates/vox-compiler/tests/json_ergonomics_test.rs` covering get/at/pointer miss → None; as_str wrong-type → None; has reports membership; closure chain works.
6. **Migrate 3 Bucket-B scripts** — surgical rewrites against new API.
7. **Update golden example** [`json_stdlib.vox`](../../../examples/golden/json_stdlib.vox) with the canonical pointer + closure-chain alongside the existing match form.
8. **Refresh corpus baseline.**

## 8. Deferred / out of scope

- **`@json_as(MyType)` decorator** — typed serde-style deserialization for known schemas. Significant language feature in its own right; deserves its own RFC. The Option+pointer surface serves the 80% case; the derive is a 90→99% improvement on top.
- **JSON merge-patch** (RFC 7396) — useful for config layering. Defer.
- **JSONPath query DSL** (`$.products[*].name`) — `pointer` covers single paths; JSONPath is multi-result. Defer until a corpus user needs it.
- **Streaming / large-file parsing** — current API is whole-document; sufficient for ~all current Vox use cases (LLM responses, configs, CLI outputs all fit in memory). Revisit if a real workload needs streaming.

## 9. Tradeoffs ratified

- **Single Option, lose distinguishing "key absent" from "key present but null":** at the navigation layer (`get`, `at`, `pointer`), both produce `None`. Callers who need the distinction (rare; mostly PATCH-style APIs) can use `Json.has(key)` to check membership separately before `.get`, then `.is_null()` on the result. Acceptable tradeoff: the API stays uniform for the 95% case.
- **No `?` shorthand:** chains use `.and_then` + closure literal. Verbose vs `?` but consistent with Vox's no-operator policy. The closure literal `fn(j: Json) to Option[str] { j.as_str() }` is verbose but explicit; future RFC may consider a method-reference sugar (`.and_then(Json::as_str)`).
- **Retire old `get_str`/`get_int` rather than coexist:** pre-1.0 we can take the breaking change; coexistence would double K-complexity for zero gain.

## 10. Migration impact

Files using `json.parse + .get_str/.get_int/.get_object/.get_array/.at` (audit grep 2026-05-23):
- `examples/golden/json_stdlib.vox` — update to canonical form (§5).
- `apps/vox-mental-tracker/src/main.vox` — uses `match std.json.parse(...)` then no typed accessors (the inner match unwraps the Json directly); unaffected.
- `scripts/quality/audit-workspace-health.vox` — rewrite against new API.
- `scripts/quality/audit-dependency-layers.vox` — rewrite (also has a Map-vs-Record issue; new API resolves the JSON side).
- `scripts/quality/generate-matrix-doc.vox` — rewrite.
- `scripts/mens-corpus/harvest.vox` — uses `json.stringify` only; unaffected.
- `scripts/generate-bench-scaffold.vox` — single use of `json.parse`; verify and update.

Expected corpus impact: +3 PASS (38 → 41 / 55, 75%).

## 11. Related

- [closures-rfc-2026-05-23.md](./closures-rfc-2026-05-23.md) — supplies the `.and_then(fn(...) {...})` chaining mechanism this API leans on.
- [vox-stdlib-gap-audit-2026-05-23.md](./vox-stdlib-gap-audit-2026-05-23.md) §10 — original identification of the 3 Bucket-B scripts blocked on this design.
- [intra-project-imports-rfc-2026-05-23.md](./intra-project-imports-rfc-2026-05-23.md) — sister RFC landed same session.
