---
title: "Mobile RN + Expo + uniffi: comprehensive implementation spec (2026)"
description: "File-by-file, crate-by-crate plan for shipping Vox to iOS and Android via React Native + Expo with vox-runtime cross-compiled to mobile through uniffi. No stubs. Includes CLI integration test harness, Tauri desktop scoping, vox-mental-tracker migration."
category: "Architecture SSOTs"
status: draft
last_updated: 2026-05-28
authors: [vox-team]
related:
  - mobile-target-evaluation-2026.md
  - mobile-rn-expo-architecture-and-migration-2026.md
  - adr-NNN-scope-tauri-desktop-only.md
  - codegen-ssot-unification-design-2026.md
---

# Mobile RN + Expo + uniffi — comprehensive implementation spec

> Companion to [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md). That doc declares *what* and *why*. This doc declares *how*, file by file, crate by crate, with LoC budgets, function signatures, and acceptance criteria sized so no part of this work is a stub.

## Reading guide

- **§0** — preconditions. Three P0 bugs must land before any phase below has solid footing.
- **§1** — the CLI integration test harness. Foundational; every later phase asserts against it.
- **§2** — codebase reality, in numbers. Grounds the design in what exists.
- **§3-§4** — the structural primitives: a target-agnostic mobile-primitive adapter and the VUV-style IR layer.
- **§5-§7** — the codegen pipeline changes: `BuildTarget::Mobile`, `codegen_ts/rn/`, mobile config emit.
- **§8-§9** — the JS-side packages: `@vox/runtime` (web/Tauri) and `@vox/runtime-rn` (Expo).
- **§10-§11** — Rust on mobile: vox-runtime mobile profile + uniffi-bindgen-react-native.
- **§12** — Tauri desktop scoping (what stays, what narrows).
- **§13** — vox-mental-tracker migration, step by step.
- **§14** — full test plan.
- **§15** — phasing with deliverables, LoC budgets, and acceptance criteria.
- **§16** — open decisions and risks.

---

## §0 Preconditions

These three items block all phases. Without them, this work builds on a broken pipeline.

### §0.1 Fix `crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:288` panic

**Symptom.** `vox build examples/golden-ts/component_state.vox -o /tmp/dist` panics in `vox build`'s Rust-emit phase at the `.expect()` on line 288:

```
HirModule serializes to JSON (Serialize derive guaranteed at decl.rs:29): Error("key must be a string", line: 0, column: 0)
```

**Root cause.** `serde_json` requires map keys to be strings (or convertible to strings). Somewhere in the `HirModule` (or one of its serializable children — `HirFn`, `HirComponent`, `HirRoutes`, etc.) there is a `HashMap` or `BTreeMap` whose key type is not a `String` (likely a struct, enum, or tuple). The `serde_json::to_string`/`to_value` call in `main_boot.rs:288` fails on that map.

**Fix protocol.** Two acceptable resolutions:

1. **Preferred:** locate the offending map and change either its key type to `String` (or to a `Display` newtype) or annotate the field with `#[serde(serialize_with = "serialize_map_with_stringified_keys")]` + a helper. Add a serialization round-trip unit test in `crates/vox-compiler/tests/` that asserts every `HirModule` produced by parsing `examples/golden-ts/*.vox` and `examples/golden/*.vox` round-trips through `serde_json` losslessly.

2. **Acceptable fallback:** convert the JSON-embedding strategy in `main_boot.rs` from `serde_json::to_string` to `bincode` or `rmp-serde` (which permit non-string map keys), and base64-encode the bytes into the emitted Rust string literal. This is a wider change and should be a separate ADR if pursued.

**Test gate.** After fix, `cargo run -p vox-cli -- build examples/golden-ts/component_state.vox -o /tmp/dist_verify` MUST produce a non-empty `Counter.tsx` and a `main.rs` containing the embedded `HirModule` JSON, exit code 0.

**LoC estimate.** Fix: 5-50 LoC depending on which collection is offending. Test: ~50 LoC.

### §0.2 Fix struct-literal-in-fn-body typeck regression

**Symptom.** `examples/golden-ts/wire_format_round_trip.vox` fails typeck with `Undefined variable: Price` when a struct literal `Price { amount: 0.0, currency: "USD" }` appears in a function body, even though `type Price { … }` is defined in the same module.

**Root cause hypothesis.** The struct-literal expression arm in `crates/vox-compiler/src/typeck/infer.rs` (around `pub fn check_expr` at line 51) either (a) does not resolve type names against `TypeEnv` for the expression form, or (b) the `TypeEnv` is not populated with same-module `type` decls before function bodies are visited.

**Fix protocol.**

1. Add `RUST_BACKTRACE=1` and reproduce to confirm the line.
2. Trace `check_expr` for the struct-literal expression variant. Confirm it does `env.lookup_type(name)` (or equivalent) rather than `env.lookup_value(name)`.
3. If the lookup is correct, audit `typecheck_hir` in `crates/vox-compiler/src/typeck/checker/mod.rs:866` to confirm `env` is populated from `module.type_decls` before `checker.check_module(module)` is called.
4. Add the file to the golden-test harness allowlist removal (it currently has `TYPECK_SKIP` in `crates/vox-codegen/tests/golden_ts_test.rs`). After fix, remove the skip and verify the test still passes.
5. Add a positive unit test: a minimal `.vox` source with `type Foo { x: int }` + `fn bar() to Foo { return Foo { x: 0 } }`, asserting typeck reports zero errors.

**LoC estimate.** Fix: 5-30 LoC. Test: ~30 LoC.

### §0.3 CLI integration test harness (specified in detail in §1)

The harness must exist before any of §3-§13 work begins. Without it, the "library works, CLI doesn't" gap that produced §0.1 and §0.2 will recur and silently block mobile work.

---

## §1 CLI integration test harness

### §1.1 The problem

The codegen library is well-tested via snapshot tests (`crates/vox-codegen/tests/golden_ts_test.rs`, etc.) that invoke `parse → lower_module → generate` directly. These tests bypass:

- `vox-cli` argument parsing and config resolution
- `vox-compiler::pipeline::run_frontend` (which includes typeck)
- The Rust-emit phase in `vox-codegen::codegen_rust::generate`
- File-system writes and directory layout
- The post-write verification (`verify_app_tsx_route_imports`)

Both bugs in §0.1 and §0.2 live in code paths that snapshot tests don't reach. The harness fixes this gap permanently.

### §1.2 Design

New crate: **`crates/vox-cli-tests/`** (separate from `vox-cli` to avoid circular test deps).

Cargo deps: `assert_cmd` (process-level test driver), `tempfile`, `serde_json`, `vox-cli` as a test dependency.

Two test modules:

#### `crates/vox-cli-tests/tests/build_e2e.rs`

A parameterized test that runs `cargo run -p vox-cli -- build <fixture>.vox -o <tempdir>` for each fixture in `crates/vox-cli-tests/fixtures/build/` and asserts:

1. **Exit code 0.**
2. **Expected output files are present.** Each fixture has a sibling `expected_files.toml` listing required filenames (no content match; just existence).
3. **TypeScript output parses.** For every `*.tsx` and `*.ts` emitted, shell out to `npx tsc --noEmit --target es2020 --jsx react --moduleResolution node <file>` (with a stable shared `tsconfig.json` in the harness) and assert exit code 0. This catches output that "looks right" but doesn't compile.
4. **Rust output compiles.** When the target emits a `target/generated/` Cargo project, run `cargo check --manifest-path <generated>/Cargo.toml` and assert exit code 0. This catches the §0.1 class of bugs.
5. **No panics in stderr.** Greps stderr for `panicked at` and fails the test.

