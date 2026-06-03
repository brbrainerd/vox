---
title: "Codegen SSOT & split-brain audit (2026)"
description: "Beginner-friendly advisory auditing every IR and emit stack on the path from .vox source to web, Tauri desktop, and React Native output; states the minimum emission set, separates necessary platform divergence from accidental duplication, and ranks the cleanup work."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-06-02"
training_eligible: true
training_rationale: "Canonical audit of the multi-target codegen pipeline; teaches the HIR-core + typed-projection model and the concrete minimum-emission set for web/Tauri/RN."
schema_type: "TechArticle"
---

# Codegen SSOT & split-brain audit (2026)

This is an **advisory**, deliberately written for a reader newer to Rust. It answers one
question: *across the web emitter, the Tauri desktop target, the React Native (Expo)
target, and Vox's own GUI, do we have "split-brain" — the same fact represented in two
places that can silently disagree — and what is the **minimum** we must emit to support
all three platforms from one source?*

It complements two existing documents and should be read alongside them:

- [WebIR / HIR split-brain inventory (2026)](./webir-hir-split-brain-inventory-2026.md) — the seam-by-seam map.
- [ADR 036 — WebIR vs HIR unification (compare-both)](../adr/036-webir-hir-unification-compare-both.md) — the decision of record.

> **Method.** Produced by a parallel audit (seven scout agents over the IR layer, the
> TS/JSX stack, the Rust/Tauri path, the RN path, the web runtime, the GUI surfaces, and
> the plan-vs-reality status), followed by an adversarial verification pass that tried to
> *disprove* each strongest finding. Claims were spot-checked by hand against current code.

## Vocabulary (so the rest reads cleanly)

- **IR (Intermediate Representation):** an in-memory data structure (`struct`/`enum`) the
  compiler holds *after* understanding your program but *before* writing output. The
  compiler's "notes."
- **Lowering:** turning one IR into a simpler one (AST → HIR). "Lower" = closer to output.
- **Projection:** a thin, read-only view derived from a bigger structure — like a SQL view,
  or a small struct you build by copying out only the fields you need. Cheaper and safer
  than a whole new IR.
- **Emitter:** code that walks an IR and writes text files (TypeScript, Rust, JSON…).
- **SSOT (Single Source of Truth):** one authoritative home for a fact, so it cannot drift.
- **Split-brain:** the failure SSOT prevents — the same fact in two+ places that disagree.

## Headline

**There is no platform-level split-brain. There is one brain.** The pipeline uses a single
semantic core — `HirModule` (the **HIR**) — and *all* targets read from it. This is a
recorded, deliberate choice: [ADR 036](../adr/036-webir-hir-unification-compare-both.md)
adopted **"Option B: one semantic core + thin typed projections"** and explicitly rejected
folding everything into one monolithic IR.

The correct mental model is: **one brain (HIR), a few specialized lenses on it, two output
languages (TypeScript + Rust).** That is a healthy design.

What the audit found on *top* of that core is **leftover mess from an in-progress
migration**: a handful of accidental duplications, some dead code, and a sprawl of
endpoint-contract structures. None of it breaks the three targets today; it is
maintenance tax plus a few latent bugs. That is the part worth acting on.

## The minimum set to support web + Tauri + React Native

### Representations — 5 (plus one tiny capability list)

| # | Thing | Defined in | Why it must exist |
|---|-------|------------|-------------------|
| 1 | **AST** | [`vox-compiler/src/ast/`](../../../crates/vox-compiler/src/ast) | Raw parse tree. Unavoidable. |
| 2 | **HIR (`HirModule`)** | [`hir/nodes/decl.rs`](../../../crates/vox-compiler/src/hir/nodes/decl.rs) | **The single source of truth.** Every target reads it. |
| 3 | **WebIR** | [`web_ir/mod.rs`](../../../crates/vox-codegen/src/web_ir/mod.rs) | UI/DOM lens: validates views, emits JSX. Shared by web, the Tauri webview, and mobile. |
| 4 | **ContractIR** | [`contract_ir/mod.rs`](../../../crates/vox-compiler/src/contract_ir/mod.rs) | API/wire-format lens (types + endpoints) for the client SDK, Zod, OpenAPI. Shared by all. |
| 5 | **ShellProjection** | [`shell_projection.rs`](../../../crates/vox-compiler/src/shell_projection.rs) | The genuinely mobile/desktop primitives — `@back_button` / `@deep_link` / `@push`. No web equivalent. |

Plus [`RequiredRuntimeCapabilities`](../../../crates/vox-compiler/src/required_capabilities.rs),
a small sorted list of permission ids (`net.http`, `deep_link`, …) used to fill the
Tauri/Android/iOS manifests.

### Emitters — 2 families

- **`codegen_rust`** — the Rust backend, with two sub-paths off one `RustAppShell` enum:
  Axum server *or* Tauri commands.
