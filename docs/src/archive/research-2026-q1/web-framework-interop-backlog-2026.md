---
title: "Web Framework Interop — Codebase Backlog 2026"
description: "Exhaustive task backlog derived from codebase audit, refined against Svelte 5, Solid 2.0, and Vite 8/Rolldown realities. Optimized to prune multi-compiler bloat in favor of a rock-solid 'Library Mode'."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-15"
parent: "web-framework-interop-research-2026.md"
training_eligible: false
training_rationale: "Actionable backlog mapping to concrete tasks across WebIR, codegen_ts, app_contract, and CLI templates. Embeds test verification directly into implementation phases."
archived_date: 2026-04-18
---

# Web Framework Interop — Codebase Backlog 2026

> Derived from the audit in [web-framework-interop-research-2026.md](web-framework-interop-research-2026.md) and April 2026 framework research. 
> 
> **Strategic Pivot:** Instead of building an entire SolidJS compiler or Svelte 5 Runes signal-emitter inside Vox (which incurs a massive maintenance liability), this backlog prioritizes **Library Mode** as the "Universal Glue." Vox generates the `schemas`, `api-client`, `types`, and `models`. The developer handles the UI in Next.js 16, Solid 2.0, or Svelte 5.
> 
> Testing is interleaved directly into implementation sections to ensure zero regressions as we build. Documentation tasks are placed at the end.

---

## Legend

| Tag | Meaning |
|---|---|
| **P0** | Ship ASAP — blocks multi-framework adoption |
| **P1** | High-value — unlocks significant new capability |
| **P2** | Medium-value — improves maintainability / cleans tech debt |
| **P3** | Low-priority — documentation and polish |
| 🧪 | Includes a verification / test step to ensure zero breakage |

archived_date: 2026-04-18
---

## A. Library Mode Build Target (P0)

> **Goal**: `vox build --mode library` emits only framework-agnostic artifacts (types, clients, CSS, schemas). This completely sidesteps the need to compile to Solid/Svelte components.

| # | Task | File | Severity | Notes |
|---|---|---|---|---|
| A1 | Add `BuildMode` enum (`App`, `Library`) to `CodegenOptions` | `emitter.rs` | 🟡 P0 | Currently only has `target`. |
| A2 | Gate component emission on `mode != Library` (skip `.tsx` files) | `emitter.rs` | 🟡 P0 | Skips React-coupled outputs. |
| A3 | Gate `vox-tanstack-query.tsx` and `server.ts` on `mode != Library` | `emitter.rs` | 🟢 P0 | |
| A4 | Keep `types.ts`, `vox-client.ts`, `schema.ts`, `*.css` in library mode | `emitter.rs` | ✅ | Already framework-agnostic. |
| A5 | Wire `--mode library` flag through CLI `vox build` subcommand | `vox-cli` | 🟡 P0 | |
| A6 | Generate `package.json` with proper exports map for library mode | `emitter.rs` | 🟡 P0 | Standard for npm consumption. |
| A7 | Generate `index.ts` barrel file re-exporting all library artifacts | `emitter.rs` | 🟡 P0 | |
| A8 | 🧪 **Test:** Add library-mode integration test | New | 🟢 P0 | Verify it produces *only* agnostic files. |
| A9 | 🧪 **Test:** Add `tsc --noEmit` check on library mode output | New | 🟡 P0 | Verify correctness without React installed. |

---

## B. Zod Schema Generation (P0)

> **Goal**: Emit `schemas.ts` with Zod validators for runtime validation at API boundaries across any framework.

| # | Task | File | Severity | Notes |
|---|---|---|---|---|
| B1 | New file: `codegen_ts/zod_emit.rs` — Zod schema from `HirTypeDef` | New | 🟡 P0 | Emit `z.discriminatedUnion`. |
| B2 | Map primitives (`int`, `float`, `str`, `bool`) to Zod | `zod_emit.rs` | 🟢 P0 | |
| B3 | Map generics (`Option`, `list`) to `z.optional()`, `z.array()` | `zod_emit.rs` | 🟢 P0 | |
| B4 | Wire Zod emission into `emitter.rs` (app and library mode) | `emitter.rs` | 🟡 P0 | |
| B5 | Add `import { z } from "zod"` header + `package.json` peerDep | `zod_emit.rs` | 🟢 P0 | |
| B6 | 🧪 **Test:** Add Zod schema correctness test on example data | New | 🟡 P0 | Run validators against generated mock data. |

archived_date: 2026-04-18
---

## C. Framework-Agnostic Types & Client (P1)

> **Goal**: Enrich the cross-framework TS boundaries with better type safety and robust API interactions.

| # | Task | File | Severity | Notes |
|---|---|---|---|---|
| C1 | Add `is<Variant>()` type guards to `adt.rs` | `adt.rs` | 🟡 P1 | `v is { _tag: "Ok" }` |
| C2 | Add `match<T>()` exhaustive pattern matching utility to `adt.rs` | `adt.rs` | 🟡 P1 | |
| C3 | Add typed error handling (`VoxApiError` class) | `vox_client.rs`| 🟡 P1 | Carries status code and path. |
| C4 | Add request/response type imports from `types.ts` to client | `vox_client.rs`| 🟡 P1 | Currently uses inline strings. |
| C5 | Add Zod validation on response (`.parse(data)`) | `vox_client.rs`| 🟡 P1 | |
| C6 | Add `AbortController` support to `$get` / `$post` | `vox_client.rs`| 🟢 P1 | |
| C7 | Add `$delete` and `$put` helpers to client | `vox_client.rs`| 🟢 P1 | |
| C8 | 🧪 **Test:** `types.ts` snapshot test | New | 🟢 P1 | Verify zero framework imports leak in. |
| C9 | 🧪 **Test:** `vox-client.ts` snapshot test | New | 🟢 P1 | |

