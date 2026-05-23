---
title: "Web Model Reference"
description: "Reference for building APIs and interactive frontends with the Vox web model."
category: "Language Reference"
status: "current"
last_updated: "2026-04-08"
training_eligible: true

schema_type: "TechArticle"
---

# Reference: Web Model

Vox embraces a server-first web architecture. UI is declared with `component`; the
codegen emits plain React/TSX components that an external React/TanStack/mobile app
imports. (Historical: the `@island` decorator was retired 2026-05-03; see
[architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).)

## Interactive Components

Client-side interactive UI is modeled with `component` declarations.

```vox
// vox:skip
component ToggleBtn() {
    let on = false
    view: <button>{if on { "Active" } else { "Inactive" }}</button>
}
```

### Inner JSX Rules

Inside the body of any function that returns `Element`, you can directly emit standard JSX elements. Note that: 
- Variables are evaluated implicitly within `{braces}`.
- Handlers (`onClick`, `onChange`) capture inline lambda functions implicitly. 
- You do not need to call `return <div/>`; trailing expressions resolve correctly. 

## Inline HTTP Layout Mappings 

Vox enables inline API mapping without full standalone Axum scaffolding using raw web directives. 

- `http get "/path" -> ResultType { }`  
  *Triggers a standard asynchronous GET routing returning raw string, UI templates, or JSON output payloads depending on structural data boundaries.*
- `http post "/path" (body: BodyType) -> ResultType { }`  
  *Determines direct incoming payload structures explicitly mapped inside Vox structural ADT data types.*

## `routes { }` (canonical syntax, 2026)

Vox emits a **`routes.manifest.ts`** (`VoxRoute[]`) for adapters; the **normative surface in `.vox`** is:

- **Paths:** string literals with **`to`** before the component name: `"/" to Home`.
- **Loaders / pending:** `with loader: myQuery` and/or `with pending: Spinner` (tuple form `with (loader: a, pending: b)` supported).
- **Nesting:** child routes inside `{ ... }` after the parent entry (path strings only inside nested blocks).
- **Global screens:** `not_found: NotFoundPage` and `error: ErrorPage` in the `routes { }` body.

**Deferred (not in the parser yet):** `"/path" as layout Shell { }`, `under LayoutName`, redirect-only entries, wildcard segments, and populating `RouteEntry.redirect` / `is_wildcard` from source — see [`react-interop-implementation-plan-2026.md`](../archive/research-2026-q1/react-interop-implementation-plan-2026.md) and [`tanstack-start-codegen-spec.md`](../archive/research-2026-q1/tanstack-start-codegen-spec.md) (historical examples may overshoot grammar).

## Route table (legacy arrow sketch)

Older prose used arrow forms; prefer **`to`** and manifests per [`vox-web-stack.md`](./vox-web-stack.md).

```vox
// vox:skip
routes {
    "/" to Home
    "/dashboard" to AccountDashboard
}
```

## Compilation and React Interop (Behind the scenes)

The compiler lowers each `component` to a plain TSX file under the generated `app/`
directory. An external React frontend imports the components directly, and calls server
endpoints declared with `@endpoint` through the generated `vox-client.ts`. There is no
island-mount harness. See [architecture/external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md).
