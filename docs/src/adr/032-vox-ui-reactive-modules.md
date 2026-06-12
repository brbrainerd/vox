---
title: "ADR 032 — `.vox.ui` reactive modules"
description: "Introduces a `.vox.ui` file-suffix convention that allows module-scope `state` / `derived` / `effect` / `on mount` / `on cleanup` reactive members. Lowers to a generated React context+provider+hook in TSX emit. Addresses the gap that today's reactive members live only inside `component { }` blocks, with no story for shared state across components."
category: "Architecture Decisions (ADRs)"
status: "current"
last_updated: "2026-05-03"
training_eligible: true
schema_type: "TechArticle"
---
# ADR 032: `.vox.ui` reactive modules

## Status

Accepted (2026-05-03). Phase D shipped end-to-end across four commits in the same session:

- `FileKind::from_path` helper at [crates/vox-compiler/src/module.rs](../../../crates/vox-compiler/src/module.rs) (commit `954ad8775`).
- `parse_with_kind` parser entry, `Parser.file_kind` field, `Decl::ReactiveModule(ReactiveModuleDecl)` AST variant, and the parser dispatch that absorbs module-scope `state` / `derived` / `effect` / `on mount` / `on cleanup` into a synthetic `ReactiveModuleDecl` (commit `26c90f9be`).
- `HirReactiveModule` HIR node + AST→HIR lowering in [hir/lower/mod.rs](../../../crates/vox-compiler/src/hir/lower/mod.rs) and emit at [codegen_ts/reactive_module_emit.rs](../../../crates/vox-codegen-ts/src/reactive_module_emit.rs) producing one `<Name>Provider.tsx` per module (typed `Value` interface + Context + Provider + `use<Name>()` hook), wired into `emitter::generate` (commit `a16417db0`).

**Remaining sub-slices for a future cycle:**

- CLI build entry points should pass the source-file basename through `parse_with_kind` so the parser fills `ReactiveModuleDecl.name` (today the codegen falls back to a `ReactiveModule<index>` default per the §"Module name" note).
- Cross-module reactive imports — recognizing imports from `.vox.ui` modules as reactive bindings inside the Phase E dep analyzer (§"Read-tracking interaction"). The analyzer already supports per-module `state_names`; the wiring is "include `.vox.ui` exports in the visible-bindings set."

## Context

Vox today supports reactive members (`state name: T = init`, `derived name = expr`, `effect: { … }`, `on mount: { … }`, `on cleanup: { … }`) inside `component { }` blocks only. The grammar accepts these tokens at top level inside a `ReactiveComponentDecl` member list ([crates/vox-compiler/src/parser/descent/decl/head.rs:253–334](../../../crates/vox-compiler/src/parser/descent/decl/head.rs)) and lowers them to React `useState` / `useMemo` / `useEffect` ([crates/vox-codegen-ts/src/reactive/mod.rs:740–815](../../../crates/vox-codegen-ts/src/reactive/mod.rs)). A working golden lives at [examples/golden/reactive_counter.vox](../../../examples/golden/reactive_counter.vox).

There is **no module-scope analog**. Authors who need shared state across multiple components must either:

1. Lift the state into a parent component and prop-drill (verbose, fragile against refactors).
2. Drop into hand-written React `useContext` via the React-hook bridge ([crates/vox-compiler/src/react_bridge.rs](../../../crates/vox-compiler/src/react_bridge.rs)) (loses the reactive-member ergonomics).
3. Pull in a third-party state-management library (out-of-scope for the Vox idiom).

Comparator framework: Svelte 6 solves the same problem with `.svelte.ts` files ("universal reactivity" — runes work outside components). Detailed competitive analysis in the [Svelte vs React frameworks research doc](../architecture/svelte-vs-react-frameworks-research-2026.md).

## Decision

Introduce a **`.vox.ui` file-suffix convention** that authorizes module-scope reactive members and lowers them to a generated React context provider + a typed `use<Name>()` hook.

### Suffix and scope

