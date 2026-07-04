---
title: "Vox Design — Naked-Objects Auto-GUI & Zero-Annotation Severity-Graded Debugging"
description: "A Vox-specific concept design for (A) auto-generating admin/CRUD GUIs from types (naked-objects step) and (B) a zero-annotation interpreter execution-event stream with inferred severity, dual-piped to humans (CLI/GUI) and an LLM, building on existing vox-codegen-ts, vox-telemetry, and the --for-llm diagnostic envelope."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
---

# Vox Design — Auto-GUI & Zero-Annotation Severity-Graded Debugging

**Status:** Concept design (no implementation; not yet a TDD plan). Grounded in five thematic research docs (which superseded the earlier combined drafts):
- [auto-GUI from pure logic](auto-gui-from-pure-logic-research-2026-06-18.md) — inline references below as *"research §2.x"* map here.
- [auto-debugging (zero-annotation)](auto-debugging-zero-annotation-research-2026-06-18.md) — inline *"research §3.x"* map here.
- [error surfacing (dual-audience)](error-surfacing-dual-audience-research-2026-06-18.md) — inline *"research §3.3/§5"* (LLM piping) map here.
- [design hygiene](auto-derivation-design-hygiene-2026-06-18.md) — opt-in, selectivity, advise-not-gate.
- [Gemini 3.5 Flash / Antigravity limitations](gemini-3-5-flash-antigravity-limitations-2026-06-18.md) — execution-target reference for the plans.
**Date:** 2026-06-18.
**Scope note:** This is a *design doc* deliverable, not an approved plan. Topic-B prior art is `[unverified-this-run]` (verifiers were rate-limited); re-verify before committing engineering.

---

## 1. Design principle (one sentence)

> Vox should derive both the **presentation layer** and the **observability layer** from the program's structure, so that an admin UI and a severity-graded debug stream exist *by default* — and both are born **dual-audience** (human + LLM) because Vox owns the format.

The research established Vox is **~70% into auto-GUI** and has a **mature-but-runtime-silent** observability stack. This design targets only the missing 30% / the silent runtime, reusing what exists. It does **not** rebuild forms, validators, telemetry facades, or the compile-time diagnostic envelope.

---

## 2. Track A — Naked-Objects Auto-GUI from types

### 2.1 What we are NOT building
Forms-from-`@form`, type→Zod/TS/DB, clap→GUI controls, `@reactive component`→React. All shipped (see research §2.4). Extending these is not the novelty.

### 2.2 What we ARE building: the naked-objects step (OPT-IN ONLY)

> **Design correction (2026-06-18):** persistence ≠ UI intent. A `@table` declares *"store this,"* not *"build me a screen."* Auto-GUI is therefore **strictly opt-in** via an explicit `@admin` marker — declaring a table NEVER generates UI on its own. This matches Django's battle-tested explicit-registration model (auto-register-all is known to cause conflicts) and bounds the blast radius. See [design hygiene §1](auto-derivation-design-hygiene-2026-06-18.md).

A single **opt-in** `@admin` annotation on a type/table emits a **complete admin surface** (list + detail + edit + create), with CRUD endpoints auto-wired — the proven Naked Objects pattern (Pawson/DSFA, research §2.1), expressed in Vox idiom and rendered through VUV's *typed* design tokens. The `@admin(...)` hint args play the role of react-jsonschema-form's `uiSchema` — a small typed presentation layer over the derived structure ([auto-GUI research §4](auto-gui-from-pure-logic-research-2026-06-18.md)).

```vox
// vox:skip — illustrative surface syntax, not yet in the grammar
@admin(title: "Users", searchable: [name, email], readonly: [created_at])
table User {
  id: uuid
  name: string
  email: email          // branded scalar → typed input + validation
  role: Role            // enum → <select>
  created_at: timestamp
}
```