- **`codegen_ts`** — TypeScript, with the web React-DOM path and the React Native variant.

### Shared runtime npm packages — 3 (should be 4)

[`@vox/runtime-types`](../../../clients/runtime-types) (the interface contract),
[`@vox/runtime`](../../../clients/runtime-web) (Tauri desktop), and
[`@vox/runtime-rn`](../../../clients/runtime-rn) (mobile). These let runtime helpers be
shared *by import* rather than copy-pasted into every app. The missing fourth — a
browser-native `@vox/runtime-web` — is a real gap (see finding T2-9).

> **Note on an older framing.** Some notes described the goal as "4 IRs → 2." The plan docs
> actually target **3 layers (HIR + WebIR + ContractIR) and 2 emitters**, so "→ 2 IRs" was
> an overstatement. The 3-layer target is the realistic SSOT shape, and the codebase is
> close to it.

## What exists today vs. that minimum

The tree currently has roughly **8 named IR-ish structures** where 5 would do, and **3
emitter entry-points** where the "2 families" hide a third (the RN one). The extras are:

- **Necessary** (leave alone): `HIR`, `WebIR`, `ContractIR`, `ShellProjection`, the
  Rust-vs-TS split, the RN view renderer (it must produce `<View>/<Text>`, not `<div>`),
  and `vox-client.ts`'s single-file `isTauri()` runtime branch (good design — one file,
  branches at runtime).
- **Same thing, second name** (cosmetic): `TypedCoreIR_v2` is an alias for `HirModule`;
  `WebProjectionIR` is an alias for `WebIrModule`. Noise, not danger.
- **Redundant or dead**: the cleanup list below.

## Findings — necessary vs. accidental

Each finding below survived an adversarial re-check (a second agent tasked with disproving
it). Severity is the agent's; the tier grouping is editorial.

### Tier 1 — real duplication worth fixing

- **T1-1 · Four parallel "what is an HTTP endpoint" representations.**
  [`AppContractModule`](../../../crates/vox-compiler/src/app_contract.rs),
  [`ContractIR`](../../../crates/vox-compiler/src/contract_ir/mod.rs), `WebIR`'s `RouteNode`
  ([`web_ir/mod.rs`](../../../crates/vox-codegen/src/web_ir/mod.rs)), and
  `RouteIR` (then in `codegen_shared/route_ir.rs`) each re-derive
  endpoint name/path/method/params from `HirModule.endpoint_fns`. Renaming one endpoint can
  touch four lowering paths. `RouteIR` was *built* to be the SSOT here, but the TypeScript
  side ignores it. **This is the single most expensive split-brain.** Fix: make `ContractIR`
  the one endpoint lens; have the others reference it.
- **T1-2 · A dead component emitter.** [`component.rs`](../../../crates/vox-codegen/src/codegen_ts/component.rs)
  `generate_component` + the AST JSX walker in
  [`jsx.rs`](../../../crates/vox-codegen/src/codegen_ts/jsx.rs) (~900 lines) have zero
  call-sites — the live loop only calls `generate_reactive_component`. There is also an
  orphaned `activity.rs` (then under `codegen_ts/`) that
  Cargo never compiles. *Caveat:* `jsx.rs` re-exports two helpers used by tests, so the fix
  is "delete the dead functions, keep the re-export," not delete the file.
- **T1-3 · The view is rendered twice and one copy is discarded.** In
  [`reactive.rs`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs) every component
  view is emitted by the canonical WebIR path *and* by the legacy `emit_hir_expr`, only to
  compare them for a "parity" counter. CI never asserts that counter is zero, so a silent
  disagreement is accepted and every build pays to render twice. Migration scaffolding past
  its purpose; remove once a one-time CI assertion confirms parity.
- **T1-4 · `VoxIrModule` is a clone of HIR.**
  [`vox_ir/lower.rs`](../../../crates/vox-codegen/src/vox_ir/lower.rs) copies 11 HIR fields
  verbatim and embeds a *second* WebIR derivation. Nothing in the real emit pipeline reads
  it — only `vox check --emit-ir` (a debug dump) and a schema test. Since `HirModule`
  already derives `serde`, this could be a thin metadata wrapper over HIR instead of a
  parallel struct.

### Tier 2 — real but smaller / partly necessary

- **T2-5 · Two Rust endpoint-body emitters** (Tauri command vs Axum handler,
  [`codegen_rust/emit/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/mod.rs) and
  [`http.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/http.rs)). Verifier
  downgrade: only the ~6-line param-extraction loop is a true clone; response wrapping
  legitimately differs (Tauri returns native Rust types, Axum returns `Result<Json<…>>`).
  Extract the small helper; leave the rest.
- **T2-6 · `Cargo.toml` emitted twice** — the Axum and Tauri templates copy-paste 12
  identical crate deps, kept in sync by a fragile find-replace on path depth.
