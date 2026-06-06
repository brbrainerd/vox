---
title: "Phase 5 Sub-Spec: Native React / React Native Component Interop (2026)"
description: "Code-anchored sub-spec for importing external React and React Native components into Vox source as ordinary TS/JS imports. Decides the import-form grammar, the opaque-extern type strategy with an opt-in flat prop-facade, JSX-tag registration, dependency/provider/styling wiring, shadcn/ui as a vendor codegen mode, and the one-IR React Native mapping. Includes a full file-and-line implementation plan."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Canonical sub-spec the five-phase interop plan defers: exact import syntax, type-bridge mechanism, JSX-tag registration, ecosystem coupling, and the React Native one-IR mapping, all anchored to real code."
---

# Phase 5 Sub-Spec: Native React / React Native Component Interop (2026)

> **Parent:** [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md) §Phase 5, which explicitly defers "the exact syntax for `import react …` and the type-bridge mechanism" and "the escape-hatch mechanism for user edits" to *this* sub-spec.
> **Companion:** [Vox–React backend interop audit (2026)](vox-react-backend-interop-audit-2026.md) (the backend/API half) and [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md) (the RN target).
> **Phase numbering:** this is the **frontend interop** sequence; see [phase-numbering-index](phase-numbering-index.md).

## 0. The reframing insight

Vox **emits TypeScript**. A `.tsx` file importing a React component is, at runtime, *only* a pass-through ES import plus a `_jsx(Component, props, key)` call — **no FFI, no marshaling, no bridge** (verified against `@types/react@19.2.0`: `interface FunctionComponent<P> { (props: P): ReactNode | Promise<ReactNode> }`; `interface ReactElement { type; props; key }`; props is a plain in-process JS object). Vox already emits `import MyButton from "./MyButton.tsx"` today ([`reactive.rs:947-964`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs)).

Therefore the runtime half of "import like any TS/JS document" is **mostly already done**. The unsolved work is not runtime. It is: (1) Vox's checker cannot see into `node_modules/*.d.ts`; (2) the language surface does not yet *let you use* an imported name as a JSX tag or richer import form; (3) ecosystem coupling (peer-dep dedupe, runtime CSS-in-JS engines, mandatory providers, the RSC `'use client'` boundary, Tailwind); (4) React Native is a separate compile target with a hard native-module boundary.

The spec is organised as **tiers of fidelity** so cost is assessed honestly — separating "match plain-JS import behaviour" (cheap, near-done) from "Vox understands the foreign type" (expensive, and partially impossible to do faithfully).

## 1. Decisions baked into this sub-spec

1. **Type strategy = Option C (opaque extern) as the default, plus a scoped flat prop-facade as an opt-in.** Imported components are an opaque external type; props pass through untyped from Vox's perspective and are validated by the consumer's `tsc` against the genuine `.d.ts` — exactly how plain JS behaves. The opt-in middle path extracts only a **flat prop facade** (`{ propName: TsTypeString }`) admitted as an **`extern` type whose fields are opaque TS-type strings**. We do **not** model generics, open unions (`OverridableStringUnion`), conditional/mapped types, render-prop function children, or polymorphic `as`. Those remain opaque (documented limitation §15).
2. **shadcn/ui is a vendor codegen mode, not an import.** shadcn is a CLI/registry that copies `.tsx` source into your repo; it is not resolvable as a dependency. Vox supports it via a scaffold step that writes the source in, then treats the result as a *local* import (§10).
3. **React Native shares the one IR.** There is one `HirModule`; the web and RN targets are two lowerings of it ([`rn/mod.rs:1-35`](../../../crates/vox-codegen/src/codegen_ts/rn/mod.rs)). We map as much as possible to RN export and make external-component import work on the RN target too, with an explicit hard boundary at native modules (§11).
4. **New import forms extend the `import` parser, not a decorator.** Per [AGENTS.md §Grammar Unification](../../../AGENTS.md) ("bare-keyword blocks declare scope; decorators modify declarations"), an import form is neither — it extends the existing four-way `import` grammar.
5. **Vox never bundles its own React.** `react`/`react-dom` stay the single app-owned copy; duplication causes the verified "Invalid hook call" failure.

## 2. Ground truth — what exists today (verified `file:line`)

