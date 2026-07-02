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
| `ci-fallback-hosted.yml` | `ubuntu-latest`, `windows-latest` | Required-equivalent outage valve: nightly `schedule` + `fleet-down`-labelled PR trigger + `workflow_dispatch`. Its `gate` job is named identically to the required context so it satisfies branch protection when the fleet is down (see runner-contract.md break-glass). **All jobs, including `gui-windows-build-smoke`, are gated on `fleet-down` for PR events** (2026-07-02: the Windows smoke job previously had no such gate and ran on every PR synchronize — fixed). |
| `setup-e2e.yml` | `ubuntu-latest`, `windows-latest`, `macos-latest` (matrix) | Clean-room `scripts/setup.vox` on all three host OSes; **nightly + path-filtered main pushes only** (not per-PR). |
| `cross-platform-check.yml` | matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) | Genuinely needs win/mac/linux compile paths the Linux Docker fleet cannot emulate. |
| `gui-cross-build.yml` | matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) | Tauri GUI compilation requires native host tooling (WebKitGTK on Linux, Xcode on macOS, MSVC on Windows). |
| `mobile-e2e-android.yml` | `macos-13` | Android emulator E2E requires macOS host tooling. |
| `mobile-e2e-ios.yml` | `macos-latest` | iOS simulator E2E requires macOS host. |
| `vox-visus-audit.yml` | `windows-latest` | Windows-native Visus audit surface. |
| `ci-timings.yml` | `ubuntu-latest` | Telemetry ingest from GitHub-hosted timing API; low-frequency advisory job. |
| `docker-eval.yml` | `ubuntu-latest` | Isolated Docker eval harness (not merge-gated). |
| `coolify-eval-sync.yml` | `ubuntu-latest` | Remote Coolify deploy hook; not merge-gated. |
| `scorecard.yml` | `ubuntu-latest` | OpenSSF Scorecard weekly supply-chain signal; read-only SARIF upload, not merge-gated. |
| `codeql.yml` | `ubuntu-latest` | Neutral-infra security scan (compute-placement.md). The Rust extractor's PR diff-range analysis OOMs in the ~4 GB self-hosted job container; 16 GB hosted runner fixes it. |
| `ci.yml` | `ubuntu-latest` (some jobs) | The required `ci-summary` aggregator runs hosted so the sole branch-protection context is fleet-independent (compute-placement.md Invariant 1); `mesh-compose-config` parses `docker compose` (self-hosted docker runner lacks the compose plugin); node smoke jobs. All heavy build/test jobs stay self-hosted. |
| `deploy-telemetry.yml` | `ubuntu-latest` | Coolify deploy critical path; the self-hosted fleet must never sit between a green main and a live deploy (Invariant 4). |
| `docker-telemetry.yml` | `ubuntu-latest` | GHCR telemetry image build on the deploy path; free public minutes by policy (compute-placement.md §vox placement). |
| `distribution-parity.yml` | `ubuntu-latest` | Fleet-independent required parity check — stays green when the fleet is down (Invariant 1). |
| `version-tag-guard.yml` | `ubuntu-latest` | Lightweight tag-only release guard; fleet-independent by design. |
| `workflow-lint.yml` | `ubuntu-latest` | actionlint + zizmor; install in seconds, need no self-hosted resources. Non-required early-warning surface. |
| `ci-health-deadman.yml` | `ubuntu-latest` | CI fleet health deadman switch; must run on a GitHub-hosted runner so it stays live when the self-hosted fleet is down. |
| `ci-health-watchdog.yml` | `ubuntu-latest` | CI health watchdog monitor; fleet-independent by design (Invariant 1). |
| `ci-health-watchdog-test.yml` | `ubuntu-latest` | Watchdog integration test; isolated harness that needs no self-hosted resources. |

> `compile-matrix.yml` no longer appears here: its Windows/macOS help-smoke jobs were cut (the Linux lane is self-hosted), so it uses no hosted runner. `cross-platform-check.yml` / `gui-cross-build.yml` keep their rows but now run Win/macOS legs only on `merge_group` + schedule (not per-PR).

Any new workflow using GitHub-hosted runners (`ubuntu-latest`, `windows-latest`, `macos-*`) must add a row here **or** migrate to `[self-hosted, linux, x64]` (plus `docker` / `browser` when needed).

**Enforcement (ENFORCED, not advisory):** `vox ci runner-policy-check` runs `--strict` inside `vox ci ssot-drift`. Both CI and the fast `vox ci pre-push` tier run `ssot-drift`, so an unregistered GitHub-hosted `runs-on` **hard-fails both**; **CI is authoritative** (pre-push can be `--no-verify`-skipped). Placement rationale: [compute-placement.md](compute-placement.md).

**Migrated to self-hosted (no exception row):** `gitleaks.yml`, `link_checker.yml`, `docs-quality.yml`, `ts-emit-noemit.yml`, `cr-l-gates.yml`, `mobile-eas-build.yml`, `mutation-pr.yml`, and most advisory/nightly Rust jobs. (`ci.yml` is *mostly* self-hosted but has a few hosted jobs — see its table row above.)

**Predominantly self-hosted:** [`ml_data_extraction.yml`](../../../.github/workflows/ml_data_extraction.yml) uses **`[self-hosted, linux, x64]`** (plus **`docker`** / **`browser`** / **`gpu`** per [runner contract](runner-contract.md)). `ci.yml` runs its heavy build/test jobs self-hosted but keeps the required `ci-summary` aggregator + a couple of smoke/compose jobs hosted (table row above). See [workflow enumeration](workflow-enumeration.md) for step-level detail.
