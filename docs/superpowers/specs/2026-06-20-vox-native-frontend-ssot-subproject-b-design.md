---
title: "Vox-Native Frontend SSOT — Sub-project B: reactive vox:// stream subscription primitive"
category: "Architecture SSOTs"
status: design
date: 2026-06-20
---

# Sub-project B — Reactive Stream Subscription Primitive

> Part of the Vox-Native Frontend SSOT program
> (`docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md`). Sub-project A
> (the `Target` emission seam + coverage ledger) is merged to `main` at `4486ec1f48`.
> This is the **make-or-break** sub-project: 10 of 25 GUI surfaces are
> `blocked:reactive-streams` in the coverage ledger.

## Problem (audit-grounded, file:line verified)

The frontend authoring substrate is healthier than the program's initial audit claimed.
**Correction of record:** `state`, `derived`, `effect depends_on(a, b):`, `on mount:`, and
`on cleanup:` are **all fully parsed, lowered, and emitted** — they are *not* dead code:

- Parser surfaces every one of them:
  `crates/vox-compiler/src/parser/descent/decl/head_component.rs:61-148` (component scope) and
  `:200-294` (module scope `.vox.ui`).
- Emit is real: `crates/vox-codegen-ts/src/reactive/effects.rs:161-218` emits `useState`,
  `useMemo` (with dependency inference via `extract_state_deps_with_diagnostics`),
  `useEffect(…, [deps])`, `useEffect(…, [])` for mount, and `useEffect(() => () => {…}, [])`
  for cleanup.
- WebIR lowering is real: `crates/vox-codegen/src/web_ir/lower.rs:826-868` maps each member to
  a `BehaviorNode` (`StateDecl`/`DerivedDecl`/`EffectDecl`).
- Supporting lints exist: `effect_deps_lint.rs`, `stale_capture_lint.rs`, `async_handler_lint.rs`.

The **actual** gap is narrow and clean: there is no primitive to **subscribe to an external,
named, push event channel**. The 13 `vox://*` streams are hand-defined in
`crates/vox-gui/ui/src/transport.ts` (`ORCH_STATUS_EVENT = 'vox://orch-status'`, etc.), each
hard-coupled to `@tauri-apps/api/event`'s `listen()`, and each manually wired in a React
surface as `listen(cb) → Promise<UnlistenFn> → .catch(pollingFallback) → return () =>
unlisten()` (canonical example: `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts:97-129`).

Nothing in `.vox` can express: *"subscribe to `vox://orch-status`, typed payload, fold each
frame into state, auto-unsubscribe on unmount, and degrade gracefully when the transport is
absent."* The eager `@tauri-apps` import is also the root of the known bare-browser
`__TAURI_INTERNALS__` crash (it executes at module load).

## What already exists and is reused

- **Producer-side streaming:** `@endpoint(kind: stream)` → Server-Sent Events at
  `/api/stream/<name>` (`crates/vox-compiler/src/web_prefixes.rs:12-14`). This is the *server
  emitting* a stream, not a *client subscribing* to a daemon channel — different direction, but
  the SSE transport shape is a reference for the browser transport seam (deferred).
- **Reactive emit substrate:** `reactive/effects.rs` — the `on stream` member emits through the
  same `useEffect` machinery (an effect whose body subscribes and whose cleanup unsubscribes).
- **Contract + parity-gate pattern:** `tool-registry.canonical.yaml` + its CI parity gate is the
  template for `channels.v1.yaml` + a channel parity gate.
- **Sub-project A seam:** `vox_codegen::frontend_backend::emit_frontend(Target, hir, opts)` —
  the channel-runtime emit is reached only on `Target::TypeScript`; other targets are unaffected.

## Locked decisions (from brainstorming, 2026-06-20)

1. **Scope = full.** B ships the `.vox` primitive **and** a transport-neutral channel runtime
   **and** a compiler-visible channel registry. Emitted frontend code never imports
   `@tauri-apps` directly.
2. **Syntax = `on stream(channel) as binding: { body }`** — an `on`-family reactive member,
   consistent with `on mount:` / `on cleanup:`. Folds uniformly over both *replace* semantics
   (orch-status: latest snapshot wins) and *fold* semantics (agent-events: append; scientia-queue:
   "changed → refetch").
3. **Registry SSOT = `contracts/channels.v1.yaml`** — language-agnostic, parity-gated like
   `tool-registry.canonical.yaml`. Honors the Rust origin of payloads (daemon emits them) while
   giving the compiler a machine-readable source for name → payload type → wire URI → fallback.
