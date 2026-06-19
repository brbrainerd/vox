# Crate Build-Time & Dependency Disentanglement — Plan Suite Index (Antigravity / Gemini 3.5 Flash)

> **🤖 EXECUTION TARGET.** Executed by **Gemini 3.5 Flash in Antigravity** (~48% real-world completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Every plan is engineered against that. Read first: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5 and [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

**Design SSOT:** [`../specs/2026-06-19-crate-build-disentanglement-design.md`](../specs/2026-06-19-crate-build-disentanglement-design.md).
**Grounding output:** [`../../src/architecture/crate-build-dependency-model-2026-06-19.md`](../../src/architecture/crate-build-dependency-model-2026-06-19.md) + `graphify-out/crate-build-map.json`.

**Verified targets (from running our real tooling 2026-06-19):** keystones by blast-radius-seconds — `workspace-hack` 492s, `vox-db` 370s, `vox-compiler` 364s, `vox-populi` 366s; **1 advisory 20-crate dev-dep cycle** anchored by `vox-test-harness` (0 hard cycles); modularity **Q=0.24** (weak); 2 layer inversions. Native graphify works live; **gaps:** no crate-level Leiden CLI; prebuilt `vox.exe` is stale vs `index/refresh/gc`.

> **PREREQUISITE for any execution:** the prebuilt binary predates Plan B/C/D subcommands. Run `cargo build -p vox-cli` first so `vox graphify` has the current surface.

## Tracks (sequence: T1 → T2 ∥ T4 → T3)

| # | Plan | Status | Goal |
|---|---|---|---|
| **T1** | [Native crate-map capability](2026-06-19-crate-build-track1-native-crate-map.md) | **WRITTEN** | `vox graphify crate-map` → native Leiden communities + blast-radius, persisted to a searchable `crate-map` corpus. The re-runnable model everything else is scored against. |
| **T2** | Disentangle the 20-crate cycle | **TO WRITE** | Invert `vox-test-harness` heavy deps to dissolve the advisory cycle; add a `dep-cycles --deny-new` regression gate over a committed back-edge allowlist. |
| **T4** | Measurement spine + gating | **TO WRITE** | Refresh the zeroed `build-bench` baseline + `crate_audit`; `vox ci crate-budget` gate on blast-radius-seconds + modularity Q deltas. |
| **T3** | Blast-radius splits | **TO WRITE** | Incremental, measured **type-only extractions** (e.g. expand `vox-db-types`) to shrink `vox-db`/`vox-compiler`/`vox-populi` blast radii; fix the 2 layer inversions + fan-in budget breaches. One module per task, `blast_s`-gated. |

**Why T1 first:** it produces the live model (`vox graphify crate-map`) that T3's per-extraction `blast_s` deltas and T4's regression gate both consume. T2 and T4 are independent. T3 is the heavy refactor and runs last, each extraction gated by T1's measurement.

**Reuse, don't duplicate:** T2's cycle gate composes with plugin-maturity Track A (arch-check Tarjan cycles R17); T4's baseline composes with the build-time measurement program (`vox ci build-bench`). Extend those, don't re-invent.

## Conventions (all plans)
- Atomic + green + committed per task; verify-before-use `rg` first; `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags; two-strike circuit breaker; rollback-on-broken-tree.
- No `cargo fmt --all` (`-p <crate>`); automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs. Commit messages end with the repo `Co-Authored-By` trailer.