- **Suffix:** `.vox.ui` (matches the precedent of suffixed file conventions established by `.generated.md` and `.voxignore`-derived `.cursorignore` / `.aiignore` / `.aiexclude` files documented in [AGENTS.md §Auto-generated documentation files](../../../AGENTS.md)).
- **Allowed top-level decls in `.vox.ui` files:** all decls legal in regular `.vox` files (`type`, `fn`, `component`, `routes`, `state_machine`, etc.) **plus** module-scope reactive members: `state`, `derived`, `effect`, `on mount`, `on cleanup`.
- **Disallowed in regular `.vox` files:** module-scope reactive members. The current parser already enforces this implicitly (the tokens are only accepted inside `finish_reactive_component_after_name`); the rule becomes explicit and surfaced as a diagnostic.

### Lowering

Each `.vox.ui` file emits one TSX module exporting:

1. A typed React `Context` whose value is the module's reactive bindings.
2. A `<NameProvider>` component that owns the underlying `useState` / `useMemo` / `useEffect` calls (mirroring the existing reactive-component lowering at [reactive.rs:740–815](../../../crates/vox-codegen-ts/src/reactive/mod.rs)).
3. A `useName()` hook that consumes the context (typed against the module's exported reactive bindings).

Where `Name` is derived from the file basename (e.g., `counter.vox.ui` → `CounterProvider` + `useCounter()`).

Cross-module imports:

```text
// vox:skip — illustrative
// counter.vox.ui
state count: int = 0
derived double = count * 2

// app.vox
import { count, double } from "./counter.vox.ui"

component App() {
  view: <p>"count = {count}, ×2 = {double}"</p>
}
```

In TSX emit, the import desugars to a `useCounter()` call inside the consuming component. Read-tracking analysis ([state_deps.rs](../../../crates/vox-codegen-ts/src/hir_emit/state_deps.rs)) must learn that imports from `.vox.ui` modules produce reactive bindings (Phase E ties this in).

### File-discovery wire-up

The current Vox toolchain has **no single dispatch point for file extensions** — CLI commands accept paths directly without extension validation. This ADR commits to extending the following entry points to recognize the `.vox.ui` suffix and select the reactive-module parser variant:

- [`crates/vox-cli/src/commands/build.rs`](../../../crates/vox-cli/src/commands/build.rs) — `run()` / `run_frontend()`
- [`crates/vox-cli/src/commands/check.rs`](../../../crates/vox-cli/src/commands/check.rs)
- [`crates/vox-cli/src/commands/dev.rs`](../../../crates/vox-cli/src/commands/dev.rs) — `vox-compilerd` JSON-RPC dev daemon
- [`crates/vox-cli/src/commands/mcp_server/`](../../../crates/vox-cli/src/commands/mcp_server/) — relevant for `vox_validate_file` / `vox_validate_source` source discrimination
- [`crates/vox-lsp/src/lib.rs`](../../../crates/vox-lsp/src/lib.rs) — LSP file-type detection

Implementation strategy: **introduce a single `vox_compiler::module::FileKind::from_path(path)` helper** that all entry points call, instead of duplicating extension-matching logic. The helper returns `FileKind::Source | FileKind::ReactiveModule | FileKind::Unknown`, and downstream code branches on the kind.

### Read-tracking interaction

Reactive bindings imported from a `.vox.ui` module must be visible to the auto-dep inference pass ([state_deps.rs](../../../crates/vox-codegen-ts/src/hir_emit/state_deps.rs)) when the consuming component declares `derived` or `effect` that reference them. The `extract_state_deps()` walker's `state_names` set must include the imported bindings; the loader needs to emit those imports as part of the reactive-binding namespace.

This is a hard dependency between Phase D (this ADR) and Phase E (cross-call dep inference). Phase D landing first means Phase E can import-aware-track from day one; Phase E landing first means Phase D can wire imports into the existing analyzer.

### Versioning

`.vox.ui` is added at Vox 0.5.x. No deprecation of any existing surface. Regular `.vox` files continue to behave exactly as before.

## Alternatives considered

1. **Allow module-scope reactive members in regular `.vox` files (no suffix).** Rejected: makes the `.vox` grammar context-sensitive (a module-level `state count = 0` would mean different things depending on whether the module is consumed by a `component`). The suffix is a load-bearing signal that the file participates in the React-context lifecycle.

2. **Use a `module reactive { … }` block instead of a file-suffix convention.** Rejected: per [AGENTS.md §Grammar Unification](../../../AGENTS.md), new bare keywords are reserved for genuinely new scope semantics. A reactive module *file* is a packaging concept, not a new scope kind. Adding a `module reactive` keyword would fight the grammar policy.

3. **Make module-scope reactive members opt-in via a top-of-file pragma** (e.g., `#![reactive_module]`). Rejected: pragmas are not currently part of the Vox grammar; introducing them for one feature is a worse precedent than a file-suffix convention.

4. **Borrow `.svelte.ts` exactly.** Rejected: the `.ts` substring would make the suffix ambiguous against a future `.vox.ts` interop story. `.vox.ui` is unambiguous.

## Consequences

### Positive

- Authors can express shared reactive state without lifting it into a parent component or pulling a third-party state library.
- Matches Svelte 6's `.svelte.ts` model — familiar to developers coming from that ecosystem.
- Centralizes file-extension dispatch into one helper, paying down a tech-debt item (no current single dispatch point).
- Consuming components remain plain `.vox` files with a normal `import` statement — the seam is honest.

### Negative

- Adds a new file kind to learn (small docs cost).
- Cross-module reactive read-tracking ties Phase D and Phase E together more tightly than the original implementation plan suggested.
- The generated TSX includes a `<NameProvider>` per `.vox.ui` module; large apps with many reactive modules will accumulate provider nesting. Mitigation: emit a single composed root provider when multiple modules are imported.

### Neutral

- No effect on the React-hook bridge ([react_bridge.rs](../../../crates/vox-compiler/src/react_bridge.rs)) — bridge stays as escape hatch.
- No effect on the [Phase 5 React interop spec](../archive/phase5-react-interop-spec-2026.md) or [external frontend interop plan](../architecture/external-frontend-interop-plan-2026.md). `.vox.ui` modules emit ordinary React; they're consumable by external React code via the same Phase 5 npm-publishing path.

## Implementation references

Concrete file changes documented in the [Svelte-Mineable Features Implementation Plan §Phase D](../architecture/svelte-mineable-features-implementation-plan-2026.md). Summary:

- New AST node `ReactiveModule` at [crates/vox-ast/src/decl/ui.rs](../../../crates/vox-ast/src/decl/ui.rs).
- Parser change at [crates/vox-compiler/src/parser/descent/mod.rs](../../../crates/vox-compiler/src/parser/descent/mod.rs) gated on `FileKind::ReactiveModule`.
- New `vox_compiler::module::FileKind::from_path(path)` helper.
- Codegen extension at [crates/vox-codegen-ts/src/reactive/mod.rs](../../../crates/vox-codegen-ts/src/reactive/mod.rs) emitting the provider+hook pair.
- Goldens at `examples/golden/counter.vox.ui` and `examples/golden/counter_consumer.vox`.

## Related

- [External Frontend Interop Plan](../architecture/external-frontend-interop-plan-2026.md)
- [Phase 5: Bidirectional Vox↔React Interop Spec](../archive/phase5-react-interop-spec-2026.md)
- [Vox GUI-Native Language Roadmap](../architecture/vox-gui-native-roadmap-2026.md)
- [Svelte vs React Frameworks Research](../architecture/svelte-vs-react-frameworks-research-2026.md)
- [Svelte-Mineable Features Implementation Plan](../architecture/svelte-mineable-features-implementation-plan-2026.md)
- [ADR 027: Dual-track UI surfaces](027-dual-track-ui-surfaces.md)
- [AGENTS.md §Grammar Unification](../../../AGENTS.md)
