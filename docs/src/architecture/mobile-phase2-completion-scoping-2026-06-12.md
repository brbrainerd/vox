---
title: "Mobile Phase 2 completion: cross-compile pipeline, iOS, mental-tracker Expo upgrade (scoping)"
description: "Verified-state scoping for the three host-blocked mobile work items: wiring the Android cross-compile + uniffi-bindgen artifact pipeline into vox build --target=mobile, the iOS path, and upgrading apps/vox-mental-tracker from its Capacitor-era shell to the Expo target. Grounded in what exists on 2026-06-12; no stubs proposed."
category: "Architecture SSOTs"
status: experimental
last_updated: "2026-06-12"
authors: [vox-team]
related:
  - mobile-rn-expo-implementation-spec-2026.md
  - adr-NNN-scope-tauri-desktop-only.md
  - mobile-eas-ci-setup-2026.md
---

# Mobile Phase 2 completion — scoping (2026-06-12)

Three work items remained "host-blocked" after the Phase 2 buildable slice landed
(profile-driven journal flush, Expo config plugin, Deferred uniffi journal, RN
`<Switch>`, Tauri-desktop-only ADR Accepted). This doc scopes each against the
**verified current state**, which is meaningfully ahead of the original spec §11/§13
assumptions.

## 0. Verified current state (what already exists)

| Piece | State | Evidence |
|---|---|---|
| `vox build --target=mobile` | Working; emits RN components/forms/routes/scaffold (`app.json`, `metro.config.js`, `eas.json`, `package.json`) | `crates/vox-codegen/src/codegen_ts/rn/`, `crates/vox-cli-tests` fixtures |
| uniffi TS binding generation | Working **on host**: `clients/runtime-rn/scripts/generate-bindings.mjs` cargo-builds `vox-runtime-rn` (host dylib) then runs `uniffi-bindgen-react-native generate jsi bindings`; output checked in under `src/__generated__/` | script + generated files exist |
| Android cross-compile gate | `crates/vox-cli-tests/tests/mobile_cross_compile.rs` builds `vox-runtime-rn` for the 4 Android targets via cargo-ndk (skippable via `VOX_CLI_TESTS_SKIP_NDK=1`); iOS test `#[ignore]`d (needs macOS) | test file |
| Expo config plugin | `clients/runtime-rn/plugin/` links `libvox_runtime_rn.so` per-ABI from the package's `android/jniLibs/` (or `androidLibDir` prop) + iOS xcframework; feature-gated permissions; model assets | 22 unit tests |
| CI end-to-end gate | `.github/workflows/mobile-eas-build.yml`: `bundle` job builds mental-tracker's real `src/main.vox` with `--target=mobile` and runs `expo export --platform android` (swapping in a JS shim for the native module); optional `eas-build` job behind `EXPO_TOKEN` | workflow file |
| mental-tracker | 756-LoC `src/main.vox` (5 `@table`, 8 `@query`, 10 `@mutation`, `@form`, 8 components, 7 routes, `@push`, `Speech.transcribe_microphone`); ships today as a **web/PWA** via Vite; Capacitor footprint is thin (config file + docs + one `cap sync` line — **zero** `@capacitor/*` deps); the actual native link is `vox-tauri-stt-guest` (Tauri) + `@tauri-apps/api` | survey 2026-06-12 |

The gap is therefore **not** "build the pipeline" — it's **connecting existing pieces**
so the cross-compiled `.so` flows into the Expo plugin's expected location, plus the
app-level migration.

## 1. Work item A — Android artifact pipeline in `vox build --target=mobile`

