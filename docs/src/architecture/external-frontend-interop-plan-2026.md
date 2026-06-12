---
title: "External Frontend Interop Plan (2026)"
description: "Five-phase plan to make Vox interoperable with the React/TS ecosystem in both directions: keep Vox's GUI authoring (TS/React emission), add bidirectional component interop, retire @island, and add a backend-only mode for users with an existing React frontend."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Strategic plan; canonical reference for backend-only mode, bidirectional Vox↔React component interop, and @island retirement."
---

# External Frontend Interop Plan (2026)

> **Phase numbering:** This plan uses the **frontend interop** phase sequence (Phases 1–5). For the other two sequences, see [phase-numbering-index](phase-numbering-index.md).

## Premise

Historically Vox supported one shape: full-stack co-generation of a Vite/React frontend and an Axum backend from the same `.vox` source, with `@island` as the bridge primitive for sprinkling React into the generated tree. **As of 2026-05-03, `@island` is retired**: the compiler, CLI, templates, contracts, examples, and docs no longer reference it; Vox lowers `component` declarations directly to plain React/TSX that any external frontend imports.

This plan **expands the model in two directions** without removing what works:

1. **Vox's GUI authoring stays a first-class language feature.** The `component` keyword and TS/React emission remain. Vox is still capable of being the whole stack.
2. **Bidirectional Vox↔React component interop becomes first-class.** A Vox component can import and render a React component from a `.tsx` file; an emitted Vox component is a normal React component that any external React app can import. `@island` is retired because proper component-level interop subsumes it.
3. **Backend-only mode is added.** A user with an existing React/TS frontend can use Vox purely as the API server, consuming a typed client SDK and standards-based schemas (OpenAPI / JSON Schema), with no Vite or React generation involved.

The two modes share one substrate: the wire-format SSOT, the OpenAPI/JSON Schema emitters, the auth/ops stdlib, and the Axum backend. The full-stack mode adds the GUI emission and component interop on top.

## Non-goals

- Removing the integrated frontend pipeline. It stays.
- Inventing a new client framework. We integrate with what React users already use (openapi-typescript, RTK Query, TanStack Query, Orval, tRPC adapters).
- A generalized Node↔Vox FFI. The optional WASM-from-Node bridge is scoped to pure Vox computations, not request handling.

## Decisions baked into this plan

- **Retire `@island`.** Replaced by general bidirectional component interop in Phase 5. No need for an island-specific bridge once Vox components and React components can reference each other directly.
- **Keep `component`, `routes`, and the Vox→TS/React emission.** These are language features.
- **Backend-only mode is additive.** Adding `--server-only` does not require deleting `--full-stack`.

---

## Phase 1 — Add backend-only mode; split emit targets

**Goal:** Make `vox build` honor an explicit emit target so backend-only and full-stack are both first-class. Add a publishable TS client SDK as a standalone artifact. The full-stack pipeline keeps working unchanged.

**Scope:**

1. New explicit target flag: `vox build --target=server | fullstack | client`. Default chosen by the project manifest (`Vox.toml [build] target = "..."`); no silent behavioral change.
   - `--target=server` skips Vite, skips `pnpm install`, skips React asset generation. Emits the Axum crate only.
   - `--target=fullstack` is the current behavior, preserved.
   - `--target=client` emits the TS client SDK only (see below).
2. New subcommand `vox emit client --lang=ts --out=./api-client` produces a self-contained, npm-publishable package:
   - Own `package.json` with `name`, `version`, `exports` (ESM + CJS + `.d.ts`).
   - Zero imports from `vox-actor-runtime`, internal Vox surfaces, or the full-stack client emit. Lives in its own crate so the dependency graph is clean.
   - Emits: types, optional Zod validators (flag), a fetch client class. Configurable `baseUrl` and a pluggable `fetch` (so users can wire RTK Query / TanStack Query / their own auth interceptor).
   - Reproducible output: identical input HIR → byte-identical files. Golden tests pin this.
