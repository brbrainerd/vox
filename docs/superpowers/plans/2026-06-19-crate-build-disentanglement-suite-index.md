# Crate Build-Time & Dependency Disentanglement — Plan Suite Index (Sonnet 4.6)

> **🤖 EXECUTION TARGET.** Executed by **Claude Sonnet 4.6** (Claude Code or Antigravity) — strong reasoning, long-context recall, low hallucination. Plans keep TDD / atomic-commit / verify-before-use as good engineering on a large evolving codebase (not anti-amnesia scaffolding), and grant judgment latitude on discovery steps with explicit fallbacks. Sonnet's real failure mode to guard against is declaring "done" without running the verification ritual, and YAGNI gold-plating.

**Design SSOT:** [`../specs/2026-06-19-crate-build-disentanglement-design.md`](../specs/2026-06-19-crate-build-disentanglement-design.md).
**Grounding output:** [`../../src/architecture/crate-build-dependency-model-2026-06-19.md`](../../src/architecture/crate-build-dependency-model-2026-06-19.md) + `graphify-out/crate-build-map.json`.

**Verified targets (from running our real tooling 2026-06-19):** keystones by blast-radius-seconds — `workspace-hack` 492s, `vox-db` 370s, `vox-compiler` 364s, `vox-populi` 366s; **1 advisory 20-crate dev-dep cycle** anchored by `vox-test-harness` (0 hard cycles); modularity **Q=0.24** (weak); 2 layer inversions. Native graphify works live; **gaps:** no crate-level Leiden CLI; prebuilt `vox.exe` is stale vs `index/refresh/gc`.

> **PREREQUISITE for any execution:** the prebuilt binary predates Plan B/C/D subcommands. Run `cargo build -p vox-cli` first so `vox graphify` has the current surface.

## Tracks (sequence: T1 → T2 ∥ T4 → T3)

| # | Plan | Status | Goal |
|---|---|---|---|
| **T1** | [Native crate-map capability](2026-06-19-crate-build-track1-native-crate-map.md) | **WRITTEN (4 tasks)** | T1.0 fix non-deterministic Leiden (cross-cutting — also fixes shipped rebuild/modules), T1.1 cycle-safe blast+counts, T1.2 audit-optional `build_crate_map`, T1.3 `vox graphify crate-map` (regen graph, persist + ingest). The re-runnable model everything else is scored against. |
| **T2** | Disentangle the 20-crate cycle | **TO WRITE** | Invert `vox-test-harness` heavy deps to dissolve the advisory cycle; add a `dep-cycles --deny-new` regression gate over a committed back-edge allowlist. |
| **T4** | Measurement spine + gating | **TO WRITE** | Refresh the zeroed `build-bench` baseline + `crate_audit`; `vox ci crate-budget` gate on blast-radius-seconds + modularity Q deltas. |
| **T3** | Blast-radius splits | **TO WRITE** | Incremental, measured **type-only extractions** (e.g. expand `vox-db-types`) to shrink `vox-db`/`vox-compiler`/`vox-populi` blast radii; fix the 2 layer inversions + fan-in budget breaches. One module per task, `blast_s`-gated. |

**Why T1 first:** it produces the live model (`vox graphify crate-map`) that T3's per-extraction `blast_s` deltas and T4's regression gate both consume. T2 and T4 are independent. T3 is the heavy refactor and runs last, each extraction gated by T1's measurement.

**Reuse, don't duplicate:** T2's cycle gate composes with plugin-maturity Track A (arch-check Tarjan cycles R17); T4's baseline composes with the build-time measurement program (`vox ci build-bench`). Extend those, don't re-invent.

## Cross-cutting bug found in audit (fix lands in T1.0)
`cluster.rs:39` runs Leiden with `LeidenConfig::default()` and **no seed** → community assignment is
non-deterministic, so every clustered corpus's `graph.json`/`graph_json_sha256` churns run-to-run.
This already affects the **shipped** rebuild/modules lens (Plan A/B), not just the crate-map. T1.0 fixes
it once (seed if available, else canonical labels + digest-exclusion).

## Conventions (all plans)
- Atomic + green + committed per task; verify-before-use `rg` first; `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags; verification ritual (with pasted output) before each commit; YAGNI (no gold-plating).
- No `cargo fmt --all` (`-p <crate>`); automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs. Commit messages end with the repo `Co-Authored-By` trailer.
- **Caveat:** `graphify-out/crate_audit.json` (compile times) is gitignored / not committed → absent on fresh checkouts; the model degrades to dependency *counts* and notes it. `contracts/ci/crate-graph.v1.json` IS committed but a snapshot (regenerated from `cargo metadata` in T1.3).
