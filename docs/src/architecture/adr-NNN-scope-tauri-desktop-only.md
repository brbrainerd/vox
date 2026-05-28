---
title: "ADR-NNN: Scope Tauri to desktop only; pick React Native + Expo + uniffi for mobile"
status: experimental
category: "Architecture SSOTs"
date: 2026-05-28
supersedes: adr-037
related:
  - mobile-target-evaluation-2026.md (decision rationale, bake-off evidence, framework comparison)
  - mobile-rn-expo-architecture-and-migration-2026.md (implementation architecture)
  - tauri-convergence-migration-plan-2026.md (existing migration plan — Phase 5+ to be rewritten)
  - codegen-ssot-unification-design-2026.md
---

# ADR-NNN: Scope Tauri to desktop only; pick React Native + Expo + uniffi for mobile

## Status

**Proposed.** Conditional on resolving two pre-existing blockers (see §Preconditions).

## Context

ADR-037 (2026-05-11, not yet filed as a doc) declared Tauri 2 the canonical Vox shell for desktop, Android, and iOS. That decision predated:

1. **Hands-on bake-off (2026-05-27)** on a representative Windows-host workflow. `cargo tauri android dev` produced an APK successfully but the dev orchestration hung for 30+ minutes after Kotlin compile, never installed or launched the app, with zero further log output and idle gradle. Manual `adb install` of the produced APK worked, demonstrating the build path itself is sound; the developer-experience orchestration is what failed.
2. **Upstream Tauri team positioning** [in the 2.0 stable announcement](https://v2.tauri.app/blog/tauri-20/): *"after the stable release the focus is shifting to providing feature parity wherever possible and to improve the development process for mobile."* Mobile is explicitly not yet first-class even by Tauri's own characterization.
3. **Plugin gap reality.** Tauri's first-party mobile plugins cover notifications, dialogs, NFC, barcode, biometric, clipboard, basic deep links. They do not cover remote push, in-app purchases, contacts, background tasks. Each missing capability requires writing a Tauri mobile plugin in Swift + Kotlin + Rust together — the [Tauri mobile plugin development guide](https://v2.tauri.app/develop/plugins/develop-mobile/) walks through this but it is non-trivial and the burden falls on the consumer.
4. **Windows-host iOS reality.** Tauri's prerequisites are explicit that "all iOS commands are only available on macOS hosts." No managed cloud-build equivalent of EAS Build exists or is announced.
5. **LLM-authorability data.** RN+Expo has approximately 50× the tutorial / Stack Overflow / community-plugin density of Tauri-mobile (`react-native` SO tag: ~150K+ questions; `tauri` SO tag: <2K questions with almost none mobile-specific). For a language whose value proposition is "LLMs write apps in this well," the training-corpus gap is decisive.
6. **Codebase reality check (2026-05-28).** Audit of current state:
    - **No Vox code has ever been cross-compiled to a mobile architecture.** Zero references to `aarch64-linux-android`, `aarch64-apple-ios`, etc. in any `Cargo.toml`. No build.rs or CI step targets mobile.
    - **No uniffi present in the repo.** Zero matches for `uniffi`, `uniffi_macros`, or `.udl`.
    - **`vox-tauri-stt` plugin has 6,037 LoC of working Kotlin (Android) + Swift (iOS) sitting idle behind a stubbed Rust glue layer (`Err("native STT bridge not yet connected …")`).** The native code was written ahead of an FFI layer that hasn't materialized.
    - **`mobile_emit.rs` (119 LoC) emits `@tauri-apps/api/event` listener wiring** for `@back_button`, `@deep_link`, `@push`, but these handlers are never registered in any built mobile app today.
    - **`vox-mental-tracker` (576 LoC of `.vox` source) builds via Capacitor**, with Vox → TS → Vite → Capacitor. The mic UI exists; transcription is stubbed because the Rust ↔ native FFI is missing.

The conclusion ADR-037 reached was directionally correct for desktop and incorrect for mobile. This ADR corrects the mobile half without disturbing the desktop half.

## Decision

1. **Tauri 2 remains canonical for Vox on desktop** (Windows, macOS, Linux). No change to `crates/vox-gui`, `crates/vox-tauri-codegen`, or the Tauri-flavored mobile-primitive emit on the desktop path.
2. **Mobile target becomes React Native + Expo (managed workflow) for the GUI layer**, with [uniffi-bindgen-react-native](https://github.com/jhugman/uniffi-bindgen-react-native) bridging Vox's existing Rust runtime crates onto the device. The migration architecture is specified in [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md).
3. **Tauri-mobile is retired** as a Vox target. Future Tauri-mobile maturity may justify revisiting; the kill-criteria for re-evaluation are in §Reconsideration triggers.
4. **Capacitor is retired** as a Vox target. The current `vox-mental-tracker` Capacitor scaffolding migrates to Expo (managed workflow) in Phase 1 of the RN+Expo migration plan.
5. **The Vox Rust emit is preserved unchanged** for desktop. The same `vox-runtime`, `vox-workflow-runtime`, `vox-actor-runtime`, MENS/Candle inference crates cross-compile to mobile architectures and ship to the device via uniffi-generated TurboModules.
6. **`vox-tauri-stt`'s native Kotlin/Swift code is retired**; on-device transcription is reimplemented in Phase 2 of the migration as a uniffi-wrapped Candle Whisper running inside `vox-runtime`. The 6K LoC of unwired native plugin code is removed (its existence today is a tech-debt liability, not an asset).

## Preconditions

This ADR is **Proposed**, not **Accepted**, until two pre-existing CLI bugs are resolved. Both predate this ADR and block any mobile codegen work because mobile would build on top of the same orchestration path.

1. **`crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:288` panic.** `vox build` panics on every component-bearing source (verified on `examples/golden-ts/component_state.vox`, 2026-05-28). Error: `HirModule serializes to JSON ... key must be a string`. The codegen library is correct in isolation; the failure is in the CLI orchestration's JSON serialization of HirModule. Tracked as a separate task.
2. **Struct-literal-in-fn typeck regression.** `examples/golden-ts/wire_format_round_trip.vox` fails typeck with "Undefined variable: <TypeName>" when a struct literal appears in a function body. Surfaced via the golden-test harness audit (2026-05-27). Tracked as a separate task.

Both bugs reveal a structural gap: **there is no end-to-end CLI integration test that runs `vox build <real component>.vox` and asserts the output compiles.** The codegen library is well-tested via snapshot tests that bypass the CLI path; the CLI path is functionally untested. Adding such a test harness is a Phase 0 deliverable of the mobile migration plan and a precondition of this ADR's Acceptance.

## Consequences

**Positive:**

- Vox ships to mobile via a path with real LLM-authorability and a managed iOS-from-Windows build pipeline (EAS Build).
- Vox's Rust runtime (workflows, actors, on-device ML inference) ships to the phone via the well-trodden uniffi path. Mozilla uses uniffi-rs in production for hundreds of millions of users.
- The "one VUV syntax → all devices" north star is preserved: VUV HIR remains the single source of truth; mobile lowers to RN+Expo, desktop lowers to React DOM + Tauri.
- Tauri desktop remains unchanged. The mature surface continues to mature.
- Removing 6K LoC of unwired Kotlin/Swift from `vox-tauri-stt` and the Capacitor scaffolding clears tech debt the team is currently carrying for no benefit.

**Negative:**

- ~5-10K LoC of new emitter, runtime, and JS adapter code over the migration period (specified in [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md)).
- Ongoing maintenance of two GUI lowerings (web React + RN). Mitigation: VUV-style IR + `@vox/runtime` JS interface contract contain the "twice" surface to a per-primitive mapping table + a generic walker.
- Expo SDK and React Native release-cadence churn (Expo: ~3 majors/year; RN: ~6 minors/year). Mitigation: `expo install` keeps SDK alignment automatic; pin emitted versions in `vox-emit` per release.
- EAS Build is paid SaaS beyond the free tier. Mitigation: free tier supports solo dev; self-hosted GH Actions macOS runner is an escape hatch.

**Neutral but worth naming:**

- The Codegen SSOT Unification 2026 plan absorbs the RN lowering as part of its "ship `@vox/runtime` npm" phase. Net additional work over that plan is the uniffi bridge and the RN-specific style IR translator.
- ADR-037's "Tauri canonical for X" framing is retained in this ADR for desktop. The ADR is not a full repeal — it's a scope reduction.

## Reconsideration triggers

Revisit this decision if any of the following land:

- Tauri ships a managed iOS-from-Windows build service equivalent to EAS Build.
- Tauri-mobile plugin parity reaches within one capability of Expo's first-party SDK (currently a four-plugin gap: remote push, IAP, contacts, background tasks).
- A published LLM-eval benchmark shows Tauri-mobile generation success within 20% of RN+Expo on representative mobile tasks.
- React Native or Expo announces deprecation, sale, or strategic pivot that materially changes the "Expo is the recommended way to use RN" baseline.

A quarterly tracking note should be maintained at `docs/src/architecture/rn-tauri-mobile-maturity-tracker.md` (not part of this ADR; created in Phase 0 of the migration).

## Compliance with existing code

| Crate / file | Action |
|---|---|
| `crates/vox-gui` | Unchanged. Continues as Tauri 2 desktop shell. |
| `crates/vox-tauri-codegen` | Scope narrowed to desktop config emission. Any mobile config-emit code (if present) retires in Phase 3. |
| `crates/vox-codegen/src/codegen_ts/mobile_emit.rs` | Refactored in Phase 1 to emit to a `@vox/runtime` adapter contract rather than `@tauri-apps/api/event` directly. Two implementations (Tauri-flavored for desktop, Expo-flavored for mobile) fulfill the contract. |
| `crates/vox-tauri-stt/src/plugin.rs` | Retired in Phase 2. The native Kotlin/Swift code is removed in the same PR. |
| `apps/vox-mental-tracker` | Migrates from Capacitor to Expo in Phase 1 of [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md). Capacitor refs deleted. |
| `crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:288` | Bug fix is a precondition of this ADR's Acceptance, not part of it. Tracked separately. |
| `crates/vox-compiler/src/typeck/...` (struct-literal-in-fn) | Bug fix is a precondition. Tracked separately. |
| New: end-to-end CLI integration test harness | Phase 0 deliverable; precondition of mobile work, not gated by this ADR but blocks downstream phases. |
| Documentation: [tauri-convergence-migration-plan-2026.md](tauri-convergence-migration-plan-2026.md) | Phase 5+ rewritten to reference [mobile-rn-expo-architecture-and-migration-2026.md](mobile-rn-expo-architecture-and-migration-2026.md). |
| Documentation: `adr-037-tauri-canonical-platform.md` (not yet filed) | Status changed from "Accepted" to "Superseded by ADR-NNN (this doc) for the mobile scope; remains in force for desktop." |

## Notes for the Acceptance review

The reviewer should verify, before changing status from Proposed to Accepted:

1. The two preconditioning bugs have landing PRs with passing CI.
2. The CLI-path integration test harness has at least one test that runs `vox build` on a real component source and asserts non-empty buildable TSX output.
3. `mobile-rn-expo-architecture-and-migration-2026.md` Phase 0 deliverables (VUV-style IR spec, mobile runtime profile spec, NativeWind-vs-Tamagui-vs-StyleSheet decision) have written drafts.
4. The cost estimate of ~5-10K LoC over the migration is reviewed against actual Phase 0 scoping work and updated if materially off.