3. `vox dev --target=server` — dev-loop that doesn't touch a frontend. Hot-reloads the Axum binary.
4. Project bootstrap: `vox init --kind=backend` produces a manifest with `target = "server"` and no React/Vite scaffolding. Existing `--kind=application` still produces a full-stack project.
5. Update [Dockerfile](../../../Dockerfile) and `vox deploy` to honor the manifest target — server-only deployments produce a leaner image with no Node/pnpm layer.
6. Add backend-only golden examples. The existing full-stack goldens ([blog_fullstack.vox](../../../examples/golden/blog_fullstack.vox), [dashboard_ui.vox](../../../examples/golden/dashboard_ui.vox)) stay; new ones (`backend_only_crud.vox`, `backend_only_auth.vox`) demonstrate the server target.

**Deliverables:** target-flag plumbing, `vox emit client` subcommand, `vox dev --target=server`, new init kind, manifest schema update, backend-only goldens, [CLI reference](../reference/cli.md) updates.

**Risks:** Two emit paths drift over time (full-stack client vs. standalone SDK emit produce different shapes). Mitigation: both consume the same OpenAPI artifact from Phase 2; the full-stack client becomes a thin specialization once Phase 2 lands.

---

## Phase 2 — Wire format SSOT and standards-based schema emit

**Goal:** Make the contract between Vox backends and external frontends explicit, versioned, and consumable by every TS/React tool that exists.

**Scope:**

1. **Wire-format SSOT doc** at `docs/src/architecture/wire-format-v1-ssot.md`. Pin:
   - `Decimal` → string (already in code; codify).
   - `BigInt` → string. Decision rationale: JSON Number loses precision past 2^53.
   - Date/Time → RFC 3339 strings (UTC). No raw epoch ints.
   - `Option<T>` → presence-or-absent JSON key (not `null`), with explicit override decorator for null-distinguished APIs.
   - Sum types → `_tag`-discriminated objects (already in code; codify and freeze the tag-key name).
   - Versioning: `wire-format-v1`, semver discipline, breaking-change protocol.
2. **OpenAPI 3.1 emit**: `vox emit openapi --out=./openapi.yaml` over the `HirEndpointFn` set. Path, method, params, request/response schemas, error envelopes. This single artifact unlocks openapi-typescript, Orval, RTK Query, Postman, Insomnia, and more.
3. **JSON Schema 2020-12 emit** per Vox `type`. Useful in isolation for validation pipelines that don't want OpenAPI's full surface.
4. **Golden compatibility tests**: a directory of fixture `.vox` types and the exact expected JSON wire bytes. Any future change to the wire format must update goldens explicitly. Sits alongside the existing Zod/TS goldens.
5. **Deprecate the bespoke `vox_client.rs` emit path** in favor of routing all TS-client generation through the OpenAPI artifact (Phase 1 client emit becomes a thin wrapper invoking openapi-typescript-codegen internally, or vendoring its templates).

**Deliverables:** SSOT doc, two new emit subcommands, golden suite, deprecation note for the legacy client emitter.

---

## Phase 3 — HTTP ergonomics as decorators

**Goal:** Express the things real backends need (explicit routes, methods, CORS, auth, rate-limits, path params) without inventing new bare keywords. Per [AGENTS.md §Grammar Unification](../../../AGENTS.md), new behavior goes on decorators.

**Scope:**

1. Extend `@endpoint`:
   ```
   @endpoint(method: GET, path: "/users/:id")
   fn get_user(id: UserId) to Result[User]
   ```
   Path params extracted by name, type-checked against the function signature at compile time. Query strings remain implicit for trailing scalar params (or explicit via `@query_param`).
2. New decorators:
   - `@cors(origins: ["https://app.example.com"], credentials: true)` — module-scoped or per-endpoint.
   - `@auth(scheme: bearer)` — declarative; resolves to a Tower middleware in the generated Axum crate. Composable with custom auth functions.
   - `@rate_limit(per: "1m", max: 60, key: by_ip)` — emits a `tower_governor` (or equivalent) layer.
   - `@public` / `@authenticated` / `@role("admin")` — guard groups.
3. Compile-time route conflict detection (already partially present in [`routes.rs`](../../../crates/vox-rn-codegen/src/routes.rs) near line 70; extend to handle path-param overlaps).
4. OpenAPI emit (Phase 2) reflects all of the above as `securitySchemes`, `x-rate-limit`, CORS notes, and proper path-param `parameters`.
5. Update auth-pattern golden examples to use the declarative form; keep one manual example as an escape hatch.

