---
title: "React interop full-repo migration charter (2026)"
description: "Governance, KPIs, cutover policy, and completion checkpoints for the framework-agnostic React + islands + v0/shadcn/Tailwind migration."
category: "Architecture SSOTs"
last_updated: "2026-04-08"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# React interop migration charter (2026)

## Authority

- Research SSOT: [react-interop-research-findings-2026.md](./react-interop-research-findings-2026.md)
- Executable technical plan: [react-interop-implementation-plan-2026.md](./react-interop-implementation-plan-2026.md)
- Shell strategy: [react-interop-minimal-shell-strategy.md](./react-interop-minimal-shell-strategy.md)
- Executable backlog (granular tasks): [react-interop-backlog-2026.md](./react-interop-backlog-2026.md)

## Policy

- **Single frontend SSOT:** generated `dist/` artifacts are **named-export React TSX**, **`routes.manifest.ts`**, **`vox-client.ts`** (typed `fetch`), and shared contracts — not framework-specific route trees.
- **No legacy emit:** `VoxTanStackRouter.tsx`, programmatic TanStack `App.tsx`, and `serverFns.ts` (`createServerFn`) are removed from codegen output.
- **User-owned scaffold:** `app/App.tsx`, `app/main.tsx`, `vite.config.ts`, `components.json`, and Tailwind entry CSS are written once (skip if present).
- **Hybrid runtime:** default path is **SPA + islands**; **SSR adapter** is supported as user-owned glue, not compiler-generated framework mode.
- **Interop target:** React 19, v0/shadcn CLI v4 (`rsc: false`). **Tailwind v4:** authors enable Tailwind when adopting shadcn/TW utilities; the **default Vox web scaffold** ships a self-contained CSS theme in [`crates/vox-cli/src/templates/spa.rs`](../../../crates/vox-cli/src/templates/spa.rs) (`index_css`) — not `@import "tailwindcss"` until we add an explicit template toggle. See [`react-interop-implementation-plan-2026.md`](./react-interop-implementation-plan-2026.md) v0/shadcn checklist.

## KPIs

- **K1:** `vox build` emits `routes.manifest.ts` whenever `routes { }` is present; no TanStack router tree files.
- **K2:** `vox-client.ts` is emitted whenever any of `@query` / `@mutation` / `@server` exist; **no** `createServerFn` in repo-generated TS.
- **K3:** CI smoke builds pass with Vite + pnpm using manifest + user `App.tsx` adapter pattern.
- **K4:** `@component fn` and other retired surfaces move to **Error** with migration hints (staged with fixture updates).

## Checkpoints (percent complete)

| % | Gate |
|---|------|
| 25% | Parser + manifest + vox-client + emitter wired; feature-complete behind review |
| 50% | CLI/templates/docs aligned; integration tests updated |
| 70% | Contracts + migration tooling + WebIR parity where required |
| 85% | Extension / visualizer / tree-sitter workspaces aligned |
| 100% | Legacy paths deleted; charter signed-off |

## Rollback

- Rollback is by **revert commit**; do not reintroduce `createServerFn` or dual TanStack trees once cutover lands on `main`.

## Frozen artifacts (compiler + CLI SSOT)

These filenames and roles are **stable contracts** for React interop; changing them requires charter update + contract/version notes:

| Artifact | Owner | Notes |
| -------- | ----- | ----- |
| `routes.manifest.ts` | `vox-compiler` (`codegen_ts/route_manifest.rs`, WebIR path target) | `VoxRoute[]` for adapters; no programmatic router TS from compiler |
| `vox-client.ts` | `vox-compiler` (`codegen_ts/vox_client.rs`) | Typed `fetch` to `/api/...`; no TanStack `createServerFn` |
| `*.tsx` pages/components | `vox-compiler` emit | Named exports; islands meta in `vox-islands-meta.ts` |
| `app/`, `src/routes/` scaffolds | `vox-cli` templates (`templates/tanstack.rs`, `scaffold.rs`) | Written once; user-edited thereafter |
| `contracts/cli/*`, `contracts/capability/*` | platform | CLI/capability registry rows for `vox build`, `vox migrate web`, flags |

## Adapter ownership

| Adapter | Owner | Responsibility |
| ------- | ----- | ---------------- |
| **SPA reference** | `vox-cli` templates + docs cookbook | Wires `RouterProvider`, imports manifest-driven route module map |
| **SSR / TanStack Start** | User repo + optional reference template | File routes, `routeTree.gen.ts`, Vite Start plugin — consumes same manifest |
| **Axum static + `/api`** | `vox-codegen-rust` + integration tests | Ordering, proxy, health — see Axum SSOT tasks |

Compiler deliverables stop at **manifest + components + client**; frameworks own router construction.

## Acceptance gates (summary)

Full numeric gates (G1–G6) and file/test mapping: [internal-web-ir-implementation-blueprint.md — Acceptance gates](./internal-web-ir-implementation-blueprint.md). Charter-level minimum:

- **G-manifest:** emitted manifest parses and matches HIR/WebIR route set (parity tests).
- **G-client:** `vox-client.ts` has deterministic HTTP methods and URL shapes; no forbidden substrings in generated TS (`createServerFn`, legacy filenames).
- **G-scaffold:** idempotent scaffold (`--scaffold`); doctor warns on divergence from expected layout env.
- **G-migrate:** `vox migrate web --check` stable JSON; `--write` patches are deterministic and golden-tested.

## Reviewer checklist (PRs touching web codegen)

1. Confirm no new **framework-specific** server-fn emission (TanStack/Next proprietary APIs) in `codegen_ts`.
2. If routes change: **`routes.manifest.ts`** schema + adapter docs or cookbook updated.
3. Run or point to **`web_ir_lower_emit`**, **`reactive_smoke`**, **`full_stack_minimal_build`** as relevant.
4. **`vox stub-check --path`** on touched compiler/cli dirs; no TOESTUB in product paths.
5. Docs: mark **historical** TanStack-only specs; SSOT narrative stays **manifest-first** ([`vox-web-stack.md`](../reference/vox-web-stack.md)).
6. CI runner labels follow [runner-contract.md](../ci/runner-contract.md) unless documented exception.