- **T2-7 · Form validation copied web↔RN** — `validate()` (~35 lines) is duplicated between
  [`form_emit.rs`](../../../crates/vox-codegen/src/codegen_ts/form_emit.rs) and
  [`rn/form.rs`](../../../crates/vox-codegen/src/codegen_ts/rn/form.rs). The rendering
  difference (`<input>` vs `<TextInput>`) is necessary; the validation logic is not.
- **T2-8 · Helper DRY drift** — `hir_type_to_ts` exists in ~5–6 places and
  `inject_key_into_jsx` in 3–4. Each is a chance to diverge when a new type is added.
- **T2-9 · No `@vox/runtime-web`.** Mobile imports its runtime from a package; the browser
  runtime (~112 lines) is instead inlined into every app as `runtime-install.ts`
  ([`web_entry.rs`](../../../crates/vox-codegen/src/codegen_ts/web_entry.rs)). Web is the one
  platform not yet using the shared-package pattern it already established for mobile.
- **T2-10 · Two mobile storage backends** — an expo-file-system NDJSON journal (JS) and a
  uniffi-bridged Rust `vox-journal` ([`vox-runtime-rn`](../../../crates/vox-runtime-rn)).
  The Rust one is the better implementation and was intended to replace the JS one.

### Tier 3 — strategic gap & latent bugs (not duplication, but surfaced)

- **T3-11 · Vox's GUI does not dogfood its own codegen.** [`vox-gui`](../../../crates/vox-gui)
  is 100% hand-written React built by Vite; it does not pass through the HIR→TSX pipeline
  that user apps use. Defensible (different kind of app) but it removes dogfooding pressure
  from the compiler, and stray dead surfaces exist (`UnifiedDashboard.tsx`, a
  `claude-dashboard/` folder of orphaned `.jsx`). Worth an explicit ADR note either way.
- **T3-12 · Latent bugs to log:** Tauri apps silently drop `@scheduled` functions (the
  desktop `main.rs` skips the durable-boot prelude the Axum path emits); the speech-to-text
  plugin is bundled into every desktop app even when unused; the GUI's `ActionManifest`
  marks `mobile: true` for CLI actions that cannot run on mobile; the web scaffold lists
  `react-router` as a dependency though the emitted router bans it.

## Plan vs. reality

The "codegen SSOT unification" effort was absorbed into
[external-frontend-interop-plan-2026.md](./external-frontend-interop-plan-2026.md). On disk:

| Phase | Intent | Status |
|---|---|---|
| P1 | Split build targets (`--target server\|fullstack\|client`) | **Landed** |
| P2 | Wire-format SSOT / ContractIR; deprecate `vox_client.rs` | **Partial** — `vox_client.rs` was redirected through ContractIR but never deprecated |
| P3 | HTTP-ergonomics decorators (`@cors`/`@auth`/`@rate_limit`) | **Not started** |
| P4 | Auth/session/observability stdlib | **Not started** |
| P5 | Bidirectional Vox↔React interop | **Partial** (`@island` retired; `extern component` not yet parsed) |
| (post-plan) | React Native / Expo target (PR #97) | **Landed** |

Also note: Tauri is now a **first-class generated target** — `generate_tauri_workspace`
([`codegen_rust/emit/mod.rs`](../../../crates/vox-codegen/src/codegen_rust/emit/mod.rs))
emits a complete `src-tauri/` crate (`main.rs` with `#[tauri::command]` handlers, a `lib.rs`
shared with the Axum path, `tauri.conf.json`, capabilities). The older inventory's "Tauri
Rust command path — stub only" line is therefore stale and should be refreshed.

## Bottom line

- **Is the brain split? No.** HIR is a genuine SSOT; platform-specific parts are correctly
  isolated into thin projections + runtime packages. The ADR 036 decision is sound.
- **Minimum to emit:** 5 representations (AST, HIR, WebIR, ContractIR, ShellProjection +
  a tiny capability list), 2 emitter families (Rust, TS), 4 shared runtime packages
  (one still missing for web).
- **The work is consolidation, not redesign:** collapse the four endpoint-contract copies
  into ContractIR (T1-1), delete the dead component emitter (T1-2), retire the parity
  double-render (T1-3), and DRY up the copy-pasted helpers/forms/`Cargo.toml`. That moves
  the tree from ~8 structures to the ~5 minimum and clears the latent bugs.

## Related

- [WebIR / HIR split-brain inventory (2026)](./webir-hir-split-brain-inventory-2026.md)
- [ADR 036 — WebIR vs HIR unification (compare-both)](../adr/036-webir-hir-unification-compare-both.md)
- [ADR 012 — Internal web IR strategy](../adr/012-internal-web-ir-strategy.md)
- [External frontend interop plan (2026)](./external-frontend-interop-plan-2026.md)
- [Frontend convergence findings (2026)](./frontend-convergence-findings-2026.md)
- [Where Things Live](./where-things-live.md)
