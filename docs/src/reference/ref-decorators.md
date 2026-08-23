---
title: "Reference: Decorator Registry"
description: "All available decorators and their technical effects."
category: "Language Reference"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---
# Reference: Decorator Registry

Vox uses decorators to provide metadata to the compiler and runtime. This registry lists all available decorators and their technical effects. Note that `actor`, `workflow`, and `activity` are core keywords, not decorators.

## Backend & Logic

### `server` / `query` / `mutation`

Bare keywords (not decorators) introduced in Phase B (audit doc §11.2, 2026-05-23) and made canonical in v0.6.0. All three produce the same `Decl::Endpoint` AST node; they differ in execution semantics:

- **`server`** — general-purpose server function. Generates an Axum handler and a typed TS client.
- **`query`** — read-only operation. Optimized for concurrent reads; cannot perform mutations.
- **`mutation`** — write operation. Wraps execution in a database transaction.

- **Effect**: Generates a Rust Axum handler and a TypeScript client.
- **Usage**:
```vox
server greet(name: str) to str {
    return name
}

query ping() to str {
    return "ok"
}

mutation reset() to bool {
    return true
}
```

#### Retired: `@endpoint(kind: ...)` (v0.6.0)

The `@endpoint(kind: server|query|mutation)` form was retired in v0.6.0 per
[`vox-stdlib-gap-audit-2026-05-23.md` §Phase H step 18](../architecture/vox-stdlib-gap-audit-2026-05-23.md). The lexer no longer recognizes `@endpoint`; the parser reports it as an unknown token at the top level. The `retired/decorator-usage` lint surfaces a friendlier `Severity::Error` finding with a `server` / `query` / `mutation` migration suggestion before the parser sees it.

Migration is mechanical:

| Retired (≤ v0.5)               | Canonical (v0.6+) |
|--------------------------------|-------------------|
| `@endpoint(kind: server)`      | `server`          |
| `@endpoint(kind: query)`       | `query`           |
| `@endpoint(kind: mutation)`    | `mutation`        |

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
- **Goal**: Marks a function as pending removal.
- **Effect**: Emits a `typecheck.deprecated_ident` warning at every call site. With a reason argument, the reason is appended to the warning message.
- **Usage**:
  - Bare: `@deprecated fn old() to int { return 0 }` → warning: `'old' is deprecated`
  - With reason: `@deprecated("Use new_function instead") fn old() to int { return 0 }` → warning: `'old' is deprecated: Use new_function instead`

## Observability

### `@traced`
- **Goal**: Emit a distributed tracing span around a function's execution.
- **Effect**: The Rust codegen prepends `#[tracing::instrument(skip_all, name = "fn_name", fields(trace_id = tracing::field::Empty))]` to the generated function and injects a body statement that records the active `trace_id` from `vox_telemetry::current_trace_context()` onto the span when a trace context is present.
- **Backend**: Uses the [`tracing`](https://docs.rs/tracing) crate. A future OpenTelemetry exporter can subscribe to `tracing` spans without any change to decorated source code.
- **Usage**:

```vox
@traced
fn process_order(order_id: str) to str {
    return "ok"
}
```

The emitted Rust contains:

```rust
#[tracing::instrument(skip_all, name = "process_order", fields(trace_id = tracing::field::Empty))]
fn process_order(order_id: String) -> String {
    if let Some(__tc) = vox_telemetry::current_trace_context() {
        tracing::Span::current().record("trace_id", tracing::field::display(&__tc.trace_id));
    }
    // …body…
}
```

> [!NOTE]
> `@traced` on **endpoints / activities / workflows** is planned (TRACE-D P8) but not yet wired — those declaration kinds still hard-code `is_traced: false`. Plain `fn` declarations are fully supported.

## Data Modeling

### `table` (Keyword)
Tables are declared with the bare `table` keyword (not a decorator); `@table` was retired in v0.6.0.
- **Goal**: Defines a persistent database table.
- **Effect**: Generates Rust migrations and typed query interfaces.
- **Usage**:
```vox
table MyRecord {
    id: str
}
```

### `index` (Keyword)
Indexes are declared with the bare `index` keyword (not a decorator); `@index` was retired in v0.6.0.
- **Goal**: Creates a database index.
- **Effect**: Generates SQL for fast lookup on specified properties.
- **Usage**: `index MyRecord.by_id on (id)`

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

### `agent` (Tombstoned)
> [!NOTE]
> The `agent` declaration is not in the active grammar (parser support tracked, not shipped). Use a plain function plus MCP `tool`/`resource` declarations instead — see [How-To: Build AI Agents and MCP Tools](../how-to/how-to-ai-agents.md).
```vox
fn assistant_greet(name: str) to str {
    return "Hello " + name + ", how can I assist you today?"
}
```
### `tool` (Keyword)
Tools are declared with the bare `tool` keyword (not a decorator). `@tool` is a **hard parse error** (`vox/decorator/tool-retired`). The older dotted `@mcp.tool` still parses but emits a `vox/decorator/mcp-tool-deprecated` warning.
- **Goal**: Exports a function as an MCP tool.
- **Effect**: Registered with the MCP server for discovery by AI agents.
- **Usage**:
```vox
tool "Calculate the sum of two integers" sum(a: int, b: int) to int {
    return a + b
}
```

### `resource` (Keyword)
Resources are declared with the bare `resource` keyword (not a decorator). `@resource` is a **hard parse error** (`vox/decorator/resource-retired`). The dotted `@mcp.resource` remains valid, non-deprecated syntax, though bare `resource` is preferred for new code.
- **Goal**: Exposes dynamic readable content to MCP.
- **Effect**: Registers a resource URI endpoint via `getResources`.
- **Usage**:
```vox
resource "notes://recent" "Recent system notes" get_recent_notes() to str {
    return "This is a note from the system."
}
```
