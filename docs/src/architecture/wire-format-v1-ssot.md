---
title: "Wire Format v1 SSOT"
description: "Versioned specification for how Vox types are encoded over HTTP between a Vox backend and any TypeScript/React consumer."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Canonical wire format contract; required reading for any Vox API consumer or emitter."
---

# Wire Format v1 SSOT

## 1. Scope and version

This document is the single source of truth for how Vox types are serialized over HTTP.
Version: **v1** (logical contract version). Current compiler paths are under **`/api/…`** without a **`/v1`** segment (see §2); breaking generations use a new path prefix (e.g. **`/api/v2/`**, §7).

**Semver discipline for breaking changes:**
- A *breaking change* increments the major version (v1 → v2) and requires a new base-path segment.
- A *backward-compatible addition* (new optional field, new endpoint) may ship without a version bump.
- *Breaking* means: removing a field, changing a type encoding, renaming a discriminant key, or altering error envelope shape.

---

## 2. Transport conventions

| Concern | Rule |
|---|---|
| Host and path roots | HTTPS base `https://<host>/`. Endpoints live under **`/api/`** with **no `/v1` segment** in current compiler output: **`/api/query/<name>`** (query), **`/api/mutation/<name>`** (mutation), **`/api/<name>`** (server fn). Constants: [`web_prefixes.rs`](../../../crates/vox-compiler/src/web_prefixes.rs). A future breaking generation would move under **`/api/v2/`** (see §7). |
| Query endpoints (`@endpoint(kind: query)`) | HTTP `GET`; parameters as query string (see §2.1) |
| Mutation/server endpoints (`@endpoint(kind: mutation)`, `@endpoint(kind: server)`) | HTTP `POST`; JSON body |
| Request `Content-Type` | `application/json` |
| Response `Content-Type` | `application/json; charset=utf-8` |
| Character encoding | UTF-8 throughout |
| TLS | Required in production; plain HTTP permitted in `dev` only |

### 2.1 Query parameter encoding

`@endpoint(kind: query)` endpoint parameters are serialized as a query string with keys in **sorted lexicographic order**. Each value is `encodeURIComponent(JSON.stringify(value))`.

**OpenAPI:** generated specs describe each query parameter with a `schema` for the logical type **after** URI decoding and `JSON.parse`. Parameter descriptions reference this section. Tools that generate clients from OpenAPI alone must still apply JSON parsing per value (not treat raw query tokens as primitive strings).

```vox
// vox:skip — illustrative endpoint definition
@endpoint(kind: query)
fn search_items(filter: str, limit: int) to str { return "" }
```

Wire URL:
```
GET /api/query/search_items?filter=%22books%22&limit=20
```
(Values are JSON text after encoding, per §2.1; the illustrative `filter` value is a quoted JSON string.)

### 2.2 Mutation body encoding

`@endpoint(kind: mutation)` and `@endpoint(kind: server)` endpoints receive a JSON object whose keys are the parameter names.

```vox
// vox:skip — illustrative endpoint definition
@endpoint(kind: mutation)
fn create_order(item_id: str, quantity: int) to bool { return true }
```

Wire request body:
```json
{ "item_id": "abc123", "quantity": 3 }
```

---

## 3. Type encoding table

### 3.1 Primitive types

| Vox type | JSON wire type | Notes |
|---|---|---|
| `bool` | `boolean` | `true` / `false` |
| `int` | `number` | 32-bit signed; safe in JSON |
| `float` | `number` | IEEE 754 double |
| `string` | `string` | UTF-8 |
| `Decimal` | `string` | Arbitrary-precision decimal; e.g. `"19.99"` |
| `BigInt` | `string` | Arbitrary-precision integer; e.g. `"9007199254740993"` |
| `Date` | `string` | RFC 3339, UTC, date-only: `"2026-05-01"` |
| `DateTime` | `string` | RFC 3339, UTC: `"2026-05-01T14:30:00Z"` |
| `Duration` | `string` | ISO 8601 duration: `"PT1H30M"` |
| `unit` / `()` | omitted | Not serialized; 204 No Content for endpoints returning unit |

**`Decimal` and `BigInt` MUST be strings.** JSON `Number` loses precision past 2^53; any Vox type whose domain exceeds that range is encoded as a quoted string on the wire. TypeScript consumers decode with `BigInt(value)` or a Decimal library.

**Date/Time MUST be RFC 3339 UTC strings.** Raw epoch integers are forbidden in v1. Consumers parse with `new Date(value)` or a date library.

### 3.2 Composite types

| Vox type | JSON wire type | Notes |
|---|---|---|
| `Option<T>` | `T` or absent key | Absent key, not `null`. See §4. |
| `List<T>` / `Vec<T>` | `readonly T[]` | JSON array |
| `Map<K, V>` | `{ [key: string]: V }` | Keys are always `string`; non-string Vox keys are `toString()`-encoded |
| Struct / record | `object` | Fields in declaration order |
| Tuple `(A, B, C)` | `[A, B, C]` | JSON array, positional |
| `Result<T, E>` | See §3.3 | Encoded as discriminated union |
| Sum type / enum | See §5 | Discriminated union with `_tag` |

