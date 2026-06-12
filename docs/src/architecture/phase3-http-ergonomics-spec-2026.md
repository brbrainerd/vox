---
title: "Phase 3: HTTP Ergonomics Decorators Spec (2026)"
description: "Design spec for explicit method/path, @cors, @auth, and @rate_limit decorators on Vox endpoints."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Implementation spec for HTTP ergonomics; required reading before touching endpoint codegen or HIR endpoint nodes."
---

# Phase 3: HTTP Ergonomics Decorators Spec (2026)

## 0. Prerequisites and scope

Read before this document:
- [`wire-format-v1-ssot.md`](wire-format-v1-ssot.md) — path param and query string serialization rules
- `crates/vox-compiler/src/hir/nodes/decl.rs` `HirEndpointFn` (lines 340–363) — current HIR shape
- `crates/vox-rn-codegen/src/routes.rs` — current Express route emitter

**Grammar unification rule (AGENTS.md):** new behavior goes on decorators, not bare keywords. Every feature in this phase is a decorator or an extension of an existing decorator.

**Phase 3 adds:** explicit HTTP method + path params, `@cors`, `@auth`, and `@rate_limit`. Phase 4 adds the stdlib JWT primitive referenced by `@auth`.

---

## 1. Extended `@endpoint` decorator

### 1.1 New parameters

| Param | Type | Required | Default |
|---|---|---|---|
| `kind` | `query\|mutation\|server` | yes (existing) | — |
| `method` | `GET\|POST\|PUT\|DELETE\|PATCH` | no | derived from `kind` (see §1.2) |
| `path` | string literal | no | `/api/{fn_name}` |

`method` and `path` are independent; either may be omitted.

### 1.2 Method defaulting

When `method` is absent the compiler applies the same heuristic as today:

- `kind: query` → `GET`
- `kind: mutation` or `kind: server` → `POST`

Supplying an explicit `method` overrides this. Overriding `query` with `POST` is permitted (useful for large filter payloads).

### 1.3 Path parameters

Path parameters are colon-prefixed segments: `/users/:id`. At compile time the compiler validates:

1. Every `:name` segment in `path` must match the name of a function parameter exactly (case-sensitive).
2. The matched parameter type must be `str`, `int`, `float`, or any type implementing `FromStr` (v1: `str` and `int` only; other types are a hard error).
3. Path param names must be unique within the path string.

Compile error examples:

```
error[E0801]: path param `:id` has no matching parameter in fn signature
  --> src/api.vox:12:16
   |
   | @endpoint(kind: query, path: "/users/:id")
   |                                       ^^^
```

```
error[E0802]: path param `:id` matches parameter `id: bool` — only `str` and `int` are valid path param types in v1
```

### 1.4 Query string parameters

Function parameters **not** bound to a path segment become query string fields on `GET` endpoints and body fields on `POST`/`PUT`/`PATCH`/`DELETE` endpoints, following the wire-format-v1 encoding rules (§2.1 and §2.2 of that document).

For `GET` with a `path`, the sorted-key `encodeURIComponent(JSON.stringify(value))` encoding from wire-format-v1 §2.1 applies to unbound params.

### 1.5 Syntax examples

```vox
// vox:skip
// Explicit GET with path param — id is extracted from the URL segment
@endpoint(kind: query, method: GET, path: "/users/:id")
fn get_user(id: str) to User {
    return db.User.get({ id: id })
}

// DELETE with path param
@endpoint(kind: mutation, method: DELETE, path: "/users/:id")
fn delete_user(id: str) to Unit {
    db.User.delete({ id: id })
}

// PUT with path param + body fields (name, active are unbound → request body)
@endpoint(kind: mutation, method: PUT, path: "/users/:id")
fn update_user(id: str, name: str, active: bool) to User {
    return db.User.update({ id: id }, { name: name, active: active })
}

// Backward-compatible: no path → auto-routes to /api/user_count
@endpoint(kind: query)
fn user_count() to int {
    return len(db.User.all())
}
```

### 1.6 Backward compatibility

Any `@endpoint` without an explicit `path` continues to auto-route to `/api/{fn_name}` exactly as before. No existing code breaks.

---

## 2. `@cors` decorator

### 2.1 Parameters

| Param | Type | Default | Notes |
|---|---|---|---|
| `origins` | `[str]` | required | List of allowed origin strings; `["*"]` permits all |
| `credentials` | `bool` | `false` | Sets `Access-Control-Allow-Credentials` |
| `max_age` | `int` | `86400` | Preflight cache seconds |

### 2.2 Scoping

`@cors` may appear at two sites:

- **Module scope** (file top-level, before any `fn`): applies to every endpoint in the file.
- **Per-endpoint** (directly above `@endpoint`): applies only to that function; overrides a module-scope `@cors` entirely for that endpoint (no merging).

### 2.3 Fail-closed behavior

If `@cors` is absent on an endpoint (and no module-scope `@cors` is in effect), the generated server emits **no** CORS headers for that endpoint. `app.use(cors())` in the current route emitter is removed in Phase 3; CORS is explicit-only.

### 2.4 Emitted code

