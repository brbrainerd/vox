---
title: "Vox-Native Frontend SSOT — author the GUI in Vox, emit through a pluggable backend, interop with React"
category: "Architecture SSOTs"
status: draft
date: 2026-06-20
---

# Vox-Native Frontend SSOT

## 1. Problem & Goal

The Vox GUI is hand-written React/TSX (`crates/vox-gui/ui/`), built with Vite/pnpm,
shipped inside a Tauri shell. This couples us to the JS toolchain in three places —
authoring (hand-written `.tsx`), build (Node/Vite/pnpm), and runtime (React) — and
splits the UI's source of truth away from the rest of the Vox codebase.

**Goal:** make the Vox *language* able to author the frontend, so that over time the
GUI converges to a single Vox source of truth. The target is to reproduce 95–99% of
the current frontend *from `.vox`*, while preserving the things we refuse to lose.

**This is explicitly NOT a dependency-minimization project.** It is a *release*-dependency
and *technical-debt* minimization project. The build toolchain may stay heavy as long as
it is auto-managed and invisible to end users.

### Non-negotiable constraints (from brainstorming)

1. **React ecosystem interop is mandatory.** A `.vox` author must be able to import and
   run a real React/JS component (a calendar, a chart) — the Zig-consumes-C model — not
   reimplement it in raw JS.
2. **Emit is a means, not an end.** We do not care what we emit (React/TSX today) as long
   as it runs, looks good, and covers **mobile + desktop**.
3. **No end-user Node.** End users must never know Vite/pnpm/Node exists or have to spin
   one up. The toolchain may be vendored/auto-installed behind `voxup`-style management
   ("move it into the factory line").
4. **Single source of truth, codebase-wide.** The `.vox → emission` pipeline carries one
   set of enforceable rules; no split-brain registries.

### Out of scope

- A Vox-native runtime that replaces React in the browser (the "Model 2 island" runtime).
  This is forfeited as a near-term goal because it would break constraint #1. It is
  preserved only as a *future backend* (see §3), not built here.
- Replacing Tauri / moving to a plain browser shell. That is a separate program; this spec
  is about the *authoring + emission* pipeline regardless of the host shell.

## 2. Chosen Architecture — Model 3 spine, Model 1 backend

Three models were considered:

- **Model 1 — React is the substrate.** Vox transpiles directly into React/TSX; imported
  React components drop in for free. Fastest, strongest interop, no runtime minimalism.
- **Model 2 — Vox-native runtime, React as islands.** Minimal Vox runtime; React mounted
  only where imported. Best minimalism, but two reactivity models must coexist — high debt.
- **Model 3 — Adapter-defined target.** Vox lowers to a stable IR (the existing **WebIR**);
  a *swappable backend* turns WebIR into a concrete frontend. SSOT + rules live in the IR.

**Decision: Model 3 spine with Model 1 as the sole concrete backend.**

```
.vox source
   │  (compiler front end → HIR)
   ▼
  HIR
   │  lower.rs            ← SSOT seam #1: all UI semantics captured here
   ▼
 WebIR  ──►  validate*.rs  ← SSOT seam #2: enforceable rules (a11y, palette, layer, overlay, keys)
   │
   │  pluggable backend (trait)
   ▼
 React/TSX  (emit_tsx.rs + vox-codegen-ts)   ← the ONE concrete backend built now
   │
   ▼
 auto-managed Vite/pnpm build (vendored, hidden)
   │
   ▼
 web bundle  →  mobile + desktop
```

The WebIR boundary is what makes this Model 3 and not plain Model 1: a future leaner
backend (Web Components, WASM) can be added behind the same trait without touching the
front end or the rule layer. We do not build that backend now — we only keep the seam honest.

### Why this matches the goal

- **SSOT:** the `.vox → HIR → WebIR` path is the single source; `validate*.rs` is the single
  rule surface. React becomes an *output format*, not the source of truth.
- **Interop:** Model 1 backend means imported React components are first-class — extends the
  already-landed `vox-codegen-ts/src/external_libs.rs` interop table.
- **Emit-agnostic:** the backend trait makes "what we emit" a swappable detail, satisfying
  "I don't care what we emit."
- **Release deps:** the Vite/pnpm step moves behind toolchain automation; end users get a
  built artifact.

## 3. Components

### 3.1 Existing foundations (reuse, do not rebuild)

| Component | Path | Role |
|---|---|---|
| WebIR + lowering | `crates/vox-codegen/src/web_ir/` (`lower.rs`, `mod.rs`) | The SSOT IR. Already lowers HIR→WebIR. |
| Rule validators | `crates/vox-codegen/src/web_ir/validate*.rs` | a11y, palette, layer, overlay, keys. The enforceable-rule SSOT. |
| TSX emitter | `crates/vox-codegen/src/web_ir/emit_tsx.rs`, `crates/vox-codegen-ts/` | WebIR→React/TSX. The Model 1 backend, partially built. |
| External-lib interop | `crates/vox-codegen-ts/src/external_libs.rs` | SSOT table of importable React libraries (MUI, Mantine, antd, Radix…). |
| Component interop sub-spec | `docs/src/architecture/external-frontend-interop-phase5-component-interop-subspec-2026.md` | Existing phased program this spec converges with. |

