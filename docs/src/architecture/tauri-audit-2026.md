---
title: "Tauri Audit 2026"
description: "Codebase audit of Tauri usage, capability coverage, build cost, and retirement candidates for Vox desktop/mobile GUI pipelines."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
---
# Tauri audit (2026-05-11)

## 1) Executive summary

**Status (2026-05-11 update):** ADR 037 implementation is **in progress on `main`**: `vox compile --target desktop|mobile-*` emits a real Tauri 2 workspace under `target/generated/` (`src-tauri/`, `tauri.conf.json`); `native-binary` keeps the Axum + embedded SPA path. `apps/vox-mental-tracker` consumes **`vox-tauri-stt-guest`** instead of Capacitor. Historical evidence below is retained; treat numbered findings as a **time-capsule** unless the surrounding paragraph says *current*.

**Canonical decision:** [ADR 037 — Tauri Convergence](../adr/037-tauri-convergence.md). Execution checklist: [Tauri convergence migration plan (2026-Q2)](tauri-convergence-migration-plan-2026.md).

Prior audit conclusion (pre-implementation):

Tauri was previously a **manifest-hint layer**, not a full build/runtime dependency; mobile dev/e2e ran through Capacitor. That split is **retired** for `vox-mental-tracker` and the codegen desktop/mobile shell.

## 2) Current-state audit (what code does today)

### 2.1 Tauri in Rust deps: absent from generated runtime

- `vox-tauri-codegen` itself is serialization + file emission (`anyhow`, `serde*`, `vox-compiler`), with no Tauri runtime crates (`crates/vox-tauri-codegen/Cargo.toml:9-14`).
- Generated backend `Cargo.toml` includes Axum/Tokio/rust-embed and Vox crates, but no `tauri` or `tauri-build` (`crates/vox-codegen/src/codegen_rust/emit/mod.rs:153-173`).

### 2.2 `vox compile` behavior by target

- `native-binary` calls bundle app mode only (`crates/vox-cli/src/commands/compile.rs:56-64`).
- `desktop`, `mobile-android`, `mobile-ios` call bundle app mode, then emit Tauri packaging hints (`crates/vox-cli/src/commands/compile.rs:66-76`, `251-272`).
- `server` is explicitly redirected to deploy workflows, not packaging (`crates/vox-cli/src/commands/compile.rs:85-89`).

### 2.3 What bundle/build produce

- `vox bundle` app path runs build pipeline + Vite/pnpm + `cargo build` for `vox_generated_app` (`crates/vox-cli/src/commands/bundle.rs:86-160`, `257-298`).
- Generated runtime binds Axum on `127.0.0.1` and serves embedded/static content (`crates/vox-codegen/src/codegen_rust/emit/http.rs:304-410`).
- `vox build` mobile path currently calls Capacitor sync (`npx cap sync`) when target is ios/android (`crates/vox-cli/src/commands/build.rs:345-360`).

### 2.4 What `vox-tauri-codegen` emits

- Emits `tauri-packaging/tauri.conf.json` + `README.md`; optionally emits `runtime-capabilities.projection.json` when capability SSOT exists (`crates/vox-tauri-codegen/src/lib.rs:195-219`, `221-246`).
- Uses `find_contracts_repo_root` to walk upward for `contracts/capability/runtime-capabilities.v1.yaml` (`crates/vox-tauri-codegen/src/lib.rs:55-68`).
- README explicitly states consumers still run `cargo tauri build` in their own Tauri workspace (`crates/vox-tauri-codegen/src/lib.rs:233-237`).

### 2.5 CI behavior (compile and mobile)

- Compile-matrix smoke builds `vox-cli`, runs `vox ci compile-matrix`, and Linux also smoke-tests compile-suite `native-binary`; no `cargo tauri build` step exists (`.github/workflows/compile-matrix.yml:33-37`, `45-55`).
- Android e2e path is Capacitor/Gradle (`pnpm build && npx cap sync android`, `gradlew assembleDebug`) (`.github/workflows/mobile-e2e-android.yml:31-43`).

## 3) Claimed-state audit (what docs currently claim)

- Packaging SSOT says desktop/mobile installers are produced via Tauri 2 scaffolding (`docs/src/architecture/vox-application-packaging-ssot-2026.md:59-63`).
- Journey C claims `vox compile --target mobile-android|mobile-ios` produces install artifacts when tooling is present (`docs/src/architecture/vox-application-packaging-ssot-2026.md:41-43`).
- GUI roadmap and ADR-024 position the dashboard/web direction around Axum-served SPA and explicitly rejected Tauri for that dashboard host (`docs/src/architecture/vox-gui-native-roadmap-2026.md:22-24`; `docs/src/adr/024-dashboard-axum-spa.md:26-29`, `35-38`).

Net: language-level roadmap and dashboard architecture are Axum/web-first, while packaging SSOT currently overstates live Tauri execution.

## 4) Tauri 2 mobile feasibility for Vox apps

### 4.1 What is already feasible

For non-ASR-heavy mobile GUI apps, Tauri 2 mobile can satisfy the same practical surface area currently used through Capacitor:
- Filesystem, clipboard, notifications, deep-link, HTTP, and shell-level integration (Tauri 2 plugin model).
- Standard webview host model (Android/iOS webviews).
- Packaging and permission model that can consume capability maps.

### 4.2 Concrete blocker found in this repo