**Goal (spec §11.4, reduced to what's missing):** after RN emit, optionally produce and
place the native runtime so a consumer can run `expo prebuild` → `expo run:android`
with a *real* (non-shim) `@vox/runtime-rn`.

**Design decision: make it a separate opt-in step, not part of every `--target=mobile`
build.** Rationale: the emit path is fast and NDK-free today (CI proves bundles in
seconds); coupling cargo cross-compiles into it would make every build require an NDK.
Proposed surface: `vox build --target=mobile --with-native-runtime` (or
`vox mobile package-runtime` subcommand — decide at implementation; the flag keeps
one entry point).

**Tasks:**

| # | Task | Where | Est. LoC | Verifiable on this host? |
|---|---|---|---|---|
| A1 | NDK preflight: detect `ANDROID_NDK_HOME`/cargo-ndk, fail with install instructions (mirrors the freshness-check style) | `vox-cli` build command | ~60 | Yes (negative path) |
| A2 | Invoke `cargo ndk -t arm64-v8a -t armeabi-v7a -t x86 -t x86_64 build -p vox-runtime-rn --release` | `vox-cli` | ~50 | Only with NDK installed |
| A3 | Copy `target/<triple>/release/libvox_runtime_rn.so` → `clients/runtime-rn/android/jniLibs/<abi>/` (the Expo plugin's documented source dir) | `vox-cli` | ~40 | With NDK |
| A4 | Run `generate-bindings.mjs` (or its logic in Rust) to refresh `__generated__` TS, diff-check against checked-in bindings; fail on drift | `vox-cli` or npm script | ~40 | Yes (host build) |
| A5 | Extend `mobile_cross_compile.rs` with a test that runs the new flag end-to-end and asserts the four `.so`s land in `jniLibs/` (gated on NDK presence like the existing tests) | `vox-cli-tests` | ~60 | With NDK |
| A6 | EAS hook: `eas-build-pre-install` script that runs A2–A3 on the EAS runner (spec §11.4 step 5) so cloud builds never need committed binaries | `clients/runtime-rn/` + `eas.json` | ~50 | No (needs EAS run) |

**Host requirement:** Android NDK + `cargo-ndk` on the dev machine (one-time install;
A1 gives the instructions). CI: the existing `mobile-eas-build.yml` can grow an
NDK-equipped job using `android-actions/setup-android` or the preinstalled NDK on
`ubuntu-latest` runners (they ship one — verify version against `cargo-ndk` needs).

**Out of scope:** `infer`/`transcribe` over uniffi (Candle cross-compile, spec §15);
they stay `UnsupportedOnPlatform`.

## 2. Work item B — iOS

**Hard constraint:** Apple tooling requires macOS. No workaround exists for local
builds on this Windows host. The honest scope is therefore *pipeline-readiness*, with
the actual build delegated to EAS's managed macOS runners:

| # | Task | Where | Verifiable on this host? |
|---|---|---|---|
| B1 | Add `aarch64-apple-ios` (+ sim targets) to the cross-compile test matrix, still `#[ignore]`d, with the `lipo`/`xcodebuild -create-xcframework` packaging steps scripted (mirrors spec §10.4) | `vox-cli-tests`, script | Script reviewable; not runnable |
| B2 | `eas-build-pre-install` (A6) branches on `$EAS_BUILD_PLATFORM == ios` to run the iOS cross-compile + xcframework merge on the EAS macOS runner | `clients/runtime-rn/` | No — verified by one EAS iOS build |
| B3 | One-shot acceptance: trigger `eas build --platform ios --profile preview` once `EXPO_TOKEN` is configured; install on simulator | manual / `eas-build` CI job | Needs user's Expo account |

**Decision needed from maintainer:** is the free-tier EAS account (memory: logged in
as `brbrainerd`) acceptable for the one-shot iOS verification, or should B3 wait for
a self-hosted macOS runner? Recommendation: EAS free tier — it's exactly the escape
hatch the ADR names.

## 3. Work item C — mental-tracker upgrade (Capacitor-era shell → Expo)

**Reality check vs spec §13.2:** the spec assumed `-3 @capacitor/* deps`; there are
**zero**. The real coupling to remove is `@tauri-apps/api` + `vox-tauri-stt-guest`
(the Tauri STT shim asserted by `tests/runtime_shim.test.ts`). Also `targets = []`
in `Vox.toml` and the `"capacitor"` keyword are stale.

**Recommended shape: dual-target, not replacement.** The app is a working PWA today
(committed `web-dist/`, Playwright e2e, service worker). The mobile target emits into
its own directory; keep the web build intact. This matches the "one VUV source → all
devices" north star and avoids destroying a working artifact.

**Phase C1 — Expo app boots with stub runtime (no NDK needed; fully verifiable here):**

| # | Task | Notes |
|---|---|---|
| C1.1 | `vox build src/main.vox --target=mobile -o mobile/` emits the Expo project (CI already proves this on this exact file) | scaffold lands in `apps/vox-mental-tracker/mobile/` |
| C1.2 | Delete `capacitor.config.ts`; move its identity (`com.vox.mentaltracker`, app name) into the emitted `app.json` via `--emit-config` overrides | identity preserved |
| C1.3 | Replace `ios/App/App/Info.plist` stub: its mic-permission strings move to the Expo plugin's `descriptions` prop in `app.json` | delete `ios/` |
| C1.4 | package.json: add `mobile:start`/`mobile:build` scripts (`expo start` in `mobile/`); **remove** `vox-tauri-stt-guest` + `@tauri-apps/api`; rewrite the `Speech` shim so web keeps a Web-Speech-API (or explicit-unsupported) path and mobile resolves to `voxRuntime.transcribeMicrophone()` | `runtime_shim.test.ts` updated to assert the new wiring |
| C1.5 | `scripts/build.vox`: drop the `npx cap sync` line; add the mobile emit step | `.vox` automation per policy |
| C1.6 | `Vox.toml`: `targets = ["web", "mobile"]`, keywords `capacitor`→`expo` | |
| C1.7 | Docs: rewrite `docs/how-to/build-android.md` (Capacitor → `vox build --target=mobile` + `expo run:android`/EAS); README build section; RELEASE_CHECKLIST mobile rows | satisfies spec Phase 3 doc deliverable for this app |
| C1.8 | Acceptance (host): `expo export --platform android` succeeds locally exactly as CI does; vitest + Playwright web suites stay green | no emulator required |

**Phase C2 — on-device persistence acceptance (needs A-pipeline or EAS):**

| # | Task |
|---|---|
| C2.1 | Wire `@vox/runtime-rn` real native module via work item A; journal-backed `db.*` tables persist through the uniffi `FileJournalHandle` (Deferred durability + lifecycle flush — already landed Rust-side) |
| C2.2 | Acceptance: install preview build on Android emulator → create entry → force-kill → reopen → entry persists, offline (spec §15 Phase 2 acceptance) |
| C2.3 | Real `transcribeMicrophone()` stays gated on Candle cross-compile (spec §15); until then the mobile Speech path returns the explicit `UnsupportedOnPlatform` error — surfaced in UI as a disabled mic with a tooltip, not a crash |

**Estimated effort:** C1 ≈ 1 focused session (mostly mechanical, fully verifiable
here); A ≈ 1 session + one-time NDK install; B ≈ small scripting + one EAS run; C2 ≈
1 session after A.

## 4. Sequencing & decision points

```
A1–A5 (needs NDK install on host) ──┐
                                    ├─→ C2 (persistence acceptance on emulator)
C1 (no NDK; do anytime) ────────────┘
A6 + B1–B2 (EAS hook, reviewable) ──→ B3 (one EAS iOS build; needs EXPO_TOKEN decision)
```

Maintainer decisions needed before execution:
1. **NDK on the dev host** — approve installing Android NDK + cargo-ndk (A2+).
2. **EAS usage** — approve free-tier EAS for the iOS one-shot (B3) and the optional
   `eas-build` CI job (`EXPO_TOKEN` secret).
3. **Flag vs subcommand** for the native-runtime packaging step (A, default
   recommendation: `--with-native-runtime` flag).
4. **PWA dual-target** — confirm keeping the web/PWA build alongside mobile (C,
   recommended) vs full replacement per the original spec.
