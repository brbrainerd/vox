---
title: "Repository layout sprawl audit (2026)"
description: "Organization-focused audit: sparse directories, overlapping categories, and provenance of non-Rust artifacts — with a prioritized consolidation backlog."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: false
---

# Repository layout sprawl audit (2026)

This document complements [repo-cleanup-ledger-2026.md](./repo-cleanup-ledger-2026.md) (which targeted tracked orphan artifacts and surface moves). Here the focus is **taxonomy and navigation**: too many folders that read like separate products but hold almost nothing, overlapping meanings (`infra/` vs `docker/` vs root compose files), and **who produces vs ingests** files that are not Rust sources.

## Metrics (tracked files only, snapshot 2026-05-11)

| Metric | Approximate value | Interpretation |
|--------|-------------------|----------------|
| Immediate parent dirs with exactly **one** tracked file | ~203 | Many are **intentional** (single crate roots like `crates/vox-build-meta/`, VS Code feature folders, leaf contract domains). Treat as a triage list, not automatic deletes. |
| `contracts/` breadth | 342 tracked files across ~37 second-level domains | SSOT is deliberate; shrinking folder count requires **`contracts/index.yaml` + consumer path updates**, not ad hoc moves. |
| Root-level tracked files | ~40 config / policy stubs | High **cognitive load** for newcomers; most must stay at repo root for Cargo / CI / IDE tooling. |

Re-run a sparse-dir report anytime:

```powershell
git ls-files | ForEach-Object { Split-Path $_ -Parent } |
  Group-Object | Where-Object Count -eq 1 | Sort-Object Name
```

## Why sprawl exists (four buckets)

1. **Cargo / Rust module granularity** — One file per directory under `crates/*/src/**` is normal; VS Code extension mirrors domain folders (`apps/editor/vox-vscode/src/chat/` …).
2. **Contract federation** — Each `contracts/<domain>/` is often a **bounded SSOT** (`contracts/naming`, `contracts/workflow`, …) referenced by hard-coded paths in crates and indexed in [`contracts/index.yaml`](../../../contracts/index.yaml).
3. **Documentation explosion** — `docs/` is intentionally deep; Astro sidebar is driven by frontmatter, not folder count.
4. **Operational duplication** — Compose and Docker material intentionally appears in **`infra/`** (Coolify / Populi), **`docker/`** (compose-relative paths for eval/SearXNG), and the **repo root** (`docker-compose.yml`, `vox-eval.compose.yml`) so operators can run `docker compose -f …` from different working directories.

## Top-level map (group like-with-like)

| Group | Paths | Purpose |
|-------|-------|---------|
| **Rust workspace** | [`Cargo.toml`](../../../Cargo.toml), [`crates/`](../../../crates/), [`rust-toolchain.toml`](../../../rust-toolchain.toml) | Primary implementation; arch enforcement via `layers.toml` + `vox-arch-check`. |
| **Contracts SSOT** | [`contracts/`](../../../contracts/), especially [`contracts/index.yaml`](../../../contracts/index.yaml) | Machine-readable policies and schemas; **`vox ci contracts-index`** and domain-specific guards. |
| **Human docs** | [`docs/src/`](../../../docs/src/) | Authoritative prose + architecture; doctests via doc pipeline. |
| **Docs site build** | [`docs-astro/`](../../../docs-astro/) | Astro app; generated sidebar — do not hand-edit `SUMMARY.md`. |
| **GUI / apps** | [`apps/`](../../../apps/) | Mental tracker, editor extension, interop marquee, experimental visualizer — ownership in [`contracts/frontend/surface-ownership.v1.yaml`](../../../contracts/frontend/surface-ownership.v1.yaml). |
| **Examples & fixtures** | [`examples/`](../../../examples/), [`tests/fixtures/`](../../../tests/fixtures/) | Sandboxes and test-only bundles; not shipped product trees. |
| **Automation** | [`scripts/*.vox`](../../../scripts/) | VoxScript-first glue (`vox run …`); thin bootstrap PS/sh only. |
| **CI / policy entrypoints** | [`.github/workflows/`](../../../.github/workflows/), [`lefthook.yml`](../../../lefthook.yml), [`deny.toml`](../../../deny.toml), [`biome.json`](../../../biome.json) | External runners and repo-wide lint gates. |
| **Deploy / ops** | [`infra/`](../../../infra/), [`docker/`](../../../docker/), root `Dockerfile*`, [`docker-compose.yml`](../../../docker-compose.yml) | Overlap is **documented** (eval sandbox: root compose + mirror under `docker/`). Prefer **documenting canonical path** over silent merges. |
| **Build-time helpers** | [`apps/build-tools/render-durable-animation/`](../../../apps/build-tools/render-durable-animation/) | Small Node helper for doc assets; driven by [`scripts/render-durable-animation.vox`](../../../scripts/render-durable-animation.vox). |
| **Training / Mens** | [`mens/`](../../../mens/) | Corpus config + local training runs (`mens/runs/` gitignored). |

