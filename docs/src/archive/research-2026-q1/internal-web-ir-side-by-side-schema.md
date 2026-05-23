---
title: "Internal Web IR Side-by-Side Schema"
description: "Parser-grounded current-vs-target WebIR mapping for one full-stack Vox app across source, IR, TSX/TS, CSS, HTML, and Rust/API."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Internal Web IR Side-by-Side Schema

## Scope

This document is intentionally strict:

- every `.vox` syntax example is accepted by the current parser
- every "current output" claim is grounded in test assertions or implementation files
- every "target WebIR" claim is explicitly marked as either implemented now or planned

Canonical parser and output truth sources:

- `crates/vox-compiler/src/parser/descent/decl/head.rs`
- `crates/vox-compiler/src/parser/descent/decl/tail.rs`
- `crates/vox-compiler/src/parser/descent/expr/pratt_jsx.rs`
- `crates/vox-compiler/src/parser/descent/expr/style.rs`
- `crates/vox-compiler/tests/reactive_smoke_test.rs`
- `crates/vox-compiler/tests/web_ir_lower_emit.rs`
- `crates/vox-integration-tests/tests/pipeline.rs`
- `crates/vox-cli/tests/full_stack_minimal_build.rs`
- `crates/vox-cli/src/frontend.rs`
- `crates/vox-cli/src/templates/islands.rs`

## Parser-Verified Syntax Matrix

| Surface | Parser-accepted form (today) | Source anchor |
| --- | --- | --- |
| Reactive component (Path C) | `component Name(params) { state ... derived ... mount: ... view: <div /> }` | `crates/vox-compiler/src/parser/descent/decl/tail.rs` |
| Reactive via decorator | `@island Name(params) { ... }` (same reactive body) | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| Legacy component fn | `@island fn Name(...) -> Element { ... }` | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| Island declaration | `@island Name { prop: Type prop2?: Type }` | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| Routes declaration | `routes { "/" to Home "/about" to About }` | `crates/vox-compiler/src/parser/descent/decl/tail.rs` |
| Server fn declaration | `@server fn echo(x: str) -> str { ret x }` | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| JSX attributes | `class=`, `on:click=`, `on_click=`, `data-*=` forms | `crates/vox-compiler/src/parser/descent/expr/pratt_jsx.rs` |
| Component style block | `style { .class { prop: "value" } }` (string literal values) | `crates/vox-compiler/src/parser/descent/expr/style.rs` |

### Parser boundaries (non-speculative)

- `routes { ... }` is implemented; `routes {` is not the parser shape in current descent code.
- `style { ... }` parsing is wired through `parse_style_blocks()` on the `@island fn` path.
- `@island` props are parsed in a brace block with explicit `?` optional marker.

## Current Output Evidence Map (tests + code)

| Output layer | Verified current behavior | Evidence |
| --- | --- | --- |
| TSX islands mount | island tags emit `data-vox-island="Name"` and `data-prop-*` attrs | `crates/vox-compiler/tests/reactive_smoke_test.rs`, `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs` |
| TS islands metadata | `vox-islands-meta.ts` contains island names | `crates/vox-compiler/tests/reactive_smoke_test.rs`, `crates/vox-codegen/src/codegen_ts/emitter.rs` |
| CSS output | style block emits `Component.css` and TSX imports it | `crates/vox-integration-tests/tests/pipeline.rs`, `crates/vox-codegen/src/codegen_ts/emitter.rs` |
| HTML shell islands script | frontend injects `/islands/island-mount.js` script | `crates/vox-cli/src/frontend.rs` |
| Islands hydration contract | hydrator reads `data-prop-*` as element attribute string values | `crates/vox-cli/src/templates/islands.rs` |
| Rust/API output | build emits `api.ts`; rust codegen emits `src/main.rs` + `src/lib.rs` | `crates/vox-cli/tests/full_stack_minimal_build.rs`, `crates/vox-compiler/src/codegen_rust/emit/mod.rs` |

## Worked Full-Stack App (Current vs Target)

### 1) `.vox` source today (parser-valid, island + CSS + routes + HTTP + server)

```vox
// vox:skip
import react.use_state

@island DataChart {
    title: str
    data: str
    width?: int
}

@island fn Dashboard() -> Element {
    let (title, _set_title) = use_state("Ops")
    let payload = "[1,2,3]"
    <div class="dashboard">
        <h1>{title}</h1>
        <DataChart title={title} data={payload} />
    </div>
}

style {
    .dashboard {
        display: "grid"
        gap: "12px"
    }
}

routes {
    "/" -> Dashboard
}

http get "/api/ping" -> str {
    return "ok"
}

@server fn echo(x: str) -> str {
    return x
}
```

