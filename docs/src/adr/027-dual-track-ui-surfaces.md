---
title: "ADR-027: Dual-Track UI Surfaces (Vox-Native vs React/TanStack Interop)"
description: "Splits Vox's UI primitives into a Vox-native reactivity track and an explicit React/TanStack interop track, with a sharp boundary so each track stays coherent and the training corpus stays clean."
category: "Architecture Decisions (ADRs)"
status: "deprecated"
last_updated: "2026-05-01"
training_eligible: false
---
# ADR 027: Dual-Track UI Surfaces

> **Superseded 2026-05-03** by [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md). The Track B `@island` surface described below is retired. This ADR is retained for historical context only.

**Layering:** Normative machine rules live in **`contracts/frontend/`**; end-to-end interop narrative lives in **[`external-frontend-interop-plan-2026.md`](../architecture/external-frontend-interop-plan-2026.md)**. This ADR is a **decision record** (superseded status below).

## Status
**Superseded** (2026-05-03) — islands retired; see [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md). Original status: Accepted (2026-04-30)

> [!NOTE]
> **Amendment (2026-05-01):** `vox-dashboard` is now the **primary user surface** for the Vox orchestrator; `apps/editor/vox-vscode/` is deprecated and retained for LSP only. New capability UX, MCP behavior, and visualization ship in `crates/vox-dashboard/`. See [ADR 031](031-deprecate-vox-vscode.md).

## Context

Vox today carries **two parallel UI surfaces** that have grown in sync but were never delineated:

1. A **Vox-native reactivity model**: `component Name(params) { state/derived/effect/on mount/on cleanup/view: }` paired with `state_machine Name { state … on Event from X -> Y }` and the `routes { … }` block. These lower through `HirReactiveComponent` / `HirStateMachineDecl` / `client_routes` into WebIR, then to TSX.
2. An **explicit React/TanStack interop model**: `@island Name { prop: Type }` for hydration islands, `@v0 from "design-id" Name { … }` for AI-generated React stubs, and the legacy decorator-on-fn component form that emits React hooks directly.

Both are documented as "supported," but the boundary between them is informal. Authors writing Vox-native components routinely import React idioms (`use_state`, `onClick`, raw `<div className=…>`); authors writing islands occasionally reach for `state_machine`. The April 2026 comprehensive audit flagged this as **corpus contamination** — the model trains on a mixture of two surfaces with no schematic separator, learning React hooks as the canonical Vox idiom.

ADR 012 ("Internal Web IR strategy") established WebIR as the lowering target for both surfaces. ADR 010 ("TanStack web spine") committed to TanStack Router/Start as the runtime. Neither said *when* to use which surface.

## Decision

Vox supports **two UI tracks**, each with a distinct surface, training-eligibility flag, and lowering pipeline. Authors must declare which track a file belongs to; mixing tracks within a file is a compile-time error.

### Track A — Vox-native reactivity (canonical for greenfield)

| Surface | Status | Lowering target |
|---|---|---|
| `component Name(params) { … view: Tag(named=props) { children } }` | 🟡 Preview | `HirReactiveComponent` → WebIR → TSX |
| `state_machine Name { … }` | 🟡 Preview | `HirStateMachineDecl` → WebIR → TSX |
| `routes { … }` | 🟢 Stable | `client_routes` → TanStack Router file routes |
| `style { … }` | 🟡 Preview | WebIR → CSS-in-JS or stylesheet |

Track A is **the default** for new Vox apps. It is **training-eligible**: examples in this track go into the MENS corpus.

Track A bans bare React imports (`use_state`, `useEffect`, `<div className>`) at the source level — diagnostics nudge authors to `state`, `effect`, and Vox class shorthand. The bans are enforced in `crates/vox-compiler/src/typeck/reactive_lints.rs` (new file, queued in this ADR).

### Track B — Explicit React/TanStack interop (for legacy, design imports, hydration)

| Surface | Status | Lowering target |
|---|---|---|
| `@island Name { prop: Type }` | 🟢 Stable (V1 mount contract, OP-0214) | `HirIsland` → TSX with `data-vox-island` |
| `@v0 from "design-id" Name { … }` | 🟡 Preview | Build hook → v0.dev API → React component stubs |
| Classic decorator-on-fn component body `{ … }` | 🟡 Preview, frozen | AST-direct → React hooks emit |

Track B is **explicit interop**. Files using these decorators must include `// @track: react-interop` as the first non-frontmatter line. Track B is **training-ineligible by default**: corpus extraction skips files marked `@track: react-interop` unless the contributor opts them in with `training_weight: > 0`.