Fixtures: `crates/vox-cli-tests/fixtures/build/`:
- `component_state/main.vox` (from existing golden)
- `form_basic/main.vox` (from existing golden, with the label fix applied)
- `routes_with_loader/main.vox`
- `mobile_back_button/main.vox`
- `endpoint_query_mutation/main.vox`
- `state_machine/main.vox`
- `full_app/main.vox` — a single file exercising components + routes + form + endpoint + state machine + tokens

Each fixture has a `Vox.toml` and an `expected_files.toml`.

#### `crates/vox-cli-tests/tests/build_target_e2e.rs`

Parametrizes over `--target=fullstack | server | client | mobile` and asserts the appropriate subset of files is emitted per target.

### §1.3 LoC budget

| File | LoC |
|---|---:|
| `crates/vox-cli-tests/Cargo.toml` | 25 |
| `crates/vox-cli-tests/src/lib.rs` (shared helpers: tempdir setup, tsc/cargo-check spawn, output assert) | 200 |
| `crates/vox-cli-tests/tests/build_e2e.rs` | 150 |
| `crates/vox-cli-tests/tests/build_target_e2e.rs` | 100 |
| Fixtures (7 × ~10 LoC `.vox` + 7 × ~5 LoC `expected_files.toml` + shared `Vox.toml`) | ~120 |
| `crates/vox-cli-tests/tests/tsconfig.json` (single file) | 25 |
| **Total** | **~620** |

### §1.4 CI integration

Add to root workspace `Cargo.toml`. Add a CI job step `cargo test -p vox-cli-tests` after the existing `cargo test --workspace` step (it needs Node.js + npx available; pin to the same Node version EAS Build uses).

### §1.5 Acceptance criteria

- All 7 fixtures pass on a fresh checkout with `cargo test -p vox-cli-tests`.
- Adding a new fixture is one `.vox` file + one `expected_files.toml`.
- CI fails when §0.1 panic is re-introduced (regression gate).

---

## §2 Codebase reality (grounding for the rest of this doc)

Real LoC as of 2026-05-28:

| Module / crate | LoC | Status |
|---|---:|---|
| `crates/vox-codegen/src/codegen_ts/` (entire dir) | 9,218 | Library production; CLI broken |
| ↳ `emitter.rs` (orchestrator) | 527 | Real |
| ↳ `reactive.rs` (Path C view emit) | 1,214 | Real, migration mode |
| ↳ `hir_emit/mod.rs` (shared HIR → TS) | 1,488 | Real |
| ↳ `hir_emit/state_deps.rs` | 687 | Real |
| ↳ `jsx.rs` (AST → JSX) | 760 | Real |
| ↳ `route_manifest.rs` | 642 | Real |
| ↳ `openapi_emit.rs` | 517 | Real |
| ↳ `component.rs` | 475 | Real |
| ↳ `state_machine_emit.rs` | 267 | Real |
| ↳ `reactive_module_emit.rs` | 261 | Real |
| ↳ `tokens_emit.rs` | 245 | Real |
| ↳ `vox_client.rs` | 233 | Real |
| ↳ `fragment_emit.rs` | 214 | Real |
| ↳ `url_emit.rs` | 190 | Real |
| ↳ `scaffold.rs` | 180 | Real |
| ↳ `form_emit.rs` | 171 | Real |
| ↳ `mobile_emit.rs` | 119 | Real, emits Tauri-shaped JS |
| ↳ `zod_emit.rs` | 121 | Real |
| ↳ other small modules | ~414 | Real |
| `crates/vox-codegen/src/codegen_rust/emit/` | 4,751 | Library production; CLI broken at `main_boot.rs:288` |
| `crates/vox-workflow-runtime/` | 3,058 | Production |
| `crates/vox-actor-runtime/` | 8,745 | Production |
| `crates/vox-tauri-codegen/` | 401 | Production |
| `crates/vox-gui/` (Rust + TS) | ~90 | Working desktop |
| `crates/vox-tauri-stt/` (Rust glue + Kotlin + Swift) | 6,138 | Native code unwired; Rust returns "not connected" |
| `crates/vox-inference/` | (unmeasured here; assumed ~3-5K) | Production for desktop |
| `crates/vox-config/` BuildTarget enum | 3 variants (Fullstack / Server / Client) at `config/gamify_web.rs:61` | Real |

Mobile-relevant absences (zero LoC today):
- `crates/vox-runtime/` umbrella crate
- `crates/vox-runtime-mobile/` profile
- `crates/vox-runtime-rn/` uniffi bindings
- `crates/vox-rn-codegen/` (or `crates/vox-codegen/src/codegen_ts/rn/`)
- `crates/vox-cli-tests/` (the §1 harness)
- `clients/runtime/` npm packages (`@vox/runtime`, `@vox/runtime-rn`)
- Any `aarch64-*-android` / `aarch64-apple-ios` references in any Cargo.toml

---

## §3 Adapter pattern: `@vox/runtime` interface contract

### §3.1 Why

Today `mobile_emit.rs` emits `import { listen } from '@tauri-apps/api/event';` directly. Two GUI targets (web/Tauri + RN/Expo) cannot both fulfill this. The fix is to lift the mobile-primitive emit to call a stable JS API that *each platform* implements.

This single change unifies the two emit pipelines on the JS side. After it, the RN lowering reuses 100% of the mobile-primitive emit code.

### §3.2 The JS API contract

A new TypeScript interface, published as `@vox/runtime` (the contract package). Both `@vox/runtime` (web/Tauri impl) and `@vox/runtime-rn` (Expo impl) implement it.

```ts
// clients/runtime/types/index.ts — single source of truth
export interface VoxRuntime {
  // Lifecycle
  onAppStateChange(handler: (state: "active" | "background" | "inactive") => void): () => void;

  // Mobile primitives
  onBackButton(handler: () => Promise<boolean>): () => void;
  onDeepLink(handler: (url: string) => Promise<string | null>): () => void;
  installPushNotifications(handlers: {
    onRegister?: (token: string) => Promise<void>;
    onNotification?: (payload: unknown) => Promise<void>;
    onAction?: (payload: unknown) => Promise<void>;
  }): Promise<void>;

  // std.mobile bridge
  notify(title: string, body: string): Promise<void>;
  takePhoto(): Promise<string>;
  vibrate(): Promise<void>;
  transcribe(audioBytes: Uint8Array, langHint?: string): Promise<string>;
  transcribeMicrophone(): Promise<string>;

  // Vox runtime calls (uniffi-backed on mobile; Tauri IPC on desktop)
  spawnActor(name: string, initState: Uint8Array): ActorHandle;
  startWorkflow(id: string, payload: Uint8Array): WorkflowHandle;
  infer(modelId: string, input: Uint8Array): Promise<Uint8Array>;
}

export interface ActorHandle { id: string; send(message: Uint8Array): void; close(): void; }
export interface WorkflowHandle { id: string; await(): Promise<Uint8Array>; suspend(): void; resume(): void; }
```

The emitter never sees `@tauri-apps/api/event` or `BackHandler` directly. It only sees `voxRuntime.onBackButton(…)`.

### §3.3 Implementation deliverables

| File | LoC budget | Phase |
|---|---:|---:|
| `clients/runtime/types/index.ts` (contract) | 100 | Phase 1 |
| `clients/runtime/types/package.json` | 25 | Phase 1 |
| `clients/runtime-web/src/index.ts` (Tauri impl, ~10 methods) | 250 | Phase 1 |
| `clients/runtime-web/package.json` | 30 | Phase 1 |
| `clients/runtime-rn/src/index.ts` (Expo stubs in Phase 1; real in Phase 3) | 300 | Phase 1 stubs / Phase 3 real |
| `clients/runtime-rn/package.json` | 35 | Phase 1 |