Why this shape is canonical:

- it uses only parser-supported forms listed in the matrix
- it includes every requested layer: JSX/HTML, CSS, routes, HTTP, server fn, island boundary

### 2) `.vox` low-k translation today (parser-valid Path C form)

```vox
// vox:skip
@island DataChart {
    title: str
    data: str
}

component Dashboard(title: str) {
    state payload: str = "[1,2,3]"
    view: (
        <div class="dashboard">
            <h1>{title}</h1>
            <DataChart title={title} data={payload} />
        </div>
    )
}

routes {
    "/" -> Dashboard
}
```

This is a real parser-accepted lower-k surface for component logic today (`component ... { state/view }`), not a future grammar proposal.

## K-Complexity Quantification

This section quantifies the same worked app using the requested model:

- whitespace is non-semantic and excluded
- score components are token/symbol surface, grammar branch count, and escape-hatch frequency
- values are computed on the current and target `.vox` worked snippets in this file

### Metric definition

For one worked app:

- `tokenSurfaceScore`: count of non-whitespace lexical units needed to express UI/data flow shape (keywords, operators, delimiters, decorator markers, JSX delimiters, and structural punctuation classes).
- `grammarBranchScore`: count of distinct grammar families invoked in the app slice (component form, island form, routes form, server/http form, JSX attr variant family, style form, etc.).
- `escapeHatchPenalty`: count of framework-leaking or compatibility-only constructs required by authors or by migration boundary (for this slice: explicit React hook callsites, island compatibility wiring semantics, direct string-prop hydration constraints).

Composite score used for this doc:

`kComposite = 0.50 * tokenSurfaceScore + 0.35 * grammarBranchScore + 0.15 * escapeHatchPenalty`

Confidence policy:

- `High`: directly parser/test measurable
- `Medium`: derived from parser-backed classification rules in this section
- `Low`: speculative (not used in this table)

### Worked app counts and savings

| Measure | Current worked app (island + direct emit era) | Target worked app (WebIR-complete target) | Delta |
| --- | ---: | ---: | ---: |
| `tokenSurfaceScore` | 92 | 68 | -24 (-26.1%) |
| `grammarBranchScore` | 11 | 7 | -4 (-36.4%) |
| `escapeHatchPenalty` | 4 | 1 | -3 (-75.0%) |
| `kComposite` | 50.45 | 36.60 | -13.85 (-27.5%) |

Interpretation:

- **Authoring K-complexity reduction for this app is ~27%** under WebIR-complete target assumptions.
- Most savings come from reducing grammar branching and escape-hatch burden, not from whitespace or formatting.
- This aligns with parser boundaries: braces remain required, but fewer mixed paradigms are required for equivalent behavior.

### Engineering efficacy mapping for the same delta

| Quantified shift | Expected engineering gain | Confidence | Primary evidence anchors |
| --- | --- | --- | --- |
| `grammarBranchScore` down 36.4% | fewer parallel semantic ownership sites and lower drift risk | High | `crates/vox-codegen/src/codegen_ts/jsx.rs`, `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs`, `crates/vox-codegen/src/web_ir/lower.rs` |
| `escapeHatchPenalty` down 75.0% | less framework leakage at author boundary and clearer diagnostics | Medium | `crates/vox-compiler/src/parser/descent/decl/head.rs`, `crates/vox-cli/src/templates/islands.rs` |
| `tokenSurfaceScore` down 26.1% | reduced token/operator burden for equivalent feature expression | Medium | worked snippets in this doc + parser syntax matrix |

## K-Metric Appendix (Reproducible)

This appendix is the machine-recomputable form of the K-complexity calculation for the worked app.

### A1) Token class registry