**Deliverables:** Decorator additions to compiler, middleware emission in generated Axum crate, doc page on HTTP ergonomics, OpenAPI integration.

---

## Phase 4 — Auth library and operational primitives

**Goal:** Make a "real production backend" achievable in Vox without leaving the standard library.

**Scope:**

1. **JWT verification primitive** in `vox-stdlib`, with key resolution through Clavis ([AGENTS.md §Secret Management](../../../AGENTS.md)). RS256/ES256/HS256 supported; JWKS fetch with caching.
2. **Session store abstraction** over `@table`. Default schema, `verify_token() -> Result[Session]`, `revoke()`, idle and absolute timeouts.
3. **Health and observability endpoints**:
   - `/healthz`, `/readyz` — auto-mounted, opt-out via flag.
   - `/metrics` — Prometheus text format, opt-in.
   - Structured logging (JSON) with request-id, span context. Opt-in via `@trace` on endpoint or module.
4. **Durability resolution** (audit complete — see [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md)):
   - **`@scheduled` and `@durable`:** Remove from the public grammar in the next release; retain as reserved identifiers. Re-introduce each when a real runtime implementation lands. The HIR `schedule_interval` and `DurabilityKind` fields stay as internal metadata.
   - **`actor` keyword:** Retain. Handler-splitting HIR work is real. Document the current limitation (no auto-mailbox wiring) explicitly and provide a manual pattern golden example.
   - **`workflow` / `activity` keywords:** Remove from the public grammar alongside `@durable`. They currently compile identically to `fn` with no semantic difference.
   - Decision record required: one ADR covering the deprecation cycle and the re-introduction criteria for each feature.
5. **CORS / rate-limit defaults** chosen to be safe for backend-only deployments (CORS off by default; must be opted in per origin).

**Deliverables:** stdlib auth module, session table/library, ops endpoint mounting, durability ADR (removes `@scheduled`/`@durable`/`workflow`/`activity` from public grammar), `actor` limitation doc + golden example.

---

## Phase 5 — Bidirectional Vox↔React component interop (`@island` retired 2026-05-03)

> **Status update (2026-05-03):** `@island` is retired across the workspace. The remaining Phase 5 work is the bidirectional import bridge: Vox-side `import_react` for consuming React components, and emitted-component packaging so Vox components are first-class npm-importable React components.
>
> **Sub-spec (2026-06-03):** the deferred decisions below (exact `import react …` syntax, the type-bridge mechanism, the user-edit escape hatch, and the React Native mapping) are now specified in [Phase 5 Sub-Spec: Native React / React Native Component Interop (2026)](external-frontend-interop-phase5-component-interop-subspec-2026.md) — opaque-extern types + an opt-in flat prop facade, shadcn/ui as a vendor codegen mode, one-IR RN mapping, and a code-anchored implementation plan.

**Goal:** Make the Vox GUI language and the React ecosystem into peer citizens. A Vox component can use any React component; an emitted Vox component is a normal React component any external React app can use.

**Scope:**