The `clients/` top-level directory is new. Existing repo precedent for npm-published artifacts: review `pkgs/` if one exists; otherwise `clients/` is the conventional location (consistent with [Codegen SSOT Unification 2026](codegen-ssot-unification-design-2026.md)'s planned `@vox/runtime` ship).

### §3.4 Emitter refactor (`mobile_emit.rs` changes)

Current emit:

```js
import { listen } from '@tauri-apps/api/event';
void listen('vox-back-button', async () => { ... });
```

Target emit:

```js
import { voxRuntime } from '@vox/runtime';
voxRuntime.onBackButton(async () => { ... });
```

Both lowerings (`codegen_ts/web` and `codegen_ts/rn`) emit identical mobile-primitive code, differing only in the npm import name (`@vox/runtime` vs `@vox/runtime-rn`).

`mobile_emit.rs` becomes target-agnostic (~80 LoC after refactor, down from 119), and the per-target adapter selection happens via `options.target` (already plumbed through `CodegenOptions`).

### §3.5 Acceptance criteria

- `vox build foo.vox` produces `mobile.ts` that imports from `@vox/runtime`, not `@tauri-apps/api`.
- A fresh Vite+React+Tauri project with `@vox/runtime-web` installed runs the back-button handler correctly on desktop.
- The same `mobile.ts` (zero changes) runs in an Expo app with `@vox/runtime-rn` installed and Android emulator BackHandler events flow through.

---

## §4 VUV-style IR (the structural prerequisite for two-target maintainability)

### §4.1 Why

Today, `reactive.rs` and `component.rs` hardcode React-DOM-shaped JSX with Tailwind class strings:

```rust
// Today, inside reactive.rs (paraphrased):
format!(r#"<div className={{["flex", "flex-col"].filter(Boolean).join(" ")}}>"#)
```

For RN, the same VUV `column()` primitive should lower to:

```tsx
<View style={styles.col}>
```

Without a refactor, every VUV view primitive — `column`, `stack`, `text`, `button`, `panel`, `heading`, `image`, `text-input`, `for`-loop, `if` branching — needs hand-edits in TWO places. With a refactor, the per-primitive logic lives ONCE in a target-agnostic IR; per-target translators are mechanical walkers.

### §4.2 Design

Add a new submodule: `crates/vox-codegen/src/web_ir/style_ir.rs` (~600 LoC).

The existing `web_ir` (`crate::web_ir`) is already a structural IR for views (`WebIrModule`, `BehaviorNode`, etc.). Extend it with a normalized style+layout layer:

```rust
// crates/vox-codegen/src/web_ir/style_ir.rs
pub enum LayoutKind { Column, Stack, Row, Panel, Heading(u8), Text, Button, Image, TextInput, ListItem, /* ... */ }
pub struct LayoutNode {
    pub kind: LayoutKind,
    pub style: StyleProps,
    pub events: EventBindings,
    pub children: Vec<LayoutNodeRef>,
}
pub struct StyleProps {
    pub flex_direction: Option<FlexDir>,
    pub gap: Option<Length>,
    pub padding: Option<Edges<Length>>,
    pub margin: Option<Edges<Length>>,
    pub background: Option<Color>,
    pub color: Option<Color>,
    pub font_size: Option<FontSize>,
    pub font_weight: Option<FontWeight>,
    pub border: Option<Border>,
    pub border_radius: Option<Length>,
    pub align: Option<Align>,
    pub justify: Option<Justify>,
    pub raw_class: Vec<String>,         // pass-through CSS classes for web
    pub raw_style_rn: Vec<RnStyleHint>, // pass-through StyleSheet entries for RN
    pub safe_area: Option<SafeAreaEdge>,
}
pub struct EventBindings {
    pub on_press: Option<HandlerRef>,
    pub on_long_press: Option<HandlerRef>,
    pub on_change: Option<HandlerRef>,
    pub on_submit: Option<HandlerRef>,
}
```

Lowering pass: HIR view expr → `LayoutNode` tree. Lives in `crates/vox-codegen/src/web_ir/lower_style.rs` (~400 LoC).

Per-target translators (each ~300-500 LoC):

- `crates/vox-codegen/src/web_ir/emit_web_dom.rs` — LayoutNode → `<div>`/`<button>`/`<input>` + Tailwind className strings. Replaces equivalent logic in `reactive.rs`.
- `crates/vox-codegen/src/web_ir/emit_rn.rs` — LayoutNode → `<View>`/`<Pressable>`/`<TextInput>` + `StyleSheet.create({…})` blocks.

After refactor, `reactive.rs` shrinks. Each new VUV primitive is added by extending `LayoutKind` + lowering + one entry in each translator's dispatch table.

### §4.3 Tailwind ↔ StyleSheet mapping

The web translator already produces Tailwind class strings (shadcn-shaped). The RN translator translates the same style props to StyleSheet objects. To avoid hand-mapping every Tailwind class, the RN translator works from `StyleProps` directly (not from class strings).

Decision deferred to Phase 0 spike: should the RN translator emit raw `StyleSheet.create` OR use [NativeWind](https://www.nativewind.dev/) (Tailwind-on-RN) so the class strings transfer literally? Tradeoffs in §16.

Default assumption for budget purposes: raw `StyleSheet.create`. If NativeWind wins the spike, the RN translator emits `className="..."` strings nearly identically to the web translator (smaller LoC, larger runtime dep).

### §4.4 Migration sequencing

The `web_ir` module already exists and the `reactive.rs` emit flows through it. The refactor is incremental:

1. Add `StyleProps` and `LayoutNode` types.
2. Add `lower_style.rs` and run it in parallel with existing emit (validation only).
3. Move web-emit logic out of `reactive.rs` into `emit_web_dom.rs`; assert byte-identical output against existing snapshots.
4. Add `emit_rn.rs`. New snapshot suite asserts RN output for each existing golden.

Step 3 is the high-risk step. Mitigation: run web-DOM output through both old and new paths in test mode and assert string equality across all 12 existing golden snapshots before deleting the old path.

### §4.5 LoC budget

| File | LoC |
|---|---:|
| `web_ir/style_ir.rs` (types) | 600 |
| `web_ir/lower_style.rs` (HIR → LayoutNode) | 400 |
| `web_ir/emit_web_dom.rs` (LayoutNode → DOM + Tailwind) | 450 |
| `web_ir/emit_rn.rs` (LayoutNode → RN + StyleSheet) | 500 |
| Refactor in `reactive.rs` (deletion + delegation) | -600 / +100 |
| **Net new** | **~1,450** |

---

## §5 `BuildTarget::Mobile` in vox-config

### §5.1 Change

`crates/vox-config/src/config/gamify_web.rs:61` — add a fourth variant to the `BuildTarget` enum:

```rust
pub enum BuildTarget {
    Fullstack,
    Server,
    Client,
    Mobile,  // NEW: Expo-flavored RN TS emit + uniffi-bridged Rust runtime
}
```

`FromStr` impl gets `"mobile" => Ok(BuildTarget::Mobile)`.

### §5.2 CLI surface

`crates/vox-cli/src/cli_args.rs` — extend `BuildTargetArg`:

```rust
pub enum BuildTargetArg {
    Fullstack,
    Server,
    Client,
    Mobile,  // NEW
}
```

### §5.3 Build command behavior

In `crates/vox-cli/src/commands/build.rs`, add a new `if resolved_target == vox_config::BuildTarget::Mobile { ... }` branch (parallel to the existing Server and Client branches, around line 60-175).

The Mobile branch:

1. Runs `codegen_ts::generate_with_options(&hir, opts_with_target_rn)` (where `opts.target = Some("rn")`).
2. Writes emitted TS to `out_dir/` (the Expo project's source root, typically `apps/<app>/src/`).
3. Writes an `app.json` (Expo config) and `metro.config.js` if not present.
4. Writes uniffi UDL files into `crates/vox-runtime-rn/uniffi/` for the runtime crate.
5. Writes EAS Build config (`eas.json`) if not present.
6. Does NOT emit Axum/Rust backend (the server-side Rust emit is preserved for desktop/server targets).

### §5.4 LoC budget

| File | LoC |
|---|---:|
| `vox-config/src/config/gamify_web.rs` (enum extension + FromStr) | 20 |
| `vox-cli/src/cli_args.rs` (BuildTargetArg variant) | 10 |
| `vox-cli/src/commands/build.rs` (new Mobile branch) | 150 |
| `vox-cli/src/commands/mobile_scaffold.rs` (NEW: emits app.json, metro.config.js, eas.json) | 200 |
| **Net new** | **~380** |

---

## §6 `codegen_ts/rn/` lowering

### §6.1 Module layout

New submodule at `crates/vox-codegen/src/codegen_ts/rn/`:

```
codegen_ts/rn/
├── mod.rs                    (~150 LoC) — module manifest + dispatch
├── component.rs              (~400 LoC) — VUV component → RN functional component
├── routes.rs                 (~350 LoC) — @routes → expo-router Stack
├── form.rs                   (~200 LoC) — @form → RN form (TextInput, Pressable, validation)
├── style_sheet.rs            (~300 LoC) — LayoutNode → StyleSheet.create({…})
├── app_scaffold.rs           (~250 LoC) — App.tsx, _layout.tsx, metro/expo config files
├── mobile_bridge.rs          (~150 LoC) — std.mobile → @vox/runtime-rn calls (target-aware via §3)
└── snapshots/                — golden RN output per primitive
```

### §6.2 Dispatch from `emitter.rs`

`generate_with_options` gains a target check at the top (after the existing target/mode checks):

```rust
// Pseudo-code addition
let is_rn = options.target.as_deref() == Some("rn");
if is_rn {
    return codegen_ts::rn::generate_rn(hir, &options);
}
// ... existing web/Tauri path continues unchanged ...
```

This isolates the RN path entirely. No risk of regressing the web emit during initial RN development.

After Phase 2 acceptance (§15), the two paths share the `LayoutNode` IR (per §4) and the conditional collapses to per-translator dispatch.

### §6.3 RN component emit shape

For a VUV `component HomeScreen() { state count: int = 0; view: column() { ... button(on_click=...) { "Tap" } } }`, the RN target emits:

```tsx
import React, { useState } from "react";
import { View, Text, Pressable, StyleSheet } from "react-native";

export function HomeScreen(): React.ReactElement {
  const [count, set_count] = useState<number>(0);
  return (
    <View style={styles.root}>
      <Pressable style={styles.btn} onPress={() => set_count(count + 1)}>
        <Text style={styles.btnText}>Tap</Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  root: { flexDirection: "column", gap: 12 },
  btn: { backgroundColor: "#0a7ea4", paddingVertical: 10, paddingHorizontal: 16, borderRadius: 6, alignItems: "center" },
  btnText: { color: "white", fontWeight: "500" },
});
```

The `styles` object is generated from the same `StyleProps` that the web target uses to generate Tailwind class strings.

### §6.4 Routes emit (expo-router)

Expo Router uses file-system routing. A `routes { "/" to Home; "/detail/:id" to Detail }` declaration emits:

```
src/app/_layout.tsx              (~30 LoC, wraps with VoxRuntimeProvider + SafeAreaProvider)
src/app/index.tsx                (re-export of Home component as default)
src/app/detail/[id].tsx          (re-export of Detail component with route params)
```

`routes.manifest.ts` (the existing web emit) is replaced by a per-route file under `src/app/`. This is structurally different from the web emit; the differences live entirely in `rn/routes.rs`.

### §6.5 Form emit

VUV `@form Item { field name: str required label("Item name") }` lowers to:

```tsx
import { TextInput, Pressable, Text, View } from "react-native";
import { submit_item } from "./vox-client";
import { useState } from "react";

export function Item() {
  const [name, setName] = useState("");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const onSubmit = async () => {
    const errs: Record<string, string> = {};
    if (!name) errs.name = "Item name is required";
    setErrors(errs);
    if (Object.keys(errs).length > 0) return;
    setSubmitting(true);
    try { await submit_item({ name }); } finally { setSubmitting(false); }
  };
  return (
    <View>
      <Text>Item name *</Text>
      <TextInput value={name} onChangeText={setName} accessibilityLabel="Item name" />
      {errors.name && <Text style={styles.err}>{errors.name}</Text>}
      <Pressable disabled={submitting} onPress={onSubmit}>
        <Text>{submitting ? "Saving…" : "Submit"}</Text>
      </Pressable>
    </View>
  );
}
```

Validation logic is unchanged from the web emit; only the JSX tags and event prop names differ (`onChange` → `onChangeText`, `onClick` → `onPress`, `<input>` → `<TextInput>`, `<button>` → `<Pressable>`).

### §6.6 Acceptance criteria

- All 7 fixtures in §1.2's `build_e2e.rs` pass with `--target=mobile`.
- A new RN snapshot suite (`crates/vox-codegen/tests/golden_rn_test.rs`) exists with one snapshot per existing web golden in `examples/golden-ts/`, asserting byte-equal RN output.
- The emitted output of one fixture (`full_app`) runs in a clean Expo + EAS Build sandbox and reaches Expo Go on Android emulator without manual edits.

### §6.7 LoC budget

| File | LoC |
|---|---:|
| `rn/mod.rs` | 150 |
| `rn/component.rs` | 400 |
| `rn/routes.rs` | 350 |
| `rn/form.rs` | 200 |
| `rn/style_sheet.rs` | 300 |
| `rn/app_scaffold.rs` | 250 |
| `rn/mobile_bridge.rs` | 150 |
| `tests/golden_rn_test.rs` | 100 |
| `tests/snapshots/golden_rn_test__*` | snapshot files only |
| **Total** | **~1,900** |

---

## §7 Mobile build artifacts (Expo config, EAS, metro)

### §7.1 What the Mobile build target emits

In addition to the `.ts` / `.tsx` files from §6, `vox build --target=mobile` emits an Expo-managed project skeleton at `out_dir` if not present:

```
<out_dir>/
├── app.json                  (Expo config, identifier, splash, version)
├── metro.config.js           (Metro bundler config, expo-asset)
├── eas.json                  (EAS Build profiles: development, preview, production)
├── tsconfig.json             (RN-flavored, jsx: react-jsx)
├── package.json              (expo, react, react-native, @vox/runtime-rn, @vox/runtime types)
├── babel.config.js           (Expo preset)
├── src/                      (Vox-emitted)
│   ├── app/                  (expo-router file system)
│   │   ├── _layout.tsx
│   │   ├── index.tsx
│   │   └── [route]/...
│   ├── components/           (per-component .tsx files)
│   ├── forms.tsx
│   ├── mobile.ts             (uses voxRuntime API)
│   ├── vox-client.ts         (reused from web emit, unchanged)
│   ├── schemas.ts
│   └── types.ts
└── android/                  (only if --eject; managed workflow normally hides this)
```

Files are written only if not present (scaffold-once behavior, matching existing `scaffold.rs`).

### §7.2 EAS Build config emission

`eas.json` minimal viable:

```json
{
  "cli": { "version": ">= 5.0.0" },
  "build": {
    "development": { "developmentClient": true, "distribution": "internal", "android": { "buildType": "apk" } },
    "preview": { "distribution": "internal", "android": { "buildType": "apk" } },
    "production": { "autoIncrement": true }
  },
  "submit": { "production": {} }
}
```

`app.json` template injects `expo.name` + `expo.slug` + `expo.ios.bundleIdentifier` + `expo.android.package` from `Vox.toml` metadata via the existing `vox-tauri-codegen`-style identifier projection.

### §7.3 LoC budget

| File | LoC |
|---|---:|
| Template strings for app.json/metro/eas (in `rn/app_scaffold.rs`) | covered in §6.7 |
| Identifier projection from Vox.toml (extension to existing logic) | 80 |
| **Net new beyond §6.7** | **~80** |

---

## §8 `@vox/runtime` (web/Tauri JS package)

### §8.1 Layout

`clients/runtime-web/` — a standalone npm package, published as `@vox/runtime`.

```
clients/runtime-web/
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts             (entry; exports voxRuntime)
│   ├── lifecycle.ts         (onAppStateChange via Tauri window events)
│   ├── mobile.ts            (onBackButton, onDeepLink, push — all via @tauri-apps/api/event)
│   ├── std_mobile.ts        (notify, takePhoto, vibrate, transcribe — via @tauri-apps/api/core invoke)
│   └── runtime.ts           (spawnActor, startWorkflow, infer — via Tauri invoke into linked vox-runtime)
└── tests/
    └── contract.test.ts     (asserts implementation satisfies the VoxRuntime interface)
```

### §8.2 Implementation pattern

```ts
// clients/runtime-web/src/mobile.ts
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export function onBackButton(handler: () => Promise<boolean>): () => void {
  let unlisten: (() => void) | undefined;
  void listen<void>("vox-back-button", async () => {
    const handled = await handler();
    if (!handled) await invoke("plugin:process|exit");
  }).then((fn) => { unlisten = fn; });
  return () => unlisten?.();
}
```

### §8.3 Vox runtime calls

`spawnActor`, `startWorkflow`, `infer` route through `@tauri-apps/api/core::invoke` to Tauri commands defined in `crates/vox-gui/src/main.rs` (and any other Tauri app linking `vox-runtime`). These commands wrap the Rust runtime API directly — no IPC envelope translation needed since both sides are local to the process.

### §8.4 LoC budget

| File | LoC |
|---|---:|
| `clients/runtime-web/src/index.ts` | 30 |
| `clients/runtime-web/src/lifecycle.ts` | 80 |
| `clients/runtime-web/src/mobile.ts` | 150 |
| `clients/runtime-web/src/std_mobile.ts` | 200 |
| `clients/runtime-web/src/runtime.ts` | 250 |
| `clients/runtime-web/package.json` + tsconfig | 50 |
| `clients/runtime-web/tests/contract.test.ts` | 100 |
| **Total** | **~860** |

---

## §9 `@vox/runtime-rn` (Expo JS package)

### §9.1 Layout

`clients/runtime-rn/` — standalone npm package, published as `@vox/runtime-rn`.

```
clients/runtime-rn/
├── package.json
├── tsconfig.json
├── plugin.js                (Expo config plugin — adds AndroidManifest.xml entries, Info.plist keys, links native lib)
├── src/
│   ├── index.ts             (entry; exports voxRuntime)
│   ├── lifecycle.ts         (onAppStateChange via react-native AppState)
│   ├── mobile.ts            (onBackButton via BackHandler, onDeepLink via expo-linking, push via expo-notifications)
│   ├── std_mobile.ts        (notify via expo-notifications, takePhoto via expo-image-picker, vibrate via expo-haptics)
│   └── runtime.ts           (spawnActor/startWorkflow/infer/transcribe — calls into uniffi-generated TurboModule)
└── ios/ + android/          (Expo Module native scaffolding linking the vox-runtime .a/.so)
```

### §9.2 The uniffi-generated TurboModule

`uniffi-bindgen-react-native` generates a TypeScript-typed TurboModule binding from the Rust crate (see §11). The generated file lives at `clients/runtime-rn/src/__generated__/vox_runtime_uniffi.ts` and is regenerated by `cargo run -p vox-runtime-rn --bin uniffi-gen` during `vox build --target=mobile`.

The hand-written `runtime.ts` wraps the generated TurboModule in the JS API shape from §3.2.

### §9.3 Expo config plugin

The Expo plugin (`plugin.js`) is what allows `@vox/runtime-rn` to integrate with a managed Expo workflow without manual native edits. It:

- Adds the precompiled `vox-runtime-rn.framework` (iOS) or `libvox_runtime_rn.so` (Android) to the appropriate native project at prebuild time.
- Adds the permission entries to AndroidManifest.xml (RECORD_AUDIO, POST_NOTIFICATIONS, etc., gated by which features the consuming app uses) and to Info.plist (NSMicrophoneUsageDescription, etc.).
- Adds `expo-asset` entries for any Candle model files declared in the consuming Vox app.

### §9.4 LoC budget

| File | LoC |
|---|---:|
| `clients/runtime-rn/src/index.ts` | 30 |
| `clients/runtime-rn/src/lifecycle.ts` | 70 |
| `clients/runtime-rn/src/mobile.ts` | 200 |
| `clients/runtime-rn/src/std_mobile.ts` | 250 |
| `clients/runtime-rn/src/runtime.ts` | 300 |
| `clients/runtime-rn/plugin.js` | 350 |
| `clients/runtime-rn/ios/VoxRuntimeRn.podspec` + Swift glue | 200 |
| `clients/runtime-rn/android/build.gradle` + Kotlin glue | 200 |
| `clients/runtime-rn/package.json` + tsconfig + expo-module.config.json | 100 |
| `clients/runtime-rn/tests/contract.test.ts` | 100 |
| **Total** | **~1,800** |

uniffi-generated code is NOT counted in the budget (it's generated, not hand-written, and lives in `__generated__/`).

---

## §10 `vox-runtime` mobile profile

### §10.1 New umbrella crate

Today there is no top-level `vox-runtime` crate. The runtime concept is distributed across `vox-workflow-runtime`, `vox-actor-runtime`, `vox-inference`, etc.

This spec introduces **`crates/vox-runtime/`** as an umbrella crate that re-exports a unified public API. It does not replace the existing runtime crates — it composes them.

```
crates/vox-runtime/
├── Cargo.toml
├── src/
│   ├── lib.rs                (re-exports + VoxRuntime struct)
│   ├── config.rs             (VoxConfig: data dir, model dir, log level, runtime profile)
│   ├── profile.rs            (RuntimeProfile: Desktop | Mobile, controls scheduler, journal, etc.)
│   ├── workflow.rs           (wraps vox_workflow_runtime; mobile profile = on-suspend flush)
│   ├── actor.rs              (wraps vox_actor_runtime; mobile profile = single-thread scheduler)
│   ├── inference.rs          (wraps vox_inference; mobile profile = on-demand model load + memory pressure unload)
│   ├── mobile_api.rs         (uniffi #[uniffi::export] surface — see §11)
│   └── lifecycle.rs          (suspend/resume implementation; iOS-grace-period flush)
```

### §10.2 RuntimeProfile

```rust
pub enum RuntimeProfile {
    /// Multi-threaded Tokio, free-running actors, leisurely journal flushes.
    Desktop,
    /// Single-threaded Tokio, suspendable actors, journal-on-lifecycle.
    Mobile,
}
```

A `VoxConfig` carries `profile: RuntimeProfile`. The mobile uniffi entry constructor passes `Mobile`; desktop callers (Tauri) pass `Desktop` (default).

### §10.3 Suspend/resume

```rust
impl VoxRuntime {
    pub fn suspend(&self) {
        // Flush journal, pause actors, evict ML model caches if memory pressure
    }
    pub fn resume(&self) {
        // Replay journal entries newer than last consumed, resume actors, lazily reload models
    }
}
```

iOS gives ~30 seconds after `applicationWillResignActive` before potential kill. The mobile profile's `suspend` flushes within 5 seconds (configurable) and is no-op safe — multiple consecutive `suspend()` calls are idempotent.

### §10.4 Cross-compile config

`crates/vox-runtime/Cargo.toml` adds `[lib]` `crate-type = ["staticlib", "cdylib"]` so it builds as a linkable `.a`/`.so` for mobile, alongside the standard `rlib` for desktop.

Mobile arch support added to the workspace `.cargo/config.toml`:

```toml
[target.aarch64-linux-android]
linker = "aarch64-linux-android24-clang"
ar = "llvm-ar"

[target.armv7-linux-androideabi]
linker = "armv7a-linux-androideabi24-clang"
ar = "llvm-ar"

[target.aarch64-apple-ios]
# uses xcrun on macOS

[target.aarch64-apple-ios-sim]
# uses xcrun on macOS
```

The `linker` invocations require Android NDK on PATH. CI (EAS Build) handles this; local dev requires `cargo install cargo-ndk` and `rustup target add aarch64-linux-android armv7-linux-androideabi aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim`.

### §10.5 LoC budget

| File | LoC |
|---|---:|
| `vox-runtime/src/lib.rs` | 150 |
| `vox-runtime/src/config.rs` | 100 |
| `vox-runtime/src/profile.rs` | 80 |
| `vox-runtime/src/workflow.rs` | 250 |
| `vox-runtime/src/actor.rs` | 250 |
| `vox-runtime/src/inference.rs` | 300 |
| `vox-runtime/src/mobile_api.rs` (uniffi surface) | 200 |
| `vox-runtime/src/lifecycle.rs` | 250 |
| `vox-runtime/Cargo.toml` + workspace config | 50 |
| `vox-runtime/tests/` (suspend/resume, profile dispatch, journal replay) | 400 |
| **Total** | **~2,030** |

---

## §11 uniffi-bindgen-react-native

### §11.1 The UDL (Uniffi Definition Language)

`crates/vox-runtime/uniffi/vox_runtime.udl` — the contract uniffi-bindgen consumes:

```
namespace vox_runtime {};

dictionary VoxConfig {
    string data_dir;
    string model_dir;
    string log_level;
    RuntimeProfile profile;
};

enum RuntimeProfile { "Desktop", "Mobile" };

interface VoxRuntime {
    constructor(VoxConfig config);
    ActorHandle spawn_actor(string name, bytes init_state);
    WorkflowHandle start_workflow(string id, bytes payload);
    [Throws=VoxError]
    bytes infer(string model_id, bytes input);
    [Throws=VoxError]
    string transcribe(bytes audio_bytes, string? lang_hint);
    void suspend();
    void resume();
};

interface ActorHandle {
    string id();
    void send(bytes message);
    void close();
};

interface WorkflowHandle {
    string id();
    [Throws=VoxError]
    bytes await();
    void suspend();
    void resume();
};

[Error]
enum VoxError {
    "NotInitialized",
    "ModelLoadFailed",
    "WorkflowNotFound",
    "Internal",
};
```

### §11.2 Code generation pipeline

```
crates/vox-runtime/uniffi/vox_runtime.udl
                    │
                    ▼
     uniffi-bindgen-react-native (CLI)
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
clients/runtime-rn/      crates/vox-runtime-rn/
  src/__generated__/       src/lib.rs (FFI scaffolding)
  vox_runtime_uniffi.ts    cbindgen header
        │                       │
        ▼                       ▼
  TypeScript-typed         Cross-compiled .a/.so
  TurboModule              per mobile arch
```

### §11.3 New crate: `vox-runtime-rn`

A thin Rust crate that:

1. Depends on `vox-runtime`.
2. Re-exports `vox-runtime`'s public API through the uniffi proc-macro layer.
3. Builds as `cdylib` + `staticlib` for the four mobile targets.

```
crates/vox-runtime-rn/
├── Cargo.toml             (cdylib + staticlib; depends on vox-runtime + uniffi)
├── build.rs               (uniffi build script: generates scaffolding from UDL)
├── src/
│   └── lib.rs             (re-export #[uniffi::export] surface from vox-runtime)
└── uniffi/
    └── vox_runtime.udl    (symlink to crates/vox-runtime/uniffi/vox_runtime.udl)
```

### §11.4 Build pipeline

`vox build --target=mobile` triggers (in this order):

1. Vox-side codegen (TS/RN emit per §6).
2. `cargo build -p vox-runtime-rn --target aarch64-linux-android --release` (and the other three targets).
3. `cargo run -p vox-runtime-rn --bin uniffi-gen -- --language typescript --out clients/runtime-rn/src/__generated__/` to produce TS bindings.
4. `cp target/<arch>/release/libvox_runtime_rn.so clients/runtime-rn/android/jniLibs/<abi>/`.
5. `lipo`-merge the iOS `.a` files into a universal `vox_runtime_rn.framework`.

On EAS Build, this happens in a build hook (`eas-build-pre-install`).

### §11.5 LoC budget

| File | LoC |
|---|---:|
| `crates/vox-runtime/uniffi/vox_runtime.udl` | 80 |
| `crates/vox-runtime-rn/Cargo.toml` | 40 |
| `crates/vox-runtime-rn/build.rs` | 80 |
| `crates/vox-runtime-rn/src/lib.rs` | 200 |
| `crates/vox-runtime-rn/tests/` (uniffi roundtrip) | 200 |
| EAS Build hook script (`eas-build-pre-install`) | 100 |
| `vox-cli` integration: invoke cargo build + uniffi-gen | 100 |
| **Total** | **~800** |

---

## §12 Tauri desktop scoping (what stays unchanged)

### §12.1 Explicit non-changes

The following are NOT modified by this spec:

- `crates/vox-gui/` — desktop dashboard, stays Tauri 2.
- `crates/vox-tauri-codegen/` (401 LoC) — continues to emit `tauri.conf.json` and capability projections for desktop only. Mobile-related projection code is removed.
- The web-target emit (`codegen_ts` non-RN modules) — no behavioral changes; only refactored to consume `LayoutNode` (§4) instead of producing JSX inline. Snapshots stay byte-equal.
- `crates/vox-codegen/src/codegen_rust/` — entirely server-side, unchanged. The mobile target does NOT emit Rust app code; the device-side Rust is `vox-runtime-rn` (via uniffi).

### §12.2 What narrows

- `crates/vox-tauri-codegen/` loses any mobile-specific config emit (e.g. mobile capability filtering for AndroidManifest entries — that work moves to the Expo config plugin in §9).
- `crates/vox-codegen/src/codegen_ts/mobile_emit.rs` becomes target-agnostic (~80 LoC), no longer emits `@tauri-apps/api` directly.

### §12.3 What retires

- `crates/vox-tauri-stt/` — entirely retired. The Kotlin/Swift native code (6,037 LoC) is deleted. On-device transcription is reimplemented as Candle Whisper inside `vox-runtime` (mobile profile) and exposed via uniffi.

### §12.4 LoC budget

| Change | LoC |
|---|---:|
| `vox-tauri-codegen` mobile bits removal | -80 |
| `mobile_emit.rs` refactor to adapter | -40 |
| `vox-tauri-stt` deletion | -6,138 |
| **Net delta** | **-6,258** |

This is a significant net deletion. The deleted code's responsibilities are absorbed by `vox-runtime`'s mobile profile + Candle Whisper integration.

---

## §13 `apps/vox-mental-tracker` migration

### §13.1 Current state

`apps/vox-mental-tracker/` today:

- `src/main.vox` (576 LoC of real Vox source) — declares tables, mutations, queries
- `dist/` — Vox-emitted TS
- `web-dist/` — Vite build output
- `capacitor.config.ts` — Capacitor 8 config
- `ios/` — Capacitor-generated iOS project
- `playwright.config.ts`, `vitest.config.ts`, `tests/e2e/voice_flow.spec.ts`
- `package.json` scripts: `build:vox`, `build:fixup`, vite build
- Currently builds as a Capacitor app; native STT bridge is stubbed (per §0 audit)

### §13.2 Phase 1 migration (RN GUI only, no on-device Rust yet)

| Action | Files affected |
|---|---|
| Delete `capacitor.config.ts`, `ios/` (Capacitor-generated), any `@capacitor/*` deps in `package.json` | -1 file, -1 dir, -3 deps |
| Add `app.json` (Expo config) with identifier `com.vox.mentaltracker` | +1 file |
| Add `metro.config.js`, `babel.config.js`, `eas.json` | +3 files |
| Update `package.json`: replace Vite + Capacitor scripts with `expo start`, `eas build` | edit |
| Replace `vite.config.ts` with `metro.config.js` (delete vite.config.ts) | -1, +1 |
| Update `scripts/build.vox` to run `vox build --target=mobile -o src/` | edit |
| Source: regenerate `dist/` → `src/app/*` via Mobile target | regenerate |
| Native STT call sites point to `voxRuntime.transcribeMicrophone()` (stub returns canned text in Phase 1) | edit |
| Playwright tests: replace with [Detox](https://wix.github.io/Detox/) (RN-native e2e) — or defer to Phase 3 | rewrite or defer |

### §13.3 Phase 2 migration (on-device Vox Core + real transcription)

| Action | Files affected |
|---|---|
| Install `@vox/runtime-rn` real impl (not Phase 1 stubs) | edit `package.json` |
| Add `@vox/runtime-rn` to `app.json` plugins array | edit |
| Add Whisper model asset declaration in `app.json` (downloaded on first launch via expo-asset) | edit |
| `src/main.vox`: switch journal storage to `voxRuntime.startWorkflow(…)` for local-first persistence | edit |
| Daily reminder via `voxRuntime.installPushNotifications(…)` + scheduled `@workflow` | edit |
| Delete the stubbed `vox-sherpa-transcribe` plugin entirely | -dir |

### §13.4 LoC budget (mental-tracker)

| Bucket | LoC |
|---|---:|
| Capacitor config + native iOS deletion | -~500 (config + native scaffolding gone) |
| `vox-tauri-stt` deletion (separate crate, see §12) | not counted here |
| New Expo config files (app.json, metro, babel, eas) | +200 |
| Source updates in `src/main.vox` (no breaking syntax changes; runtime calls swap) | +50 (additions only) |
| Test rewrite (Detox or deferred) | +500 if rewritten in Phase 1; +0 if deferred |
| **Phase 1 net delta** | **~-250 to +750** |

---

## §14 Test plan

### §14.1 Test layers

1. **Unit tests** (per-crate): existing `cargo test --workspace` continues to run library-level tests on all codegen modules.
2. **Snapshot tests for codegen output**:
   - Existing: `crates/vox-codegen/tests/golden_ts_test.rs` (web emit).
   - NEW: `crates/vox-codegen/tests/golden_rn_test.rs` (RN emit). Same fixture set, asserts RN output snapshots.
   - Both share the typeck gate (added in the form-label fix; see harness changes from 2026-05-27).
3. **CLI integration tests** (§1): `cargo test -p vox-cli-tests` runs `vox build` end-to-end on all fixtures and asserts:
   - Exit code 0
   - Expected files present
   - TS compiles (`tsc --noEmit`)
   - Rust compiles (`cargo check` on generated)
4. **uniffi-bindgen roundtrip tests**: `crates/vox-runtime-rn/tests/` verifies that each UDL-declared method roundtrips through generated TS bindings without type mismatch.
5. **Native build tests**: A nightly CI job runs `cargo build -p vox-runtime-rn --target aarch64-linux-android --release` for each mobile arch. Failure indicates either toolchain regression or Vox runtime cross-compile issue.
6. **Expo build smoke tests**: A weekly CI job (EAS-hosted) runs `eas build --platform android --profile preview` on a known-good fixture and asserts the APK is produced.
7. **Device smoke tests** (manual): Pixel 6 AVD + iPhone simulator on a Mac runner. Run the mental-tracker preview build, exercise: create entry, background app, kill app, reopen, verify entry persists.

### §14.2 Coverage matrix

| Layer | Web emit | RN emit | uniffi | Mental-tracker |
|---|---|---|---|---|
| Unit | ✓ existing | ✓ NEW | ✓ NEW | n/a |
| Snapshot | ✓ existing | ✓ NEW | n/a | n/a |
| CLI integration | ✓ NEW | ✓ NEW | n/a | n/a |
| Native build | n/a | n/a | ✓ NEW (nightly) | n/a |
| Expo build smoke | n/a | ✓ NEW (weekly) | n/a | ✓ via this test |
| Device smoke | n/a | manual | manual | ✓ Phase 2 gate |

### §14.3 LoC budget (tests)

| File | LoC |
|---|---:|
| `crates/vox-cli-tests/` (§1) | 620 |
| `crates/vox-codegen/tests/golden_rn_test.rs` + fixtures | 200 |
| `crates/vox-runtime/tests/` | 400 |
| `crates/vox-runtime-rn/tests/` | 200 |
| `clients/runtime-web/tests/contract.test.ts` | 100 |
| `clients/runtime-rn/tests/contract.test.ts` | 100 |
| CI workflow updates (.github/workflows) | 200 |
| **Total** | **~1,820** |

---

## §15 Phasing, deliverables, acceptance criteria

### Phase 0 — Preconditions (Week 1-2)

**Deliverables:**

- Fix §0.1 (`main_boot.rs:288` panic). Reproducible test added to `vox-compiler` unit tests.
- Fix §0.2 (struct-literal typeck regression). `wire_format_round_trip` removed from `TYPECK_SKIP`.
- Land §1 (CLI integration test harness with 7 fixtures, tsc+cargo-check validation).
- Land Phase 0 spec deliverables from [the architecture doc](mobile-rn-expo-architecture-and-migration-2026.md):
  - VUV-style IR spec (becomes basis for §4).
  - mobile runtime profile spec (becomes basis for §10).
  - NativeWind-vs-Tamagui-vs-StyleSheet decision spike (resolves §4.3 deferred).
- Author ADR-NNN scoping Tauri to desktop ([draft already at adr-NNN-scope-tauri-desktop-only.md](adr-NNN-scope-tauri-desktop-only.md)). Status: Proposed → Accepted at end of Phase 0.

**Acceptance:**

- `cargo run -p vox-cli -- build examples/golden-ts/component_state.vox -o /tmp/dist` exits 0 and produces a `Counter.tsx` that compiles via `tsc --noEmit`.
- `cargo test -p vox-cli-tests` green on all 7 fixtures.
- ADR-NNN status changed to Accepted.

**LoC delivered:** ~700 (mostly tests + the bug fixes).

### Phase 1 — Web-emit refactor + RN scaffolding (Week 3-6)

**Deliverables:**

- VUV-style IR (§4): `style_ir.rs`, `lower_style.rs`, `emit_web_dom.rs`. Existing snapshots stay byte-equal.
- `BuildTarget::Mobile` (§5): enum variant + CLI flag + new build branch in `vox-cli`.
- `codegen_ts/rn/` (§6) skeleton: enough to emit a working "hello world" RN app from a `.vox` source with one component + one route + one form.
- `clients/runtime-web/` and `clients/runtime-rn/` packages (§8, §9) — Phase 1: `runtime-web` is real; `runtime-rn` stubs the Rust-backed methods (returns canned values).
- Mobile build artifacts (§7): app.json, metro.config.js, eas.json scaffolding.
- `mobile_emit.rs` refactor to adapter (§3.4): both lowerings emit `@vox/runtime` calls.

**Acceptance:**

- A fresh `.vox` source with `component HomeScreen() { view: column() { ... } }` builds with `vox build --target=mobile` and reaches Expo Go on the Android emulator.
- All existing web snapshots are byte-equal.
- `golden_rn_test.rs` snapshots exist for all 12 existing goldens.

**LoC delivered:** ~4,500 (the bulk of the RN lowering + IR refactor).

### Phase 2 — uniffi + Vox Core on device (Week 7-12)

**Deliverables:**

- `crates/vox-runtime/` umbrella crate (§10).
- `crates/vox-runtime-rn/` + uniffi UDL (§11).
- Cross-compile config for the four mobile architectures.
- `clients/runtime-rn/` real implementation (uniffi-backed).
- Expo config plugin links the `.a`/`.so` into the consuming app's native projects at prebuild time.
- Mental-tracker (§13 Phase 2) uses on-device durable journal for entries.

**Acceptance:**

- A mental-tracker preview build installs on Android emulator AND iOS simulator (latter via a one-shot EAS Build).
- Create an entry → force-kill the app → reopen → entry persists. Entirely offline.
- `voxRuntime.transcribeMicrophone()` returns real Whisper output (not canned).

**LoC delivered:** ~4,800 (vox-runtime + vox-runtime-rn + uniffi UDL + Expo plugin + tests).

### Phase 3 — Polish + retirement (Week 13-15)

**Deliverables:**

- `crates/vox-tauri-stt/` deleted (§12.3).
- `crates/vox-codegen/src/codegen_ts/mobile_emit.rs` finalized: no `@tauri-apps/api` references in either lowering.
- Tauri-mobile-specific projections in `vox-tauri-codegen` removed.
- Docs: `docs/how-to/build-android.md`, `docs/how-to/build-ios.md` (new).
- Tutorial: `docs/tutorials/build-a-mobile-app.md`.
- Mental-tracker README updated, Vox.toml keywords updated.

**Acceptance:**

- A developer following the new docs can build a Vox mobile app in < 1 day on a fresh Windows machine, with no prior Vox knowledge.

**LoC delivered:** ~-6,000 (deletion) + ~800 docs.

### Phase 4 — Codegen SSOT integration (Week 16-20)

**Deliverables:**

- Per [Codegen SSOT Unification 2026](codegen-ssot-unification-design-2026.md): VUV-style IR formally adopted by both translators. The branch in `emitter.rs` (§6.2) collapses to a dispatch table.
- `@vox/runtime` and `@vox/runtime-rn` interface types are published from a shared `clients/runtime-types/` package consumed by both.
- Single golden-test harness validates BOTH lowerings per `.vox` file (one new snapshot, two snap files).

**Acceptance:**

- Adding a new VUV primitive (e.g. `text-input`) requires ≤ 50 LoC across both targets, validated by a stopwatch experiment during Phase 4.
- All web + RN snapshots remain byte-equal across the refactor (regression gate).

**LoC delivered:** net-neutral refactor; ~500 LoC of new test infra.

### Cumulative LoC budget

| Phase | Net delta LoC |
|---:|---:|
| Phase 0 | +700 |
| Phase 1 | +4,500 |
| Phase 2 | +4,800 |
| Phase 3 | -5,200 (mostly deletion) |
| Phase 4 | +500 |
| **Cumulative** | **+5,300 new + ~9,000 refactored + ~6,200 deleted** |

This is in the 5-10K-LoC range originally estimated. The deletions (vox-tauri-stt, mental-tracker Capacitor scaffold) offset the new code meaningfully.

---

## §16 Open decisions and risks

### §16.1 Open decisions (must resolve in Phase 0 or fail the phase)

1. **NativeWind vs Tamagui vs raw StyleSheet** (§4.3). Recommended: **NativeWind** for class-string reuse with the web emit (one design system, one set of class strings, web and RN agree). Cost: adds NativeWind as a hard dep of the RN target.
2. **Whisper model selection and quantization**. Candidates: `whisper-small` Q4_K_M (~150MB), `whisper-base` Q4_K_M (~70MB), `distil-whisper-small` (~120MB, faster). Recommended: **distil-whisper-small** for the mental-tracker default; let users override via `Vox.toml`.
3. **Workspace structure for `clients/`**. New top-level dir vs `pkgs/` vs `npm/`. No prior precedent in repo; recommend `clients/` for consistency with prior SSOT-plan naming.
4. **Expo SDK version pin policy.** Recommended: pin to the latest stable Expo SDK at each Vox release; auto-bump via `expo install` in `vox build --target=mobile`. Document in Phase 3 release notes.
5. **Detox vs Playwright vs Expo's own e2e for mental-tracker tests** (§13.2). Recommended: **defer Detox to Phase 3**; Phase 1 ships without device-level e2e (CLI tests + manual smoke are sufficient).

### §16.2 Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| uniffi-bindgen-react-native stalls or has a breaking change | Medium | High | Mozilla actively maintains uniffi-rs (production use at scale); pin uniffi-bindgen-react-native version per Vox release; have a fallback design using JSI bindings hand-written for the top-5 methods (~500 LoC escape hatch) |
| Phase 1 §4 refactor breaks existing web snapshots | High (refactor risk) | Medium | Byte-equal snapshot assertion as the gate; do not delete old emit path until both produce identical bytes for all 12 goldens |
| Cross-compile toolchain breaks on Windows | High | Medium | EAS Build does cross-compile in CI on managed Linux + macOS hosts; document local dev as Linux-or-macOS-preferred but Windows-workable via `cargo-ndk` |
| Expo SDK breaking change between Phase 1 and Phase 2 | Medium | Medium | Pin Expo SDK version in `package.json` template; treat upgrade as its own ticket |
| Mental-tracker user-visible regression during migration | Medium | Low (small user base) | Phase 1 ships the new build alongside Capacitor (parallel) for 1 sprint; switch default after acceptance criteria pass |
| iOS App Store size limits exceeded by Candle model + multi-arch Rust | Low | Medium | Expo's `expo-asset` downloads Candle models on first launch (not bundled); per-arch app thinning is automatic |
| `@vox/runtime-rn` Expo config plugin breaks on Expo prebuild ejection edge case | Low | Low | Expo's config-plugin API is stable; provide a manual fallback README for ejected apps |

### §16.3 Tracked-for-future-revisit

- **RN-desktop unification** (react-native-windows / macos / linux). Deferred per [the architecture doc §Tauri question](mobile-rn-expo-architecture-and-migration-2026.md). Quarterly maturity tracker doc at `docs/src/architecture/rn-tauri-mobile-maturity-tracker.md`; formal revisit Q3 2027.
- **WASM build of vox-runtime** for browser-only Vox apps. Not in scope; separate research project.
- **HealthKit, Apple Sign-in, WidgetKit, Live Activities** as Vox primitives. Defer until recurring user need; add per-app as Expo Modules in the meantime.

---

## §17 Cross-references

- Decision rationale: [mobile-target-evaluation-2026.md](mobile-target-evaluation-2026.md)
- Architecture overview: [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md)
- ADR scoping Tauri to desktop: [adr-NNN-scope-tauri-desktop-only.md](adr-NNN-scope-tauri-desktop-only.md)
- Absorbing into emit-unification: [codegen-ssot-unification-design-2026.md](codegen-ssot-unification-design-2026.md)
- The existing migration plan that this supersedes for Phase 5+: [tauri-convergence-migration-plan-2026.md](tauri-convergence-migration-plan-2026.md)

---

## §18 Acceptance criteria summary

This spec is complete and implementable when the reviewer can answer YES to all of:

- [ ] Every section §1-§13 has a file-by-file path list with rough LoC budget.
- [ ] No section references a stub or "TBD" implementation.
- [ ] Every Phase in §15 has at least one acceptance criterion measurable by automated test.
- [ ] The cumulative LoC budget (§15) is internally consistent with the per-section budgets.
- [ ] The two known toolchain bugs (§0.1, §0.2) have explicit fix protocols.
- [ ] The CLI integration test harness (§1) is specified deeply enough to be implemented without further design work.
- [ ] Every "open decision" in §16.1 has a default recommendation that allows Phase 0 to proceed if no other input arrives.
