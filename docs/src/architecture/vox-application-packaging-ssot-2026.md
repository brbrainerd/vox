---
title: "Vox application packaging SSOT (2026)"
description: "Sibling contract to deploy/OCI portability: end-user native installers (desktop + mobile), workspace suites, and `vox compile` journeys."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
schema_type: "TechArticle"
---

# Vox application packaging SSOT (2026)

This document is the **normative contract for shipping installable Vox applications** (desktop `.exe` / `.msi` / `.dmg` / `.AppImage`, mobile `.apk` / `.aab` / `.ipa`, and script binaries). It **does not replace** [Vox portability SSOT](../reference/vox-portability-ssot.md), which remains authoritative for **Docker/OCI-backed server deploys**.

## Relationship to other contracts

| Concern | Authority |
| --- | --- |
| Server/container deploy | [vox-portability-ssot](../reference/vox-portability-ssot.md), `vox deploy`, `vox-container` |
| End-user native installers | **This document**, `vox compile`, `[bundle]` and `[workspace]` in `Vox.toml` |
| Toolchain binary releases (`vox` CLI zips) | [binary-release-contract](../ci/binary-release-contract.md), `vox ci release-build` |
| CLI surface | [cli.md](../reference/cli.md), `contracts/cli/command-registry.yaml` |

## User journeys and acceptance UX

### Journey A — Solo developer, desktop

1. Author `.vox` + frontend; `vox run` for dev.
2. Run `vox compile --target desktop` (or `--target native-binary` for Axum + embedded SPA without Tauri bundler).
3. **Acceptance:** an executable or installer appears under `dist/` (or project-configured output); double-click or `./app` starts the UI without requiring a separate `cargo run` from `target/generated`.

### Journey B — Workspace suite

1. Root `Vox.toml` declares `[workspace.members]` and optional root `[bundle]` (brand, version, identifiers).
2. Member packages each have their own `Vox.toml` and entry `.vox`.
3. `vox compile --workspace` builds every member package with the requested `--target`; split target-specific packages into separate workspaces when a suite needs different packaging lanes.
4. **Acceptance:** one coherent version line and shared `[bundle]` defaults; members may override per-package.

### Journey C — Mobile (Tauri)

1. `vox compile --target mobile-android` emits the Tauri workspace under the repository `target/generated/` tree (shared with `vox build` / `vox bundle`). Final `.apk` / `.aab` requires Android SDK/NDK and `cargo tauri android build` after project init.
2. `vox compile --target mobile-ios` on macOS + Xcode emits the same generated tree; `.ipa` / simulator builds use `cargo tauri ios build` / Xcode.
3. **Acceptance:** documented prerequisites (`vox doctor --compile-target …` surfaces Rust targets, SDK roots, and `cargo tauri --version`); failures print toolchain hints.

### Journey D — Script binary

1. `vox compile --target script path/to/script.vox` (requires `script-execution` feature).
2. **Acceptance:** single native binary (or `.wasm` for WASI), analogous to “compiled script” UX.

## Declarative manifest

Project-local **`Vox.toml`** MAY include:

- `[bundle]` — identifier, display name, version, asset paths, signing env hints (see `contracts/manifest/vox-bundle.v1.schema.json`).
- `[workspace.members]` — array of relative paths to member packages (Cargo-style workspace list).

Secrets MUST NOT be embedded in `[bundle]`; use Clavis / `vox-secrets` at runtime per [AGENTS.md](../../../AGENTS.md).

## Native shell (Tauri 2)

Desktop and mobile installers are produced via **Tauri 2** scaffolding emitted next to the generated Rust + Vite app (`vox-tauri-codegen`). The Axum + React emit path remains the codegen core; Tauri wraps the webview and bundling.

`vox compile --target desktop|mobile-*` also emits **`runtime-capabilities.projection.json`** beside `tauri.conf.json` when the workspace contains [`contracts/capability/runtime-capabilities.v1.yaml`](../../../contracts/capability/runtime-capabilities.v1.yaml) — a machine-readable projection filtered to the app's compiler-derived capability ids (`@uses` / `uses` plus inferred packaging capabilities) with Tauri permission IDs / Android `uses-permission` / iOS plist keys for merging into upstream Tauri manifests.

For a codebase-wide implementation-vs-claim audit and follow-up decision criteria, see [Tauri audit (2026-05-11)](tauri-audit-2026.md).

For the accepted convergence decision and executable migration sequence, see [ADR 037 — Tauri Convergence](../adr/037-tauri-convergence.md) and [Tauri convergence migration plan (2026-Q2)](tauri-convergence-migration-plan-2026.md).

## Verification

- `vox ci retirement-audit` — scan `vox-deprecated-since` / `retire-by` markers against the workspace semver; fails when a marker is overdue.
- `vox ci compile-matrix` — local smoke for `vox compile --help` (prefers an existing `target/{debug,release}/vox` binary when present so Windows avoids relinking a locked `vox.exe`; if cargo run fails to relink, run from the built `vox.exe`; falls back to `cargo run -p vox-cli -- compile --help`); CI matches `.github/workflows/compile-matrix.yml`.
- CI: `.github/workflows/compile-matrix.yml` exercises compile wiring on Linux self-hosted, Windows, and macOS hosts.
- Fixture workspace layout: [`examples/compile-suite/`](../../../examples/compile-suite/README.md) (`[workspace.members]` + per-member `Vox.toml`).

## Related

- [GUI authoring / VUV](gui-authoring-syntax-2026.md) — authoring layer unchanged; native promotion hooks are codegen concerns.
- [Frontend convergence](frontend-convergence-findings-2026.md) — Contract IR and TS emit remain SSOT seams.