4. **Fallback = runtime-transparent via contract.** Each channel may declare
   `poll: <refetch command>` + `every: <interval>`. The neutral runtime subscribes when the
   transport is live and transparently polls that command when it is not. `.vox` authors write
   only `on stream(...)` — zero fallback boilerplate.
5. **Browser transport = interface + Tauri impl + dev-mock (this sub-project).** B defines the
   channel-transport interface, ships the Tauri impl (wrapping today's `listen()`) and a dev-mock
   impl, and uses **guarded lazy resolution** so a bare browser degrades to mock/poll instead of
   crashing. The production browser WS/SSE gateway transport is a **declared seam** consumed by a
   later sub-project; it is explicitly out of scope here.

## Architecture & data flow

```
.vox:  on stream(orch_status) as s: { status = s }
   │
 parser → ReactiveMemberDecl::OnStream { channel: "orch_status", binding: "s", body }   (new)
   │
 HIR    → HirReactiveMember::OnStream { channel, binding, body }                          (new)
   │
 lower  → BehaviorNode::StreamSub { channel, binding, body }                              (new)
   │
 emit (Target::TypeScript only) →
     useEffect(() => {
       let unsub: (() => void) | undefined;
       let cancelled = false;
       voxChannel.subscribe("orch_status", (s) => { /* body: status = s → setStatus(s) */ })
         .then((u) => { if (cancelled) u(); else unsub = u; });
       return () => { cancelled = true; unsub?.(); };
     }, []);
```

The channel name is validated at compile time against `channels.v1.yaml`; the binding `s` is
typed from the channel's declared payload type. The emitted module imports the generated
`voxChannel` runtime, never `@tauri-apps`.

### Components (clear boundaries, independently testable)

| # | Component | Responsibility | Crate / path |
|---|---|---|---|
| 1 | `on stream` grammar | Parse `on stream(name) as bind: <block>` → AST `OnStream` member | `vox-compiler` parser (`head_component.rs`, both component + module scope) |
| 2 | AST/HIR nodes | `ReactiveMemberDecl::OnStream`, `HirReactiveMember::OnStream`, HIR lowering | `vox-compiler` ast/hir |
| 3 | WebIR lowering | `HirReactiveMember::OnStream` → `BehaviorNode::StreamSub` | `vox-codegen` `web_ir/lower.rs` |
| 4 | Channel contract + loader | `contracts/channels.v1.yaml` SSOT + a typed reader | `contracts/` + `vox-compiler` (or `vox-codegen`) loader |
| 5 | Channel runtime emit | Generate `vox-channel.ts` (transport interface + Tauri impl + dev-mock + fallback + lazy guard) and the typed channel map | `vox-codegen-ts` |
| 6 | Validation + parity gate | Compile-time: unknown channel name = diagnostic; binding typed from contract. CI: contract ↔ daemon emitter names parity | `vox-compiler` typeck + `vox-cli ci` |

### Channel contract schema (`contracts/channels.v1.yaml`)

```yaml
schema_version: 1
channels:
  - name: orch_status                 # the .vox-facing identifier (snake_case)
    uri: "vox://orch-status"          # wire event name (matches transport.ts today)
    payload: OrchestratorStatus       # type name; mapped to a TS type in generated runtime
    semantics: replace                # replace | fold  (advisory; both emit the same effect)
    poll:                             # optional runtime-transparent fallback
      command: get_orchestrator_status
      every_ms: 5000
  - name: agent_events
    uri: "vox://agent-events"
    payload: AgentEventFrame
    semantics: fold
    # no poll: → when transport absent, runtime no-ops (append stream has no snapshot refetch)
  # … all 13 channels enumerated …
```

The channels (from `transport.ts`): `orch_status`, `agent_events`, `scientia_queue`,
`scientia_discovery_surfaced`, `browser_frame`, `preview_available`, `secretary_proposed`,
`pty_output`, `pty_exit` are the explicit event constants today (9). The program's audit cited
"13 `vox://*` streams"; the discrepancy is unreconciled. **A plan task must read `transport.ts`
as current-truth, enumerate the exact set, and reconcile the count 1:1** — the contract enumerates
whatever is actually wired, not a remembered number.

### Neutral channel runtime (generated `vox-channel.ts`)

```ts
export interface VoxChannelTransport {
  subscribe(uri: string, onFrame: (raw: unknown) => void): Promise<() => void>;
}
// Guarded lazy resolution — NO eager @tauri-apps import at module load.
function resolveTransport(): VoxChannelTransport { /* tauri if __TAURI_INTERNALS__, else mock */ }
export const voxChannel = {
  subscribe<K extends keyof ChannelMap>(name: K, onFrame: (p: ChannelMap[K]) => void): Promise<() => void>
};
```

- **Tauri transport:** dynamically imports `@tauri-apps/api/event` and wraps `listen()`. Only
  reached when `__TAURI_INTERNALS__` is present.
- **Dev-mock transport:** returns an immediately-resolved no-op unsubscribe; optionally replays
  fixture frames in tests. Guarantees bare-browser never throws.
- **Fallback:** if `subscribe` rejects/transport is mock and the channel declares `poll:`, the
  runtime starts the interval refetch transparently.

## Error handling

- **Unknown channel name** in `on stream(foo)` → typed compile diagnostic
  (`vox/web/unknown-channel`) listing valid names. Mirrors existing classified parse/typeck errors.
- **Binding type:** `s` is typed from the contract's `payload`. If the payload type is unknown to
  the type system, emit `any` + a diagnostic (same posture as `map_hir_type_to_ts` fallbacks).
- **Transport absent at runtime:** guarded resolver → mock/poll, never an uncaught throw. This is
  the bare-browser crash fix.
- **Parity drift:** CI gate fails if `channels.v1.yaml` and the daemon's emitter names diverge.

## Testing strategy

- **Parser:** `on stream(c) as s: { … }` round-trips to `OnStream` at component and module scope;
  malformed forms (`on stream` without `(`, missing `as`, unknown trailing token) produce
  classified errors. (`parser/descent/tests.rs` pattern.)
- **Lowering:** `HirReactiveMember::OnStream` → `BehaviorNode::StreamSub` (unit test on the
  lowering, mirroring the `const_emit_test.rs` HIR pattern
  `lower_module(&parse(lex(src)).expect("parse"))`).
- **Emit (golden):** a component with `on stream` emits a `useEffect` that calls
  `voxChannel.subscribe(...)` and returns an unsubscribe cleanup; asserts **no**
  `@tauri-apps/api/event` import appears in component output.
- **Runtime emit:** generated `vox-channel.ts` contains the transport interface, the typed
  `ChannelMap`, the guarded resolver, and a poll branch for channels declaring `poll:`. Assert no
  top-level `@tauri-apps` import.
- **Contract validation:** unknown channel name → `vox/web/unknown-channel` diagnostic.
- **Parity gate:** a `vox-cli` test asserting the gate fires when a channel is added to the
  contract but missing from the daemon emitter set (and vice-versa).
- **Coverage-ledger impact:** flipping at least one surface (e.g. Dashboard or Tasks) from
  `blocked:reactive-streams` toward `expressible` is the acceptance signal; the ledger +
  drift-guard from Sub-project A are updated as surfaces convert (most conversion is Sub-project G,
  but B should demonstrate one end-to-end).

## Explicitly out of scope (deferred)

- **Production browser WS/SSE gateway transport** (lighting up the dark daemon gateway) — declared
  seam only; later sub-project.
- **Migrating the 173 `.tsx` surfaces** to `.vox` — Sub-project G.
- **Ecosystem-import completion** (Sub-project C), **mobile-first rule/PWA** (D), **toolchain
  automation** (E), **convergence CI gate** (F).
- Changing `@endpoint(kind: stream)` producer-side SSE behavior.

## Acceptance (Definition of Done for B)

1. `on stream(channel) as s: { … }` parses (component + module scope) with classified errors on
   malformed forms.
2. It lowers HIR → WebIR `StreamSub` and emits a `useEffect` subscribe/unsubscribe through the
   generated `voxChannel` runtime, with **zero** direct `@tauri-apps` import in emitted component
   or runtime module load path.
3. `contracts/channels.v1.yaml` enumerates all 13 channels; unknown-channel use is a compile
   diagnostic; bindings are typed from the contract.
4. The generated runtime degrades transparently (mock + contract `poll:`), provably not crashing
   without Tauri.
5. A CI parity gate guards contract ↔ daemon emitter-name drift.
6. At least one ledger surface is demonstrated end-to-end as `.vox`-expressible via `on stream`.
7. Full green: new parser/lowering/emit/contract/parity tests + existing `vox-codegen` /
   `vox-compiler` suites; `cargo clippy` clean on touched crates.