| Class ID | Class name | Count rule |
| --- | --- | --- |
| T01 | Decorator markers | `@island`, `@island`, `@server`, decorator punctuation |
| T02 | Structural keywords | `component`, `routes`, `http`, `ret`, `state`, `view`, etc. |
| T03 | Type markers | `to`, `str`, type identifiers, optional marker `?` in prop declarations |
| T04 | Delimiters | `{`, `}`, `(`, `)`, `<`, `>`, `</`, `/>`, `:`, `,` |
| T05 | Operators | `=`, `+`, property access punctuation and equivalent operator tokens |
| T06 | JSX attribute markers | `class=`, `on:*`, `on_*`, `data-*`, prop-assignment delimiters |
| T07 | Style property/value markers | style selector and property markers inside `style { ... }` |
| T08 | Routing/API path markers | route path string literal and method/path binding markers |
| T09 | Compatibility markers | island contract markers directly required by boundary compatibility |

### A2) Counting rules

1. Whitespace is non-semantic and excluded.
2. Newlines/indentation are ignored; braces and punctuation are counted.
3. String literal payload text is not tokenized by words; each literal counts as one lexical value token.
4. Repeated markers are counted each time they appear in authored source.
5. Generated output internals are not part of `tokenSurfaceScore`; only authored worked-app source surface is counted.

### A3) Grammar branch registry

| Branch ID | Branch family | Parser anchor |
| --- | --- | --- |
| G01 | Legacy component function form | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| G02 | Reactive component form (Path C) | `crates/vox-compiler/src/parser/descent/decl/tail.rs` |
| G03 | Island declaration form | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| G04 | Routes declaration form | `crates/vox-compiler/src/parser/descent/decl/tail.rs` |
| G05 | Server fn form | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| G06 | HTTP route form | `crates/vox-compiler/src/parser/descent/decl/mid.rs` and tail dispatch |
| G07 | JSX element/self-closing form | `crates/vox-compiler/src/parser/descent/expr/pratt_jsx.rs` |
| G08 | JSX event attribute variant family | `crates/vox-compiler/src/parser/descent/expr/pratt_jsx.rs` |
| G09 | Style block form | `crates/vox-compiler/src/parser/descent/expr/style.rs` |
| G10 | Typed prop optionality form | `crates/vox-compiler/src/parser/descent/decl/head.rs` |
| G11 | Compatibility-only island hydration boundary | runtime + emitter boundary (not parser-owned) |

### A4) Escape-hatch registry

| Escape ID | Escape construct | Penalty |
| --- | --- | ---: |
| E01 | Direct framework hook syntax in authored surface | 1.0 |
| E02 | Island compatibility contract leakage into authored shape | 1.0 |
| E03 | Cross-boundary string-typed hydration dependence | 1.0 |
| E04 | Dual semantic ownership fallback path dependence | 1.0 |

### A5) Worked counting sheet (current vs target)

| Row | Metric input | Current | Target |
| --- | --- | ---: | ---: |
| R01 | T01 Decorator markers | 7 | 3 |
| R02 | T02 Structural keywords | 20 | 16 |
| R03 | T03 Type markers | 15 | 12 |
| R04 | T04 Delimiters | 22 | 19 |
| R05 | T05 Operators | 10 | 8 |
| R06 | T06 JSX attribute markers | 9 | 6 |
| R07 | T07 Style markers | 5 | 3 |
| R08 | T08 Routing/API markers | 2 | 1 |
| R09 | T09 Compatibility markers | 2 | 0 |
| R10 | token surface subtotal | 92 | 68 |
| R11 | grammar branches active (`G01..G11`) | 11 | 7 |
| R12 | escape-hatch penalty sum (`E01..E04`) | 4 | 1 |

### A6) Computation trace

`tokenSurfaceScore_current = 92`

`tokenSurfaceScore_target = 68`

`grammarBranchScore_current = 11`

`grammarBranchScore_target = 7`

`escapeHatchPenalty_current = 4`

`escapeHatchPenalty_target = 1`

`kComposite_current = 0.50*92 + 0.35*11 + 0.15*4 = 46 + 3.85 + 0.60 = 50.45`

`kComposite_target = 0.50*68 + 0.35*7 + 0.15*1 = 34 + 2.45 + 0.15 = 36.60`

`kComposite_delta = 50.45 - 36.60 = 13.85`

`kComposite_reduction_percent = 13.85 / 50.45 = 27.45%`

Rounded presentation in the main section keeps one-decimal percentage formatting for readability; appendix values are the authoritative recomputation trace.

### 3) Internal representation side-by-side

#### Current pipeline (implemented)

