---
title: "Mobile target evaluation 2026 — Tauri-mobile vs RN + Expo"
description: "Bake-off comparing Tauri-mobile (ADR-037) and React Native + Expo as Vox's mobile target."
category: "Architecture SSOTs"
status: research
last_updated: 2026-05-27
related:
  - adr-037 (Tauri canonical for desktop + mobile, 2026-05-11)
  - codegen-ssot-unification-design-2026
  - tauri-convergence-migration-plan-2026
---

# Mobile target evaluation 2026

> Should Vox's mobile target be **Tauri 2 mobile** (per ADR 037) or **React Native + Expo**? Decided by: research + hands-on Android bake-off on a Windows host. Recommendation upstream of the Codegen SSOT Unification adding any new emit lowering.

## Question

Vox is an AI-first language whose TypeScript emitter produces React/TSX today. The mobile surface needs ONE target so we don't carry two GUI libraries forward. The author host is Windows. The north-star metric is "LLMs can write Vox mobile apps well."

ADR 037 (2026-05-11) selected Tauri 2 for desktop, Android, and iOS. This document re-litigates the mobile portion of that decision in light of:

1. published 2025-2026 evidence on Tauri-mobile's actual maturity vs RN+Expo,
2. a hands-on Android bake-off on the author's Windows host,
3. and the fact that the TS emitter today lowers Vox's `column`/`text`/`button` VUV primitives to **React DOM + Tailwind**, not abstract widgets — which materially changes the cost of each path.

## Methodology

