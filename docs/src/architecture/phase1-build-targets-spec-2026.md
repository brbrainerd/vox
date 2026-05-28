---
title: "Phase 1: Build Target Split Spec (2026)"
description: "Design spec for vox build --target=server|fullstack|client, vox emit client, and vox init --kind=backend."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Implementation spec for the build target split; required reading before touching vox-cli build or Vox.toml manifest code."
---

# Phase 1: Build Target Split

This document specifies the changes needed to make `server`-only and TypeScript
`client` SDK builds first-class modes alongside the existing `fullstack` default.
No existing behavior changes. All three modes share the same compiler frontend
(parse → HIR → validation); only the codegen and artifact layout differ.

---

## 1. The Three Targets

### `fullstack` (existing default, unchanged)

Produces everything the compiler currently produces:

- `dist/` — TypeScript/React frontend files (`App.tsx`, `routes.manifest.ts`, etc.)
- `target/generated/` — Axum Rust backend crate
- `dist/app/` — Vite scaffold (written when `--scaffold` / `VOX_WEB_EMIT_SCAFFOLD=1`)

`vox run` continues to detect `has_frontend` from `.tsx` files in `dist/` and
launches Vite + Axum exactly as today.

### `server`

Produces only the Rust backend. Skips all TypeScript codegen and Vite scaffolding.

- `target/generated/` — Axum Rust backend crate (identical to fullstack)
- No `dist/` directory is created or touched
- `vox run --target=server` starts Cargo without the Vite guard

Intended for: pure API services, microservices, background workers, teams that
bring their own frontend.

### `client`

Produces a zero-runtime TypeScript SDK package. Skips Rust codegen entirely.

- `dist-client/` (or `--out=<dir>`) — npm-publishable package (see §3)
- No `target/generated/` Rust crate is created or touched

Intended for: publishing a typed fetch client to npm so external projects can
call the Vox API without copying generated types by hand.

---

## 2. Manifest Change (`Vox.toml`)

Add an optional `[build]` table to per-project `Vox.toml` manifests:

```toml
[build]
# Accepted values: "fullstack" | "server" | "client"
# Omitting this key is equivalent to target = "fullstack".
target = "server"
```

**Override order (highest to lowest priority):**

1. `--target=<value>` CLI flag
2. `VOX_BUILD_TARGET` environment variable
3. `[build] target` in `Vox.toml`
4. Implicit default: `fullstack`

The existing workspace-level `Vox.toml` (kind = "workspace") is not affected;
the `[build]` table lives only in per-application manifests.

**Reading the value in vox-config:**

```rust
// vox-config/src/lib.rs  (new field on VoxConfig)
pub build_target: BuildTarget,   // default: BuildTarget::Fullstack

pub enum BuildTarget { Fullstack, Server, Client }
```

---

## 3. `vox emit client --lang=ts --out=<dir>`

A new subcommand that runs the compiler up through HIR, then emits a
self-contained TypeScript SDK package. No Vox runtime is imported; the package
has zero mandatory runtime dependencies.

### Output package structure

```
<out>/
  package.json        # name, version, "type":"module", exports, devDependencies (TS only)
  index.ts            # re-exports everything; barrel file
  types.ts            # shared request/response types, generated from @table + @server/@query/@mutation signatures
  client.ts           # VoxClient class (fetch wrapper, see below)
  schemas.ts          # (optional, --zod flag) zod validators for each type
```

**`package.json` shape:**

```json
{
  "name": "@your-org/my-app-client",
  "version": "0.1.0",
  "type": "module",
  "main": "./index.ts",
  "exports": { ".": "./index.ts" },
  "devDependencies": { "typescript": "^5.0.0" }
}
```

The `name` and `version` are read from the project's `Vox.toml` `[package]`
table; both are required when using `vox emit client`.

### Fetch client interface contract

`client.ts` exposes a single class. No axios, no node-fetch, no internal Vox
symbols — consumers supply a `fetch`-compatible function.

```typescript
// generated client.ts

import type { FetchFn, VoxClientOptions } from "./types.js";

export class VoxClient {
  private baseUrl: string;
  private fetch: FetchFn;

  constructor(options: VoxClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, "");
    this.fetch = options.fetch ?? globalThis.fetch;
  }

  // For each @query fn user_count() to int:
  async userCount(): Promise<number> {
    const res = await this.fetch(`${this.baseUrl}/user_count`);
    if (!res.ok) throw new Error(`userCount failed: ${res.status}`);
    return res.json();
  }

  // For each @mutation fn seed_user(name: str) to Unit:
  async seedUser(name: string): Promise<void> {
    const res = await this.fetch(`${this.baseUrl}/seed_user`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name }),
    });
    if (!res.ok) throw new Error(`seedUser failed: ${res.status}`);
  }
}
```

`VoxClientOptions` in `types.ts`:

```typescript
export type FetchFn = typeof globalThis.fetch;

export interface VoxClientOptions {
  baseUrl: string;
  fetch?: FetchFn;  // injectable for tests, React Native, Node 18+, etc.
}
```

### Zero internal Vox runtime imports

The emitter must not write any `import ... from "vox-*"` or `import ... from
"@vox/*"` lines into client package files. A CI lint rule (similar to the
existing `UnlabeledCodeFence` lint) should enforce this over the output dir.

---

## 4. `vox init --kind=backend`

`vox init` gains a `--kind` flag (analogous to the existing `--template` flow).

