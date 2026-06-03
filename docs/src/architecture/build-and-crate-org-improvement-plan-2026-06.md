---
title: "Build-Time, Crate-Org & Target-Sprawl Improvement Plan (2026-06)"
description: "Synthesis of a 7-lens parallel audit of the Vox workspace: build-time hotspots, crate inventory/layers, dead/combine/new crates, feature-gates-vs-crates, plugin boundaries, and the 400+ GB target-folder sprawl. Records the quick wins already landed (with measured before/after) and the gated workstreams that follow."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Build-Time, Crate-Org & Target-Sprawl Improvement Plan (2026-06)

Synthesis of a parallel 7-lens audit (crate inventory/layers, build-time hotspots,
dead/combine/new crates, feature-gates-vs-crates, plugin boundaries, target-folder
sprawl, prior-art reconciliation) plus an execution pass. Supersedes the build-time
sections of [comprehensive-audit-v2-2026.md](./comprehensive-audit-v2-2026.md) (dated
2026-04-18, crate count stale).

## Headline findings (verified)

- **Target-folder sprawl is the biggest, cheapest win.** ~406 GB of duplicated build
  artifacts: main `target/` 22.7 GB; worktree `target/` dirs totalled **379.9 GB**
  (one worktree 94 GB, another 63 GB). Root cause: the **git-tracked**
  [`.cargo/config.toml`](../../../.cargo/config.toml) sets
  `CARGO_TARGET_DIR = { value = "target", relative = true }`, which resolves relative
  to *each worktree's own* checked-in config — so every worktree gets a private
  `target/`, the opposite of the comment's "shared root" intent.
- **`vox-cli` default build = 833 crates**, and `--no-default-features` is *identical*
  (833) — feature gates buy ~zero cold-build relief because every heavy subsystem rides
  in via **non-optional path deps**, not features:
  - `wasmtime` (147-crate subtree) via non-optional `vox-wasm-engine` ([vox-cli/Cargo.toml:139](../../../crates/vox-cli/Cargo.toml)).
  - `tantivy` (102-crate subtree) forced on by `vox-orchestrator` / `vox-dei-shim` /
    `vox-orchestrator-mcp` unconditionally enabling `vox-search` features.
  - `candle`/gemm (504-crate subtree) into every default `vox-ml-cli` build via
    non-optional `vox-quantize`.
  - `tauri-utils` (+ a duplicate `html5ever 0.38` / `dom_query` / `selectors 0.36`
    parser stack) injected into every non-GUI build through `workspace-hack`.
- **Zero dead crates.** All 103 crates (perfect 3-way parity: cargo metadata = on-disk =
  `layers.toml`) are live or legitimately WIP. All 14 orphan/staleness-exempt crates were
  hand-verified. **This plan proposes zero retirements** (per the
  verified-by-hand caveat: a prior audit had 5/10 retirement candidates wrong).
- **Plugin CPU/GPU/mobile boundary is already good.** Candle/CUDA/Metal/NVML/Whisper/
  browser are already cdylib plugins absent from the default CLI; `vox-runtime` /
  `vox-runtime-rn` (mobile) are minimal. The remaining work is *finishing two in-tree
  candle duplications* and *adding guards*, not re-architecting.
- **One dead hub edge:** `vox-compiler` (44.5K LoC, 18 dependents, center of the deepest
  21-edge chain) declared `vox-deploy-codegen` with **zero** source uses.

## Execution log (2026-06-03) — landed & verified

All measured before/after; dependency counts via `cargo tree -p <crate> -e normal` unique crates.

| Item | Before → After | Verification |
|---|---|---|
| WS1-T1 reclaim orphan `target/` | **−83.03 GB** | per-dir measured; only unregistered, >47 h-idle worktrees; `target/` only |
| WS2-T1 hakari `tauri-utils` exclude | vox-cli 833 → 790 | `cargo tree` |
| WS2-T2 wasmtime gate (`script-execution`) | vox-cli 790 → **719** (−71) | `cargo check` default **+** script-execution rc=0 |
| WS2-T3 tantivy gate (`heavy-retrieval`) | vox-cli 719 → **692** (−27) | libs + lean + heavy-retrieval all rc=0; release_build keeps shipped full search |
| WS2-T4 quantize gate + dead-stub removal | vox-ml-cli 613 → **593** (−20) | `cargo build` default **+** `--features quantize` rc=0; candle ~20s/30 MB measured |
| WS3-T1 dead `vox-deploy-codegen` edge | edge removed | `cargo check -p vox-compiler` rc=0 |
| WS1-T3 sccache | installed + machine-local `~/.cargo/config.toml` | server up; per-worktree+sccache strategy |
| WS1-T4 GC-script worktree guard | gated behind `VOX_CLEAN_WORKTREES` | `vox check` rc=0 |
| ai-fixtures-v1 ×3, CUDA-CI fix, WS6-T1 banner | done | committed |

