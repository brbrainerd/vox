---
title: "Vox-Native Frontend SSOT — author the GUI in Vox, emit through a formalized backend seam, interop with React"
category: "Architecture SSOTs"
status: draft
date: 2026-06-20
audited: true
---

# Vox-Native Frontend SSOT

> **Audit status:** every "current state" claim below is verified against the code at the
> cited `file:line`. Findings that corrected the first draft are marked **[audit-corrected]**.

## 1. Problem & Goal

The Vox GUI is hand-written React/TSX (`crates/vox-gui/ui/` — **173 `.tsx` files across 25
surface directories**, all hand-authored, zero Vox source), built with Vite/pnpm, shipped
inside a Tauri shell. This couples us to the JS ecosystem at three layers — authoring
(hand-written `.tsx`), build (Node/Vite/pnpm), runtime (React) — and splits the UI's source
of truth away from the rest of the Vox codebase.

**Goal:** make the Vox *language* able to author the frontend, so the GUI converges to a
single Vox source of truth, reproducing 95–99% of the current frontend *from `.vox`*, while
preserving what we refuse to lose.

**This is NOT a dependency-minimization project.** It minimizes *release* dependencies and
*technical debt*. The build toolchain may stay heavy if it is auto-managed and invisible to
end users.

### Non-negotiable constraints (from brainstorming)

1. **React ecosystem interop is mandatory** — a `.vox` author must import and run a real
   React/JS component (calendar, chart), Zig-consumes-C style, not reimplement in raw JS.
2. **Emit is a means, not an end** — output format is free (React/TSX today) as long as it
   runs, looks good, and covers **mobile + desktop**.
3. **No end-user Node** — end users must never know Vite/pnpm/Node exists. The toolchain may
   be vendored/auto-installed behind `voxup`-style management ("move it into the factory line").
4. **Single source of truth, codebase-wide** — one `.vox → emission` pipeline, one enforceable
   rule surface, no split-brain registries.

### Out of scope

- A Vox-native browser runtime replacing React (the "Model 2 island" runtime). Forfeited as a
  near-term goal because it breaks constraint #1; preserved only as a *future backend* (§2).
- Replacing Tauri / moving to a plain browser shell — a separate program. This spec is about
  the *authoring + emission* pipeline regardless of host shell.

## 2. Chosen Architecture — Model 3 spine, Model 1 backend

Three models were weighed: **M1** React is the substrate (Vox transpiles into React/TSX);
**M2** Vox-native runtime with React as islands (forfeited — dual reactivity = high debt);
**M3** adapter-defined target (Vox lowers to a stable IR; a swappable backend renders it).

**Decision: Model 3 spine with Model 1 (React/TSX) as the sole concrete backend.**

**[audit-corrected] The Model 3 seam already partially exists.** It is NOT a greenfield trait.
`crates/vox-codegen/src/emission_profile.rs:54` defines:

```rust
pub fn validate_bundle_with_registry(&self, bundle: &ProjectionBundle, registry: Option<&TokenRegistry>)
    -> Vec<ProfileDiagnostic> {
    match self.target {
        Target::TypeScript | Target::RustTauri => validate_web_for_bundle(&bundle.web, registry),
        Target::RustAxum | Target::Interpreter  => validate_runtime_for_bundle(bundle),
    }
}
```

A `Target` enum already selects validation behavior. What's missing is that *emission itself*
is still a direct call — `vox-codegen-ts::generate_with_options(&hir, opts)` consumes
`bundle.web` directly (`emitter.rs:198,231`); React/TSX is hard-coded with no emit-side
indirection. `emit_tsx.rs` is explicitly **diagnostic/parity-only** (`emit_tsx.rs:1-7`), not
the production emitter.

```
.vox source → HIR
   │  lower_hir_to_web_ir(hir)              lower.rs:923      ← SSOT seam #1 (all UI semantics)
   ▼
 WebIR (ProjectionBundle.web)
   │  validate_web_ir_full()               validate.rs:960   ← SSOT seam #2 (a11y/palette/layer/
   │                                                            overlay/keys, run as ONE pass)
   ▼
 EmissionProfile::for_target(target)       emission_profile.rs ← the seam to FORMALIZE
   │
   ▼  [Model 1 backend — the only one built now]
 React/TSX  (vox-codegen-ts, emitter.rs)
   │
   ▼  auto-managed Vite/pnpm build (vendored, hidden)  →  web bundle  →  mobile + desktop
```

