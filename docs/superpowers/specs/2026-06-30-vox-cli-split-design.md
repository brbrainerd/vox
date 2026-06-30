---
title: Splitting the vox-cli monolith for build-time reduction
category: Build
status: design
date: 2026-06-30
source: 4-agent parallel analysis (structure / deps / build-hotspots / command-groups) + synthesis
---

# Splitting the vox-cli monolith

## 1. The problem

`vox-cli` is a **77,271-LOC / 458-file** monolith that compiles into a single ~115 MB debug
binary, and **every source change anywhere in the crate forces a full relink of that binary**
because there is only one crate boundary — cargo cannot cache, parallelize, or skip the final
link. Build time is dominated by three things: (a) the **CI subsystem**, **34,571 LOC across
~108 files (~45% of the whole crate)**, compiled into the default build via `pub mod`; (b) the
`cmd_enums.rs` clap `Subcommand` enum (1,421 LOC, 100+ variants) whose derive-macro
monomorphization re-expands the entire parser tree on any subcommand change; and (c) the
unconditional linking of ~57 internal `vox-*` crates. `lld-link` already halved the *link* step
(separate work); the remaining lever is **structural** — break the one crate into several
library sub-crates so cargo caches the untouched ones and parallelizes the rest.

## 2. Split strategy

Convert `vox-cli` from a fat binary into a **thin binary shell** depending on a fan of cohesive
library sub-crates, following the existing **`vox-cli-core` / `vox-cli-ci`** precedent (optional
deps behind features, a re-exported `pub mod` surface, minimal core deps). Each command group
becomes its own crate exporting a handler surface; the shell owns only `main`, the top-level
clap `Cli` enum, dispatch routing, and shared harness (`cli_args.rs`, `command_catalog.rs`,
`pipeline.rs`, `lib.rs`).

| Crate | Source today | LOC | Gate |
|---|---|---|---|
| `vox-cli-share` | `utils/share/` (Tailscale/Cloudflare/LAN) | ~1,400 | feature per backend |
| `vox-cli-review` | `commands/review/coderabbit/` (+ DEI review dispatch) | ~6,200 | `coderabbit` |
| `vox-cli-extras` | `commands/extras/ludus/` + `extras/ars/` | ~4,300 | `extras-ludus`, `ars` |
| `vox-cli-research` | research infra/eval/run | ~900 | — |
| `vox-cli-model` | `commands/model/` (eval, pricing, explain) | ~1,800 | default-optional |
| `vox-cli-runtime` | `commands/runtime/` (run, dev, shell, info) | ~2,500 | `script-wasi` carries wasmtime |
| `vox-cli-db` | `commands/db/` incl. `publication/` | ~5,000 | `db` |
| `vox-cli-ci` *(thicken existing)* | `commands/ci/` (pre_push, guards, runner_scale, cmd_enums, …) | ~34,500 | default, isolated |
| `vox-cli-diagnostics` | `commands/diagnostics/` (doctor, stub_check, tools) | ~4,000 | `stub-check` |
| `vox-cli-contracts` *(new thin seam)* | `CheckRunner`/`PolicyContract` traits + `cargo_bin()`/`repo_root()` | small | — |

## 3. Ordered extraction plan (ROI = build payoff × independence)

**Tier 1 — extract first (high independence, low risk):**
1. **`vox-cli-share`** (1.4K) — pure library, zero dispatch coupling; drops Cloudflare/Tailscale
   SDKs from the default build. Cleanest first move — proves the seam end-to-end.
2. **`vox-cli-review`** (6.2K) — already `coderabbit`-gated, zero inter-command imports; pulls
   `vox-forge` (gix, GitHub adapters, ~12–15 crates) out of the default closure. **Highest ROI
   of the clean candidates.**
3. **`vox-cli-extras`** (4.3K) — both gated (`extras-ludus`/`ars`), self-contained; removes
   ~15–20 crates from default.
4. **`vox-cli-research`** (~900) + dispatch/workflow modules — tiny, near-zero effort; bundle
   with Tier 1 to validate the parallelization story.

**Tier 2 — after the seam is proven:**
5. **`vox-cli-model`** (1.8K) — leaf talking to the orchestrator daemon.
6. **`vox-cli-runtime`** (2.5K) — lets `script-wasi`/wasmtime (~69 crates, ~5 min) be carried by
   an optional crate, not interleaved `#[cfg]` in `lib.rs`.
7. **`vox-cli-db`** (5.0K) — depends only on `db_cli`/`db_retention`; pulls `vox_sql`/`vox_codex`.

