---
title: "Reference: Decorator Registry"
description: "All available decorators and their technical effects."
category: "Language Reference"
status: "current"
last_updated: "2026-05-26"
training_eligible: true

schema_type: "TechArticle"
---
# Reference: Decorator Registry

Vox uses decorators to provide metadata to the compiler and runtime. This registry lists all available decorators and their technical effects. Note that `actor`, `workflow`, and `activity` are core keywords, not decorators.

## Backend & Logic

### `@server` / `@query` / `@mutation`

Bare-form endpoint decorators introduced in Phase B (audit doc §11.2, 2026-05-23) and made canonical in v0.6.0. All three produce the same `Decl::Endpoint` AST node; they differ in execution semantics:

- **`@server`** — general-purpose server function. Generates an Axum handler and a typed TS client.
- **`@query`** — read-only operation. Optimized for concurrent reads; cannot perform mutations.
- **`@mutation`** — write operation. Wraps execution in a database transaction.

- **Effect**: Generates a Rust Axum handler and a TypeScript client.
- **Usage**:
```vox
@server
fn greet(name: str) to str {
    return name
}

@query
fn ping() to str {
    return "ok"
}

@mutation
fn reset() to bool {
    return true
}
```

#### Retired: `@endpoint(kind: ...)` (v0.6.0)

The `@endpoint(kind: server|query|mutation)` form was retired in v0.6.0 per
[`vox-stdlib-gap-audit-2026-05-23.md` §Phase H step 18](../architecture/vox-stdlib-gap-audit-2026-05-23.md). The lexer no longer recognizes `@endpoint`; the parser reports it as an unknown token at the top level. The `retired/decorator-usage` lint surfaces a friendlier `Severity::Error` finding with a `@server` / `@query` / `@mutation` migration suggestion before the parser sees it.

Migration is mechanical:

| Retired (≤ v0.5)               | Canonical (v0.6+) |
|--------------------------------|-------------------|
| `@endpoint(kind: server)`      | `@server`         |
| `@endpoint(kind: query)`       | `@query`          |
| `@endpoint(kind: mutation)`    | `@mutation`       |

See also: [migration guide 0.5 → 0.6](./migration-0.5-to-0.6.md).

### `@scheduled`
> [!NOTE]
> Planned — not yet parseable.
- **Goal**: Run a background task periodically.
- **Effect**: Compiles to a Tokio timer loop or cron job scheduling block.
- **Usage**:
```vox
// vox:skip
@scheduled("0 * * * *")
fn hourly_task() { 
    // Logic here
}
```

### `@pure`
> [!NOTE]
> Planned — not yet parseable.
- **Goal**: Designates a function as side-effect free.
- **Effect**: Allows the compiler to aggressively optimize and caching the output.
- **Usage**: `@pure fn compute_hash(data: str) to str { return data }`

### `@deprecated`
> [!NOTE]
> Planned — not yet parseable.
- **Goal**: Marks a function or type as pending removal.
- **Effect**: Emits compiler warnings when used.
- **Usage**: `@deprecated("Use new_function instead")`

## Data Modeling

### `@table`
- **Goal**: Defines a persistent database table.
- **Effect**: Generates Rust migrations and typed query interfaces.
- **Usage**:
```vox
// vox:skip
@table type MyRecord {
    id: str
}
```

### `@index`
- **Goal**: Creates a database index.
- **Effect**: Generates SQL for fast lookup on specified properties.
- **Usage**: `@index MyRecord.by_id on (id)`

### `@require`
- **Goal**: Adds runtime validation guards.
- **Effect**: Injects validation checks before assignment/constructor.
- **Usage**:
```vox
// vox:skip
@require(len(self.pwd) > 8)
type User {
    pwd: str
}
```

## UI & Frontend

#### Replaced: `@island` (retired)

`@island` is no longer recognized by the compiler. Use `component` for UI; the compiler emits plain React/TSX for external React apps to import. See [architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).

### `@loading`
- **Goal**: Suspense / transition UI for TanStack Router while a lazy route or data boundary resolves.
- **Effect**: Emits `{Name}.tsx`. When `routes { }` produces the router shim, this becomes the `pendingComponent`.
- **Usage**:
```vox
@loading
fn Spinner() to Element {
    return text() { "loading" }
}
```

### `@v0`
- **Goal**: Retrieve an AI-generated React component natively via Vercel's unofficial CLI.
- **Effect**: Downloads `.tsx` implementation and emits it as a React component.
- **Usage**: `@v0 "chat-id" fn Dashboard() to Element { return text() { "loading" } }`

## Testing & Tooling

### `@test`
- **Goal**: Marks a function as a test case for `vox test`.
- **Effect**: Included in the project test suite.
- **Usage**: `@test fn check_auth() { assert(true) }`

### `@mock`
> [!NOTE] 
> **Planned.** Not yet supported by the parser. Use standard functions for test setup or `spawn` dependencies.

### `@fixture`
> [!NOTE] 
> **Planned.** Not yet supported by the parser. Use helper functions called within `@test` blocks instead.

### `agent` (Keyword)
Agents are defined using the `agent` keyword (not a decorator).
```vox
// vox:skip
agent Assistant { 
    instructions: "Help the user"
    tools: [search_kb]
}
```
### `@mcp.tool`
- **Goal**: Exports a function as an MCP tool.
- **Effect**: Registered with the MCP server for discovery by AI agents.

```vox
{{#include ../../../examples/golden/ref_orchestrator.vox:mcp_tool}}
```

### `@mcp.resource`
- **Goal**: Exposes dynamic readable content to MCP.
- **Effect**: Registers a resource URI endpoint via `getResources`.

```vox
{{#include ../../../examples/golden/ref_orchestrator.vox:mcp_resource}}
```