The deliverable for the seam is to route production emission through `EmissionProfile`/`Target`
(formalize the existing partial seam), **not** to invent a parallel `WebIrBackend` trait. A
future leaner backend (Web Components/WASM) becomes a new `Target` arm; it does not touch
`lower.rs` or `validate*.rs`. **We do not build that backend now — we only keep the seam honest.**

### Why this matches the goal
- **SSOT:** `.vox → HIR → WebIR` is the single source; `validate*.rs` the single rule surface.
  React becomes an output format, not the source of truth.
- **Interop:** Model 1 backend makes imported React components first-class (§3.3).
- **Emit-agnostic:** `Target` selection makes "what we emit" a swappable detail.
- **Release deps:** the Vite/pnpm step moves behind toolchain automation (§3.4).

## 3. Mobile decision (LOCKED) — M3: first-class responsive web + PWA, React Native omitted

Mobile is served by **responsive web + installable PWA from the single React-DOM backend**,
with native device reach (when needed) from a **WebView shell + capability plugins** — *not* a
second UI runtime. React Native is **omitted** as a near-term target and preserved only as a
deferred `Target` arm.

### The decisive argument
The thing we prize most — React ecosystem support — is what a React Native path quietly breaks.
A React-DOM calendar/chart does **not** run under RN; RN has its own smaller, incompatible
component universe and would force a *second* interop table. Omitting RN is not the compromise;
it is what keeps ecosystem interop coherent. "Second-class mobile" is solved by making mobile a
first-class *layout/capability* concern of the IR, not by forking the runtime.

### Cost/benefit (on the axes that matter: quality, maintainability, tech debt, performance)
| Axis | M3 chosen (responsive web + PWA) | Rejected (dual web + RN) |
|---|---|---|
| Code quality / SSOT | One IR, one backend, one primitive set | Two backends, two primitive dialects, drift |
| Maintainability | Mobile = layout/capability concern | Every primitive & surface maintained twice |
| Technical debt | No second toolchain, no RN treadmill | RN/Metro/native-module churn, parity drift |
| Performance | Good on modern mobile web; ceiling below native for heavy gesture/animation | Native ceiling, only where actually needed |
| React ecosystem interop | Strengthened — one React-DOM universe | Fractured — two component registries |

**Accepted loss (on record):** the top-end native gesture/animation performance ceiling and
platform-native *look* (web styled like iOS ≠ real UIKit). For Vox GUI workloads (dashboards,
chat, task lists, terminals) that ceiling is not in play.

### [audit-corrected] What this requires vs. what exists
- **`vox-rn-codegen` crate DOES exist** and partially emits RN (`component.rs:1609-1787`),
  with a known external-import tag-registration gap (`component.rs:1661`, `lib.rs:66-71`).
  Under M3 this backend is **explicitly deprioritized, not deleted** — it stays as the
  embryonic future `Target::ReactNative` arm but receives no investment here.
- **No PWA infrastructure exists** — `index.html` has no manifest/service-worker/theme-color;
  `vite.config.ts` has no PWA plugin. **Net-new** (Sub-project D).
- **No mobile-first enforcement** — no validator requires responsive/touch-target/mobile
  layout; responsive today is *manual* Tailwind passthrough (`primitives/mod.rs` strips
  `md:`/`hover:` prefixes as opaque pass-through; ~90 ad-hoc `sm:`/`md:` usages in the React
  GUI). A **mobile-first VUV layout rule + validator is net-new** (Sub-project B/E).

## 3.1–3.4 Components

### 3.1 Existing foundations (reuse, do not rebuild) — all verified
| Component | Path:line | Role | Audit note |
|---|---|---|---|
| Lowering entry | `web_ir/lower.rs:923` `lower_hir_to_web_ir` | HIR→WebIR, single entry | ✅ |
| Projection bundle | `projection_bundle.rs:32` `project_bundle_from_hir` | one place lowering happens | ✅ |
| Validation pass | `web_ir/validate.rs:960` `validate_web_ir_full` | all 5 validators in ONE pass | ✅ run before emit gate |
| Target seam (partial) | `emission_profile.rs:54` `Target` enum | per-target validation already | ✅ **seam to formalize** |
| Production emitter | `vox-codegen-ts/src/emitter.rs:198` `generate_with_options` | HIR+WebIR→TSX | ✅ React hard-coded |
| TSX parity emitter | `web_ir/emit_tsx.rs:37` | diagnostic/parity ONLY | ⚠️ not production |
| External-lib SSOT | `vox-codegen-ts/src/external_libs.rs:61` | 12 libs (MUI/Mantine/antd/Radix/…+3 RN) | ✅ |
| Import grammar | `parser/.../head_import.rs:87` | `import react X from "pkg"` parses | ✅ |
| Interop sub-spec | `docs/src/architecture/external-frontend-interop-phase5-component-interop-subspec-2026.md` | existing program S1–S8 | converge with it |