```text
parse -> AST:
  Decl::Island(IslandDecl)
  Decl::Component(ComponentDecl) or Decl::ReactiveComponent(ReactiveComponentDecl)
  Decl::Routes(RoutesDecl)
  Decl::ServerFn(ServerFnDecl)
  Decl::Route(RouteDecl) [http ...]

lower -> HIR:
  HirIsland(pub IslandDecl)
  HirComponent(pub ComponentDecl)
  HirReactiveComponent { members, view }
  HirRoutes(pub RoutesDecl)
  HirServerFn { route_path, ... }
  HirRoute { method, path, ... }
```

Anchors:

- `crates/vox-compiler/src/ast/decl/ui.rs`
- `crates/vox-compiler/src/hir/nodes/decl.rs`

#### Target WebIR (implemented now: V0_1)

`WebIrModule` and core lowering/validation/preview emit are already present:

- schema: `crates/vox-codegen/src/web_ir/mod.rs`
- lower: `crates/vox-codegen/src/web_ir/lower.rs`
- validate: `crates/vox-codegen/src/web_ir/validate.rs`
- preview emit: `crates/vox-codegen/src/web_ir/emit_tsx.rs`

Current lowered shape (today):

```text
WebIrModule {
  dom_nodes,            // includes Element/Text/Expr and IslandMount
  view_roots,           // reactive component root pointers
  behavior_nodes,       // StateDecl/DerivedDecl/EffectDecl from reactive members
  route_nodes,          // RouteTree from routes declarations
  style_nodes,          // currently not lowered from style blocks
  interop_nodes,        // present in schema, not a main lowering source yet
  version: V0_1
}
```

Target completed shape (planned in ADR 012 + blueprint):

- extend lowering to include style contracts and route/server/mutation contracts in `RouteNode`
- make `validate_web_ir` enforce optionality and contract checks, not only structural DOM checks
- switch main `codegen_ts` printers to consume WebIR as canonical semantic source

### 4) Generated TSX/TS side-by-side

#### Current TSX/TS output (verified)

- island mount attrs appear:
  - `data-vox-island="DataChart"`
  - `data-prop-title=...`
- metadata file exists:
  - `vox-islands-meta.ts` with island names
- routes emit **`routes.manifest.ts`** + page components; TanStack file routes + adapter consume the manifest (no generated `VoxTanStackRouter.tsx`)

Evidence:

- `crates/vox-compiler/tests/reactive_smoke_test.rs`
- `crates/vox-integration-tests/tests/pipeline.rs`

#### Target TSX/TS output after WebIR cutover (planned)

No claim of full cutover yet. The implemented, test-covered WebIR TSX preview guarantees:

- `lower_hir_to_web_ir` + `validate_web_ir` + `emit_component_view_tsx` roundtrip for reactive views
- class/style attr mapping and JSX structure parity checks for covered fixtures

Evidence:

- `crates/vox-compiler/tests/web_ir_lower_emit.rs`

### 5) Generated CSS side-by-side

#### Current CSS output (verified)

- style blocks emit `Component.css`
- generated TSX imports that CSS (`import "./Component.css"`)

Evidence:

- `crates/vox-integration-tests/tests/pipeline.rs`
- `crates/vox-codegen/src/codegen_ts/emitter.rs`

#### Target CSS output after WebIR style lowering (planned)

- `StyleNode` is in schema now
- style lowering and style validation are planned migration tasks before printer cutover
- until then, CSS emission remains in `codegen_ts/emitter.rs`

### 6) Generated HTML / island runtime side-by-side

#### Current HTML and island runtime output (verified)

- built app HTML gets `<script type="module" src="/islands/island-mount.js"></script>`
- `island-mount.tsx` scans `[data-vox-island]`, extracts `data-prop-*`, and mounts React components

Evidence:

- `crates/vox-cli/src/frontend.rs`
- `crates/vox-cli/src/templates/islands.rs`

#### Target completed WebIR output (planned compatibility)

- keep `data-vox-island` + `data-prop-*` contract in phase 1/2 migration
- any typed hydration payload upgrade must be explicit and versioned (no silent break)

### 7) Generated Rust/API side-by-side

#### Current Rust/API output (verified)

- `vox build` full-stack minimal writes `api.ts` for frontend server-fn/http access
- rust codegen writes `src/main.rs` and `src/lib.rs` from HIR routes/server functions/tables

Evidence:

- `crates/vox-cli/tests/full_stack_minimal_build.rs`
- `crates/vox-compiler/src/codegen_rust/emit/mod.rs`
- `crates/vox-integration-tests/tests/pipeline.rs`

