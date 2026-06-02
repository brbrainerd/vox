---
title: "GitHub-hosted runner exceptions"
description: "Official documentation for GitHub-hosted runner exceptions for the Vox language. Detailed technical reference, architecture guides, and i"
category: "CI & Quality"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# GitHub-hosted runner exceptions

The repository defaults to **self-hosted** runners for main Rust CI (see [runner contract](runner-contract.md)). The following workflows intentionally use **GitHub-hosted** runners:

| Workflow | Runner | Reason |
|----------|--------|--------|
| `docs-deploy.yml` | `ubuntu-latest` | GitHub Pages deploy + mdBook; portable Pages API. |
| `docs-quality.yml` | `ubuntu-latest` | mdBook + `vox-doc-pipeline --check` + link/SUMMARY gates; no self-hosted pool dependency; matches other docs-advisory jobs. |
| `link_checker.yml` | `ubuntu-latest` | External link checks; no secrets to self-hosted pool. |
| `release-binaries.yml` | `windows-latest`, `macos-latest` (×2 targets: x86_64 and aarch64 macOS jobs) | Publish tagged Windows/macOS binaries; Linux **build** lane remains self-hosted; **publish** job runs on Linux self-hosted. |
| `publish-ci-runner.yml` | `ubuntu-latest` | Chicken-and-egg: builds the self-hosted runner image itself; cannot run on the fleet it produces. Publishes to GHCR so contributors can `docker pull ghcr.io/<org>/vox-ci-runner:latest`. |
| `deploy-hetzner.yml` (`smoke-ci`) | `ubuntu-latest` | Thin pre-deploy check: **`cargo build -p vox-cli --locked`** only. Full fmt/clippy/tests run on self-hosted **`ci.yml`** before merge to **`main`**. |
| `setup-e2e.yml` (`Clean Room Setup + Idempotency`) | `ubuntu-latest`, `windows-latest`, `macos-latest` | Clean-room E2E proves `scripts/setup.vox` provisions a workspace from scratch on **all three host OSes** (it strips the toolchain and re-bootstraps). The self-hosted pool is Linux-x64 only, so cross-platform host runners are required to exercise the Windows/macOS provisioning paths. |

Any new workflow using GitHub-hosted runners (`ubuntu-latest`, `windows-latest`, `macos-latest`) must add a row here or switch to the self-hosted tuple.

**Not GitHub-hosted (self-hosted only):** [`ci.yml`](../../../.github/workflows/ci.yml) and [`ml_data_extraction.yml`](../../../.github/workflows/ml_data_extraction.yml) use **`[self-hosted, linux, x64]`** (plus **`docker`** / CUDA lanes per [runner contract](runner-contract.md)). They are listed here so agents do not mistake them for missing exceptions — see [workflow enumeration](workflow-enumeration.md) for step-level detail.

