---
title: "Workflow enumeration (GitHub Actions)"
description: "Official documentation for Workflow enumeration (GitHub Actions) for the Vox language. Detailed technical reference, architecture guides,"
category: "CI & Quality"
last_updated: "2026-06-03"
training_eligible: true

schema_type: "TechArticle"
---

# Workflow enumeration (GitHub Actions)

| File | Purpose |
|------|---------|
| `.github/workflows/ci.yml` | **`runs-on: [self-hosted, linux, x64]`** (basic Linux pool). `cargo build -p vox-cli`, then guards via **`vox ci`** (`cargo run -p vox-cli --quiet -- ci …`): `manifest`, `line-endings` (forward-only diff vs `GITHUB_BASE_SHA`…`GITHUB_SHA` on PRs), `check-codex-ssot`, `check-docs-ssot` (includes stale doc/workflow ref scan), `doc-inventory verify`, **`eval-matrix verify`**, **`eval-matrix run --milestone m3-dei-contracts`** (bounded matrix-runner smoke), **`cargo check -p vox-cli --features gpu`** (compile smoke), `workflow-scripts`, `toestub-scoped`, `feature-matrix`, `no-vox-orchestrator-import`, `cuda-features`, **`openclaw-contract`** (protocol fixture guard); `cargo fmt --check`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `cargo clippy --workspace --all-targets -- -D warnings`, repository/orchestrator/MCP smoke, `cargo check -p vox-cli --features gpu,mens-qlora,stub-check`, **`cargo llvm-cov nextest --workspace --profile ci`** (toolchain **`llvm-tools-preview`** + **`cargo-llvm-cov`**), then **`cargo llvm-cov report`** without **`--workspace`** (text + JSON summary + LCOV; **`report`** only aggregates the last instrumented run), **`vox ci coverage-gates --mode enforce`**, artifact upload, **`cargo test --workspace --doc`**, **`mens-gate --profile ci_full`** (full Mens gate matrix from `scripts/populi/gates.yaml`). **Sibling job `vox-browser-cdp-smoke`:** **`runs-on: [self-hosted, linux, x64, browser]`**, **`cargo test -p vox-browser -- --ignored`** with **`VOX_BROWSER_NO_SANDBOX=1`** (Chromium/CDP via chromiumoxide; requires Chrome/Chromium on the runner). Optional shell twins: [`scripts/README.md`](../adr/index.md). Intentional duals: [command-surface-duals](command-surface-duals.md). |
| `.github/workflows/docs-deploy.yml` | Build `vox-doc-pipeline`, run doc pair extraction, mdBook build, Pages artifact. |
| `.github/workflows/deploy-hetzner.yml` | **`push: main`** Automated deployment to Hetzner Coolify VPS. Gate 1 is a thin **`cargo build -p vox-cli --locked`** on **`ubuntu-latest`** (fmt/clippy/tests are **`ci.yml`**); Gates 2–4 poll Coolify and probe public **`/health`**. |
| `.github/workflows/docs-quality.yml` | **`runs-on: ubuntu-latest`** (documented exception). mdBook toolchain, **`cargo run -p vox-doc-pipeline -- --check`** (blocking), advisory mdBook build / markdownlint / internal link steps. |
| `.github/workflows/link_checker.yml` | Link validation for docs site. |
| `.github/workflows/ml_data_extraction.yml` | ML / corpus maintenance jobs. Grammar drift via **`vox ci grammar-drift --emit github`**; eval summary via **`vox corpus eval --print-summary`** (no Python). |
| `.github/workflows/release-binaries.yml` | Tag-only release publish (`v*`): matrix **`vox ci release-build --package both`** for Linux x64, Windows x64, macOS x64 + **Apple Silicon** (`aarch64-apple-darwin`), using **`cargo run --locked`**. Each matrix job builds and smoke-tests both `vox` and `vox-bootstrap` archives (`vox --version`, `vox-bootstrap --help`) before upload; publish job merges `checksums.txt`. See [binary release contract](binary-release-contract.md). |
| `.github/workflows/pm-provenance-verify.yml` | **`workflow_dispatch` only:** writes a minimal `vox.pm.provenance/1` fixture under `.vox_modules/provenance/` and runs **`vox ci pm-provenance --strict`** (PM publish lane smoke; separate from binary tags). Add a `schedule:` block locally if you want periodic self-hosted runs. |
| `.github/workflows/mutation-nightly.yml` | **Schedule / `workflow_dispatch`:** **`cargo mutants -p vox-compiler`** with **`cargo-nextest`** (pilot; config `.cargo/mutants.toml`). Self-hosted Linux pool. |

**CUDA / GPU compile gates:** when a job needs `nvcc` or CUDA-enabled `cargo check`, use the **Docker** self-hosted profile (`[self-hosted, linux, x64, docker]`) per [runner contract](runner-contract.md); keep `runs-on` explicit per job.

> **GitLab mirror retired (2026-06-03).** The `.gitlab-ci.yml` mirror that
> once tracked these Rust guards, tests, docs, and ML jobs has been **deleted**;
> GitHub Actions (`.github/workflows/`) is the sole CI surface and GitLab CI is
> no longer a supported target. The GitLab `vox-ci-guards` job listing and the
> GitLab↔GitHub job-parity table that previously lived here are gone with it.

`vox-workflow-runtime` tests also validate representative interpreted journal event rows against `contracts/workflow/workflow-journal.v1.schema.json` (including retry and mesh event families across feature modes), so CI catches v1 contract drift in both event shape and replay paths. The compose smoke lanes (`mens-compose-config`, `docker-vox-image-smoke`) live in `ci.yml`; see [deployment compose SSOT](../reference/deployment-compose.md).