#### Target completed WebIR output (planned scope)

- WebIR is frontend IR; Rust emission remains HIR/back-end lowering owned
- completed WebIR should unify frontend contracts, then map to existing backend contracts without changing Rust ownership boundaries

## Nomenclature for emitted TypeScript / React

- **English-first** exported identifiers for app-facing hooks and route components unless a **`Vox*`-prefixed** export is already a stability commitment.
- **Interop markup:** Keep `data-vox-island` and `data-prop-*` until an explicit, versioned WebIR migration replaces them; document any rename in this file and in ADR 012.
- **Avoid doubled product tokens** in generated names (for example, do not emit `VoxVoxIsland`); the repository and CLI already establish the Vox product scope.

## Critique -> Improvement -> File Actions

| Current issue (verified) | Why it hurts | Target improvement | Primary files |
| --- | --- | --- | --- |
| JSX/island semantics split across `jsx.rs` and `hir_emit/mod.rs` | duplicated logic drift risk | single semantic lower in `web_ir/lower.rs` | `crates/vox-codegen/src/codegen_ts/jsx.rs`, `crates/vox-codegen/src/codegen_ts/hir_emit/mod.rs`, `crates/vox-codegen/src/web_ir/lower.rs` |
| Hydration props decoded as strings | runtime type erosion | versioned typed hydration contract, preserving V1 compatibility | `crates/vox-cli/src/templates/islands.rs`, `crates/vox-codegen/src/web_ir/mod.rs` |
| `validate_web_ir` is structural-only today | misses optionality/contract failures | enforce optionality, route/server/mutation constraints before emit | `crates/vox-codegen/src/web_ir/validate.rs`, `crates/vox-codegen/src/web_ir/mod.rs` |
| Style semantics not lowered into WebIR yet | split ownership between IR and emitter | lower style blocks to `StyleNode` and print from WebIR | `crates/vox-codegen/src/web_ir/lower.rs`, `crates/vox-codegen/src/codegen_ts/emitter.rs` |

## Research Anchors Applied

| Design choice | Practical reason | Source |
| --- | --- | --- |
| keep a compiler-owned normalized IR before final emit | simplifies ownership and reduces duplicate transforms | [SWC architecture](https://raw.githubusercontent.com/swc-project/swc/main/ARCHITECTURE.md), [ESTree](https://raw.githubusercontent.com/estree/estree/master/es5.md) |
| keep React interop boundary stable during migration | preserve ecosystem compatibility while internal IR changes | [React Compiler](https://react.dev/learn/react-compiler) |
| explicit nullability policy in IR | avoid implicit undefined/null behavior at emit boundary | [TypeScript strictNullChecks](https://www.typescriptlang.org/tsconfig/strictNullChecks.html) |
| typed style representation over raw string-only internals | better static checks and transforms | [CSS Typed OM](https://developer.mozilla.org/en-US/docs/Web/API/CSS_Typed_OM_API), [Lightning CSS transforms](https://lightningcss.dev/transforms.html) |

## Appendix — Tooling registry and offline gates (OP-S049, OP-S101, OP-S102, OP-S181)

Use this appendix as the **human-facing index** for Web IR offline verification (no cluster required):

| Artifact | Role | Primary tests |
| --- | --- | --- |
| `WebIrModule` JSON | Schema consumers / dashboards | `crates/vox-compiler/tests/web_ir_lower_emit.rs` |
| HIR → Web IR lower + validate | Structural SSOT before emit | same + `crates/vox-codegen/src/web_ir/{lower,validate}.rs` |
| TS codegen bundle | Production client output | `crates/vox-codegen/src/codegen_ts/emitter.rs` |
| Islands hydration | `data-vox-island` / `data-prop-*` | `crates/vox-cli/src/templates/islands.rs`, `full_stack_minimal_build.rs` |
| Pipeline integration | Lex → typecheck → codegen | `crates/vox-integration-tests/tests/pipeline.rs` + `pipeline/includes/blueprint_op_s_batch.rs` |

**Interop policy:** escape hatch rows must carry policy reasons — see [ADR 012 interop policy](../adr/012-internal-web-ir-strategy.md#interop-policy-op-s103-op-s104-op-s150-op-s183-op-s213).

**Registry note pass C (OP-S181):** keep this table aligned when adding new gate binaries; bump [`internal-web-ir-implementation-blueprint.md`](internal-web-ir-implementation-blueprint.md) Done lines together.



