---
title: "GitHub-hosted runner exceptions"
description: "Registered exceptions for workflows that intentionally use GitHub-hosted runners instead of the local self-hosted fleet."
category: "CI & Quality"
last_updated: "2026-06-10"
training_eligible: true

schema_type: "TechArticle"
---

# GitHub-hosted runner exceptions

The repository defaults to **self-hosted** runners for CI (see [runner contract](runner-contract.md) §Local-first CI). The following workflows intentionally use **GitHub-hosted** runners:

| Workflow | Runner | Reason |
|----------|--------|--------|
| `docs-deploy.yml` | `ubuntu-latest` | GitHub Pages deploy + Astro docs; portable Pages API. |
| `release-binaries.yml` | `windows-latest`, `macos-latest` (matrix) | Publish tagged Windows/macOS binaries; Linux build lane is self-hosted. |
| `release-installers.yml` | `windows-latest`, `ubuntu-latest`, `macos-latest` (matrix) | Cross-platform installer packaging. |
| `release-gui.yml` | matrix (`ubuntu-latest`, etc.) | Tauri GUI release matrix across host OSes. |
| `bundle-release.yml` | matrix (`ubuntu-latest`, etc.) | Multi-target bundle publishing. |
| `publish-ci-runner.yml` | `ubuntu-latest` | Chicken-and-egg: builds the self-hosted runner image; cannot run on the fleet it produces. |
| `deploy-hetzner.yml` | `ubuntu-latest` | Remote deploy orchestration from a portable Linux host. |
| `ci-fallback-hosted.yml` | `ubuntu-latest`, `windows-latest` | **Manual only** (`workflow_dispatch`). Portable smoke when the self-hosted fleet is down. |
| `setup-e2e.yml` | `ubuntu-latest`, `windows-latest`, `macos-latest` (matrix) | Clean-room `scripts/setup.vox` on all three host OSes; **nightly + path-filtered main pushes only** (not per-PR). |
| `cross-platform-check.yml` | matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) | Genuinely needs win/mac/linux compile paths the Linux Docker fleet cannot emulate. |
| `gui-cross-build.yml` | matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) | Tauri GUI compilation requires native host tooling (WebKitGTK on Linux, Xcode on macOS, MSVC on Windows). |
| `compile-matrix.yml` | `windows-latest`, `macos-latest` | Windows/macOS compile smoke (Linux lane is self-hosted). |
| `mobile-e2e-android.yml` | `macos-13` | Android emulator E2E requires macOS host tooling. |
| `mobile-e2e-ios.yml` | `macos-latest` | iOS simulator E2E requires macOS host. |
| `vox-visus-audit.yml` | `windows-latest` | Windows-native Visus audit surface. |
| `ci-timings.yml` | `ubuntu-latest` | Telemetry ingest from GitHub-hosted timing API; low-frequency advisory job. |
| `docker-eval.yml` | `ubuntu-latest` | Isolated Docker eval harness (not merge-gated). |
| `coolify-eval-sync.yml` | `ubuntu-latest` | Remote Coolify deploy hook; not merge-gated. |
| `scorecard.yml` | `ubuntu-latest` | OpenSSF Scorecard weekly supply-chain signal; read-only SARIF upload, not merge-gated. |

Any new workflow using GitHub-hosted runners (`ubuntu-latest`, `windows-latest`, `macos-*`) must add a row here **or** migrate to `[self-hosted, linux, x64]` (plus `docker` / `browser` when needed).

**Enforcement:** `vox ci runner-policy-check` warns when a workflow uses a hosted `runs-on` without a table row (default advisory; `--strict` to fail).

**Migrated to self-hosted (no exception row):** `ci.yml`, `gitleaks.yml`, `link_checker.yml`, `docs-quality.yml`, `ts-emit-noemit.yml`, `cr-l-gates.yml`, `mobile-eas-build.yml`, `codeql.yml` (main push + weekly schedule only), `mutation-pr.yml`, and most advisory/nightly Rust jobs.

**Not GitHub-hosted:** [`ci.yml`](../../../.github/workflows/ci.yml) and [`ml_data_extraction.yml`](../../../.github/workflows/ml_data_extraction.yml) use **`[self-hosted, linux, x64]`** (plus **`docker`** / **`browser`** / **`gpu`** per [runner contract](runner-contract.md)). See [workflow enumeration](workflow-enumeration.md) for step-level detail.