### 3.3 `Result<T, E>` encoding

`Result<T, E>` is a sum type with two variants and follows the `_tag` convention (§5):

```json
{ "_tag": "Ok",  "value": <T> }
{ "_tag": "Err", "value": <E> }
```

**Endpoint-level errors** use the error envelope (§6) instead of `Result` on the wire; `Result<T, E>` as an encoded field type is for domain values that carry explicit failure state.

---

## 4. Null vs absent distinction

**Rule:** `Option<T>` serializes as an **absent key** when the value is `None`. It never serializes as JSON `null`.

```vox
// vox:skip
type UserProfile {
    id: string
    display_name: string
    bio: Option<string>
}
```

Wire JSON when `bio` is present:
```json
{ "id": "u1", "display_name": "Alice", "bio": "Rust & coffee" }
```

Wire JSON when `bio` is absent:
```json
{ "id": "u1", "display_name": "Alice" }
```

**`@nullable` override:** applying `@nullable` to an `Option<T>` field instructs the emitter to serialize `None` as JSON `null` and keep the key present. Use only for interop with consumers that require explicit nulls (e.g., some SQL-backed ORMs).

```vox
// vox:skip
type LegacyRow {
    id: string
    @nullable
    deprecated_field: Option<string>
}
```

Wire JSON when `deprecated_field` is absent:
```json
{ "id": "r1", "deprecated_field": null }
```

The `@nullable` override is explicit opt-in. The default is always absent-key.

---

## 5. Sum type / enum discriminant convention

All Vox sum types encode as a discriminated union using a `_tag` string-literal field. The `_tag` value is the exact variant name as declared in Vox source.

```vox
// vox:skip
type Shape {
    Circle { radius: Decimal }
    Rectangle { width: Decimal, height: Decimal }
    Point
}
```

Wire JSON examples:

```json
{ "_tag": "Circle", "radius": "5.00" }
```

```json
{ "_tag": "Rectangle", "width": "10.00", "height": "4.50" }
```

```json
{ "_tag": "Point" }
```

TypeScript discriminated union (emitted by `codegen_ts`):
```typescript
type Shape =
  | { _tag: "Circle";    radius: string }
  | { _tag: "Rectangle"; width: string; height: string }
  | { _tag: "Point" };
```

**Rules:**
- `_tag` is always the first key in the serialized object.
- Unit variants (no payload fields) serialize as `{ "_tag": "VariantName" }` only.
- `_tag` is a reserved key name in Vox struct fields; the compiler rejects a struct with a field named `_tag`.

---

## 6. Error envelope

All endpoint errors (4xx, 5xx) return a JSON body with this shape:

```typescript
type ErrorEnvelope = {
  ok: false;
  code: string;       // machine-readable, stable, SCREAMING_SNAKE_CASE
  message: string;    // human-readable; may change across releases
  request_id?: string; // present when the server assigned a trace ID
  details?: unknown;  // optional structured payload; shape is code-specific
};
```

Wire example:

```json
{
  "ok": false,
  "code": "ITEM_NOT_FOUND",
  "message": "No item with id 'abc123' exists.",
  "request_id": "req_01hv2k3mxnpqr"
}
```

**Rules:**
- `ok` is always the literal `false` for errors; success responses do not include an `ok` field.
- HTTP status code is authoritative for the error class (4xx = client, 5xx = server).
- `code` values are owned by the Vox application; the wire format does not reserve any specific codes beyond requiring SCREAMING_SNAKE_CASE.
- Validation errors MAY include a `details` array of per-field diagnostics; shape is unspecified in v1 and must not be relied upon by consumers without coordination.

---

## 7. Versioning policy

| Change type | Action |
|---|---|
| New endpoint (additive) | Ship in v1; no bump |
| New optional response field | Ship in v1; no bump |
| New optional request parameter | Ship in v1; no bump |
| Remove or rename a field | **Breaking** — requires v2 base path |
| Change a type encoding (e.g. `int` → `string`) | **Breaking** — requires v2 base path |
| Rename a `_tag` variant | **Breaking** — requires v2 base path |
| Change error envelope shape | **Breaking** — requires v2 base path |
| New required request parameter | **Breaking** — requires v2 base path |

**v2 signaling:** a v2 API is hosted at `/api/v2/`. v1 and v2 may coexist during a migration window; the sunset date for v1 is communicated via a `Deprecation` response header (`Deprecation: true; sunset="2027-01-01"`). The Vox compiler emits both base paths when `[api] compat_versions = ["v1", "v2"]` is set in `Vox.toml`.

---

## 8. Golden test requirement

Any change to type encoding, the error envelope shape, or query parameter serialization MUST update the corresponding golden test fixtures under `crates/vox-codegen/tests/golden/wire-format/` (see `crates/vox-codegen/tests/wire_format_golden.rs`) before the change is merged. Expand that directory as new wire fixtures are added.
