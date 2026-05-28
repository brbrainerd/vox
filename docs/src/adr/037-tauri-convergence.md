---
title: "ADR 037 — Tauri Convergence"
description: "Decision record for converging Vox desktop and mobile application packaging on Tauri 2."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-05-11"
training_eligible: true
schema_type: "TechArticle"
---

# ADR 037 — Tauri Convergence

**Status**: Accepted  
**Date**: 2026-05-11

---

## Context

Vox needs one maintainable path for generated GUI applications across desktop and mobile. The current implementation does not yet provide that. `vox compile --target desktop|mobile-*` runs the app bundle path and emits Tauri packaging hints, but the generated runtime is still an Axum server that binds `127.0.0.1` and serves embedded assets. Mobile application development and Android E2E still use Capacitor.

The codebase-wide Tauri audit found three important facts:

- Tauri is currently a manifest-hint layer, not a runtime/build dependency for generated applications.
- The generated user app uses Axum and `rust-embed`; those are appropriate for explicit server/dashboard surfaces but not for the native application shell.
- The only concrete blocker to full mobile convergence is the `vox-sherpa-transcribe` Capacitor plugin in `apps/vox-mental-tracker`.

This ADR records the product and architecture decision so implementation can proceed as a mechanical migration instead of a recurring strategy debate.

---

## Decision

1. **Tauri 2 is the canonical generated application shell** for `vox compile --target desktop`, `vox compile --target mobile-android`, and `vox compile --target mobile-ios`.
2. **Generated desktop/mobile applications must produce real Tauri projects and invoke Tauri build tooling**, not only emit `tauri-packaging/` hints.
3. **Axum remains canonical for server/dashboard surfaces only**: `vox dashboard`, explicit server targets, daemon/API surfaces, and ADR-024 dashboard integration. The generated desktop/mobile user-app shell must not depend on `axum` or `rust-embed`.
4. **Capacitor is retired for new Vox application work**. Existing Capacitor call sites are temporary, explicitly allowlisted, and removed by the Tauri convergence migration plan.
5. **`vox-sherpa-transcribe` is ported to a Tauri 2 mobile plugin** rather than preserving a long-term Tauri desktop / Capacitor mobile split.
6. **Retirement is enforced by architecture checks**: `docs/src/architecture/layers.toml` owns forbidden patterns for retired Capacitor and Axum-as-app code, with current call sites listed in `exempt_files` until each phase removes them.

---

## Rejected alternatives

- **Tauri desktop + Capacitor mobile**: keeps `vox-mental-tracker` moving in the short term, but leaves Vox with two native shells, two plugin models, and two CI/toolchain paths for the same generated GUI product.
- **Keep Tauri hint-only**: cheap and honest about current implementation, but fails the goal of shipping one native desktop/mobile application pipeline.
- **Retire Tauri entirely**: reduces build complexity, but gives up the strongest existing route to native installers and mobile webview packaging from the Vox language.
- **Keep Axum-localhost as the desktop app shell**: remains useful for `native-binary` and dashboard/server surfaces, but does not satisfy the user expectation that `desktop` means a native application bundle.
- **Defer Sherpa indefinitely**: avoids native plugin work, but keeps the one real mobile application in the repo pinned to Capacitor.

---

## Consequences

- `vox compile --target desktop|mobile-*` becomes a heavier build path. CI must explicitly account for Tauri, Android, and iOS toolchain costs rather than hiding them behind hint generation.
- `native-binary` remains available for the Axum + embedded SPA shape where a local server binary is the desired artifact.
- The generated app codegen split becomes clearer: server targets emit Axum, application targets emit Tauri.
- Capability projection moves from "merge hints for a downstream shell" toward generated Tauri config/capability files.
- `apps/vox-mental-tracker` becomes the acceptance fixture for proving Tauri mobile can carry real Vox app features, including on-device ASR through the Sherpa plugin port.
- Future contributors and coding agents get hard guardrails: adding new `@capacitor/*`, `npx cap sync`, or Axum-as-app generation outside the active migration allowlist fails architecture checks.

---

## References

- [Tauri audit (2026-05-11)](../architecture/tauri-audit-2026.md)
- [Tauri convergence migration plan (2026-Q2)](../architecture/tauri-convergence-migration-plan-2026.md)
- [Vox application packaging SSOT (2026)](../architecture/vox-application-packaging-ssot-2026.md)
- [ADR 024 — Dashboard as local Axum-served SPA](024-dashboard-axum-spa.md)
- [Vox GUI-Native Language Roadmap (2026)](../architecture/vox-gui-native-roadmap-2026.md)