### 3.2 What this program adds / completes

1. **Backend trait (`WebIrBackend`).** Formalize the WebIR→output step as a trait with one
   concrete impl (`ReactTsxBackend`) wrapping the existing emitter. This is the Model 3 seam.
   Small, mechanical, but it is the architectural commitment.
2. **Authoring-surface coverage closure.** Audit which of the current hand-written React
   surfaces in `vox-gui/ui/` can be expressed in `.vox`/WebIR today and which need new
   WebIR primitives (events, controlled inputs, effects, the 13 `vox://*` reactive streams).
   Produces a coverage ledger (the "95–99%" is measured against this, not asserted).
3. **Ecosystem-import path completion.** Extend `external_libs.rs` + the import lowering so a
   `.vox` file can `import react Calendar from "@some/calendar"` and have it emit, inject
   required CSS/providers, and pass validation. This is the Zig/C interop deliverable.
4. **Toolchain automation ("factory line").** Make the Vite/pnpm build vendored and driven by
   `vox`/`voxup` so building the emitted frontend needs no user-installed Node. End users
   consume the built artifact only.
5. **Rule-enforcement gate.** A CI gate that fails if a UI surface bypasses the SSOT
   (e.g. new hand-written `.tsx` that should have been `.vox`), so convergence is enforced,
   not aspirational.

## 4. Decomposition (this spec → multiple plans)

This is too large for one implementation plan. It decomposes into ordered sub-projects,
each getting its own plan via writing-plans. **This spec's plan covers Sub-project A only;**
the rest are sequenced follow-ons.

- **A. WebIR backend seam + coverage ledger (FOUNDATION — planned now).**
  Introduce `WebIrBackend` trait + `ReactTsxBackend`; produce the authoring-coverage ledger
  measuring current `.tsx` surfaces against WebIR expressibility. Output: the seam exists,
  and we have a measured gap list driving B–E. Low risk, high leverage, unblocks the rest.
- **B. WebIR primitive gaps.** Add the WebIR primitives the ledger shows missing (event
  handlers, controlled inputs, reactive `vox://*` stream binding).
- **C. Ecosystem-import completion.** Finish the `import react` path end to end against
  `external_libs.rs`. Depends on B.
- **D. Toolchain automation.** Vendor/auto-manage the build so it is node-free for end users.
- **E. Convergence gate + surface migration.** CI gate + migrate real `vox-gui/ui/` surfaces
  to `.vox`, surface by surface, until 95–99% coverage is reached.

## 5. Data Flow & Interfaces

- **Input:** `.vox` UI declarations (existing syntax + new primitives from B).
- **`WebIrBackend` trait:** `fn emit(&self, ir: &WebIr) -> Result<EmittedFrontend, EmitError>`.
  One impl now (`ReactTsxBackend`). Adding a backend never touches `lower.rs` or `validate*.rs`.
- **Validation contract:** all backends consume WebIR *after* `validate*.rs` has run; rules are
  enforced once, at the IR, not per-backend.
- **Interop contract:** `external_libs.rs` is the SSOT for which React packages are importable
  and what providers/CSS they require; the emitter injects these automatically.

## 6. Error Handling

- Lowering / validation errors surface as Vox compiler diagnostics (reuse the existing
  `vox_compiler::Diagnostic` path; note the known codegen↔compiler Diagnostic crate boundary).
- Unknown imported React package → a diagnostic pointing at `external_libs.rs` with the
  "promote to contract" guidance, not a silent passthrough.
- Backend emit failure is a typed `EmitError`, never a panic in the codegen lane.

## 7. Testing

- **Golden tests** per WebIR→TSX surface (extend existing `emit_tsx` goldens; structural-only
  goldens are insufficient — assert emitted behavior, not just shape).
- **Coverage ledger test:** a checked-in ledger of which `vox-gui/ui/` surfaces are
  `.vox`-expressible; CI asserts the ledger is current.
- **Interop e2e:** a `.vox` file importing one real library from `external_libs.rs` builds and
  renders (per-target: web + RN where the lib supports it).
- **Rule gate test:** a new hand-written `.tsx` UI surface that should be `.vox` fails the gate.

## 8. Risks & Open Questions

- **Reactive streams (the 13 `vox://*` channels)** are the hardest authoring-coverage gap — they
  are imperative event wiring today. Closing them in WebIR (Sub-project B) is the make-or-break
  for "99%". The ledger in A must quantify this honestly.
- **Mobile (React Native) parity:** `external_libs.rs` already models `LibTarget::Rn`; but the
  current `vox-gui/ui/` is web-only. Whether mobile is "same `.vox`, two backends" or a separate
  surface is an open question to resolve in B/E, flagged not decided here.
- **Convergence cost:** migrating every `vox-gui/ui/` surface (Sub-project E) is large and is
  deliberately deferred behind the seam + ledger so it can be done incrementally and measured.
