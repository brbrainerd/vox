---
title: "CI/CD Compute Placement Policy"
description: "Where each CI, CD, and nightly job runs — Hetzner VPS (always-on) vs local self-hosted fleet (CPU/disk/GPU) vs GitHub-hosted (neutral/free) — for vox (public) and FableForge (private), chosen by the gating resource."
category: "CI & Quality"
status: "current"
---

# CI/CD Compute Placement Policy

Three compute tiers are available. Place each job where its **gating resource** is
cheapest, subject to the free-tier economics below.

## Decision rule — classify by gating resource, then place

| Gating resource | Host | Why |
|---|---|---|
| CPU-parallel (compile, `clippy --all-targets`, mutation, test matrix) | Local fleet | Many cores vs few shared vCPU |
| Disk-IO / cache (cargo target, Docker layers, sccache, graphify) | Local fleet | 4 TB NVMe vs ~160–240 GB VPS disk |
| GPU (inference, LoRA/QLoRA, qwen nightly, ComfyUI) | Local fleet | RTX-class GPU; the VPS has none |
| RAM-heavy (Next.js build, Playwright/Stagehand) | Local fleet | 64 GB vs ~16 GB VPS |
| Uptime / network (deploy CD, health/TLS probes, DB maintenance, dep-bots) | Hetzner VPS | Must fire regardless of workstation state |
| Reproducibility on neutral infra (portability gate, cross-OS release, security scans) | GitHub-hosted | No private-hardware dependency |

## Free-tier economics

- **vox = PUBLIC** → unlimited free GitHub-hosted minutes. Run the entire **deploy
  critical path** (image build, Coolify trigger, Gate-3 probe) on `ubuntu-latest`
  so deploys never wait on the workstation. Use the local fleet only for raw speed
  / GPU on latency-tolerant heavy jobs.
- **FableForge = PRIVATE** → 2,000 free min/mo. Keep only the light merge gate +
  the deploy trigger on hosted; push all heavy jobs to the local fleet to conserve
  minutes.

## vox placement

| Tier | Jobs |
|---|---|
| Local fleet (`[self-hosted, linux, x64]`) | `ci.yml` build+clippy+test, `mutation-nightly`, `compile-matrix`, `bench-nightly` (pinned to one host for comparable timings), `qwen35-native-nightly` (GPU), `ml_data_extraction` |
| Hetzner VPS | deploy triggers + Gate-3 probes (`deploy-hetzner`, `deploy-telemetry`), nightly ClickHouse maintenance (TTL/OPTIMIZE, backup → object storage), live-endpoint uptime, link/dep bots |
| GitHub-hosted | Gate-1 portability build, `docker-telemetry` / `docker-eval` image builds, `release-*` cross-OS, mobile EAS, `codeql`/`scorecard`/`gitleaks`; `ci-fallback-hosted.yml` = safety valve |

> The telemetry workflows (`docker-telemetry.yml`, `deploy-telemetry.yml`) both use
> `runs-on: ubuntu-latest` — deploy critical path on free hosted minutes, by policy.

## FableForge placement

| Tier | Jobs |
|---|---|
| Local fleet | `nextjs-build-check`, `e2e-*`/stagehand/semantic-vrt, full test suites, studio-pipeline (GPU), test-coverage |
| Hetzner VPS | `deploy-hetzner`/convex-deploy/deploy-guard triggers, nightly-live-audit, archive-cron, coderabbit-ingest |
| GitHub-hosted | lint/typecheck merge gate, coverage-check reporting |

## Invariants

1. The merge gate never hard-depends on the workstation — Gate-1 portability +
   `ci-fallback-hosted` keep PRs unblockable when the fleet is down.
2. `bench-nightly` is pinned to one host (local) so timings stay comparable run-to-run.
3. DB maintenance + backups run where the data lives (Hetzner → object storage).
4. The telemetry and eval deploy critical paths stay on GitHub-hosted runners; the
   self-hosted fleet is never on the path between a green `main` and a live deploy.

> **Out of scope (follow-up):** reconciling the `runs-on` of the ~40 existing
> workflows to these matrices. This doc states the policy and applies it to the new
> telemetry workflows; a separate sweep PR should migrate existing workflows one at
> a time with CI green between each.