The route emitter wraps the endpoint's Axum handler in a `tower_http::cors::CorsLayer` configured from the decorator arguments. For Express (current backend), a scoped `cors()` call with an options object replaces the current global `app.use(cors())`.

```vox
// vox:skip
// Module-scope CORS: applies to all endpoints in this file
@cors(origins: ["https://app.example.com"], credentials: true)

@endpoint(kind: query, path: "/users/:id")
fn get_user(id: str) to User {
    return db.User.get({ id: id })
}

// Per-endpoint CORS override (open during development)
@cors(origins: ["*"])
@endpoint(kind: query, path: "/debug/ping")
fn ping() to str {
    return "pong"
}
```

### 2.5 OpenAPI output

CORS origins are emitted as an `x-cors-origins` extension on the OpenAPI path item. `credentials` maps to `x-cors-credentials`.

---

## 3. `@auth` decorator

### 3.1 Parameters

| Param | Type | Default | Notes |
|---|---|---|---|
| `scheme` | `"bearer"` | required | Only `bearer` in v1 |
| `optional` | `bool` | `false` | If `true`, missing token proceeds unauthenticated; handler receives `Option[Claims]` |

### 3.2 Behavior

When `@auth(scheme: bearer)` is present on an endpoint:

1. The generated middleware extracts the `Authorization: Bearer <token>` header.
2. The token is validated using the Phase 4 stdlib `jwt.verify()` primitive (stubbed in Phase 3 tests).
3. On validation failure → `401` with the wire-format-v1 error envelope:
   ```json
   { "ok": false, "code": "UNAUTHORIZED", "message": "Bearer token missing or invalid" }
   ```
4. On success the decoded `Claims` struct is injected into the handler context.

`optional: true` is intended for endpoints that customize their response for authenticated vs. anonymous callers. The function signature must accept an `Option[Claims]` context parameter (exact injection syntax TBD in Phase 4 when the stdlib type is defined; Phase 3 spec reserves the behavior).

### 3.3 `@role` companion decorator

`@role` is proposed as a separate decorator (not merged into `@auth`) so it can be composed independently.

```vox
// vox:skip — @role requires Phase 4 Claims injection
@auth(scheme: bearer)
@role("admin")
@endpoint(kind: mutation, method: DELETE, path: "/users/:id")
fn admin_delete_user(id: str) to Unit {
    db.User.delete({ id: id })
}
```

When `@auth` is present without `@role`, any valid token passes. When `@role` is also present, the middleware checks the `roles` claim and returns `403` with `code: "FORBIDDEN"` on mismatch.

### 3.4 Current auth pattern migration

The existing manual pattern from `examples/golden/auth_patterns.vox` (manual `verify_token` + `require_admin` calls inside the handler body) remains valid. `@auth` + `@role` is the declarative shorthand for the common case.

### 3.5 OpenAPI output

`@auth(scheme: bearer)` emits an OpenAPI `securitySchemes` entry:

```yaml
securitySchemes:
  bearerAuth:
    type: http
    scheme: bearer
    bearerFormat: JWT
```

And a `security` constraint on the path operation:

```yaml
security:
  - bearerAuth: []
```

`optional: true` is noted as `x-auth-optional: true` on the operation.

---

## 4. `@rate_limit` decorator

### 4.1 Parameters

| Param | Type | Required | Notes |
|---|---|---|---|
| `per` | `"1s"\|"1m"\|"1h"` | yes | Window duration |
| `max` | `int` | yes | Max requests per window |
| `key` | `by_ip\|by_user\|by_endpoint` | yes | Keying strategy |

`by_user` requires `@auth` to be present on the same endpoint (or module-scope auth); if `@auth` is absent with `by_user` the compiler emits a hard error.

### 4.2 Scoping

Same rules as `@cors`: module-scope or per-endpoint. Per-endpoint overrides module-scope entirely.

### 4.3 Emitted code

Axum backend: emits a `tower_governor::GovernorLayer` configured per the decorator params. Express backend: emits an `express-rate-limit` middleware wrapping the individual route handler.

On limit exceeded → `429` with envelope:

```json
{ "ok": false, "code": "RATE_LIMITED", "message": "Too many requests", "details": { "retry_after_ms": 1000 } }
```

`Retry-After` response header is also set (seconds, integer).

### 4.4 Example

```vox
// vox:skip
// Module-scope rate limit: 60 req/min per IP for all endpoints in file
@rate_limit(per: "1m", max: 60, key: by_ip)

@auth(scheme: bearer)
@rate_limit(per: "1s", max: 5, key: by_user)
@endpoint(kind: mutation, method: POST, path: "/messages")
fn send_message(body: str) to Unit {
    db.Message.insert({ body: body })
}
```

### 4.5 OpenAPI output

Rate limit params are emitted as an `x-rate-limit` extension on the path operation:

```yaml
x-rate-limit:
  per: "1m"
  max: 60
  key: by_ip
```

---

## 5. Compile-time validation

### 5.1 Duplicate route detection

The existing `validate_express_route_emit_input` in `routes.rs` performs `(method, path)` deduplication on literal paths. Phase 3 extends this to:

1. **Exact duplicate:** `POST /users` registered twice → hard error (existing behavior).
2. **Path-param overlap:** `/users/:id` and `/users/:slug` with the same method → hard error. The param name does not change the effective route pattern; two routes matching the same structural pattern conflict.

Detection algorithm: normalize each path by replacing `:name` segments with the placeholder `{*}`, then check for `(method, normalized_path)` duplicates.

```
error[E0803]: route conflict: `GET /users/:slug` and `GET /users/:id` match the same URL pattern
  --> src/api.vox:20:1
```

### 5.2 Path param type check

Enforced during HIR lowering (before codegen). See §1.3. This is a hard error; no warning level.

### 5.3 `@auth` required for `by_user` rate limit

Enforced at the decorator validation pass after HIR lowering. Emits:

```
error[E0804]: `@rate_limit(key: by_user)` requires `@auth` on the same endpoint or module scope
```

### 5.4 OpenAPI 3.1 completeness

All Phase 3 decorators must be reflected in OpenAPI output before a Phase 3 endpoint is considered complete for CI:

- Path params → `parameters` array with `in: path`, type derived from Vox param type.
- Query params → `parameters` array with `in: query`, wire-format-v1 encoding noted.
- `@auth` → `securitySchemes` + per-operation `security`.
- `@cors` → `x-cors-origins` / `x-cors-credentials` extensions.
- `@rate_limit` → `x-rate-limit` extension.

---

## 6. HIR and codegen changes

### 6.1 `HirEndpointFn` additions

The current shape (decl.rs lines 340–363) adds these fields:

```rust
// vox:skip — proposed HIR additions
pub struct HirEndpointFn {
    // --- existing fields ---
    pub kind: HirEndpointKind,
    pub id: DefId,
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: Vec<HirStmt>,
    pub route_path: String,
    pub is_pure: bool,
    pub effects: HirEffectSet,
    pub span: Span,

    // --- Phase 3 additions ---
    /// Explicit HTTP method; None = derive from kind (existing behavior).
    #[serde(default)]
    pub http_method: Option<HirHttpMethod>,

    /// Path parameter names extracted from route_path (e.g. ["id"] for "/users/:id").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_params: Vec<String>,

    /// CORS config; None = no CORS headers emitted.
    #[serde(default)]
    pub cors: Option<HirCorsConfig>,

    /// Auth config; None = no auth middleware.
    #[serde(default)]
    pub auth: Option<HirAuthConfig>,

    /// Rate limit config; None = no rate limit middleware.
    #[serde(default)]
    pub rate_limit: Option<HirRateLimitConfig>,
}
```

`HirCorsConfig`, `HirAuthConfig`, and `HirRateLimitConfig` are new structs mirroring the decorator param shapes above. All are `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`.

`HirHttpMethod` already exists (used by `HirRoute`); reuse it for `http_method` rather than adding a parallel type.

### 6.2 Route validation changes (`routes.rs`)

- `validate_express_route_emit_input`: add normalized-path duplicate check (§5.1).
- `sorted_endpoint_fns`: no changes needed; sort is already by `route_path` then name.
- `generate_routes_from_ctx`: per-endpoint CORS and rate-limit middleware wrapping replaces the global `app.use(cors())` call. Auth middleware injection added before body execution.

### 6.3 New `codegen_rust/` middleware emitter

A new module `crates/vox-compiler/src/codegen_rust/middleware.rs` emits Tower layer stacks. It is not a concern of the existing TypeScript emitter. The TypeScript (Express) emitter handles Phase 3 decorators inline in `routes.rs` for parity during the transition period.

### 6.4 Estimated scope

| Area | Estimated effort |
|---|---|
| HIR struct additions + serde | 0.5 day |
| Decorator parser → HIR lowering | 1 day |
| Compile-time validation (§5) | 1 day |
| Express codegen (routes.rs) | 1 day |
| Axum middleware emitter (codegen_rust) | 2 days |
| OpenAPI 3.1 output | 1 day |
| Golden tests + wire-format fixture updates | 1 day |
| **Total** | **~7.5 days** |

Phase 4 dependency: `@auth` middleware is fully wired only after the stdlib `jwt.verify()` primitive lands. Phase 3 ships `@auth` with a stub validator that accepts any well-formed JWT so the decorator pipeline and OpenAPI output can be validated end-to-end independently.

---

## 7. Changelog from Phase 2

| Concern | Phase 2 state | Phase 3 change |
|---|---|---|
| HTTP method | Derived from `kind` only | `method` param on `@endpoint` overrides |
| Path | Auto `/api/{fn_name}` | `path` param with compile-time param binding |
| CORS | Global `app.use(cors())` always on | Explicit `@cors`; fail-closed |
| Auth | Manual inside handler (see `auth_patterns.vox`) | Declarative `@auth` + `@role` |
| Rate limiting | Not supported | `@rate_limit` per-endpoint or module-scope |
| Duplicate detection | Exact `(method, path)` match | Extended to path-param structural overlap |
| OpenAPI | Basic routes + types | Adds security schemes, CORS/rate-limit extensions, path param `parameters` |