### 3.2 Authoring coverage — [audit-corrected], the real gap is narrower and sharper
| Authoring capability | Status | Evidence |
|---|---|---|
| Elements, layout, styles | ✅ works | `lower.rs` stages R/S/B/D |
| Event handlers (`on_click={fn(){…}}`) | ✅ works | `lower.rs:483-505`; real use `apps/vox-mental-tracker/src/main.vox:470` |
| Controlled inputs (`bind={state}`) | ✅ works | expands value+onChange, `hir_emit/mod.rs:206-229` |
| `state` declarations | ✅ works | → `useState` |
| `on mount:` init | 🟡 partial | one-shot only; deps hardcoded `["mount"]` `lower.rs:851` |
| `derived` computed | 🟡 internal only | `BehaviorNode::DerivedDecl` exists, **no `.vox` syntax** |
| Custom effects w/ deps | ❌ gap | no `on deps[x,y]:` form |
| Cleanup (`on cleanup:`) | ❌ dead code | `OnCleanup` lowered but **not parseable** `lower.rs:859` |
| **External reactive streams (`vox://*`)** | ❌ **CRITICAL gap** | 13 streams hand-wired in React `transport.ts`, `App.tsx:414-458`; no `.vox` subscription primitive |

The **13 `vox://*` streams** (orch-status, agent-events, pty-output, browser-frame, …) are the
make-or-break for "99%". They are imperative `listen()`→`useEffect`→`unlisten` wiring with
`.catch` degradation — none expressible in `.vox` today. Closing them (a subscription primitive
+ effect deps + cleanup) is the highest-value, highest-risk authoring work.

### 3.3 Ecosystem-import path — [audit-corrected] mostly works on web; converge with Phase 5
End-to-end on web **works**: `import react Button from "@mui/material"` parses
(`head_import.rs:87`), lowers to HIR with `es_module_specifier`/`es_import_kind`
(`hir/lower/mod.rs:160-194`), emits grouped ES imports + auto-injects required CSS/provider
guidance (`reactive/imports.rs:3-94`), and renders `<Button/>` as a JSX tag
(`reactive/bindings.rs:117`). Verified gaps to finish (track Phase 5 slices S1–S8):
- **S7 — RN external-import tag registration** (`component.rs:1661`) — deprioritized under M3.
- **React dedupe** — scaffold doesn't emit Vite `resolve.dedupe:['react','react-dom']` → risk
  of duplicate-React "Invalid hook call".
- **Manifest auto-add** — imported packages not written to `package.json`; user must `pnpm add`.
- **Type bridge (opt-in flat facade, `vox import-types`)** — designed, not implemented.
- **Provider enforcement** — mandatory providers (Chakra/Mantine) emitted as *guidance comment*
  only; no validator fails when the app root lacks the provider.

### 3.4 What this program adds / completes
1. **Formalize the backend seam** on `EmissionProfile`/`Target` so production emission is routed
   through it (the Model 3 commitment), not the from-scratch trait the first draft implied.
2. **Authoring-coverage ledger** — checked-in, measured (not asserted) map of which of the 173
   `.tsx` surfaces are `.vox`-expressible today vs. blocked on a named gap (§3.2). The "95–99%"
   is defined against this ledger.
3. **Reactive-stream authoring primitive** — close the `vox://*`/effect-deps/cleanup gap.
4. **Ecosystem-import completion** — finish Phase 5 web slices (dedupe, manifest, providers,
   type bridge); leave RN slice deprioritized.
5. **Mobile-first VUV rule + PWA scaffold** — a validator making responsive/touch first-class,
   plus PWA manifest/service-worker emission.
6. **Toolchain automation ("factory line")** — vendored/`voxup`-driven Vite/pnpm so building the
   emitted frontend needs no user-installed Node; end users consume the built artifact only.