| Flag | Scaffold produced |
|---|---|
| `--kind=application` | existing default: fullstack app with Vite/React |
| `--kind=backend` | server-only project, no frontend files |

**`--kind=backend` scaffold:**

```
my-api/
  Vox.toml            # [build] target = "server"
  src/
    main.vox          # @query/@mutation/@server stubs, no @page declarations
  .gitignore
  README.md
```

`main.vox` stub:

```vox
@query
fn hello() to str {
    return "hello from vox backend"
}
```

No `app/`, no `vite.config.ts`, no `package.json` is written. Running `vox dev`
in this scaffold starts the Axum server only (see §5).

---

## 5. `vox dev --target=server`

When `target` resolves to `server` (from flag, env, or manifest):

- Skip `build_frontend()` / `scaffold_react_app()` entirely — no pnpm invocation
- Skip `OrchestratedViteGuard::maybe_spawn()` — `has_frontend` is forced `false`
- Print: `Backend at http://127.0.0.1:<port>` (existing path when `!has_frontend`)
- Hot-reload: watch `.vox` source files, re-run `cargo build` in
  `target/generated/` on change (same mechanism as today)

The change is a single early-return guard in `run.rs` before the `has_frontend`
detection block:

```rust
let has_frontend = if resolved_target == BuildTarget::Server {
    false
} else {
    fs::read_dir(&out_dir) /* existing detection */ ...
};
```

No new process manager is needed; the existing Cargo runner handles reload.

---

## 6. Docker / Deploy Impact

`vox deploy` (and any generated `Dockerfile`) conditionally includes the
Node/pnpm layer based on `target`.

**Current multi-stage Dockerfile (fullstack):**

```dockerfile
FROM node:20-alpine AS frontend-builder
RUN npm i -g pnpm
COPY dist/app ./app
RUN pnpm install && pnpm run build

FROM rust:1.78 AS backend-builder
...

FROM debian:bookworm-slim
COPY --from=frontend-builder /app/dist ./public
COPY --from=backend-builder /target/release/server ./server
```

**`target=server` Dockerfile** (no Node layer):

```dockerfile
FROM rust:1.78 AS backend-builder
COPY target/generated ./
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=backend-builder /target/release/server ./server
CMD ["./server"]
```

This removes the Node/pnpm layer entirely, reducing the final image by
approximately 200–350 MB depending on frontend dependencies.

---

## 7. Migration — No Breaking Change

- `fullstack` is the default when no target is specified anywhere; existing
  projects continue to build without modification.
- Existing `build::run(file, out_dir, target, ...)` already accepts
  `Option<String>` for target; we narrow the `Option<String>` to a typed
  `BuildTarget` enum and thread it through.
- Projects that set `target` in `Vox.toml` before this change: the `[build]`
  table did not exist, so no existing manifests are affected.
- CI pipelines that call `vox build` without `--target` continue to work.

---

## 8. Implementation Notes

### Crates touched

| Crate | Change |
|---|---|
| `vox-config` | Add `BuildTarget` enum + `build_target` field to `VoxConfig`; read from `[build] target` in `Vox.toml` |
| `vox-cli` | Thread `BuildTarget` through `build::run` and `run::run`; add `--target` flag to `BuildArgs` in `cli_args.rs`; add `emit client` subcommand; add `init --kind` flag |
| `vox-compiler` | Add `ClientSdkEmitter` (new file: `crates/vox-codegen/src/codegen_ts/client_sdk.rs`) that walks HIR endpoint_fns and produces `client.ts` / `types.ts` / `index.ts`; gate `generate_routes` / scaffold calls behind target flag in `CodegenOptions` |

### Minimal diff shape

1. **`vox-config`**: +30 lines — `BuildTarget` enum, serde deserialization,
   `Default::default()` → `Fullstack`.
2. **`vox-cli/src/cli_args.rs`**: add `#[arg(long)] target: Option<BuildTarget>`
   to `BuildArgs`; derive `ValueEnum` on `BuildTarget`.
3. **`vox-cli/src/commands/build.rs`**: replace `Option<String>` target param with
   `BuildTarget`; early-return before TS codegen when `target == Server`; early-
   return before Rust codegen when `target == Client`.
4. **`vox-cli/src/commands/run.rs`**: the `has_frontend` bool is gated on
   `BuildTarget` (one `if` guard, ~3 lines).
5. **`vox-compiler/src/codegen_ts/client_sdk.rs`**: new file ~120 lines; iterates
   `hir.endpoint_fns` sorted by `route_path + name` (mirrors `sorted_endpoint_fns`
   in `routes.rs`); emits the `VoxClient` class and `types.ts`.
6. **`vox-cli/src/commands/emit.rs`**: new file ~50 lines; `vox emit client`
   subcommand calls `ClientSdkEmitter`.
7. **`vox-cli/src/commands/init.rs`**: add `--kind` match arm for `backend`;
   write minimal scaffold files listed in §4.

### Test surface

- Unit test in `client_sdk.rs`: compile `crud_api.vox` → assert `client.ts`
  contains `userCount()` and `seedUser(name: string)` methods; assert no
  `import.*vox` lines.
- Integration test: `vox build --target=server examples/golden/crud_api.vox`;
  assert `dist/` is absent, `target/generated/src/main.rs` exists.
- Snapshot test: add `crud_api.vox` → `client_sdk` golden output to the existing
  codegen snapshot suite.