---

## D. App Contract & Route Decoupling (P1)

> **Goal**: Make `vox-app-contract.json` and routes usable by any outer routing layer (Solid Router, SvelteKit, etc.).

| # | Task | File | Severity | Notes |
|---|---|---|---|---|
| D1 | Add `buildMode` and `targetFramework` fields to App Contract | `app_contract.rs`| 🟢 P1 | |
| D2 | Add `types` and `tables` sections to App Contract | `app_contract.rs`| 🟡 P1 | |
| D3 | Add `generatedFiles` array and `scheduledJobs` | `app_contract.rs`| 🟡 P1 | |
| D4 | Emit `routes.manifest.json` (framework-agnostic JSON format) | `route_manifest.rs` | 🟡 P1 | JSON array of `{path, componentName}`. |
| D5 | Emit JSON route manifest instead of TS manifest in library mode | `emitter.rs` | 🟡 P1 | |
| D6 | 🧪 **Test:** `vox-app-contract.json` round-trip serialization test | New | 🟢 P1 | |
| D7 | 🧪 **Test:** `routes.manifest.json` serialization test | New | 🟢 P1 | |

archived_date: 2026-04-18
---

## E. CLI Template Refinements (P1-P2)

> **Goal**: Update scaffold defaults to match the state-of-the-art 2026 ecosystem (Vite 8 Rolldown) and remove outdated dependencies.

| # | Task | File | Severity | Notes |
|---|---|---|---|---|
| E1 | Add `--framework` flag to `vox init web` | `vox-cli` | 🟡 P1 | `react` (default), `library`. |
| E2 | Create `templates/library.rs` npm package scaffold | `vox-cli` | 🟡 P1 | For setting up a TS package. |
| E3 | Update `scaffold.rs` Vite to `^8.0.0` (Rolldown support) | `scaffold.rs` | 🟢 P1 | 2026 standard bundle pipeline. |
| E4 | Remove `react-router` from scaffold | `scaffold.rs` | 🟡 P1 | Fix violation of ADR 010 (TanStack Router). |
| E5 | Update React to `^19.1.0` in scaffold | `scaffold.rs` | 🟢 P2 | |
| E6 | 🧪 **Test:** `@tanstack/react-query` import assertion check | `tanstack.rs` | 🟢 P2 | |

---

## F. Tech Debt: Cleanups & Reliability (P2)

> **Goal**: Fix identified bugs, data races, and anti-patterns affecting the core compiler stability.

| # | Task | File | Severity | Notes |
|---|---|---|---|---|
| F1 | Extract duplicate CSS `camelCase -> kebab-case` closure | `emitter.rs` | 🟢 P2 | Deduplicate lines 197 and 222. |
| F2 | Remove static `AtomicU64` counters in `reactive.rs` | `reactive.rs` | 🟡 P2 | Data race hazard in concurrent tests. Return stats instead. |
| F3 | Replace `env_var_explicitly_disabled` with `CodegenOptions` | `web_migration_env.rs` | 🟡 P2 | Fixes Env-var anti-pattern. |
| F4 | Add cycle detection to WebIR DOM tree walk | `validate.rs` | 🟡 P2 | Fix infinite recursion stack-overflow risk. |
| F5 | Validate unique view root names in WebIR | `validate.rs` | 🟡 P2 | Currently unchecked. |
| F6 | Validate node strings (`tag`, `predicate`, `event`) are non-empty | `validate.rs` | 🟢 P2 | |
| F7 | 🧪 **Test:** WebIR validate uniqueness constraint test | New | 🟢 P2 | |

archived_date: 2026-04-18
---

## G. Documentation & DX (P3)

> **Goal**: Provide the integration guides necessary to consume Library Mode across the ecosystem. 
> *Note: Conduct these only after all P0 and P1 tests are passing.*

| # | Task | File | Severity | Notes |
|---|---|---|---|---|
| G1 | Write "Using Vox with Svelte 5 (Runes)" | `docs/src/guides/` | 🟡 P3 | |
| G2 | Write "Using Vox with Solid 2.0" | `docs/src/guides/` | 🟡 P3 | |
| G3 | Write "Using Vox with Next.js 16" | `docs/src/guides/` | 🟡 P3 | |
| G4 | Update `vox-web-stack.md` to document library mode | `vox-web-stack.md` | 🟡 P3 | |
| G5 | Add `vox build --mode library --help` text | `vox-cli` | 🟢 P3 | |
| G6 | Add `README.md` generation for library mode packages | `emitter.rs` | 🟢 P3 | |

---

## Execution Order Recommendation

1. **Phase 1: Foundation (P0)**: Sections A and B. Implements `library` mode and `schemas.ts` generation, immediately enabling safe multi-framework usage.
2. **Phase 2: Data Boundaries (P1)**: Sections C and D. Improves TS types, client, and makes the contract and routing layers framework-agnostic.
3. **Phase 3: Scaffolds & Tech Debt (P1-P2)**: Sections E and F. Updates the generated ecosystem to modern standards and quashes data races.
4. **Phase 4: Documentation (P3)**: Section G. User-facing integration guides.


