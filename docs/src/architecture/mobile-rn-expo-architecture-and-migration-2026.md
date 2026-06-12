---
title: "Mobile architecture and migration plan — RN + Expo + uniffi (2026)"
description: "Mobile platform target evaluation: React Native + Expo + uniffi bridging strategy for Vox apps."
category: "Architecture SSOTs"
status: research
last_updated: "2026-05-27"
authors: [vox-team]
related:
  - mobile-target-evaluation-2026.md (decision rationale)
  - adr-037 (Tauri canonical for desktop + mobile — to be scoped to desktop-only)
  - codegen-ssot-unification-design-2026
  - tauri-convergence-migration-plan-2026 (Phase 5+ to be rewritten against this doc)
  - external-frontend-interop-plan-2026
---

# Mobile architecture and migration plan — RN + Expo + uniffi (2026)

> The decision (see [mobile-target-evaluation-2026.md](mobile-target-evaluation-2026.md)) is React Native + Expo for mobile, Tauri 2 for desktop. This doc specifies HOW: the codegen architecture, the Rust-on-mobile path, the migration phases, and the tech-debt minimization strategy that keeps "one VUV syntax → all devices" honest as Vox grows.

## TL;DR

1. **One VUV HIR, two GUI lowerings.** VUV stays the single GUI syntax. The TS emitter grows a second lowering — `codegen_ts/rn/` — that translates the same HIR to React Native + Expo Router + StyleSheet instead of React DOM + Tailwind. The existing web lowering is unchanged.
2. **Same Rust runtime on every device.** `vox-runtime` (workflows, actors, MENS inference) cross-compiles to `aarch64-apple-ios` / `aarch64-linux-android` / etc. and ships to the phone as a TurboModule via [uniffi-bindgen-react-native](https://github.com/jhugman/uniffi-bindgen-react-native) (Mozilla, production-proven). Desktop keeps linking the same crate directly via Tauri's existing IPC.
3. **Tauri stays for desktop only.** The desktop story is mature and serves Vox well. Tauri-mobile retires (revise ADR 037). Capacitor never lands.
4. **Tech debt is minimized by structure, not discipline.** A canonical VUV-style IR + a single `@vox/runtime` JS API with two implementations (Tauri-flavored + Expo-flavored) means new VUV features ship in ONE place. The "twice" surface is contained to: element-to-component mapping (table), style translation (mechanical walker), and platform-specific runtime adapters.
5. **vox-mental-tracker is the proving ground.** Phase 1 ships mental-tracker as RN+Expo using only the new GUI lowering. Phase 2 adds the uniffi-wrapped Vox Core for on-device durable journaling + Whisper transcription.

## North star

> One VUV syntax → desktop and mobile, with the full Vox runtime (workflows, actors, on-device ML) available on both, and per-feature engineering cost that doesn't scale with the number of target platforms.

## Non-goals

- Replacing the existing Rust emit. The TS+Rust split stays.
- Forcing Vox apps to ship a backend. Mental-tracker is local-first; no Axum server.
- Linux desktop unification on RN. Tracked as a deferred review, not in-scope.
- Cross-compiling Vox Core to WebAssembly for the browser. Possible but out of scope.
- Picking specific iOS / Android frameworks beyond what Expo bundles (HealthKit, BLE, etc.) — those land per-app, not as Vox primitives, until proven necessary.

## Why this architecture (one paragraph)

Vox's value to the user is "AI writes a real app and a real app means it works on the device its users have." Every architecture decision here serves that — the RN lowering exists because it's the only path to native widgets that the LLM ecosystem can author well; the uniffi path exists because Vox's distinguishing features (durable workflows, actors, MENS inference) are Rust and we want them on the phone too; Tauri stays for desktop because it works and replacing it would be undirected work that doesn't change what users can build.

---

## Current state audit

| Component | Today | After this plan |
|---|---|---|
| `vox build` default | TS/React (DOM + Tailwind) + Rust/Axum | TS/React (web) OR TS/RN (mobile) + Rust/Axum |
| `vox build --target` | `fullstack` / `server` / `client` | adds `mobile` (TS/RN + uniffi-wrapped vox-runtime) |
| Desktop GUI | [crates/vox-gui](../../../crates/vox-gui) on Tauri 2 | unchanged |
| Mobile GUI | `crates/vox-codegen-ts/src/mobile_emit.rs` emits `@tauri-apps/api/event` | emits `@vox/runtime` calls (adapter-shaped) |
| Mobile app of record | `apps/vox-mental-tracker` (planned) on Capacitor | on Expo (managed workflow) |
| Speech-to-text | `vox-sherpa-transcribe` Capacitor plugin | Expo Module wrapping Candle Whisper via uniffi |
| iOS-from-Windows builds | Impossible (Tauri-mobile requires Mac) | EAS Build managed cloud |
| OTA updates | None | EAS Update |
| Rust on device | Indirectly via Tauri-mobile (under-equipped) | Direct via uniffi TurboModule (`@vox/runtime-rn`) |

Three claims-vs-reality gaps already documented:
- ADR 037 (2026-05-11) said Tauri = desktop + Android + iOS canonical. The bake-off (2026-05-27) confirmed the Windows-host build hangs; the [Tauri team itself](https://v2.tauri.app/blog/tauri-20/) calls mobile not-yet-first-class. This plan scopes ADR 037 down to desktop.
- `mobile_emit.rs` already retired Capacitor in favor of Tauri events, but never reached a working iOS build flow. The Tauri events become one of two `@vox/runtime` adapters under this plan.
- The Codegen SSOT Unification 2026 plan (in flight) anticipated a "ship `@vox/runtime` npm" phase. This plan absorbs that work and extends it with `@vox/runtime-rn`.

---

## Target architecture

### One VUV HIR, two GUI lowerings, one Rust runtime

```
                          .vox sources
                                │
                                ▼
                       [vox-compiler frontend]
                       parse → HIR → typeck
                                │
                                ▼
                          VUV HIR
                  (single source of truth for
                  components, routes, forms,
                  state, mobile primitives)
                                │
                ┌───────────────┴───────────────┐
                │                               │
                ▼                               ▼
   ┌──────────────────────┐         ┌──────────────────────┐
   │ codegen_ts/web       │         │ codegen_ts/rn  (NEW) │
   │   <div> + Tailwind   │         │   <View> + StyleSheet│
   │   TanStack Router    │         │   Expo Router        │
   │   @vox/runtime       │         │   @vox/runtime-rn    │
   │   Vite/SSR bundle    │         │   Metro bundle       │
   └──────────┬───────────┘         └──────────┬───────────┘
              │                                │
              ▼                                ▼
   ┌──────────────────────┐         ┌──────────────────────┐
   │ Tauri 2 desktop      │         │ Expo build pipeline  │
   │ shell                │         │ (EAS Build → IPA/APK)│
   │ (window, menu, tray, │         │                      │
   │  IPC, auto-updater)  │         │                      │
   └──────────┬───────────┘         └──────────┬───────────┘
              │                                │
              ▼                                ▼
   ┌─────────────────────────────────────────────────────────┐
   │              vox-runtime  (one Rust crate)              │
   │   workflows · actors · MENS inference · journaling      │
   │   ─────────────────────────────────────────────────     │
   │   Desktop:  linked into Tauri's src-tauri/              │
   │   Mobile:   exposed as JSI TurboModule via              │
   │             uniffi-bindgen-react-native                 │
   └─────────────────────────────────────────────────────────┘
                                │
                ┌───────────────┴───────────────┐
                ▼                               ▼
   ┌──────────────────────┐         ┌──────────────────────┐
   │ vox-runtime-desktop  │         │ vox-runtime-rn  (NEW)│
   │ linked statically    │         │ cross-compiled       │
   │ into Tauri Rust app  │         │   .a / .so per arch  │
   │                      │         │ wrapped by Expo      │
   │                      │         │   Module             │
   └──────────────────────┘         └──────────────────────┘
```

**The invariant:** every horizontal slice represents work that happens once. Going from a new VUV primitive to "shipping on iOS + Android + Windows + macOS + Linux + web" requires touching ONE HIR node + ONE entry in the style IR + (rarely) ONE runtime API + ONE leaf translator per GUI target.

### Crates impacted

| Crate | Status | Change |
|---|---|---|
| [vox-codegen](../../../crates/vox-codegen) | exists | Add `codegen_ts/rn/` lowering parallel to existing web emit; introduce VUV-style IR |
| [vox-compiler](../../../crates/vox-compiler) | exists | No semantic changes; expose VUV-style IR as a public HIR augmentation |
| `vox-runtime` (umbrella) | exists | Factor `vox-runtime-mobile-profile` (single-thread default, suspend hooks) |
| [vox-workflow-runtime](../../../crates/vox-workflow-runtime) | exists | Add `suspend()` / `resume()` to Journal trait; document iOS lifecycle mapping |
| [vox-actor-runtime](../../../crates/vox-actor-runtime) | exists | Single-threaded scheduler mode; opt-out for desktop |
| `vox-inference` | exists or P0 | Confirm Metal (iOS) + Vulkan/CPU (Android) backends in Candle |
| `vox-runtime-rn` | **NEW** | Generated by uniffi-bindgen-react-native from `vox-runtime` public API. Published to npm as `@vox/runtime-rn`. |
| `vox-rn-bridge` | **NEW** | Expo Module config-plugin scaffolding emitted by `vox compile --target=mobile` |
| `vox-runtime` (JS adapter) | **NEW** | Published as `@vox/runtime` (web/Tauri adapter); thin wrapper over Tauri's `@tauri-apps/api` |
| [vox-tauri-codegen](../../../crates/vox-tauri-codegen) | exists | Stays; desktop only |
| `apps/vox-mental-tracker` (planned) | planned | Migrate Capacitor → Expo; replace sherpa plugin |
| `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe` (planned) | planned (Capacitor) | Rewrite as Expo Module wrapping Candle Whisper via uniffi |

### Style normalization — the central tech-debt minimizer

VUV `view:` syntax already abstracts the leaf widgets (`column()`, `text()`, `button()`, `panel()`, `heading()`). Today the TS emitter inlines specific Tailwind class strings into the React DOM output. That works fine for one target; it would be the source of "write twice forever" pain across two.

**The refactor:** introduce a canonical **VUV-style IR** layer between HIR and the per-target lowering.

```
VUV HIR  →  VUV-style IR  →  per-target translator
            (canonical)        (mechanical)
```

VUV-style IR uses ordinary, target-agnostic terms: `flex-direction: column`, `gap: 12px`, `padding: 16px`, `text-size: lg`, `font-weight: semibold`. The web translator emits Tailwind class lists. The RN translator emits StyleSheet objects (or NativeWind class lists if Phase 0 picks NativeWind).

**Consequence:** adding a new VUV primitive is one HIR node + one VUV-style IR mapping. Each translator picks it up automatically because the translator is a generic walker over the style IR, not a hand-written per-primitive switch.

This is the structural change that prevents the "two lowerings forever" tax from compounding. Without it, every new VUV primitive needs two hand-edits. With it, the translators are mostly written once.

### Mobile primitive normalization

The annotations `@back_button`, `@deep_link`, `@push`, plus the `std.mobile` module's `notify` / `take_photo` / `vibrate` / `transcribe` calls, currently lower to `@tauri-apps/api/event` listeners directly. The Tauri runtime fulfills them; on Capacitor, they would have needed a different lowering entirely.

**Refactor:** the lowering emits calls to a stable runtime API:

```ts
// JS side, in both adapters
import { voxRuntime } from "@vox/runtime";
voxRuntime.onBackButton(handler);
voxRuntime.notify("title", "body");
const photoUri = await voxRuntime.takePhoto();
const text = await voxRuntime.transcribe(audioBytes);
```

Two implementations, same API:

- **`@vox/runtime`** (web/Tauri adapter): `onBackButton` ⇒ `listen('vox-back-button', …)`; `notify` ⇒ `invoke('plugin:notification|notify', …)`; etc.
- **`@vox/runtime-rn`** (Expo adapter): `onBackButton` ⇒ `BackHandler.addEventListener(…)`; `notify` ⇒ `Notifications.scheduleNotificationAsync(…)`; `transcribe` ⇒ uniffi call into vox-runtime's Whisper.

**Consequence:** the Vox source never sees the platform. Adding a new mobile primitive means: one HIR node + one method on the runtime API + one implementation per adapter. No emitter logic in two places.

### Rust runtime on mobile — the uniffi path

`vox-runtime` (and its sibling crates) compile for `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-apple-ios` (for sim on Intel Macs), `x86_64-linux-android` (for emulator).

[uniffi-bindgen-react-native](https://github.com/jhugman/uniffi-bindgen-react-native) (Mozilla + Filament, Dec 2024) generates a React Native TurboModule with TypeScript types from a UDL spec describing the Rust public API. Mozilla uses uniffi-rs in production for hundreds of millions of users (Firefox sync, Firefox Suggest, telemetry, AOSP subsystems) — it's not a science project.

Vox's exposed mobile API surface (initial draft):

```rust
// vox-runtime/src/mobile_api.rs
use uniffi;

#[derive(uniffi::Object)]
pub struct VoxRuntime { /* internal state */ }

#[uniffi::export]
impl VoxRuntime {
    #[uniffi::constructor]
    pub fn new(config: VoxConfig) -> Arc<Self> { /* ... */ }

    pub fn spawn_actor(&self, name: String, init_state: Vec<u8>) -> Arc<ActorHandle>;
    pub fn start_workflow(&self, id: String, payload: Vec<u8>) -> Arc<WorkflowHandle>;
    pub fn infer(&self, model_id: String, input: Vec<u8>) -> Result<Vec<u8>, VoxError>;
    pub fn transcribe(&self, audio_bytes: Vec<u8>, lang_hint: Option<String>) -> Result<String, VoxError>;

    // Lifecycle hooks — called by the Expo Module on app suspend / resume.
    pub fn suspend(&self);
    pub fn resume(&self);
}
```

The Expo Module wraps the generated TurboModule and surfaces it as `@vox/runtime-rn`. The JS-side adapter (`@vox/runtime-rn`) then implements the same `voxRuntime` API the emitter targets, fulfilling each method by calling into the TurboModule.

### Mobile profile considerations for vox-runtime

Vox's runtime is currently designed for a desktop process: a multi-threaded Tokio scheduler, free-running actors, journal flushes on a leisurely schedule. None of that is right for mobile. The mobile profile:

- **Single-threaded Tokio scheduler by default.** Multi-threaded available behind a flag for performance-critical work, but battery is the primary axis.
- **Journal flushes on lifecycle events**, not just intervals. Suspend = flush. Resume = no-op (state is in memory; journal is a recovery substrate).
- **Workflow journal storage in app-private dir.** iOS: `NSDocumentDirectory`. Android: `getFilesDir()`. Exposed by Expo's `FileSystem` constants.
- **Actor pause on suspend, resume on foreground.** Active actors get a `suspend()` message; on resume, they re-subscribe to their mailboxes.
- **MENS/Candle model lifecycle.** Models are big. Load on-demand (first use after foreground), unload on memory pressure or backgrounding. Track per-model memory budget.
- **iOS background-execution reality.** iOS gives ~30 seconds after `applicationWillResignActive` before it can be killed. The journal-replay design handles this gracefully — every actor's state is recoverable from the journal, so a hard kill is not data loss.

### MENS/Candle on the phone

Candle's mobile story (Metal on iOS, Vulkan/CPU on Android, CPU fallback everywhere) is sufficient for small-to-medium quantized models. Specifically for vox-mental-tracker's primary on-device need (Whisper-tier speech recognition):

- Ship a quantized Whisper-small (Q4_K_M) model: ~150-200 MB
- Load on-demand when `voxRuntime.transcribe(...)` is first called
- Metal backend on iOS: ~real-time transcription on iPhone 13+
- CPU backend fallback on Android: slower but workable for short clips

Model assets are shipped via Expo's `Asset` API (downloaded once on first launch, cached in app-private dir). Avoids bloating the initial APK/IPA past store-friendly sizes.

### The Tauri question, addressed directly

**Keep Tauri for desktop.** Reasons:

1. Tauri 2 desktop is mature; the [bake-off issues](mobile-target-evaluation-2026.md) were specifically with mobile, not desktop.
2. [crates/vox-gui](../../../crates/vox-gui) is already Tauri 2; rewriting it is work without payoff.
3. Native menus, dialogs, system tray, auto-updater, code-signing — all mature on Tauri 2.
4. Tauri's IPC (`invoke`/`listen`) is the natural bridge for desktop Vox apps that want to call into the linked vox-runtime.

**Eventually consider RN-everywhere.** [react-native-windows](https://github.com/microsoft/react-native-windows) and [react-native-macos](https://github.com/microsoft/react-native-macos) (Microsoft-maintained) plus community react-native-linux could in principle unify Vox on a single GUI emit target across all platforms. But:

- react-native-linux is community-only and less mature than the Microsoft-backed Windows/macOS variants.
- Tauri desktop apps feel and behave more "native-on-Linux" than RN does today.
- The user-facing benefit of unification is small if Tauri-desktop works (which it does).

**Decision:** keep Tauri for desktop now; track RN-desktop maturity quarterly; formal re-evaluation Q3 2027 or sooner if react-native-linux reaches feature parity with the other two.

---

## Migration plan

Five phases. Desktop work continues uninterrupted throughout; mobile work proceeds in parallel.

### Phase 0 — Foundations (Weeks 1-3)

Specs and ADRs. No user-visible artifacts yet.

- [ ] Spec doc: VUV-style IR layer (canonical between HIR and per-target translators).
- [ ] Spec doc: `vox-runtime` mobile profile (single-thread default, suspend hooks, journal-on-lifecycle).
- [ ] Spec doc: `@vox/runtime` (web/Tauri) and `@vox/runtime-rn` (Expo) API surfaces — single TS interface, two implementations.
- [ ] ADR-NNN: scope ADR 037 to "Tauri = desktop only" with explicit retirement of Tauri-mobile.
- [ ] Decision spike: NativeWind vs Tamagui vs raw StyleSheet for the RN style target. (Likely recommendation: NativeWind for class-string reuse, but write it up before committing.)
- [ ] Decision spike: which Whisper-class model and what quantization (Q4_K_M vs Q5_1) on which Candle backend for mobile.
- [ ] Update Codegen SSOT Unification 2026 (`docs/src/architecture/codegen-ssot-unification-design-2026.md`, planned) to absorb the RN lowering as an addition to its "ship @vox/runtime npm" phase.

### Phase 1 — vox-mental-tracker on RN+Expo (Weeks 4-7)

Ships the GUI path end-to-end without depending on the uniffi work yet. Proves the RN lowering on a real app.

- [ ] Implement `codegen_ts/rn/` lowering: column, stack, text, button, heading, panel, image, plus `@form`.
- [ ] Implement `@vox/runtime-rn` stubs (JS only — `notify`, `onBackButton`, `transcribe` etc. either no-op or call Expo built-ins, no Rust yet).
- [ ] Migrate `apps/vox-mental-tracker` (planned) to RN+Expo scaffolding. Capacitor refs deleted.
- [ ] Sherpa transcription temporarily stubbed (returns canned response) or talks to a remote backend, until Phase 2.
- [ ] EAS Build pipeline configured; app reaches TestFlight + Play Internal Test.
- [ ] Snapshot tests for every VUV primitive in BOTH lowerings (web + RN). CI gate.

Exit criterion: mental-tracker installable on a real iPhone and a real Android device through the store internal-test channels, all UI functional except real on-device transcription.

### Phase 2 — uniffi + Vox Core on device (Weeks 8-12)

The differentiating phase. Vox Core actually runs on the phone.

- [ ] Implement vox-runtime mobile profile (vox-runtime-mobile-profile crate or feature flag).
- [ ] Define the uniffi UDL covering the public mobile API (initial draft above).
- [ ] Wire `uniffi-bindgen-react-native` into the build: generate the TurboModule + TS types.
- [ ] Cross-compile vox-runtime for the four mobile architectures via cargo-xcframework / `cargo ndk` integration with EAS Build hooks.
- [ ] Publish `@vox/runtime-rn` (real implementation now, wrapping the TurboModule).
- [ ] Rewrite vox-sherpa-transcribe as an Expo Module wrapping the uniffi-exported Candle Whisper.
- [ ] vox-mental-tracker uses on-device durable journal for entries (workflow runtime on phone).
- [ ] Suspend/resume integration: Expo Module subscribes to app-state events and calls `voxRuntime.suspend()` / `voxRuntime.resume()`.

Exit criterion: a mental-tracker entry is journaled locally, app is force-killed, app is reopened, the entry is still there and the workflow continues — entirely offline, entirely on-device.

### Phase 3 — Polish + retirement (Weeks 13-15)

- [ ] Retire Tauri-specific paths in `mobile_emit.rs`. The mobile emit now exclusively targets `@vox/runtime` (which dispatches per-platform at runtime).
- [ ] Delete remaining Capacitor references from the repo.
- [ ] Update `docs/how-to/build-android.md` (planned) → RN+Expo workflow.
- [ ] Create `docs/how-to/build-ios.md` (planned; new — Tauri-mobile never had a working version).
- [ ] Update `docs/user/privacy.md` (planned) — on-device transcription claim now real.
- [ ] Tutorial: "Build a mobile Vox app from scratch."
- [ ] Update vox-mental-tracker README and Vox.toml keywords.

Exit criterion: a new developer following the docs can ship a Vox mobile app without prior Vox knowledge in under one day.

### Phase 4 — Codegen SSOT integration (Weeks 16-20)

The RN lowering folds into the in-flight Codegen SSOT Unification 2026 plan's unified IR phase. By the end of Phase 4 there is ONE codegen pipeline with multiple per-target translators sharing one IR, not two parallel emit stacks.

- [ ] VUV-style IR formalized and adopted by both web and RN translators.
- [ ] `@vox/runtime` and `@vox/runtime-rn` share a TypeScript interface definition.
- [ ] Single golden-test harness validates BOTH lowerings per `.vox` file.
- [ ] `vox compile --target=mobile` emits not just the TS bundle but also the Expo config-plugin entries (uniffi Rust dep, asset bundling, EAS Build env).

### Phase 5 — Future (deferred; tracked, not scheduled)

- RN-desktop investigation: track react-native-windows / macos / linux maturity quarterly.
- iOS-specific surfaces (HealthKit, Apple Sign-in, WidgetKit) as Vox primitives if recurring user need.
- Android-specific WorkManager-style scheduling.
- Mobile-specific Vox annotations (`@haptic`, `@biometric_gate`, `@background_task`).
- WASM target for browser-only Vox apps (separate research project).

---

## Tech debt minimization — five rules

1. **VUV HIR is source of truth.** Never define a UI primitive's behavior in a lowering. Lowerings translate; they don't decide. If you find yourself adding logic to a translator, you're doing it wrong — that logic belongs upstream.
2. **One mobile primitive = one runtime API = one implementation per adapter.** `@vox/runtime` defines the contract; web/Tauri and Expo each fulfill it. Vox source code never references the adapter.
3. **Canonical VUV-style IR is the only place styling logic lives.** Per-target translators are generic walkers, not switch statements over primitives.
4. **All non-UI emit is unchanged across targets.** Forms, queries, mutations, contracts, OpenAPI, state machines, vox-client SDK — all single-source.
5. **CI runs both lowerings against every VUV primitive.** Snapshot tests in both targets. Drift fails the build, not a PR review.

The "twice" surface that remains:
- Element-to-component mapping table (one row per primitive: `column` → `<div>` / `<View>`)
- A single per-target translator function (generic over the style IR)
- A single per-adapter implementation of each `@vox/runtime` method (when a new method is added)

That surface is bounded and predictable. The 5K-LoC up-front cost for the RN lowering should produce a per-feature ongoing cost of <50 LoC per new VUV primitive across both targets combined.

---

## vox-mental-tracker MVP scope

Minimum needed to ship vox-mental-tracker as a real app on TestFlight + Play Internal Test:

| Feature | Phase | Primary path |
|---|---|---|
| Daily mood log entry | 1 | `@form` lowered to RN (text input, radio, submit) |
| Calendar view of past entries | 1 | `column` + `for` + `ItemCard` lowered to FlatList |
| Reminder push notifications | 1 | `@push` lowered to `expo-notifications` schedule |
| Local-first journal persistence | 2 | `@workflow` with mobile journal in `documentDirectory` |
| Voice journal entry (on-device) | 2 | `voxRuntime.transcribe()` via uniffi-wrapped Candle Whisper |
| Weekly summary | 2 | `@workflow` scheduled, replays from journal |
| Privacy-first storage | 2 | All on-device; no remote backend; aligns with `apps/vox-mental-tracker/docs/user/privacy.md` (planned) |

Phase 1 alone is enough to demonstrate "Vox ships to mobile" via real artifacts. Phase 2 demonstrates "Vox uniquely enables this" — on-device durable workflow + on-device Whisper transcription, both authored in pure Vox source.

The mental-tracker is intentionally chosen as the proving ground because:
- It exercises every essential primitive (forms, navigation, push, durable state, ML inference).
- Its privacy posture forces on-device execution, which is the differentiated thing Vox enables.
- It's small enough that one developer can carry the whole app through both phases.

---

## Biggest wins (the differentiators)

What this architecture unlocks that competing AI-first or low-code mobile tools don't:

1. **Durable workflows running native on a phone.** Apps that survive app kills, OS reboots, and low-power suspend without losing state, with replay from journal on resume. Mental-tracker reminders become reliable. Daily check-ins resume gracefully. The natural fit between Vox's `@workflow` design and mobile suspension lifecycle is rare among AI-first stacks.

2. **On-device ML inference in a "write the app in Vox" workflow.** No "set up TensorFlow Lite, write the bridge, learn the API" — Vox's `mobile.transcribe(...)` or `@infer model("whisper-small") with input ...` lowers to a uniffi call into Candle. The user writes Vox; the LLM writes Vox; both get on-device AI without any human writing Swift or Kotlin.

3. **Single VUV syntax targeting desktop and mobile.** The same `component HomeScreen() { view: ... }` works on Windows, macOS, Linux desktop AND on iPhone and Android. Learn once. Maintain in one place.

4. **Actor model for mobile UI workflows.** Async, message-passing state per UI feature, with proper isolation, on a battery-constrained device. Significantly cleaner than juggling React hooks for complex flows.

5. **AI-authored mobile apps with native escape hatch.** LLMs author Vox source prolifically (RN+Expo has the largest mobile-relevant training corpus by ~50× over the alternatives). Vox lowers to RN+Expo + on-device Rust. When a feature needs native capability, Vox extends the runtime in Rust once; the AI keeps authoring Vox above it.

---

## Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| uniffi-bindgen-react-native is young (<2yr at time of writing) | Medium | High if it stalls | Mozilla uses uniffi-rs in production for hundreds of millions of users; Filament/Mozilla are active maintainers; Phase 1 ships without uniffi so the RN GUI path is independently shippable |
| EAS Build is paid SaaS | Low | Medium | Free tier supports solo dev; self-host GH Actions macOS runner ($0.08/min) as escape hatch; pricing has been stable since 2024 |
| RN / Expo breaking changes per major | Medium | Medium | `expo install` keeps SDK alignment automatic; pin RN+Expo SDK in `vox-emit`; absorbed via the same plumbing that already handles TanStack / React major bumps |
| Two GUI lowerings drift despite normalization | Medium | High | Per-primitive snapshot tests across BOTH lowerings; CI gate; VUV-style IR refactor prevents most drift by construction |
| iOS App Store size limits with Rust runtime + Candle model | Low | Medium | Per-arch app thinning; Candle models downloaded on first launch via expo-asset; initial APK/IPA stays under store-friendly limits |
| iOS background-execution kills workflows mid-run | High | Medium | Journal-replay design handles this natively; explicit `suspend()` hook in vox-runtime mobile profile flushes journal within iOS's grace period |
| Tauri desktop and Expo mobile diverge in feel | Medium | Low | Acceptable — both are "native to platform"; uniformity is a non-goal, faithfulness to each platform is |
| Vox-side complexity from "single source, multiple targets" creates undebuggable bugs | Medium | Medium | Heavy investment in the VUV-style IR + snapshot tests + `@vox/runtime` interface contract; LLM debugging story is best-in-class on RN+Expo so AI-assisted root-cause is feasible |
| Microsoft deprecating CodePush 2024 was Expo's win — could EAS Update follow? | Low | Medium | EAS Update is Expo's core revenue product; deprecation would damage Expo's business model; track changes in EAS Build pricing as a leading indicator |

---

## Open questions / decisions deferred

1. **NativeWind vs Tamagui vs raw StyleSheet for the RN style target.** NativeWind preserves Tailwind class strings on RN — biggest reuse from the existing web emit. Tamagui adds compile-time optimization but a learning curve and another vendor. Raw StyleSheet is RN-native but loses Tailwind reuse. Phase 0 decision spike with a written recommendation.
2. **Tauri desktop bundle size with `@vox/runtime` JS in addition to the current emit** — measure in Phase 0 to confirm no regression.
3. **iOS Apple Developer Program membership.** Solo dev needs this ($99/yr) for shipping to physical iOS devices. Confirm budget acceptable.
4. **Whisper model selection.** Phase 0 decision: which Whisper-class model and what quantization on which Candle backend for mobile. Open question: is there a smaller distilled model that meets vox-mental-tracker's quality bar at lower size/RAM?
5. **vox-runtime mobile-profile feature flag vs separate crate.** Phase 0 spec decision.
6. **Future RN-desktop unification timing.** Suggest: track quarterly via a single "RN-desktop maturity" review doc; formal architectural revisit Q3 2027 or earlier if react-native-linux reaches Microsoft-tier maintenance.

---

## Companion docs

- [mobile-target-evaluation-2026.md](mobile-target-evaluation-2026.md) — the decision rationale (research, bake-off, WebView quirks, framework comparison).
- (forthcoming) ADR-NNN scoping ADR 037 to desktop-only.
- (forthcoming) Phase 0 spec for VUV-style IR layer.
- (forthcoming) Phase 0 spec for vox-runtime mobile profile.
- (forthcoming) Phase 0 spike report: NativeWind vs Tamagui vs StyleSheet.
- Codegen SSOT Unification 2026 (`docs/src/architecture/codegen-ssot-unification-design-2026.md`, planned) — this plan absorbs into Phase 4 of that work.
- [external-frontend-interop-plan-2026](external-frontend-interop-plan-2026.md) — bidirectional React interop in Phase 5 of that plan continues to apply to the web lowering; RN lowering is independent.

---

## Appendix A — Why not Capacitor as a stepping stone

Considered and rejected. Capacitor 8 has a real "zero emitter work" appeal: Vox's existing TSX+Tailwind output runs unchanged inside a WebView shell, and the iOS-from-Windows story is solved via Ionic Appflow or self-hosted GH Actions macOS runners. The cost of choosing it is low enough that "ship today" becomes possible without committing to RN.

But it traps Vox in WebView forever for mobile UI. WebView quirks specifically biting Vox in 2026:

- iOS keyboard covers bottom-fixed inputs ([WebKit bug 192564](https://bugs.webkit.org/show_bug.cgi?id=192564)) — hits `@form` directly.
- iOS 26 keyboard regression breaks the native app's keyboard layout after WebView text-field focus ([Apple dev forum thread](https://developer.apple.com/forums/thread/802159), autumn 2025).
- WebView swipe-back gesture conflicts with web app navigation ([w3c/pointerevents#358](https://github.com/w3c/pointerevents/issues/358)).
- Android keyboard overlap behavior diverges from iOS — same Vox source, different broken behavior per platform.
- Position-fixed / safe-area inconsistency requires WebView-specific lowering glue forever.
- Scroll inertia and bounce feel "web-like" not "native-like" — users sense within 30 seconds.
- No native date/time/contact pickers in WebView.

These are not abstract concerns. Every one of them would require per-WebView glue in the Vox emitter, in addition to the work it takes to ship via Capacitor in the first place. The "Capacitor first, RN later" path is two emit pipelines plus an eventual retirement — strictly more work than going to RN directly.

The user's stated north star ("native mobile support down the line") forecloses Capacitor as a long-term destination. Choosing it now would mean paying for the migration off it later.

## Appendix B — Why not Tauri-mobile

Considered, recommended in ADR 037, rejected here. Reasons:

1. The [Tauri team itself](https://v2.tauri.app/blog/tauri-20/) says Tauri 2.0 is "not the mobile-as-first-class-citizen release" and that mobile parity with desktop is still a work-in-progress focus area.
2. The bake-off (2026-05-27) on this Windows host confirmed Tauri Android dev orchestration hangs after Kotlin compile with no progress for 30+ minutes; APK was built but never installed/launched by the dev tooling.
3. iOS requires a macOS host — no managed cloud alternative exists or is announced. Hard blocker for Windows-host solo dev.
4. Mobile plugin gap vs desktop is real; missing capabilities (push, IAP, contacts, background tasks) require writing Tauri mobile plugins in Swift + Kotlin + Rust together — three languages per gap.
5. LLM training corpus for Tauri-mobile is microscopic vs RN+Expo (~50× density gap in tutorials and StackOverflow); AI-assisted authoring and debugging are noticeably worse.

ADR 037 should be revised in Phase 0 to "Tauri = desktop only."

## Appendix C — Why keep the Rust emit at all

Vox's Rust emit isn't UI work — it's the runtime that makes Vox not-just-another-React-meta-framework. Specifically:

- `@workflow` durable execution with crash recovery requires a Rust journal + replay loop. No JS/TS runtime offers this without external orchestration (Temporal, Inngest) that breaks Vox's "batteries included" promise.
- `@actor` lightweight process spawning with proper supervision and mailboxes uses Tokio. JS event emitters approximate but lack the isolation and concurrency model.
- MENS / Candle ML inference uses native compute (CUDA / Metal / Vulkan). TF.js / ONNX.js fall short on quantization, streaming, and performance.
- SafeTensors model loading, native FFI, GpuCompute primitives — all genuinely require Rust.

The mobile architecture in this doc explicitly preserves the Rust emit's role: vox-runtime cross-compiles to mobile architectures and ships to the device alongside the JS bundle. Going TS-only would gut Vox's distinguishing features. Going Rust-everywhere via WebAssembly is a different and longer-term research direction (Appendix D).

## Appendix D — Why not WebAssembly on mobile

Compiling vox-runtime to WASM and shipping it as a JS dependency in the RN bundle is theoretically possible — RN supports WASM via Hermes flags or via JSC bindings on iOS, V8 on Android.

Reasons not pursued in this plan:

- WASM doesn't have direct access to native threads, file I/O, or Metal/Vulkan/CUDA. Vox's durable workflow journal + on-device ML inference need all three.
- WASM-in-JS-engine performance for Tokio-style async runtimes is meaningfully worse than native Rust.
- The uniffi path is straightforward, production-proven (Mozilla), and gets Vox the full native capabilities of the device.
- WASM remains interesting for browser-only Vox apps (no native mobile shell) and may be revisited as a separate research project.
