# Test-Suite Baseline Measurements (2026-05-27)

Working measurement log. Numbers feed the design at
`docs/superpowers/specs/2026-05-27-test-suite-perf-and-gate-tiers-design.md`.

## Environment

- CPU: Intel i9-14900KS — 24 cores / 32 logical threads, 3.2 GHz base
- RAM: 63.72 GB total / 29.62 GB free at measurement start
- OS: Windows 11 Home, PowerShell 7+
- Toolchain: cargo 1.92.0, cargo-nextest 0.9.95, cargo-llvm-cov 0.8.5
- `target/` size at start: **492.56 GB**
- `.cargo/config.toml`: `jobs = 8` (under-subscribed for 32T CPU); `lld-link` linker commented out; `CARGO_TARGET_DIR=target` (relative)
- `.config/nextest.toml`: default profile retries=2, slow-timeout 60s; ci profile retries=1, slow-timeout 180s

## Workspace shape (from earlier audit)

- 109 crates (107 workspace members + 2 excluded fixture packages)
- 3,167 unit tests + 1,546 integration tests + 246 ignored/skipped ≈ **4,713 cases**
- 105 insta snapshot tests
- 576 `#[tokio::test]`; 34 `#[serial_test]`; 77 `sleep()` calls
- 16 build.rs files

## Headline numbers (warm)

| Tier / step | Wall-clock | Notes |
|---|---:|---|
| TOESTUB scoped (`vox-cli ci toestub-scoped --mode enforce-warn`) | **25.8s** | Audit estimated <2 min; warm cache much faster |
| `vox-arch-check` (`vox-cli run scripts/arch-check.vox`) | **164.6s (~2.7 min)** | Above the "1–2 min" estimate. Sits in pre-push fast tier |
| `cargo nextest run --workspace --no-run` (build-only, warm) | **57s** (rustc 51s) | Compile is NOT the warm bottleneck |
| `cargo nextest run --workspace --no-fail-fast --retries 0` (warm, no cov) | **170s** | Headline. 4,967 ran / 4,965 pass / **2 fail** / 229 skipped |
| → 1 test: `vox-arch-check::integration arch_check_smoke_test` | **116.767s** (**69% of total**) | Single dominant cost center |
| → 2nd: `vox-scientia replay::sandbox timeout_kills_long_running_child` | 29.3s | By-design timeout test |
| → 3rd: `vox-orchestrator orch_smoke plan_dag_unblocks_next_node_on_complete` | 8.4s | Real cost (DAG/IO) |
| → 4th: `vox-skills openclaw_fallback_test` | 4.2s |  |
| → 5th: `vox-publisher partial_channel_failure_...` | 2.6s |  |
| **Implied test execution without the one slow test** | **~55s** | What the local loop *would* be if `arch_check_smoke_test` is tiered/split |
| `cargo llvm-cov nextest --workspace` (warm, with cov, retries=0) | **263.3s** | **+55% (+93s) coverage tax** over plain. Slow-test identity rotates due to separate `target/llvm-cov-target` dir. |
| `cargo clippy --workspace --all-targets` (warm) | **75.3s** | Below audit's 2–3 min estimate |
| `cargo test --workspace --doc` (warm) | **50.6s** | Essentially all compile; doctest exec itself 0.34s |
| `vox-cli ci ssot-drift` (warm) | **14.6s** | Below audit's 30–60s estimate. Flagged a real drift (`crates/vox-ml-cli/` nomenclature) — orthogonal triage |
| `vox-cli ci toestub-scoped --mode audit` full crates | _deferred to harness_ | Audit estimated 3–4 min |

### Cost-center summary (warm)

The 109-crate workspace is **NOT** slow in aggregate. Three concentrated costs dominate:

1. **`vox-arch-check` runs (~282s combined)** — once as a script in pre-push (164s), once as a workspace integration test (117s). Likely doing redundant work. Caching/incrementalization here is the single highest-leverage target.
2. **`vox-scientia` sandbox timeout test (29s)** — by-design timeout assertion. Move to a `slow` partition or shrink the timeout.
3. **`vox-codegen ai_fixture_bundle_compiles` (25s under cov)** — runs nested `cargo check`; cov instruments the nested compile. Either cache the bundle or strip cov from this one path.