- **Research:** WebSearch/WebFetch sweep of mid-2024 → May 2026 community signal (HN, GitHub Issues, Reddit, eng-blog retros). Filtered marketing.
- **Sample:** small Vox source exercising VUV primitives (component, state, button, list, mobile-bridge call). Note: `vox build` panics on every component source today (see [Toolchain finding](#toolchain-finding-vox-build-broken-on-windows)) — the bake-off used the existing snapshot output (`crates/vox-codegen/tests/snapshots/golden_ts_test__golden_ts_emit@component_state.snap`) as ground truth for what Vox emits.
- **Path A — Tauri-mobile:** wrap the snapshot TSX in a Vite+React frontend, `cargo tauri init` + `cargo tauri android init` + `cargo tauri android dev` → Pixel 6 AVD (Android 36, x86_64, Play Store image).
- **Path B — RN+Expo:** `npx create-expo-app --template blank-typescript`, hand-port the same snapshot TSX to RN equivalents, `npx expo start --android` → same AVD via Expo Go.
- **Out of scope:** iOS (would require a Mac; relative comparison signal is the same).

## Findings

### 1. Setup-cost asymmetry (measured on this Windows host)

| | Path A (Tauri-mobile) | Path B (RN+Expo) |
|---|---|---|
| Steps to "hello world" on Android | download cmdline-tools (150 MB) → install NDK r27c (~2 GB) → `rustup target add` ×4 Android targets → `cargo install tauri-cli` → set `ANDROID_HOME`/`NDK_HOME` → `cargo tauri init` → `cargo tauri android init` → `cargo tauri android dev` | `npx create-expo-app --template blank-typescript` → `npx expo start --android` |
| Disk required up front | ~3 GB | ~300 MB |
| One-time wall time (warm network) | ~45 min | ~2 min |
| Environment variables required | `ANDROID_HOME`, `NDK_HOME` (plus the dev-server `TAURI_DEV_HOST=0.0.0.0` workaround for Windows networking) | none |

This is a meaningful **LLM-onboarding tax**: an LLM walking a Vox newcomer through Path A burns 30-50 turns before any code runs. Path B is one command.

### 2. Vite dev-server networking on Windows

Tauri Android dev defaulted to a network IP (`192.168.50.33:5173`) the emulator could reach, but the Vite default binds to `localhost` only. Result: silent stall on `Warn Waiting for your frontend dev server to start on http://192.168.50.33:5173/...` until manually fixed by:

```ts
// vite.config.ts
server: { host: true, port: 5173, strictPort: true }
```

…plus setting `TAURI_DEV_HOST=0.0.0.0`. This trap is documented in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) but not flagged in the `cargo tauri android init` output. Path B has no equivalent — Metro auto-detects the emulator over `adb` without any host-side configuration.

### 3. Emit-pipeline fit — the unexpected wrinkle

Vox's TS emitter lowers VUV primitives to **React DOM + Tailwind (shadcn-shaped classnames)**:

```tsx
// from snapshot: component_state.snap (Counter)
<div className={["flex", "flex-col"].filter(Boolean).join(" ")}>
  <h1 className={["text-3xl", "font-semibold"].filter(Boolean).join(" ")}>
    {"Count"}
  </h1>
  <button className={[..."bg-primary text-primary-foreground hover:bg-primary/90"...].filter(Boolean).join(" ")} onClick={...}>
    {"Increment"}
  </button>
</div>
```

This means:

- **Path A is genuinely free.** Tauri-mobile is "React DOM in a mobile WebView." The existing emitter output drops in unchanged.
- **Path B is *not* free.** Every emitted element needs structural rewriting: `<div>` → `<View>`, `<h1>` → `<Text>`, `<button onClick>` → `<Pressable onPress>` containing a `<Text>` child, `<p>` → `<Text>`, Tailwind classes → `StyleSheet.create` (or NativeWind). The hand-port of even the trivial Counter required ~50 lines of new code per ~25 lines of input.

Picking RN+Expo therefore means **building a new emit lowering target in `crates/vox-codegen/src/codegen_ts/`** — not a small task, and it lands inside the in-flight Codegen SSOT Unification 2026 plan as a new IR lowering target. Hand-porting on every build is not a real option.

### 4. Hands-on results

> Filled in after both paths complete; placeholder until then.

**Path A — Tauri-mobile (Pixel 6 AVD):**

- First `cargo tauri android dev` cold compile time: _TBD_
- Subsequent rebuild time: _TBD_
- HMR working: _TBD_
- Screenshot: (screenshot not yet captured — bake-off pending)
- Notes: _TBD_

**Path B — RN+Expo via Expo Go (same AVD):**

- `npx expo start --android` time to first render: _TBD_
- HMR working: _TBD_
- Screenshot: (screenshot not yet captured — bake-off pending)
- Notes: _TBD_

### 5. Research summary (full sources in appendix)

| Dimension | Winner | One-line evidence |
|---|---|---|
| Hot reload / dev loop | RN+Expo | Tauri Rust changes force full rebuild; HMR known-broken on some Android configs ([tauri#10509](https://github.com/orgs/tauri-apps/discussions/10509)) |
| Debugger | RN+Expo | RN DevTools default since 0.76; Tauri mobile-debugger [tauri#12174](https://github.com/tauri-apps/tauri/issues/12174) still open |
| Plugin / native gap | **RN+Expo (decisive)** | Push, IAP, contacts, background tasks all first-party in Expo; community-only or missing in Tauri ([tauri#11651](https://github.com/tauri-apps/tauri/issues/11651) on remote push) |
| Build times / CI | RN+Expo | EAS Build managed cloud (M4 Pro since Mar 2025); no Tauri equivalent |
| Signing / distribution | **RN+Expo (decisive)** | `eas submit` + `eas update` OTA; Tauri has no OTA story |
| Production failures | Lean RN+Expo | RN new-arch migration is a one-time hump; Tauri Android-7 WebView crashes ([tauri#8788](https://github.com/tauri-apps/tauri/issues/8788)) are device-fleet-dependent |
| **Windows host experience** | **RN+Expo (decisive)** | EAS builds iOS from Windows; Tauri iOS [requires macOS host](https://v2.tauri.app/start/prerequisites/) — no equivalent of EAS exists or is announced |
| 2025-26 trajectory | RN+Expo | RN 0.82 made new arch mandatory; 88% "right direction" in [State of RN 2024](https://results.stateofreactnative.com/en-US/); Tauri mobile-plugin cadence slowed in 2025 |
| LLM-friendliness | **RN+Expo (decisive)** | RN-specific eval suite ([Callstack RN Evals](https://www.callstack.com/blog/announcing-react-native-evals)) exists; nothing comparable for Tauri-mobile; ~50× SO/tutorial density advantage |

**Net:** Tauri-mobile wins zero dimensions. RN+Expo wins decisively on plugin gap, Windows host, signing, and LLM-friendliness.

## Recommendation

> _Filled in after the bake-off completes._

### Kill criteria

If the recommendation is RN+Expo, we **revert** to Tauri-mobile if:

1. Tauri 2 ships an iOS-from-Windows build service (EAS-equivalent) before our v1.0 cut.
2. Tauri 2 reaches first-party plugin parity on the four currently-missing capabilities (remote push, IAP, contacts, background tasks).
3. A public LLM-eval benchmark shows Tauri-mobile generation success within 20% of RN+Expo.

If the recommendation is Tauri-mobile, we **switch** to RN+Expo if:

1. Tauri mobile-plugin cadence stays below one first-party plugin per quarter through 2026 Q4.
2. Anyone on the team needs to ship iOS without buying a Mac.
3. The codegen SSOT plan lands a unified IR that makes adding RN lowering cheap.

## Implications

### For ADR 037

ADR 037 selected Tauri 2 for desktop + Android + iOS as a single bet. The desktop and Android-via-WebView arguments hold up; the iOS-on-Windows argument does not. This document recommends a **follow-up ADR scoping Tauri to desktop (and *optionally* Android-via-WebView for users who want a single Rust shell)**, with RN+Expo as the canonical mobile target.

### For the Codegen SSOT Unification 2026 plan

If RN+Expo wins, the SSOT plan needs to absorb a second TS-emit lowering target during its "ship `@vox/runtime` npm" phase. Concretely:

- A new lowering from VUV HIR → RN components + StyleSheet + expo-router primitives.
- `@vox/runtime-rn` published alongside `@vox/runtime` for the web bundle.
- Mobile-specific annotations (`@back_button`, `@deep_link`, `@push`) lower to `react-native-back-handler` + `expo-linking` + `expo-notifications` instead of `@tauri-apps/api`.

This is the work the recommendation creates. It is **not** speculative if the SSOT unification is already restructuring the lowerings.

### For the Tauri convergence migration plan

Phases 1-4 (desktop GUI on Tauri 2) stay. Phase 5 (mental-tracker Capacitor → Tauri-mobile) becomes Phase 5 (mental-tracker Capacitor → RN+Expo) instead. The voice/STT plugin work (`vox-sherpa-transcribe` → `vox-tauri-stt`) becomes `vox-sherpa-transcribe` → `expo-modules-core`-based plugin.

## Toolchain finding: `vox build` broken on Windows

During the bake-off I discovered that `cargo run -p vox-cli -- build <any-component-source.vox>` panics in `crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:276` with `HirModule serializes to JSON ... key must be a string`. The frontend ("Frontend passed with 0 warning(s)") completes; the panic is in the Rust-backend codegen path. The snapshot tests bypass this path so it isn't caught in CI. Repro:

```
cargo run -p vox-cli -- build C:/Users/Owner/vox/examples/golden-ts/component_state.vox -o /tmp/dist
```

This is independent of the mobile decision but blocks anyone from building a real Vox app on Windows today. Spawned as a separate task.

## Appendix: research sources

See [research-sources.md](_screenshots/research-sources.md) for the full link list (preserved separately to keep this doc focused on the decision).