Emits, with **no hand-written UI**:
- **List view** — columns from fields; `searchable` fields → filter controls (from indexes where present).
- **Detail view** — read-only projection of one row.
- **Edit/Create form** — reuses the existing `form_emit.rs` machinery; honors `readonly`.
- **CRUD endpoints** — generated `@query`/`@mutation` wired to the form, reusing the existing client/validator codegen.

### 2.3 Required enabling work (the real gaps)

1. **Richer typed field inference.** Today `hir_type_to_input_type()` is `int/float/bool/timestamp/else→text`. Extend the *type* table (not per-form code) so derivation is structural:
   - enum → `<select>` over its variants
   - branded scalars (`email`, `url`, `uuid`, `phone`) → typed input + HTML5/Zod validation (one entry in the scalar registry flows to input type + validator + DB constraint simultaneously)
   - nested struct → fieldset; `list<T>` → repeating block / multi-select
2. **`@admin` annotation + a `contracts/gui/admin-registry.yaml` SSOT** (mirroring the existing `command-registry.yaml` / `dynamic_mapping.rs` pattern) so hints are declarative and registry-checked, not scattered.
3. **Admin codegen module** in `vox-codegen-ts` that composes existing list/detail/form emitters.

### 2.4 Honoring the intent/affordance limit (research §2.3)
`@admin` is **escape-hatchable by construction**: it generates an envelope; any view can be overridden with a hand-authored VUV component. Default to generated; opt out per-surface. We explicitly do **not** claim consumer-grade/graphics-rich UI — this targets the structurally-regular envelope (admin, CRUD, internal tools, data exploration) where Naked Objects is proven and where Hasura/Retool/Django-admin already win commercially.

### 2.5 K-complexity ledger (Track A)
- **Added language surface:** one annotation (`@admin`) + N branded scalars. Small, additive, optional.
- **Removed:** entire hand-written admin UIs per entity.
- **Net:** strongly reductive for the target envelope.

---

## 3. Track B — Zero-annotation, severity-graded debugging

### 3.1 The three missing pieces (research §3.5)
Vox has the telemetry facade, trace propagation, and a dual-audience **compile-time** diagnostic envelope. It lacks: (1) automatic **runtime** event capture, (2) **inferred** severity, (3) a runtime **human+LLM multiplexer**. This track adds exactly those, reusing the rest.

### 3.2 Component 1 — Execution event stream (the capture layer)

A new L2 crate `vox-execution-tracer` instruments the interpreter eval loop (`vox-compiler::eval::*`) to emit a structured stream **without any source annotation**:

```
ExecutionEvent { step_id, kind, span, value_summary, elapsed_ns, scope_depth }
  kind ∈ { FunctionCall, Return, VariableBind, Branch, Loop, EffectPerformed, Error }
```