Everything else (3,000+ unit tests, 1,500+ integration tests, snapshots) cost ~55s in aggregate. That's already excellent for the workspace size.

### Tier wall-clock totals (composed from measured parts, warm)

| Tier | Composition | Estimated wall-clock | Budget gap |
|---|---|---:|---|
| pre-commit (fmt + TDD guard) | <5s expected | ~5s | OK |
| pre-push **fast** | fmt + line-endings + ssot-drift (15s) + scoped TOESTUB (26s) + arch-check (165s) + scoped doc lint | **~215–230s (3.5–3.8 min)** | Doc says "1–3 min". **Over budget** mostly due to arch-check (77% of tier) |
| pre-push **complete** | fast + full doc lint/inv + clippy (75s) + scoped TOESTUB full | **~325s (5.4 min)** | Doc says "2–8 min" — within band |
| pre-push **full** (no cov) | complete + nextest workspace (170s) | **~495s (8.3 min)** | Doc says "10–25+ min" — already faster than doc claim |
| **local full + cov** (new tier) | complete + llvm-cov nextest workspace (263s) | **~590s (9.8 min)** | New tier — needs explicit budget |

### Headline gain ceiling (theoretical, from the data)

- Fix `vox-arch-check` so the script + test share a single ~30s run instead of doing 282s of overlapping work → save **~252s** across the fast + full+cov tiers
- Add `--since <ref>` impacted-crate test selector → typical small edit goes from 263s → ~3–15s (**~20–80x for inner loop**)
- Knob tuning (`jobs=24`, `lld-link`) → mostly cold-compile wins (5–10s saved warm; 30–60s saved cold)

### Remaining triage items surfaced during measurement

1. **2 pre-existing test failures** (cargo test):
   - `vox-arch-check::integration description_rule_produces_output_on_clean_workspace`
   - `vox-orchestrator orchestrator::tests::populi_single_owner::route_replay_tests::replay_moves_queued_task_to_group_default_agent`
   - Their pass/fail status rotates between `target/` and `target/llvm-cov-target` — likely build-cache / shared-state flakiness, not real logic bug
2. **SSOT drift error** in `docs/src/architecture/ai-laziness-remediation-plan-2026.md` (canonical path `crates/vox-ml-cli/` violation)
3. **`target/` at 492 GB** — 5.7x the "post-tuning" reference. Likely orphaned worktree/agent dirs or stale incremental.
4. **`jobs = 8`** under-utilizes 32-thread CPU. `lld-link` commented out (line 13 of `.cargo/config.toml`).
5. **Parallel cargo invocations conflict on `target/`** — multi-gate parallelization needs per-gate target dirs or strict serialization.

### Pre-existing test failures discovered during baseline run

Both reproduce on warm `cargo nextest run --workspace --no-fail-fast --retries 0`:
- `vox-arch-check::integration description_rule_produces_output_on_clean_workspace` (2.77s)
- `vox-orchestrator orchestrator::tests::populi_single_owner::route_replay_tests::replay_moves_queued_task_to_group_default_agent` (0.15s)

These are not caused by measurement; they predate this session. Filed as a separate triage item — speed work is orthogonal to fixing them, but the design's CI budget enforcement should NOT be applied until these are resolved or marked `#[ignore = ...]`.

## Meta-findings already validated by partial data

1. **Parallel cargo invocations conflict on `target/`.** Two simultaneous `cargo run`/`cargo test` calls fail with `os error 5: Access is denied` on `target/debug/<bin>.exe`. Any multi-gate parallelization plan must use distinct `CARGO_TARGET_DIR` per invocation (e.g. `target/gate-<name>/`) or serialize.
2. **`jobs = 8` on a 32-thread CPU is a free 3–4x parallelism win** if rustc parallelism is the bottleneck (TBD by build-only measurement).
3. **`lld-link` is sitting commented-out** in `.cargo/config.toml:13`. Windows MSVC linker is the default slow path. Easy uncomment + `rustup component add llvm-tools` step.
4. **`target/` at 492 GB** is ~5.7x the "after-tuning" reference (86 GB) cited in profile comments. Likely sources: stale incremental, orphaned worktree target dirs, agent target dirs predating the unified CARGO_TARGET_DIR config.
5. **nextest default retries=2** means flake-masking is on; baseline measurements use `--retries 0` to surface real timing.