Track B exists for three concrete cases:
- **Legacy migration**: existing React/TanStack apps being incrementally Vox-ified.
- **Design imports**: v0.dev / Figma / similar AI-design tools that emit React.
- **Hydration boundary**: declaring which Vox-native components hydrate as islands (a Track A `component` can be exported as a Track B `@island` shell, but that island file lives under Track B rules).

### What collapses

- The classic decorator-on-fn component track is **frozen** — no new features land. It remains for migration but is not the canonical form.
- The previous "Path C optional" framing in ADR 012 is replaced: `component`/`state_machine`/`routes` are the **default** Vox-native path, not optional.
- The "shelve Vox-native reactivity indefinitely" stance from the April 2026 comprehensive audit (item #15) is **overturned** — Track A becomes the primary surface for greenfield. The audit's concerns (corpus pollution, two-emitter maintenance cost) are addressed by the explicit track boundary, not by removing Track A.

### What stays

- ADR 012's WebIR lowering layer continues to serve **both** tracks. This is the unifying compiler floor.
- ADR 010's TanStack Router/Start commitment continues — Track A still emits TanStack-flavored TSX, Track B still uses TanStack idioms directly.
- The island mount contract V1 (OP-0214) stays stable. V2 migration still possible inside Track B.

## Consequences

### Positive
- **Corpus cleanliness.** MENS training only sees Track A by default; the model learns Vox-native idioms as canonical, React-hook idioms as a separate interop dialect.
- **Two surfaces, two coherent stories.** No more "is `state_machine` the right tool, or should I use `useState`?" — the file's track header decides.
- **Migration is bounded.** Existing React-embedded apps stay on Track B with no forced rewrite. Greenfield has one clear answer.
- **Closes a long-running governance gap.** ADR 010, ADR 012, and the April 2026 audit gave conflicting answers; this ADR is the tiebreaker.

### Negative / costs
- **Two emitters maintained indefinitely.** `codegen_ts/reactive.rs` (Track A) and `codegen_ts/component.rs` + `codegen_ts/island_emit.rs` (Track B) both stay. Bug fixes may need to land twice.
- **Track-header churn.** Every existing UI file needs a track header retrofitted — call this **TASK-3.2**, queued.
- **Diagnostics workload.** The `reactive_lints.rs` rules need to be written and tuned to avoid false positives on legitimate JSX inside Track A `view:`.

### Migration plan (TASK-3.2 series)

1. **TASK-3.2a** — Add track-header parser support: detect `// @track: vox-native` (default) or `// @track: react-interop`. Emit a warning when a Track B decorator (`@island`, `@v0`, `@component`) appears in a Track A file.
2. **TASK-3.2b** — Retrofit track headers across `examples/golden/*.vox` and `crates/*/src/**/*.vox`. Existing island-using files become Track B; pure-`component` files become Track A.
3. **TASK-3.2c** — Implement `reactive_lints.rs`: ban `use_state`, `useEffect`, raw React imports inside Track A files.
4. **TASK-3.2d** — Update `vox-corpus` extractors to skip Track B files unless `training_weight: > 0`.
5. **TASK-3.2e** — Update README "Web UI & rendering" stability row and the mens-training documentation to reference both tracks explicitly.

## Alternatives considered

- **(a) Stay React-only** — drop Track A, freeze `component`/`state_machine`. Rejected: throws away two months of Path C work and locks Vox into the React ecosystem permanently. Vox-native UI is a strategic differentiator for the MENS corpus.
- **(b) Pure Vox-native UI** — drop `@island`/`@v0`/TSX emit entirely. Rejected: months of work, breaks every existing example, severs interop with v0.dev / Figma → React design pipelines. Worth reconsidering at v1.0+ but not now.
- **(c) Dual-track with a sharp boundary** — *this ADR.*

## References

- ADR 010 — TanStack web spine
- ADR 012 — Internal Web IR strategy
- ADR 024 — Dashboard as Axum SPA
- `docs/src/architecture/comprehensive-audit-v2-2026.md` (item #2: "React idiom contamination"; item #15: "Vox-native reactivity DSL — shelved")
- `docs/src/architecture/path-b-decommission-2026.md`
- `crates/vox-compiler/src/hir/nodes/decl.rs` (`HirReactiveComponent`, `HirIsland`, `HirStateMachineDecl`)
- `crates/vox-codegen-ts/src/reactive/mod.rs` and `codegen_ts/island_emit.rs`