**Tier 3 — DEFER (entangled; needs the contract seam first):**
8. **`vox-cli-ci`** (34.5K — **the biggest prize, the hardest**). Internally cohesive but makes
   outbound calls to audit/policy/runtime/scientia, and `run_body.rs` imports 25+ siblings.
   **Do not extract until `vox-cli-contracts` exists** so CI depends on a trait, not the modules.
   When it lands it is the single largest default-build reduction and lets cargo cache the
   100-variant `cmd_enums.rs` clap codegen separately.
9. **`vox-cli-diagnostics`** (4.0K) — pulls CI transitively; must follow CI or depend on the
   contract crate directly.

Single-file heavy hitters (`scientia_phase_handlers`, `repair`, `openclaw`, `toolchain_upgrade`,
`dei`, `secrets`) are **not extractable as-is** — re-modularize into submodule dirs first.

## 4. The seam (how the thin shell wires sub-crates)

- **Each sub-crate** exposes a flat `pub mod` surface + entry-point fns
  (e.g. `vox_cli_review::run(args) -> Result<ExitCode>`). No sub-crate owns `main` or `Cli`.
- **Shared infra → `vox-cli-contracts`** (new, tiny, sync-only — follow the `vox-cli-ci`
  template: anyhow/glob/regex/walkdir, no tokio). Owns `cargo_bin()`, `repo_root()`,
  `nvcc_available()`, and the `CheckRunner`/`PolicyContract` traits so CI calls audit/policy/
  scientia through a trait object instead of `super::` imports. **Prerequisite for Tier 3 —
  build it early even though CI extraction is deferred.**
- **The shell `vox-cli`** keeps `cli_args.rs`, `command_catalog.rs`, `pipeline.rs`, `lib.rs`, the
  top-level `Cli` enum, and the dispatch `match`. Each command's `Args` migrates into its owning
  sub-crate; the shell composes via clap `#[command(flatten)]`/`Subcommand` delegation, and
  feature-gated variants (`#[cfg(feature="coderabbit")]` → `vox-cli-review`) compile in
  conditionally. **The CLI surface is unchanged** (`vox review`, `vox ci`, `vox db …`) — only the
  hosting crate changes.
- **Dependency direction:** sub-crates → core/contracts only. Never sub-crate → shell, never new
  cross-sub-crate edges (would re-serialize the build). Tier-1/2 candidates do **not** propagate
  features into `vox-orchestrator`/`vox-search` — preserve that (resolver-2 feature unification).

## 5. Risks + measurement

**Measurement (mandatory per extraction):** capture `cargo build --timings -p vox-cli` before
and after, for three scenarios — (a) incremental edit to a *touched* command, (b) incremental
edit to an *untouched* command (caching should now skip the sub-crate), (c) cold build. Track
binary size + default crate count (`cargo tree --no-default-features` vs default). *(cargo-timings
gotcha: `UNIT_DATA` is a `const`, duration is float-seconds `as_f64×1000`, filter `duration>0`.)*

**Success per step:** an edit to an *untouched* command no longer recompiles the extracted crate
and the relinked unit shrinks. If (b) doesn't improve, the extraction didn't cut a coupling edge.

**Regressions to bound:** longer *cold* builds (more rustc invocations — net win is on incremental,
measure cold so it's known); dependency/feature unification under resolver-2 (gate `script-wasi`/
`heavy-retrieval` exactly as today); clap surface drift (guard with `command-registry.yaml` +
a CLI golden test before/after); entanglement landmines (the Tier order exists to avoid circular
deps — don't jump ahead); the binary-freshness CI gate (confirm it still passes).

## 6. Verdict

**Worth it — sequence it; the big payoff is back-loaded.** The single-crate boundary is *the*
reason a one-line change relinks 115 MB, unfixable without crate boundaries. Tier 1 is low-risk,
removes ~12K LOC + 30–50 transitive crates from the default build, and proves the
precedent/parallelization at minimal cost — do it first. The dominant win is **Tier 3 CI
extraction** (~45% of the crate, the 100-variant clap enum) — est. 15–25 s saved per non-CI
rebuild — gated on `vox-cli-contracts`. Combined Tier 1–3 payoff on a 6-core machine: est.
**25–40% incremental-rebuild critical-path cut**, ~30% default crate-graph reduction; cold builds
marginally slower (accepted, measured). **Proceed: build `vox-cli-contracts` + Tier 1 first, gate
each step on `--timings` evidence, treat CI extraction as the headline milestone, not the start.**

## Next step

This is the design. The implementation is a multi-PR effort (one sub-crate per PR, each gated on
its before/after `--timings` evidence). Recommend starting with `vox-cli-share` (Tier 1.1) as the
seam-proving PR, then `vox-cli-contracts`, then the rest of Tier 1.