**Cumulative: vox-cli default 833 → 692 crates (−141, −16.9%); vox-ml-cli 613 → 593.** Commits:
`7ba5a96810` (quick wins), `f122932a75` (wasmtime+quantize), WS2-T3, WS1-T4 on
`cc_bdesktop2/objective-wilbur-19a990`.

**Reality-checks (assumptions corrected by measurement):** the audit's "subtree" sizes were
marginal-cost overcounts (wasmtime −71 vs ~147 claimed; candle −20 vs "504"); `cargo hakari verify`
fails on the *untouched* baseline (pre-existing); the CUDA-CI "retarget to vox-ml-cli" rec was wrong
(those are vox-cli subcommands; GPU is a runtime plugin); WS2-T2 had a real compile break (ungated
`Cli::Wasm`) caught by verification; and A-5's `vox-build-driver` extraction is blocked by the shared
`pipeline` spine (see WS5-T3). Net: **the 83 GB reclaim is the dominant win; the dep cuts are real but
modest.**

## Workstreams

IDs are stable. Status reflects the 2026-06-03 execution pass.

### WS1 — Target-folder sprawl & multi-agent build hygiene
- **WS1-T1 ✅ DONE (2026-06-03)** — Reclaimed **83.03 GB** by deleting `target/` (build
  output only; source preserved) from 7 *unregistered, idle* orphan worktree dirs
  (`youthful-jones-c15854` 63 GB, `naughty-dirac-825348` 19.65 GB + 5 small). Active
  (git-registered) worktrees were left untouched.
- **WS1-T2 (gated on decision)** — Stop the bleed: remove the `[env] CARGO_TARGET_DIR`
  block from the tracked `.cargo/config.toml`; set an **absolute** target dir in an
  **untracked** machine-local `%USERPROFILE%/.cargo/config.toml`. Correct the misleading
  comment. RISK: a single live shared target serializes concurrent agent builds via
  Cargo's per-package lock — pair with WS1-T3.
- **WS1-T3 (gated on decision)** — Install + wire **sccache** (machine-local
  `RUSTC_WRAPPER`) as the multi-agent compilation cache. RECOMMENDED primary: keep
  per-worktree targets (full parallelism, no lock) and let sccache dedup compilation.
  `sccache` is **not currently installed**.
- **WS1-T4** — Extend [`scripts/clean-build-artifacts.vox`](../../../scripts/clean-build-artifacts.vox)
  with an orphan-dir sweep (cross-ref `git worktree list`; skip dirty/unpushed; skip
  dirs touched in last N hours / with a live target lock), a size-budget WARN (~150 GB),
  and a rename-then-delete fallback for Windows locked binaries. Schedule weekly. Cap
  concurrent agent worktrees (decision needed: 6–8?). Author as `.vox` per policy.
- **WS1-T5** — gitignore the cache dir; document target placement + *why* `relative=true`
  does not share, in `where-things-live.md`.

### WS2 — Cold-build dependency-volume cuts
- **WS2-T1 ✅ CONFIG LANDED (2026-06-03)** — Added `tauri-utils` + `tauri-build` to
  [`.config/hakari.toml`](../../../.config/hakari.toml) `[final-excludes].third-party`.
  **Measured: `vox-cli` default 833 → 790 crates (−43)** via `cargo tree`. `vox-gui`
  still resolves Tauri (it has a real `tauri` dep). CAVEAT: `cargo hakari verify` already
  failed on the **untouched baseline** (a pre-existing `aho-corasick`/`vox-tauri-stt`
  under-unification); CI gates via `cargo hakari generate --diff` on **linux** runners,
  so the canonical `workspace-hack` regen should land from that environment. **Do not
  hand-edit `workspace-hack/Cargo.toml`.**
- **WS2-T2 (gated)** — Gate `wasmtime` behind `script-execution` (make `vox-wasm-engine`
  optional on `vox-cli`); route default WASI through the `vox-plugin-runtime-wasm` skill
  runtime. Removes ~147 crates from the lean CLI. RISK: grep for non-`script-execution`
  `vox_wasm_engine::` call sites first.
- **WS2-T3 (gated)** — Gate `vox-search` heavy features at the **orchestrator boundary**
  (resolver-2 unification means gating `vox-cli` alone does nothing). Removes `tantivy`
  (102 crates). First confirm whether `vox search` is a default-surface command.
- **WS2-T4 (gated)** — Gate `vox-quantize` behind a `quantize` feature on `vox-ml-cli`
  (cfg `commands/quantize.rs` + `commands/schola/merge_qlora.rs`). Removes a 504-crate
  candle/gemm subtree from the default `mens-base` build — the largest default-build win.