- Routed through the existing `record_event!` macro / `vox-telemetry` (new `METRIC_TYPE_EXECUTION_EVENT`, sensitivity S1).
- **Off by default; opt-in via `vox run --trace` or config** — the interpreter stays zero-overhead in production. This is "zero-annotation" (no `print` in source) not "always-on" (which the research's over-logging finding, §3.2, warns against).
- The deterministic single-threaded interpreter (`run_frontend_str_with_options`) makes the stream **replayable**, echoing time-travel debugging (research §3.1) without a full record/replay engine.

### 3.3 Component 2 — Severity inference (the selectivity layer)

The research is unambiguous: **selectivity is the whole game** — only 30–42% of catch blocks are logged by humans; blanket tracing is trace-spam (§3.2, §3.4). So severity is **inferred and used to filter**, not to label everything:

`infer_severity(event, context) -> Severity` over **structural + dynamic** signals (a rules-first, ML-later staircase — no model dependency to ship v1):
- **Error / Warn:** event is in/under an `Error` kind; effect failed; panic/`EvalError`; null-deref-adjacent; type-narrowing failure.
- **Notice:** anomaly vs. the run's own baseline — branch hit far more/less than siblings; recursion depth spike; a hot loop (echoes the structural categories LogAdvisor/*Where Do Developers Log* learned, research §3.2).
- **Debug/Trace:** ordinary binds/returns — emitted only when the user asks for that verbosity.

Default surfacing threshold = **Notice+**, so "run it and see what's interesting" shows anomalies and errors, not every assignment. v2 can swap the rules for a DeepLV-style learned model behind the same interface.

### 3.4 Component 3 — Dual-audience multiplexer (the presentation layer)

Generalize the *already-proven* `--for-llm` diagnostic pattern from compile-time to runtime. One event stream, three sinks:

```
enum DebugSink {
  HumanCli  { color, severity_threshold }      // reuse vox-cli-core/diagnostics.rs
  HumanGui  { timeline + severity heatmap }     // new vox-gui surface
  Llm       { minimal_repro + execution_context + suggested_cause }  // reuse VoxCompilerDiagnosticPayload shape
}
```

- **Human CLI:** `vox run --trace` prints a severity-colored execution timeline to stderr (Notice+ by default).
- **Human GUI:** a vox-gui timeline/heatmap surface (auto-generatable via Track A's machinery — pleasing self-consistency).
- **LLM:** the runtime event window around an error is serialized into the **existing** `VoxCompilerDiagnosticPayload` envelope (gaining an `execution_context` field), so the LLM repair loop gets *runtime* context, not just static source — directly answering "pipe debug info to an LLM." This is where Vox can lead: the research found little prior art on **one stream serving both audiences** (research §3.3, §5), and Vox already half-owns it.

### 3.5 K-complexity ledger (Track B)
- **Added language surface:** *zero.* No new keywords, no `print`. The feature is flags + config + one crate.
- **Removed:** the habit of sprinkling `print`/`log` and hand-choosing levels.
- **Net:** the strongest K-complexity-reduction case in the whole design — the cost moves from *source text* (where it rots) to *runtime/config* (centralized, toggleable), exactly the §3.4 argument.

---

## 3b. Track C — VUV as an ideal target for external AI UI generators

> Added 2026-06-18. Research: [AI UI generators & Vox-as-target](ai-ui-generators-and-vox-as-target-research-2026-06-18.md). Where Track A is *Vox-as-generator*, Track C is *Vox-as-target*: let external tools (v0.dev, Claude Design, Cursor, Claude Code) emit VUV and inherit Vox's compile-time guarantees.

**The thesis:** the whole field concludes good AI UI comes from **constraints encoded in the system, not the model**, and the integration standard is a **component/token registry read over MCP**. Vox already has the rare half nobody else does — **compile-time contrast (`validate_palette.rs`), occlusion/z-tier (`validate_layer.rs`), and a11y (`validate_a11y.rs`) enforcement** + typed tokens (`contracts/tokens/tokens.v1.json`) + an MCP server. So Track C is **exposure + cataloguing + modularization**, not new capability:

1. **Modular rule registry (SSOT, GUI-registered).** Keep compile-time enforcement, but stop hardcoding the rule *set*: register each GUI design rule as a `GuiDesignRule` domain entry in the **existing** `contracts/policy/policy-registry.v1.yaml` (reusing its schema, `vox ci policy-registry` parity gate, loader, and GUI `policy.rs` surface). The `web_ir::validate_*` passes read enabled/severity/thresholds from the registry (e.g. the 4.5:1 contrast constant → a registry param) instead of hardcoding. Result: add/subtract rules = registry entry + executor; GUI shows them automatically; drift-gated; one SSOT. (Research [§6b](ai-ui-generators-and-vox-as-target-research-2026-06-18.md).)
2. **Component registry** — `contracts/gui/component-registry.v1.json` (shadcn-registry-compatible shape: components, props/variants, a11y constraints), kept in sync with `web_ir::primitives::resolve` by a parity test.
3. **Typed token catalog + DTCG interop** — generate TS discriminated unions from the token SSOT (constrains generators/pickers to valid tokens); add **W3C DTCG** import/export adapters so Vox tokens round-trip with v0/shadcn/Figma/Style Dictionary/Tokens Studio.
4. **MCP tools** on the existing server — four tools forming the complete read-rules → emit → validate loop: `vox_gui_components` (component registry with props/variants/a11y), `vox_gui_tokens` (token catalog in W3C DTCG format), **`vox_gui_rules`** (lists the registered `gui-design-rule/*` policy entries so a generator reads constraints *before* emitting), and **`vox_validate_vuv`** (runs the lex→parse→HIR→web-IR→validate pipeline on submitted VUV and returns rule-linked diagnostics with `rule_id` pointing back to the `gui-design-rule/*` that was violated). Gives external generators shadcn-MCP-style no-hallucinated-props access **plus** a hard correctness check Vox alone can offer, with rule-linked actionable feedback closing the loop.
5. **GUI design-system panel** — reuse the `config-gui-codegen` → `GENERATED_FIELDS` → Tauri toggles/sliders pipeline + reactive `vox://design-rules-changed` so rules/thresholds are continuously registerable in the GUI.

Together these are the **"Vox Design System"**: tokens (DTCG-interop) + components (shadcn-shaped registry) + rules (`GuiDesignRule` in policy-registry), surfaced in the GUI and over MCP, with Git as the SSOT.

**Deferred (documented, not now):** EBNF + kwarg catalog (gap 4); escape-hatch (`raw_class`/`raw_css`) policy matrix + `@unsafe` gating (gap 5).

**Boundary:** advisory at the generator (fast `validate` feedback), blocking at compile (build error). Vox guarantees correctness/consistency, not taste. Plan: `../superpowers/plans/2026-06-18-track-c-vox-as-ai-ui-target.md`.

---

## 4. How the tracks share one substrate

All three walk a **standardized structural representation** and run generic code over it (research §4):
- Track A walks the **type/HIR graph** → widgets.
- Track B walks the **execution graph** → events.
- Track C exposes the **VUV component/token graph + validators** over MCP so external generators emit into the same structure.

Concretely Tracks A and B meet twice: (1) Track B's GUI timeline is itself produced by Track A's auto-GUI machinery; (2) both terminate in the **same dual-audience envelope** (`VoxCompilerDiagnosticPayload` for LLM; `vox-cli-core/diagnostics` for human). Building them together means one reflective core, one output multiplexer — not two features.

---

## 5. Proposed crate / file touch-map (for a future plan)

| Track | New | Reuse / extend |
|---|---|---|
| A | `vox-codegen-ts/src/admin/` (admin codegen), `contracts/gui/admin-registry.yaml` | `form_emit.rs`, `type_maps.rs`, scalar registry, `dynamic_mapping.rs` |
| B | `vox-execution-tracer` (L2), new `METRIC_TYPE_EXECUTION_EVENT` | `vox-compiler::eval::*` (injection), `vox-telemetry`, `VoxCompilerDiagnosticPayload` (+`execution_context`), `vox-cli-core/diagnostics.rs` |
| C | `contracts/gui/component-registry.v1.json`, token TS export, `vox-orchestrator-mcp/src/gui_registry_tools.rs` | `web_ir::primitives::resolve`, `contracts/tokens/tokens.v1.json`, `web_ir::validate_{palette,layer,a11y}`, existing MCP `register()` |
| all | — | `where-things-live.md` rows; `layers.toml` (L2 placement, fan-in) |

## 5b. Implementation constraints (Rust ecosystem) — applies to ALL tracks

Hardening rules distilled from a review pass; every plan references these (its Rule 10).

1. **`Span` in test fixtures:** construct via `vox_compiler::ast::span::Span::new(0, 0)` — the re-exported path used throughout `vox-codegen-ts`/`vox-codegen`. Do **not** write `vox_ast::span::Span` unless the crate directly depends on `vox-ast` (most codegen crates reach it only via `vox_compiler`).
2. **Additive struct fields are breaking changes.** Adding `Interpreter.tracer`, `VoxCompilerDiagnosticPayload.execution_context`, or new `PolicyEntry` fields breaks every struct-literal site. Use `Option<T>` defaulting to `None`; let `cargo build` enumerate the sites and fix each; for serialized structs add `#[serde(default, skip_serializing_if = "Option::is_none")]`.
3. **No `.unwrap()`/`.expect()` in library or MCP-handler code.** Handlers and codegen return `Result`/`Option` and surface errors as diagnostics; reserve `unwrap` for tests.
4. **Deterministic output.** Codegen/registry/token emission must be order-stable so golden tests don't flake. `serde_json::Map` is a `BTreeMap` when `preserve_order` is off (current workspace state) → iteration is sorted; still sort explicitly when building lists from `HashMap`/`HashSet`.
5. **No allocation in interpreter hot paths.** The step hook must not `format!`/allocate per step. Record cheaply, gate on `tracer.is_some()`, and **cap event volume with a budget** (Log2 principle) so memory is bounded on long runs.
6. **Path-embedding brittleness.** Prefer a runtime read of a workspace-relative contract path (resolved once via `CARGO_MANIFEST_DIR`) over deep `include_str!("../../../…")` chains; if `include_str!` is used, verify the `../` depth from the *source file's* location.
7. **Layering/cycles.** New crates/deps must pass `cargo run -p vox-arch-check` (no cycles; respect L-tiers and fan-in budgets). A new dep on a central crate (e.g. `vox-compiler`) must be one-directional.
8. **Env/global state in tests is forbidden.** No `std::env::set_var` (flaky, parallel-unsafe, `unsafe` in 2024 edition); inject config/flags as function parameters and test pure helpers.

Layer/arch note: `vox-execution-tracer` is L2 consuming `vox-telemetry` (L1) and instrumenting `vox-compiler` (L2) — verify fan-in/cycle rules with `cargo run -p vox-arch-check` before any code.

---

## 6. Phasing (suggested, not committed)

1. **A-1:** richer typed field inference (enum + branded scalars). ~1 wk. Pure codegen, low risk, immediately useful even without `@admin`.
2. **A-2:** `@admin` + registry + admin codegen (list/detail/edit/CRUD). ~2–3 wks.
3. **B-1:** `vox-execution-tracer` capture + `vox run --trace` human CLI timeline. ~2 wks.
4. **B-2:** rules-based severity inference + Notice+ default filtering. ~1 wk.
5. **B-3:** `execution_context` in the LLM envelope + GUI timeline surface. ~2 wks.
6. **(B-4, deferred):** swap rules for a DeepLV-style learned severity model. Research-frontier; only after B-2 proves the interface.

---

## 7. Explicit non-goals / risks

- **Not** consumer-grade auto-UI (intent/affordance gap, research §2.3).
- **Not** always-on tracing (over-logging risk, research §3.2) — opt-in + selective by default.
- **Not** a full record/replay/time-travel engine — we exploit interpreter determinism instead, but do not claim Pernosco parity.
- **Topic-B feasibility is RESOLVED** (2026-06-18) — capture core confirmed (Pernosco/rr), selectivity (Log2) and severity (DeepLV) prior art verified; see [auto-debugging research](auto-debugging-zero-annotation-research-2026-06-18.md). Only the dual-audience-single-stream benefit remains A/B-gated ([error-surfacing §4](error-surfacing-dual-audience-research-2026-06-18.md)).
- ML severity (B-4) is the only research-frontier item; everything else is prior-art-proven or already half-built in Vox.

---

## 8. Open questions to resolve before a plan

1. Re-verify Topic-B prior art (rate-limited this run).
2. Minimal hint set for `@admin` — what's the smallest typed knob set that closes the affordance gap for the target envelope?
3. Does runtime `execution_context` measurably improve LLM repair vs. static source alone? (Design an A/B before B-3.)
4. Overhead budget for `--trace` on the interpreter — acceptable ceiling before it changes program behavior/timing.