## Non-Rust artifacts — producer / consumer matrix

Only high-traffic families are listed; extend this table when consolidating a directory.

| Artifact(s) | Produced by | Ingested by |
|-------------|-------------|-------------|
| [`docs/agents/doc-inventory.json`](../../../docs/agents/doc-inventory.json) | `cargo run -p vox-cli -- ci doc-inventory generate` ([`vox-doc-inventory`](../../../crates/vox-doc-inventory/) walks `crates/`, `docs/`, `apps/editor/vox-vscode`, `scripts/`, `.github/workflows/` — see [`walk.rs`](../../../crates/vox-doc-inventory/src/walk.rs)) | `vox ci doc-inventory verify`, agents / IDE context policies |
| [`contracts/index.yaml`](../../../contracts/index.yaml) + [`contracts/index.schema.json`](../../../contracts/index.schema.json) | Human editors + CI generators (`vox ci operations-sync`, capability sync, …) | `vox ci contracts-index`, multiple crates via stable path literals |
| Root [`vox.tokens.json`](../../../vox.tokens.json) | Human / design | [`contracts/tokens/tokens.v1.json`](../../../contracts/tokens/tokens.v1.json) schema; TS/CSS/codegen in `vox-codegen` |
| [`docs/src/SUMMARY.md`](../../../docs-astro/README.md), [`docs/src/feed.xml`](../../../docs-astro/README.md) | Astro build (gitignored committed stubs per AGENTS.md) | Docs site |
| [`.cursorignore`](../../../.cursorignore), [`.aiignore`](../../../.aiignore), … | `vox ci sync-ignore-files` from [`.voxignore`](../../../.voxignore) | IDE exclusion surfaces |
| [`contracts/toestub/suppressions.v1.json`](../../../contracts/toestub/suppressions.v1.json) | Humans + audit tooling | `vox-code-audit`, CI |
| `docker/**`, `infra/**` compose files | Humans / ops | `vox` research infra helper ([`infra.rs`](../../../crates/vox-cli/src/commands/research/infra.rs)), Coolify, deployment docs |

## Consolidation backlog (risk-tiered)

### Tier S — safe wins (documentation + pointers)

- Keep overlapping compose paths but **link one canonical row** in [`where-things-live.md`](./where-things-live.md) (done in same PR as this audit).
- When adding new ops material, prefer **`infra/`** for long-form deployment docs + compose **unless** you need `docker compose -f docker/...` path stability (eval/SearXNG).

### Tier M — structural (requires reference sweep)

- **Merge thin `contracts/<x>/` domains** only when a domain has ≤2 files **and** shares an owner with an adjacent domain — must update **`contracts/index.yaml`**, `rg contracts/old-path`, and any crate literals.
- **Fold `tools/` into `apps/build-tools/`** — done for `render-durable-animation`; keep new adjunct CLIs under [`apps/build-tools/`](../../../apps/build-tools/).

### Tier L — avoid without RFC

- Flattening `crates/*/src` module folders — fights Rust idioms and review ergonomics.
- Moving root `Cargo.toml`, `rustfmt.toml`, `deny.toml`, etc. — breaks ecosystem defaults.

## Next steps

1. Triage the ~203 single-file parent dirs: tag each as **idiomatic** | **candidate merge** | **generated** — baseline list in [`repo-layout-single-file-parent-dirs-triage-2026.md`](./repo-layout-single-file-parent-dirs-triage-2026.md).
2. For each Tier M move: attach an `rg` evidence block (reference count) in the PR description.
3. Extend this matrix when introducing new generated JSON/YAML under `contracts/` or `docs/agents/`.