- **WS2-T5 (gated)** — Gate `self_update` (161 crates) + audit `feed-rs`; collapse
  first-party duplicate majors (`which` 7+8, align `zip`, evaluate `tokio-tungstenite`
  0.24→0.29). Keep documented-intentional duals (`rand` 0.8/0.9, `schemars` 0.8/1).

### WS3 — Hub dead-edge removal & rebuild-cascade pruning
- **WS3-T1 ✅ DONE (2026-06-03)** — Removed the unused `vox-deploy-codegen` dep from
  [`vox-compiler/Cargo.toml`](../../../crates/vox-compiler/Cargo.toml) (0 source refs;
  `vox-cli` + `vox-codegen` declare it directly). Edits to `vox-deploy-codegen` no longer
  rebuild `vox-compiler` + the whole stack above it. `cargo tree` confirms edge gone.
- **WS3-T2 (investigate)** — Determine whether the candle plugins' `vox_compiler::ast_eval`
  call needs the full compiler or only `vox-eval`; if narrowable, drop the heavy edge.
- **WS3-T3 (investigate, low priority)** — Whether `vox-tauri-codegen` needs the full
  compiler or only types.

### WS4 — Plugin boundary & CPU/GPU/mobile hardening
- **WS4-T1** — Finish oratio Unit-4: delete `vox-oratio`'s DEPRECATED `stt-candle`
  backend; route `vox-ml-cli` transcription through `vox-plugin-host` (plugin already owns
  the Candle Whisper backend).
- **WS4-T2** — Finish populi candle-qlora extraction: drop in-crate candle from
  `vox-populi` once callers dispatch `--backend qlora` through the plugin host.
- **WS4-T3** — Make the `no-cdylib-compile-dep` CI guard self-maintaining (scan all 11
  cdylib plugins, not the hard-coded 4).
- **WS4-T4** — Add a mobile lean-build exclusion guard (`vox-runtime`/`vox-runtime-rn`
  must transitively exclude wasmtime/candle/sherpa/tantivy/gix/jj-lib/cdylib plugins).
- **WS4-T5** — Refresh stale plugin/extraction docs; add a "where does GPU code live" row.

### WS5 — Structural extractions & LoC/fan-in drift (execute-not-redesign)
- **WS5-T1 (codegen-ssot owner)** — `vox-codegen` is the only crate **over** its hard LoC
  budget (27,345 / 25,000). Raise budget with rationale OR extract `codegen_ts/rn/*`,
  keeping `project_bundle_from_hir` as the single assembly point.
- **WS5-T2** — Hand-audit `vox-config` fan-in (37 / 20); bump budget or facade.
- **WS5-T3 (prereq) — REASSESSED 2026-06-03, blocked-as-specified.** The inversion is 3
  call sites in `vox-ml-cli/src/commands/ai/workflow.rs` (under `cfg(not(workflow-runtime))`):
  `vox_cli::commands::build::run`, `::fs_utils::run_target_dir_for_workspace`, `::RustAppShell`.
  Verification found the `vox-build-driver` plan understated the cost: `build::run` transitively
  needs `crate::pipeline` (365 LoC) which is used by **~17 other vox-cli commands** (check, doc,
  repair, play, debug, emit, db, ci/doctest, compilerd, …), plus `utils::ssg`, `v0_tsx_normalize`
  (174), `fs_utils` (207), and the codegen graph (RustAppShell ← vox-codegen). Extracting
  `build::run` therefore either drags the shared `pipeline` spine into the driver (17+ shim sites)
  or creates a `vox-build-driver → vox-cli` cycle. **This is a large, invasive refactor, not a
  quick do-now.** Real options to actually remove the inversion: (a) extract the `pipeline`/build
  spine into an L3 `vox-build-driver` (large; the honest scope); (b) have vox-ml-cli's fallback
  shell out to the `vox` binary (`vox build`) instead of the library call — removes the inversion
  with no extraction, but a behavior change the inversion's own reason-text discouraged;
  (c) leave the documented `known_inversion` as-is (it is sanctioned and stable). A self-contained
  partial win available regardless: merge the 207-LoC `fs_utils` superset into `vox-cli-core`
  (dedups the 59-LoC stub) and repoint the one `fs_utils` inversion call — but that alone does
  not remove the `known_inversion` (build::run + RustAppShell edges remain).