1. **Vox imports React (Vox source uses React components):**
   - New import form: `import react MyButton from "../ui/MyButton.tsx"` (or equivalent — exact syntax in this phase's sub-spec).
   - Type bridge: import the component's TS prop types via either (a) reading the `.tsx`/`.d.ts` directly, (b) a generated `.vox.types.json` sidecar, or (c) a `vox import-types` step that produces a Vox type alias. Prefer (a) when feasible.
   - In the emitted TS output, the import passes through unchanged — no marshaling layer, the React component is rendered as-is.
   - Props passed from Vox to the React component are serialized through the wire-format SSOT (Phase 2) when they cross a serialization boundary; in-process they pass as native JS values.
2. **React imports Vox (external React app uses emitted Vox components):**
   - Vox component compilation produces `.tsx` files that are first-class React components: real `export`, real prop type aliases, no hidden runtime dependency on `vox-actor-runtime` for component code.
   - Output directory has a generated `package.json` so it can be a workspace package (or published to npm) and consumed via standard `import { MyVoxComponent } from "@myorg/vox-ui"`.
   - Re-emit-stable: re-running the compiler produces a clean diff. Developers should generally not hand-edit emitted files (the source of truth is the `.vox` source); a designated `// vox:user-edit` zone or sidecar override file is the escape hatch — exact mechanism in the sub-spec.
   - Emitted components are typed against the Phase-1 client SDK, so a button bound to a mutation gets the typed call for free.
3. **Retire `@island`:**
   - The decorator and its codegen path are deleted from the compiler. Document in the migration ADR.
   - All island use sites in goldens/examples are rewritten to use the new bidirectional interop (typically: a Vox component that imports a React component, or vice versa).
   - The `routes` block stays — it remains the way Vox authors a route tree.
   - `component` keyword stays — it remains the Vox UI authoring primitive.
4. **Optional WASM-from-Node bridge** (lower priority; can defer to 5b):
   - npm package `@vox/wasi-runtime` that loads a Vox-compiled `.wasm` (the existing `--isolation wasm` artifact, see [`wasi.rs`](../../../crates/vox-cli/src/commands/runtime/run/backend/wasi.rs)) and exposes typed exported functions to Node.
   - Use case: Node worker calling pure Vox computations in-process. Not for HTTP request handling — that path stays Axum.
   - Defer N-API/cdylib indefinitely unless concrete pull emerges.
5. **Tutorials in `docs/src/tutorials/`:**
   - "Use a React component from Vox" — bidirectional interop, Vox-side.
   - "Use a Vox component from a React app" — bidirectional interop, React-side.
   - "Bring your own React frontend" — end-to-end backend-only path: `vox init --kind=backend`, write endpoints, `vox emit openapi`, consume from Vite/Next/Remix, deploy via `vox deploy`.

**Deliverables:** import-react syntax + type bridge, emitted-component packaging, `@island` removal commit + ADR, three tutorials, optional WASM-from-Node package, sub-spec for the import-types and re-emit-stability mechanisms.

---

## Cross-cutting concerns

- **Versioning:** `wire-format-v1` is independent of Vox compiler version. Breaking changes require a new major (`v2`) and a parallel-emit grace period.
- **Documentation governance:** all five phases produce docs that go through [Documentation governance](../contributors/documentation-governance.md) — auto-indexed via `vox-doc-pipeline`, never hand-edited indexes.
- **Telemetry:** every new emit subcommand emits `vox.script.*`-class events for observability ([AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md)).
- **Security:** auth and CORS defaults must fail closed. CORS must reject by default; `@auth` must reject by default; rate-limit decorators must be additive, not subtractive.
- **Migration support:** ship `vox migrate drop-island` (Phase 5) — rewrites `@island` use sites to the bidirectional import form — and `vox migrate wire-format` (Phase 2 → future v2) so users are never stranded.
- **Emitted component code is generated, not authored.** Per the project's "auto-generated docs" policy, emitted `.tsx` files should not be hand-edited; the `.vox` source is canonical. Any escape-hatch user-edit zones must be explicitly delimited so the compiler can preserve them across re-emits.

## Sequencing and dependencies

```
Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 4
              │
              └────────────────────► Phase 5
```

Phase 1 unblocks everything because the `--target=server` split is a prerequisite for offering backend-only at all. Phase 2 must precede Phase 3 because route decorators are only useful if their semantics show up in the OpenAPI artifact. Phase 5 depends on Phase 2 (typed prop bridges and the wire-format SSOT for cross-component data) but is otherwise orthogonal to Phases 3 and 4 and can run in parallel.

## What this plan does *not* yet decide

- The exact syntax for `import react ...` and the type-bridge mechanism for React component props (Phase 5 sub-spec).
- The escape-hatch mechanism for user edits in emitted `.tsx` files (Phase 5 sub-spec; default stance is "don't hand-edit").
- Specific OpenAPI tooling: vendored templates vs. shelling out to `openapi-typescript-codegen` (Phase 2 implementation choice).
- The durability runtime's true status — `@scheduled`/`@durable`/actor/workflow/activity wiring (Phase 4 audit will produce the answer).

Each of these is flagged in its phase as an explicit follow-up.