| Capability | State | Evidence |
|---|---|---|
| `import react MyButton from "../ui/MyButton.tsx"` parses | ✅ default-only | [`parser/descent/decl/head.rs:95-150`](../../../crates/vox-compiler/src/parser/descent/decl/head.rs); AST `ImportPathKind::ReactComponent { local_name, module_specifier }` ([`ast/decl/types.rs:33-55`](../../../crates/vox-ast/src/decl/types.rs)) |
| Destructured + slash/dot symbol imports | ✅ | `import lib/chrome as { A, B }` and `import lib.chrome.X` both pass ([`parser_import_syntax_test.rs`](../../../crates/vox-compiler/tests/parser_import_syntax_test.rs)) |
| Lowers to HIR | ✅ | `HirImport.es_module_specifier: Option<String>` ([`hir/nodes/decl.rs:227-249`](../../../crates/vox-compiler/src/hir/nodes/decl.rs)); [`hir/lower/mod.rs:139-151`](../../../crates/vox-compiler/src/hir/lower/mod.rs) |
| Web target emits the ES import | ✅ default-only | [`reactive.rs:947-964`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs) → `format!("import {item} from \"{spec}\";")` |
| **RN target emits the ES import** | ❌ **missing** | [`rn/component.rs:1221-1277`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs) has no Phase-5 block; PascalCase tags become `RnNode::CustomComponent` with **no import** ([`rn/component.rs:469-492`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs)) |
| JSX imported name usable as a tag `<MyButton/>` | ❌ **not registered** | web: [`reactive.rs:808`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs) requires `uppercase && known_components.contains(tag)`; `known_components` = `hir.components` + forms, not imports |
| JSX prop typechecking | ⚠️ **already permissive** | [`typeck/checker/expr.rs:716-730`](../../../crates/vox-compiler/src/typeck/checker/expr.rs) checks attribute *values* only, returns `Ty::Element` — no prop-shape validation |
| HIR opaque/extern type | ❌ none | `HirType` = `Named \| Generic \| Function \| Tuple \| Unit \| Decimal` ([`hir/nodes/stmt_expr.rs:90-103`](../../../crates/vox-compiler/src/hir/nodes/stmt_expr.rs)) |
| `map_hir_type_to_ts` catch-all | `_ => "any"` | [`lowering_shared/jsx.rs:24-39`](../../../crates/vox-compiler/src/lowering_shared/jsx.rs) |
| RN **component emitter** (one IR) | ✅ exists | [`rn/mod.rs:57`](../../../crates/vox-codegen/src/codegen_ts/rn/mod.rs) `generate_rn`; intrinsic→RN map [`rn/component.rs:339-511`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs) |
| Per-dependency / provider / peer registry | ❌ none | no contract YAML maps npm packages → peers/providers/styling |
| Web scaffold | React `^19`, react-dom `^19`, `tsconfig jsx:"react-jsx"`, `moduleResolution:"Bundler"`, `module:"ESNext"`; **no `resolve.dedupe`** | [`scaffold.rs:45-118`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs) |
| RN scaffold | expo `^52`, **react `18.3.1`**, react-native `0.76.9`, safe-area-context `4.12.0`, zod, `@vox/runtime-rn` | [`rn/scaffold.rs:154-197`](../../../crates/vox-codegen/src/codegen_ts/rn/scaffold.rs) |
| `@island` | ✅ fully removed | 0 hits in compiler/codegen |

> **Divergence flagged:** the web scaffold pins React `^19`; the RN scaffold pins React `18.3.1`. The per-library peer SSOT (§9.1) must reconcile per target.

## 3. The React / JSX contract Vox must satisfy (primary-source verified)

Verified against `react@19.2.0`, `@types/react@19.2.0`, the TypeScript handbook, and react.dev:

- **Automatic JSX runtime** (what `jsx:"react-jsx"` produces, Vox's current tsconfig): emit `import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime"`, then `_jsx(type, props, key?)` (0–1 child) or `_jsxs(type, props, key?)` (static array). `react/jsx-runtime` resolves via the package's `exports` map; no `import React` needed.
- **`children` is a field on the props object. `key` is the *3rd positional* arg of `_jsx`, never inside props** (the automatic runtime moved it out of props).
- **React 19: `ref` is a plain prop** for function components — `forwardRef` optional, but every library still ships `forwardRef` components, so Vox must *accept* `ref` as a pass-through prop.
- Types Vox references when typing imports: `React.ComponentProps<typeof X>` (extract foreign props), `React.ComponentType<P>` / `React.FC<P>` (annotate an import), `ReactNode` (view return).

Vox already targets the automatic runtime via `react_import_line` + the `use_*`→React-hook map ([`react_bridge.rs`](../../../crates/vox-compiler/src/react_bridge.rs)). The contract is satisfiable today; what is missing is plumbing imported symbols into the JSX and (optionally) type layers.

## 4. Verified ecosystem structures (condensed)

All facts below were fetched from primary sources (unpkg `package.json`/`.d.ts`, official docs) and passed a skeptic verification pass.

| Library | Module shape | React coupling | Provider | Styling runtime | Interop friction |
|---|---|---|---|---|---|
| **Radix** `@radix-ui/react-*` | dual ESM/CJS `exports`, `sideEffects:false`, dual-name exports (`Root` + `DialogTrigger`) | react/react-dom broad peers; `@types/react` **optional** | none (per-component roots) | **none** (unstyled; `data-state` attrs; `asChild`+`Slot`) | **lowest** |
| **Headless UI** `@headlessui/react` | dual ESM/CJS `exports`, `sideEffects:false` | react/react-dom peers only | none | none (unstyled; render-prop slot bag) | **low** |
| **React Aria** hooks (`react-aria`) | per-hook subpath `exports`, `sideEffects:false`; `useButton(props,ref)=>{buttonProps,isPressed}` | react/react-dom peers only | none (hooks) | none (returns prop bags) | **lowest** |
| **react-aria-components** | wildcard subpath `exports`; `sideEffects:["*.css"]` | peers | recommends `I18nProvider`; threads `*Context` | none required | low–medium |
| **MUI** `@mui/material` | dual ESM/CJS `exports`; `'use client'` on every file | react/react-dom + `@emotion/react`+`@emotion/styled` peers | `ThemeProvider` (theming; default fallback exists) | **Emotion runtime** | medium |
| **antd** | **legacy `main`/`module`, no `exports`, `typings` not `types`** | react/react-dom `>=18` peers | `ConfigProvider` **optional** | `@ant-design/cssinjs` auto-inject (dep) | medium (resolver quirk) |
| **Chakra** `@chakra-ui/react` | `type:module`, full `exports` | `@emotion/react` peer | `ChakraProvider` **mandatory** | Emotion runtime | medium–high |
| **Mantine** `@mantine/core` | `exports` exposes `./styles.css`; React **19.2** + exact `@mantine/hooks` pin | peers | `MantineProvider` **mandatory** | **must `import '@mantine/core/styles.css'`** | high |
| **shadcn/ui** | **not an npm package** — CLI writes registry `files[].content` `.tsx` into your repo; deps `radix-ui`+`cva`+`cn=twMerge(clsx)`+Tailwind | (your source) | none | **Tailwind build scan** | vendor mode (§10) |

**React Native (verified):** RN ≠ web React — Fabric renders **native host views, not DOM**; Metro resolves platform extensions `name.android.js → name.native.js → name.js` (not replaceable by Vite/webpack); Hermes AOT-compiles to `.hbc`; **native modules (TurboModules) are compiled C++/Kotlin/Obj-C++ linked via autolinking at `pod install`/Gradle — never a runtime JS import**. JS-only components hot-load; native-module components require a native rebuild (and `expo prebuild`/CNG). RN libs carry heavier peers: Paper needs `react-native-safe-area-context` + (lazy) vector-icons + `PaperProvider`; NativeWind needs `tailwindcss` (peer) + a Babel/Metro transform + `react-native-reanimated` (peer); Tamagui needs its compiler + `TamaguiProvider` (only `react` peer).

## 5. Interop model: tiers + the 7 concerns

| # | Concern | Needed by |
|---|---|---|
| C1 | Import-form grammar (default/named/namespace/aliased/subpath/bare) | Radix dual-name, MUI default-per-subpath, React Aria per-hook |
| C2 | JSX-tag registration so `<Foo>`/`<Dialog.Root>` resolve | everything used as an element |
| C3 | Type strategy (opaque extern + opt-in flat facade) | compile-time prop awareness |
| C4 | Dependency manifest + peer-dep dedupe | all (duplicate React = invalid hook call) |
| C5 | Provider/Context injection | Chakra, Mantine (mandatory), MUI theme, RAC |
| C6 | Styling-runtime wiring (engine present / CSS import / Tailwind / `'use client'`) | MUI, Chakra, Mantine, antd, shadcn |
| C7 | RSC `'use client'` boundary | MUI + interactive components |

- **Tier 0** (`C1`+`C2`+`C4`): import and render a pure-JS component, untyped in Vox, with the downstream `tsc`/bundler as the type authority. Matches plain-JS import behaviour. Covers Radix, Headless UI, React Aria, Recharts, RHF, TanStack Table.
- **Tier 1** (`+C3` opt-in flat facade): `vox check`-time prop-name/required-prop checking via the flat facade.
- **Tier 2** (`+C5`/`C6`/`C7`): the provider-/CSS-coupled libraries (MUI, Chakra, Mantine, antd).
- **RN** (§11): orthogonal target.

`C1`+`C2`+`C4` are the mandatory spine — without them no import functions end-to-end.

## 6. Type strategy (C3) — opaque extern + opt-in flat facade

### 6.1 Why opaque is the right default, and nearly free

The JSX checker is **already permissive**: [`typeck/checker/expr.rs:716-730`](../../../crates/vox-compiler/src/typeck/checker/expr.rs) only type-checks each attribute *value* expression and returns `Ty::Element`; it performs **no** prop-shape validation. So an imported component used as a JSX tag already typechecks without error today — Option C requires **no typecheck change**. The real type authority is the consumer's `tsc`, which reads the genuine `.d.ts` (verified: `moduleResolution:"bundler"` walks `exports.types`; MUI/Radix ship full `.d.ts`). This is exactly plain-JS behaviour: a prop typo surfaces at `tsc`, not `vox check`.

### 6.2 The opt-in flat prop facade

For the top-N components where authoring ergonomics matter, add a **flat facade** without modelling TS's type system:

- Add `HirType::OpaqueExtern(String)` to the enum ([`hir/nodes/stmt_expr.rs:90-103`](../../../crates/vox-compiler/src/hir/nodes/stmt_expr.rs)) — a type carrying a **verbatim TS-type string**.
- `map_hir_type_to_ts` ([`lowering_shared/jsx.rs:24-39`](../../../crates/vox-compiler/src/lowering_shared/jsx.rs)) gains, **before** the catch-all `_ => "any"`:
  ```rust
  HirType::OpaqueExtern(ts) => ts.clone(),
  ```
- A facade is a Vox `extern type` whose fields are `OpaqueExtern` strings, e.g. produced by `vox import-types`:
  ```vox
  // vox:skip — illustrative only; the flat-facade `extern type` (S5) was PRUNED (see §15). Not real Vox syntax.
  extern type ButtonProps {            // facade for @mui/material Button
    variant: ts "Button['variant']"    // opaque TS-type string, not modeled
    color: ts "Button['color']"
    disabled: bool                     // primitives still map normally
  }
  ```
- `extern type` is a new `TypeDefDecl` flag (or a dedicated `ExternTypeDecl`); fields are `Vec<(String, HirType)>` and already accept any `HirType` ([`hir/nodes/decl.rs:370-396`](../../../crates/vox-compiler/src/hir/nodes/decl.rs)).
- Wire/Zod projection: `HirType::OpaqueExtern → WireType::Unknown` in [`contract_ir/project.rs`](../../../crates/vox-compiler/src/contract_ir/project.rs); `WireType::Unknown` already emits `z.any()` ([`zod_emit.rs:261`](../../../crates/vox-codegen/src/codegen_ts/zod_emit.rs)). So extern types do not corrupt the wire format — they degrade to `z.any()`, which is correct (they are never sent on the wire; they only type props in-process).
- Validation: when the JSX checker resolves a tag to a facade, it checks **prop names** (unknown prop → diagnostic) and **required props** (missing → diagnostic) by string set; it does **not** check value types (those are opaque, deferred to `tsc`).

### 6.3 `vox import-types` (the only place a `.d.ts` is read)

A **dev-time** step (`vox import-types <specifier>`) invokes the TypeScript compiler API (Node) to extract a flat `{ propName: TsTypeString, required: bool }` map and emits a `*.vox.extern.json` sidecar the compiler reads. We do **not** reimplement a `.d.ts` reader in Rust (reproducing TS generics/conditional/mapped types is a multi-person-year trap — do not attempt). Honest cost: this introduces a Node dev-toolchain dependency; it is a build step, not committed glue, so it does not violate the VoxScript-first policy for project automation. It is opt-in; Tier 0 (opaque) needs none of it.

## 7. Import-form grammar (C1)

Extend the existing `import` parser ([`head.rs:95-150`](../../../crates/vox-compiler/src/parser/descent/decl/head.rs)) and `ImportPathKind::ReactComponent` ([`ast/decl/types.rs:33-55`](../../../crates/vox-ast/src/decl/types.rs)) from default-only to:

```vox
// vox:skip — illustrative import-form surface; `Dialog` appears as both named and namespace to show the variants, so this is not a single compile unit.
import react Button from "@mui/material/Button"                       // default
import react { Dialog, DialogTrigger } from "@radix-ui/react-dialog"  // named
import react { useButton } from "react-aria"                          // named hook
import react * as Dialog from "@radix-ui/react-dialog"                // namespace → <Dialog.Root/>
import react { Button as MuiButton } from "@mui/material"             // aliased
```

- The `module_specifier` is preserved and emitted **verbatim**; Vox does no path rewriting — the bundler's `exports`-map resolution does the work.
- AST: extend `ReactComponent` with `kind: Default | Named(Vec<(imported, local)>) | Namespace(local)`; HIR `HirImport` gains the same; the emitter switches `format!` per kind (`import X from`, `import { a as b } from`, `import * as X from`).
- Record bare-specifier package names (e.g. `@mui/material`) for C4 manifest emission.
- This is parse-time only (no new keyword/token), tested in [`parser_import_syntax_test.rs`](../../../crates/vox-compiler/tests/parser_import_syntax_test.rs), not in the LSP language-surface SSOT.

## 8. JSX-tag registration (C2) — web + RN seams

A JSX tag must resolve in this order: lowercase intrinsic → Vox component (`hir.components`) → **imported React component** → diagnostic. Build `imported_names: HashSet<String>` from `hir.imports` (local + namespace bindings) and thread it into **both** classification seams:

- **Web (legacy + WebIR):** `collect_jsx_component_refs` ([`reactive.rs:801-819`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs)) gates on `uppercase && known.contains(tag)`. Union `imported_names` into the `known`/`known_components` set used here and in `collect_component_import_refs` ([`reactive.rs:876-912`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs)), and in the WebIR view lowering/validation ([`web_ir/lower.rs`](../../../crates/vox-codegen/src/web_ir/lower.rs), [`web_ir/validate.rs:300`/`:356`](../../../crates/vox-codegen/src/web_ir/validate.rs)). Do **not** add imported names to the sibling-`./Name` import collector (the ES import from §7 already emits them).
- **RN:** `jsx_to_rn` ([`rn/component.rs:469-492`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs)) already turns any PascalCase tag into `RnNode::CustomComponent` but **emits no import** — so the import line is missing. Add a Phase-5 import-emit loop to the RN component emitter ([`rn/component.rs:1221-1277`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs)) mirroring the web block.
- **Namespace tags:** `<Dialog.Root/>` is a dotted/member tag — the parser's JSX tag must accept dotted names and both emitters pass them through unchanged (React supports `<Foo.Bar/>`).

## 9. Dependency / provider / styling wiring (C4–C7)

### 9.1 Per-library SSOT (new contract)

No registry exists today. Create `contracts/frontend/external-component-libraries.v1.yaml` (+ JSON Schema) mapping a bare package specifier → `{ peers: {pkg: range}, provider: {component, mandatory: bool, import}, styling: {kind: emotion|cssinjs|css_file|tailwind|none, css_imports: []}, use_client: bool, targets: [web|rn] }`. Seed it from the verified §4 data. This table is curated knowledge (whether a provider is *mandatory* is documentation, not derivable). Loaded compile-time like other contract YAML (cf. [`vox-mcp-registry`](../../../crates/vox-mcp-registry/), [`vox-capability-registry`](../../../crates/vox-capability-registry/)).

### 9.2 Manifest + dedupe (C4 — mandatory, part of the spine)

On any `import react … from "<bare>"`, add `<bare>` and its **required** peers (from §9.1) to the emitted `package.json` `dependencies` ([`scaffold.rs:91-118`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs) / [`rn/scaffold.rs:154-197`](../../../crates/vox-codegen/src/codegen_ts/rn/scaffold.rs)), keep `react`/`react-dom` app-owned, and emit Vite `resolve.dedupe: ['react','react-dom']` ([`scaffold.rs:45-70`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs) currently has none). Enforce the per-target React range (web `^19` vs RN `18.3.1`) and version-pin constraints (Mantine needs React `^19.2` + exact `@mantine/hooks`) — emit a `vox check` error on conflict rather than producing an `npm install` that fails.

### 9.3 Providers (C5), styling (C6), `'use client'` (C7)

- **Providers:** a `providers` block / `@provider` on the `routes` root; the emitter wraps the React tree. Compiler emits a `vox check` **error** when an imported library's SSOT row says `provider.mandatory` and no matching provider is mounted (e.g. imported `@mantine/core` without `MantineProvider`).
- **Styling:** branch per `styling.kind` — Emotion/cssinjs (no CSS emit; ensure engine in deps), `css_file` (**emit static `import '@mantine/core/styles.css'`**, survive tree-shake), `tailwind` (extend `tailwind.config` `content` to include emitted files).
- **`'use client'`:** stamp the exact `'use client'` line as the **first line** of any emitted component that uses hooks or imports a client component.

## 10. shadcn/ui as a vendor codegen mode

shadcn is **not resolvable as a dependency** (verified: CLI fetches registry-item JSON whose `files[].content` embeds literal `.tsx`, written into your repo per `components.json` aliases; deps `radix-ui`+`cva`+`cn=twMerge(clsx)`+Tailwind). Add a Vox command (`vox add component <name>`-style) that:

1. Reads the shadcn registry item (or runs the shadcn CLI) and writes the `.tsx` source into the project under the `components.json` aliases Vox already emits ([`scaffold.rs`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs) emits `components.json`).
2. Ensures `radix-ui`, `class-variance-authority`, `clsx`, `tailwind-merge`, and the Tailwind build are in the manifest (C4), and the Tailwind `content` scan includes the vendored files (C6).
3. The vendored component is then used via an ordinary **local** `import react Button from "./components/ui/button"` (§7) — no special casing after the vendor step.

This is codegen/scaffold, not a language feature. Because shadcn rides on Radix, the **Radix Tier-0 path (S2) transitively delivers most shadcn value** even before this mode lands.

## 11. React Native — one IR, map as much as possible, hard native boundary

### 11.1 What "one IR" already buys

Confirmed: there is one `HirModule`; web and RN are two lowerings, and CI asserts both pass `tsc --noEmit` ([`rn/mod.rs:1-35`](../../../crates/vox-codegen/src/codegen_ts/rn/mod.rs)). The RN target ([`generate_rn`, `rn/mod.rs:57`](../../../crates/vox-codegen/src/codegen_ts/rn/mod.rs)) already emits per-component `.tsx`, `App.tsx`, Expo Router `app/*`, `forms.tsx`, `vox-client.ts`, `schemas.ts`, `types.ts`, `state_machines.ts`, and the Expo/Metro scaffold (`app.json`, `babel.config.js`, `metro.config.js`, `eas.json`, `package.json`). Shared HIR-expr emit goes through [`hir_emit::emit_hir_expr`](../../../crates/vox-codegen/src/codegen_ts/hir_emit/) (both targets); RN-specific JSX→primitive translation is the `RnNode` tree built by `jsx_to_rn` ([`rn/component.rs:339-511`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs)). Intrinsic mapping (verified): `div`/`column`/`stack`→`View`, `row`→`View`/`ScrollRow`, `text`/`p`→`Text`, `span`→`Text`, `button`→`Pressable`, `input`/`text_input`→`TextInput`; StyleSheet table at [`rn/component.rs:1043-1096`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs). Hooks: `useState`/`useEffect` via `emit_state_declarations`/`detect_react_hooks`/`emit_lifecycle_hooks` ([`rn/component.rs:1099-1210`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs)).

### 11.2 The RN external-import gap (the fix)

`jsx_to_rn` ([`rn/component.rs:469-492`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs)) lowers any PascalCase tag to `RnNode::CustomComponent` **with no membership check and no import emission**. So an imported RN component renders as `<CustomComponent/>` with no `import` line → broken. Fix: (a) register `imported_names` (§8) into the RN known set, and (b) add a Phase-5 import-emit loop to the RN component emitter ([`rn/component.rs:1221-1277`](../../../crates/vox-codegen/src/codegen_ts/rn/component.rs)) that emits `import X from "<spec>"` for `HirImport.es_module_specifier`, exactly like the web block. The opaque-extern type (§6) and SSOT (§9.1, `targets: [rn]`) apply unchanged.

### 11.3 The hard boundary (non-negotiable, do not hand-wave)

- **JS-only RN component libraries** (pure JS over `View`/`Text`) → importable via §11.2 into the existing RN Expo/Metro project. Cleanest first targets: **Tamagui** (only `react` peer, but needs its Babel/compiler plugin) and **React Native Paper** (needs `PaperProvider` + `react-native-safe-area-context` + vector-icons).
- **Native-module RN libraries** → require autolinking + a native rebuild (`pod install`/Gradle, `expo prebuild`). Vox can declare the dep + config plugin in the manifest but **cannot make it a pure `vox build`**. Emit an explicit `vox check` diagnostic: "`<pkg>` requires a native module; run `expo prebuild` and a native build — it cannot be hot-imported."
- **Web and RN components are NOT interchangeable** (Fabric renders native host views, not DOM). The SSOT `targets` field guards this: importing a web-only library on the RN target is a compile error, and vice-versa.

## 12. Reverse direction (React imports Vox) — mostly already working

Vox already emits real `.tsx` (`export function Name(props): React.ReactElement`, [`reactive.rs:1018-1027`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs)) and a library `package.json` ([`library_package_emit.rs`](../../../crates/vox-codegen/src/codegen_ts/library_package_emit.rs)). Remaining: proper exported prop-type aliases, a publishable `exports` map, and re-emit-stability (don't hand-edit generated `.tsx`; the `.vox` source is canonical). Lower-risk than the consume direction; largely covered by the Phase-1 client/library work.

## 13. Priority ranking (verified npm weekly downloads, ~2026-05-31)

Sources: npm registry download API (`api.npmjs.org/downloads/point/last-week/<pkg>`), State of React / State of React Native 2024, GitHub.

**Web build order (friction-weighted):**
1. **react-hook-form** — 49.5M/wk, hooks-only, zero provider/CSS, react-only peer. Highest-leverage, no real competitor (formik 3.7M and falling → **skip**).
2. **Radix primitives** — dialog 52M/wk, slot 163M/wk; headless, no provider/CSS; **transitively unlocks shadcn** (116k★, 80% satisfaction). Best friction-to-payoff in the set.
3. **recharts** — 48.7M/wk, pure-JS SVG, no provider/CSS (beats nivo).
4. **@tanstack/react-table** — 13M/wk, headless, same clean shape.
5. **@mui/material** — 8.7M/wk, popularity anchor for *styled* output; accept Emotion peer.
6. **@headlessui/react** — 5.8M/wk, second headless option.

Defer: Mantine / Chakra (provider + CSS), antd (heavy + resolver quirk), ag-grid / nivo (niche), react-bootstrap (external CSS).

**React Native build order:**
1. **NativeWind** — 1.18M/wk, className-based, fastest-growing; pair with Tailwind class output like web.
2. **React Native Paper** — 397k/wk, the one *kit* worth first-classing; accept `PaperProvider`.
3. *(later)* **Tamagui** — 227k/wk, only if unified web+native styling becomes a goal (heaviest setup).

Skip early: gluestack (fragmented 55k/42k), `@rneui/themed` (51k, declining).

## 14. Implementation plan (code-anchored)

> **Policy reminders for the implementing agent:** test-first ([AGENTS.md §Test-First](../../../AGENTS.md)); no `.ps1`/`.sh`/`.py` automation (use `vox run scripts/*.vox`); no `std::env::var` for secrets (`vox_secrets`); on Windows run cargo via `& "$env:USERPROFILE\.cargo\bin\cargo.exe"`. New crate → update root `Cargo.toml` + [`layers.toml`](layers.toml) + a row in [`where-things-live.md`](where-things-live.md).

| Slice | Delivers | Touches (file:line) | Acceptance |
|---|---|---|---|
| **S1 — Import grammar (C1)** | named / namespace / aliased import forms | `ast/decl/types.rs:33-55`, `parser/descent/decl/head.rs:95-150`, `hir/nodes/decl.rs:227-249`, `hir/lower/mod.rs:139-151`, emit `reactive.rs:947-964` | extend `parser_import_syntax_test.rs`; `cargo test -p vox-compiler import`; golden `.tsx` diff |
| **S2 — JSX-tag registration (C2)** | `<Imported/>` + `<Ns.Member/>` resolve on **web** | `reactive.rs:801-819` + `:876-912`, `web_ir/lower.rs`, `web_ir/validate.rs:300/356`; new `HirType::OpaqueExtern` (`hir/nodes/stmt_expr.rs:90-103`) + `lowering_shared/jsx.rs:24-39` arm | golden: a Vox component renders an imported Radix dialog; `cargo test -p vox-codegen` |
| **S3 — Manifest + dedupe + SSOT (C4)** | per-library SSOT YAML; emit deps + peers; Vite `dedupe` | new `contracts/frontend/external-component-libraries.v1.yaml` (+schema); `scaffold.rs:45-118`, `library_package_emit.rs`, `rn/scaffold.rs:154-197` | `pnpm install && tsc --noEmit` on a fixture passes; `vox check` errors on React-range conflict |
| **S4 — Providers + styling + `use client` (C5/C6/C7)** | provider injection, CSS-file/engine wiring, `'use client'` stamping, mandatory-provider diagnostic | emitter + SSOT; `routes` root wrap; `reactive.rs` component header | integration: MUI button renders themed; Mantine `styles.css` imported; `vox check` errors when `@mantine/core` imported w/o provider |
| **S5 — Flat-facade type bridge (C3 opt-in)** | `vox import-types` → `*.vox.extern.json`; `extern type`; facade prop-name/required checks | new dev step (TS compiler API, Node); `TypeDefDecl`/new `ExternTypeDecl` (`ast/decl/typedef.rs:83-102`, `hir/nodes/decl.rs:370-396`); `contract_ir/project.rs` (`OpaqueExtern→Unknown`); `typeck/checker/expr.rs:716-730` facade check | `vox check` flags unknown / missing-required prop on a faceted component; opaque path unaffected |
| **S6 — shadcn vendor mode (§10)** | `vox add component <name>` writes registry `.tsx`; wires Tailwind + radix deps | new CLI command `crates/vox-cli/src/commands/` (register in `commands/mod.rs`); reuse `components.json`; SSOT deps | vendored `button.tsx` compiles; `import react Button from "./components/ui/button"` renders |
| **S7 — RN external import (§11.2)** | register imports + emit ES import on RN; native-module diagnostic | `rn/component.rs:469-492` (membership) + `:1221-1277` (import loop); SSOT `targets`/native flag | Expo fixture with Tamagui builds (`tsc --noEmit` + cross-compile gate); native-module lib emits the rebuild diagnostic |
| **S8 — Docs + goldens** | tutorials + golden coverage | `docs/src/tutorials/` (use-a-React-component, use-a-Vox-component, RN); `examples/golden/` | doc-pipeline frontmatter passes; goldens green |

**Sequencing:** S1→S2→S3 is the mandatory spine (any import works end-to-end). S4 unblocks Tier-2 libraries. S5 (types), S6 (shadcn), S7 (RN) are independent and parallelizable after the spine. S8 throughout.

```
S1 ─► S2 ─► S3 ─┬─► S4
                ├─► S5
                ├─► S6
                └─► S7
```

## 15. Honest limitations (what this does NOT solve)

1. **Vox does not faithfully model TS prop types.** Open unions (`OverridableStringUnion`), generics, conditional/mapped types, render-prop function children, and polymorphic `as` stay opaque. The flat facade catches prop-name/required-prop errors only; the consumer's `tsc` is the real authority. `vox check` will never fully understand MUI's variant union — that is inherent, not a bug.
2. **The per-library SSOT is hand-curated.** Whether a provider is *mandatory* is documentation knowledge; new libraries need a row.
3. **Native-module RN libraries cannot be a pure `vox build`.** Autolinking + native compile is unavoidable; Vox declares and diagnoses, it cannot eliminate.
4. **shadcn is codegen, not import.** Treating it as a dependency is a category error.
5. **RSC nuances** beyond first-line `'use client'` stamping (a full Server-Components target) are out of scope.
6. **`vox import-types` adds a Node dev-toolchain dependency.** It is opt-in; Tier 0 needs none.

## 16. Risks

- **Web/RN React-version divergence** (`^19` vs `18.3.1`) — the SSOT must resolve peers per target or installs break (S3).
- **WebIR vs legacy emit drift** — registration (S2) must be applied to both the legacy `reactive.rs` path and the canonical WebIR lowering, or imported tags work in one path and not the other.
- **Duplicate React** — forgetting `resolve.dedupe` reintroduces the invalid-hook-call failure; dedupe is part of the spine (S3), not polish.
- **`exports`-less packages** (antd uses `typings` not `types`, no `exports`) — the manifest/resolution assumptions must tolerate legacy `main`/`module` packages.

## 17. Related documents

- [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md) — parent five-phase plan (this is the Phase-5 sub-spec).
- [Vox–React backend interop audit (2026)](vox-react-backend-interop-audit-2026.md) — backend/API half.
- [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md) — the RN target architecture.
- [vox-runtime-rn-mobile-cross-compile.md](vox-runtime-rn-mobile-cross-compile.md) — mobile cross-compile SSOT.
- [Where Things Live](where-things-live.md), [layers.toml](layers.toml) — crate placement for new artifacts.
- [Wire Format v1 SSOT](wire-format-v1-ssot.md) — `OpaqueExtern → Unknown → z.any()` rationale.