- **WS5-T4 — AUDITED 2026-06-03: legitimate but NOT urgent; DEFER.** Re-examined with data
  rather than the earlier "URGENT" label. Findings: ci/ = **26,344 LoC / 101 files (~32% of
  vox-cli's 82,862)**; **~65% of recent vox-cli commits touch ci/** (highest-churn subtree);
  incremental `cargo check -p vox-cli` after touching a ci/ file = **10s** (warm + sccache).
  BUT: (1) the `max_loc = 90_000` budget is a **`warn`, not an error**, and is **not promoted
  to error in CI** — there is **no hard wall** at 90K, so "URGENT at 92%" was overstated;
  (2) the incremental-rebuild win is **variable** — vox-cli depends on vox-cli-ci, so
  API-changing ci edits recompile both crates (≈ same total work); only internal-only ci
  edits would drop from an 82K-LoC recompile to ~26K + relink; (3) the cost is real — the
  **HAZARD-1 `build_catalog()` → `VoxCliRoot` cycle** needs a dependency-injection refactor
  (design work), plus a 26K-LoC move, ~12 seam repoints, and "don't-run-concurrently"
  coordination. **Determination:** the separation has genuine merit (build hygiene, parallel
  compile, faster internal-edit rebuilds) but is **not forced**. Defer until either the
  loc_budget warn is promoted to error, or it is funded as a dedicated isolated PR that does
  the `build_catalog` DI refactor FIRST, then the mechanical move. Do NOT rush it.
- **WS5-T5 (HOLD)** — `vox-orchestrator-core` extraction is correctly gated on a Rule-13
  growth trigger that has **not** fired (~8% headroom); a coherence-constrained ~17–25K
  LoC co-move. Do **not** start now.

### WS6 — Doc-drift guardrails & build telemetry
- **WS6-T1 ✅ DONE/pre-satisfied (2026-06-03)** — `crate-classification-2026-05-08.md` was
  already `status: deprecated` with a snapshot banner (acute risk pre-mitigated). Added a
  dated supersession note to `comprehensive-audit-v2-2026.md`.
- **WS6-T2 (gated)** — Re-measure CLI/orchestrator incremental `cargo check` times after
  WS2/WS5 land, to re-anchor [build-time-baseline.md](./build-time-baseline.md).

## Measurement methodology (per the "verify assumptions with real before/after" mandate)

- **Dependency volume** (cold-build crate count) is the exact, free before/after metric
  for dep cuts — measured via `cargo tree -p <crate> -e normal` unique-crate count.
- **Incremental build time** (the headline KPI) is measured via the documented
  `touch <file> + time cargo check -p <crate>` method once a warm target exists.
- **Compilation correctness** is gated with `cargo check`.
- Full *cold timed* build comparisons are reserved for the high-stakes WS2 gates
  (wasmtime/quantize/tantivy) and WS5 extractions, run in a stable environment — not for
  provably-safe edits (unused-dep removal, empty-feature deletion), where crate-count
  deltas + grep proofs suffice.

## Open decisions (need the human's call)

1. **Sprawl strategy:** per-worktree targets + sccache (recommended) vs single shared
   absolute target vs both. Affects WS1-T2/T3 and changes machine-local config.
2. **Concurrent agent-worktree cap + disk budget** (WS1-T4): 6–8?
3. **CUDA CI:** the three `vox-cli --features gpu` sites (`ci.yml:393`,
   `qwen35-native-nightly.yml:25/28/35`) are invalid — `vox-cli` has no `gpu` feature and
   `mens train` / `run --interp` are `vox-cli` subcommands (GPU is a runtime-loaded
   plugin, not a compile feature). The audit's "retarget to `vox-ml-cli`" is wrong for the
   subcommand lines. Decide: drop the bogus `--features gpu` (build vox-cli normally, load
   the cuda plugin at runtime) vs build the cuda plugin explicitly vs route through
   `vox-ml-cli`.
4. **`vox-codegen` over budget** (WS5-T1): raise budget vs extract `codegen_ts/rn/*`.
5. **Big extractions this cycle:** land A-5 then `vox-cli-ci` now (recommended) vs hold.

## Risks & caveats

- **Verified-by-hand:** zero retirements proposed; `crate-classification-2026-05-08.md`'s
  DEAD column is stale and must not be used as a current signal.
- **resolver-2 feature unification:** gating a dep on `vox-cli` alone is a no-op if an L3
  consumer enables it unconditionally — move gates to the real boundary.
- **Shared-target build-lock contention:** do not adopt a single live shared target as
  primary without sccache; the final `vox-cli` link is not cached by sccache.
- **Destructive GC:** guard orphan-dir deletion on mtime + live lock + dirty/unpushed.
- **Generated files:** never hand-edit `workspace-hack/Cargo.toml`, `SUMMARY.md`,
  `architecture-index.md`, `*.generated.md` — rerun the generator. New `docs/src/` files
  need YAML frontmatter. Automation is authored as `.vox`.