`vox-mental-tracker` documents why it remains on Capacitor:
- Packaging note says migration waits for Tauri-native Sherpa equivalent (`apps/vox-mental-tracker/README.md:5`).
- App deps/scripts are explicitly Capacitor-first (`apps/vox-mental-tracker/package.json:8-16`, `23`, `40-42`).

This is the key gating item for full mobile unification in this codebase.

## 5) Build-time and CI cost picture

### Today
- No Tauri Rust compile in generated backend path.
- Compile matrix validates command wiring and one Linux native-binary workspace smoke only (`.github/workflows/compile-matrix.yml:33-37`).
- Mobile CI cost currently sits in Node+Capacitor+Gradle (`.github/workflows/mobile-e2e-android.yml:31-43`).

### If real Tauri builder were adopted
- Generated app crate would need Tauri runtime/build deps (not present today in `emit_cargo_toml`) (`crates/vox-codegen/src/codegen_rust/emit/mod.rs:153-173`).
- CI would need explicit platform Tauri build lanes (desktop and/or mobile), adding webview/toolchain setup and longer cold build/link stages.

### If staying hint-only
- Current compile performance profile remains mostly unchanged; only docs/validation surface changes.

## 6) Strategic option matrix (cost/risk framing)

| Option | Engineering cost | CI impact | Runtime alignment | Main risk |
|---|---:|---:|---|---|
| Adopt Tauri desktop+mobile now | High | High | Strong single shell | Tauri mobile maturity + Sherpa port effort |
| Tauri desktop, Capacitor mobile | Medium | Medium | Split pipeline | Ongoing dual-maintenance burden |
| Keep hybrid/hints, clean seams | Low-medium | Low | Honest to current reality | Defers full installer unification |
| Retire Tauri hints entirely | Low | Low | Simplifies now | Loses forward path to installer-native lane |

## 7) Free-wins retirement candidates (independent of strategy)

1. `tauri_stub` seam appears unused:
   - Module + re-export exist (`crates/vox-codegen/src/codegen_rust/emit/tauri_stub.rs:1-12`, `crates/vox-codegen/src/codegen_rust/emit/mod.rs:18`, `30`).
2. `mobile_emit` hard-imports `@tauri-apps/api/*` when mobile hooks exist (`crates/vox-codegen/src/codegen_ts/mobile_emit.rs:26-31`) but templates/scaffolds do not provision those packages by default in this repo's common generation path.
3. `init` prints a `target/generated/tauri-packaging` next-step for mobile templates, but current emission there writes to project-root `tauri-packaging/` (`crates/vox-cli/src/commands/init.rs:33-35`, `94-103`).
4. Packaging SSOT wording should be made explicitly current-state or explicitly planned-state to avoid drift (`docs/src/architecture/vox-application-packaging-ssot-2026.md:59-63`).
5. Add a generation contract test around `mobile.ts` import assumptions before changing strategy.

## 8) Recommendation framing (advisory)

Given the stated goal ("single pipeline we have to maintain mobile and desktop"):
- **If on-device Sherpa transcription remains mandatory in the near term**, either:
  - Keep hybrid while scheduling Sherpa plugin port to Tauri mobile, then converge, or
  - Accept temporary split and clearly timebox it.
- **If Sherpa dependency can be deferred/substituted**, full Tauri convergence becomes materially simpler.

This is a product/engineering trade-off decision, not a tooling mystery:
- most non-ASR mobile capabilities are not the blocker,
- native Sherpa bridge shape is the blocker in this repo.

## 9) Decision criteria for follow-up ADR

The follow-up ADR should explicitly answer:
1. Is temporary dual-shell operation acceptable, and for how long?
2. Who owns `vox-sherpa-transcribe` Tauri porting, and what acceptance tests gate completion?
3. Should `native-binary` remain a first-class Axum lane even after Tauri adoption?
4. Should capability projection remain merge-hint JSON or become directly consumed by generated Tauri config/capability files?
5. What CI lanes become required (desktop only vs full mobile)?

## 10) References

### In-repo
- [Vox application packaging SSOT (2026)](vox-application-packaging-ssot-2026.md)
- [Vox GUI-Native Language Roadmap (2026)](vox-gui-native-roadmap-2026.md)
- [ADR 024 — Dashboard as local Axum-served SPA](../adr/024-dashboard-axum-spa.md)
- [Dashboard Migration Research (2026)](dashboard-migration-research-2026.md)
- [compile-matrix workflow](../../../.github/workflows/compile-matrix.yml)
- [mobile e2e workflow](../../../.github/workflows/mobile-e2e-android.yml)

### External context (for mobile maturity and plugin availability checks)
- [Tauri 2.0 stable release post](https://v2.tauri.app/blog/tauri-20/)
- [Tauri mobile plugin development docs](https://v2.tauri.app/develop/plugins/develop-mobile/)
- [Tauri notification plugin docs](https://v2.tauri.app/plugin/notification/)
- [silvermine/tauri-plugin-sqlite](https://github.com/silvermine/tauri-plugin-sqlite)
- [tauri-plugin-audio-recorder](https://crates.io/crates/tauri-plugin-audio-recorder)

## Notes

- This audit intentionally records **what is implemented now** and separates that from strategic direction decisions.
- No code or pipeline behavior is changed by this document.
- Resolved by: [ADR 037 — Tauri Convergence](../adr/037-tauri-convergence.md) and [Tauri convergence migration plan (2026-Q2)](tauri-convergence-migration-plan-2026.md).