7. **Convergence gate** — CI fails when a new UI surface is hand-written `.tsx` that should be
   `.vox`, so SSOT convergence is enforced, not aspirational.

## 4. Decomposition (this spec → ordered plans; each its own writing-plans cycle)

Too large for one plan. **This spec's first plan covers Sub-project A only;** the rest are
sequenced follow-ons, each measured against the A ledger.

- **A. Backend-seam formalization + coverage ledger (FOUNDATION — planned next).** Route
  production emission through `EmissionProfile`/`Target`; produce the audited coverage ledger.
  Low risk, high leverage, unblocks and *measures* B–G. No user-visible behavior change.
- **B. Reactive-stream + effect authoring primitive.** The `vox://*`/deps/cleanup gap — the
  make-or-break for 99%. Depends on A's ledger to scope exact surface needs.
- **C. Ecosystem-import completion (web).** Finish Phase 5 web slices (dedupe, manifest,
  providers, type bridge). Converge with the existing sub-spec; do not duplicate it.
- **D. Mobile-first rule + PWA scaffold.** Mobile-first VUV validator + PWA emission.
- **E. Toolchain automation.** Node-free-for-end-users build behind `voxup`.
- **F. Convergence gate.** CI gate forbidding new off-SSOT `.tsx`.
- **G. Surface migration.** Migrate the 173 `.tsx` surfaces to `.vox`, surface by surface,
  ledger-driven, until 95–99% coverage. Largest; deliberately last and incremental.

## 5. Data Flow & Interfaces
- **Input:** `.vox` UI declarations (existing syntax + B's reactive primitives).
- **Seam contract:** emission selected by `EmissionProfile::for_target(target)`; adding a target
  never edits `lower.rs` or `validate*.rs`. One concrete arm now (`Target::TypeScript`).
- **Validation contract:** all targets consume WebIR *after* `validate_web_ir_full` runs; rules
  enforced once, at the IR.
- **Interop contract:** `external_libs.rs` is the SSOT for importable packages + required
  providers/CSS; the emitter injects these automatically.

## 6. Error Handling
- Lowering/validation errors surface as Vox compiler diagnostics (reuse existing path; note the
  known codegen↔`vox_compiler::Diagnostic` crate-boundary seam).
- Unknown imported package → a diagnostic pointing at `external_libs.rs` ("promote to contract"),
  never a silent passthrough.
- Backend emit failure is a typed error, never a panic in the codegen lane.

## 7. Testing & execution model (Claude Sonnet 4.6 via Claude Code)
This program is executed by **Claude Sonnet 4.6 in the Claude Code harness**. Plans MUST be
shaped for it:
- **TDD is mandatory** — each slice writes the failing test first: golden `.vox→.tsx` tests
  (extend existing `emit_tsx`/`semcov` goldens — assert emitted *behavior*, not just shape;
  structural-only goldens are insufficient), validator unit tests, and the ledger-currency test.
- **Parallelizable slices** — sub-projects and independent slices within them are written so the
  harness can fan out parallel sub-agents / a workflow (e.g. per-validator, per-surface-family
  migration in G). Mark each slice `[PARALLEL-SAFE]` or `[SEQUENTIAL]` in its plan.
- **Verification before completion** — every slice ends with the concrete command + observed
  output proving green; no "done" without evidence.
- **Coverage-ledger test** — CI asserts the checked-in ledger is current; this is how 95–99%
  is measured rather than claimed.
- **Interop e2e** — a `.vox` importing one real `external_libs.rs` package builds and renders.
- **Convergence-gate test** — a new off-SSOT `.tsx` surface fails the gate (Sub-project F).

## 8. Risks & Open Questions
- **Reactive streams (Sub-project B)** are the single biggest risk to "99%". A's ledger must
  quantify exactly how many of the 173 surfaces depend on `vox://*` wiring.
- **React dedupe** (§3.3) is a runtime-crash risk for ecosystem interop if not handled in the
  scaffold — prioritize within C.
- **`derived`/`on cleanup` are half-built** (lowered but unparseable) — B should finish these
  honestly rather than leave dead code paths.
- **Mobile look-and-feel acceptance** — the accepted native-look loss (§3) should be re-confirmed
  with stakeholders once the first responsive surface ships under D.
- **Surface-migration cost (G)** — 173 `.tsx` files; deferred behind the seam + ledger so it is
  incremental and measured, never big-bang.
