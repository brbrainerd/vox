# PR-4 continuation: incremental CI guard migration to vox-cli-ci

> Execution plan for the rest of PR-4 (CI extraction). The contracts seam + cmd_enums +
> a first guard batch are LANDED; this is the turnkey recipe for the remaining guards
> and the final dispatcher move. Spec: docs/superpowers/specs/2026-06-30-vox-cli-contracts-ci-extraction.md.

## Landed so far (origin/main) — ALL 60 Tier-1 guards migrated
- Step 1 `eaa59179d4`: cmd_enums (CiCmd + 11 nested enums + dependency-free ReleasePackage/
  CompletionGateMode) → vox-cli-ci; re-exported via `commands::ci::CiCmd`. clap goldens pass.
- Batches 3a–3h (`f47419d6e6`, `d485349882`, `38ad7fd70c`, `accb52b8d3`, `80b508bbe4`,
  `9e2561d292`, `63a841a498`): 60 guards moved in 8 batches (leaf → helper-dependent → the
  two spec-mislabeled Tier-1s completion_quality/data_storage_guard + mens_scorecard).
- Helper extraction `3089b2e374`: repo_root/cargo_bin/nvcc + constants → vox-cli-ci (re-exported
  to vox-cli), which unlocked the coupling=1 guards.
- Feature passthrough `c3309e4544`: vox-cli-ci gained a `completion-toestub` feature (+ optional
  vox-code-audit) forwarded from vox-cli.
- vox-cli-ci deps now: clap, chrono, tokio, owo-colors, sysinfo, which, reqwest, strsim, toml,
  toml_edit + vox-bounded-fs/doc-pipeline/jsonschema-util/compiler/git/config/repository/db/
  http-client/secrets/graph-reader/rule-pack/orchestrator/orchestrator-mcp/grammar-export/
  scaling-policy/plugin-{api,catalog,host,types}/publisher/cli-contracts (+ insta/proptest dev).

## REMAINING (the final step): ~15 Tier-2 guards + the dispatcher move
Still in `crates/vox-cli/src/commands/ci/` because they reach into vox-cli internals
(`crate::command_registry_model`, `crate::frontend`, `crate::commands::runtime`, `crate::benchmark`,
`crate::commands::scientia`) or reference the dispatcher: **build_timings, capability_sync,
command_sync, eval_matrix, exec_policy_contract, gui_catalog_parity, gui_surface_coverage,
gui_surface_registry, operations_catalog, pipeline_parity, policy_allowlist_parity, policy_registry,
pre_push, release_build, runner_scale** (+ providers.rs, run_body.rs, workspace_artifacts, the
profile_parity shim). These are the HeavyGuardHost set — do the dispatcher move + `HeavyGuardHost`
(Steps 2/3-final/4 below) as ONE careful pass; it's the un-chunkable hub. Freshness (run_body.rs:56)
STAYS in vox-cli before the delegating call.

## Key insight (why this is safe + incremental)
The dispatcher (`run_body.rs`) STAYS in vox-cli for now; moving a guard just changes its caller from
`super::guard::run` to `vox_cli_ci::guard::run`. Each moved guard recompiles in vox-cli-ci instead of
vox-cli's full surface — the build-time win accrues per guard. No HeavyGuardHost needed until the
dispatcher itself moves (the final step).

## The proven per-batch recipe
1. **Pick a batch** of Tier-1 guards with `coupling=0` first (no `crate::`/`super::<non-ci>` refs):
   `grep -cE "crate::|super::[a-z]" <guard>.rs`. Move connected sibling-referencing guards together.
2. `git mv crates/vox-cli/src/commands/ci/<g>.rs crates/vox-cli-ci/src/<g>.rs` for each.
3. **Comprehensive dep scan** (NOT just `vox_*` — that misses sysinfo/owo-colors/tokio):
   `grep -rhoE "\b[a-z][a-z0-9_]+::"` the moved files, cross-check vs vox-cli-ci/Cargo.toml + workspace.
   Add runtime deps to `[dependencies]`, test-only (insta/proptest/tempfile) to `[dev-dependencies]`.
4. `pub mod <g>;` in vox-cli-ci/lib.rs (after `pub mod affected;`); delete `mod <g>;` from ci/mod.rs.
5. Rewire vox-cli refs (both the `::`-suffixed and bare-module forms):
   `s#\b(super::<g>|crate::commands::ci::<g>)\b#vox_cli_ci::<g>#g`.
6. `cargo build -p vox-cli-ci` then `-p vox-cli`. Widen any `pub(crate)/pub(super) fn` called
   cross-crate to `pub` (compiler flags them as E0624 private).
7. `cargo clippy --fix --lib -p vox-cli-ci` (the moved guards may carry collapsible-if etc.); confirm 0.
8. Commit per batch. Use `env -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER CARGO_BUILD_RUSTC_WRAPPER=""`.

## Remaining Tier-1 guards (~54) — suggested batches
Group by likely-shared deps to minimize Cargo churn; build after each:
- **config_*** : config_aggregate, config_gui_codegen, config_hygiene, config_registry_parity
- **plugin_*** : plugin_abi_parity, plugin_catalog_parity, plugin_catalog_sync, plugin_skill_parity,
  plugin_surface, generate_plugin_catalog_docs, mcp_vox_surface_parity
- **gui_*** : gui_honesty, gui_smoke, gui_version_sync
- **scientia/policy** : scientia_heuristics_parity, scientia_worthiness_contract, policy_allowlist_parity,
  retired_symbol_check, safety_inventory, model_routing_check
- **test/build** : test_governance (uses test_inventory — already moved), compile_matrix, build_bench,
  job_timings, determinism_audit, dev_loop_audit, detect_rules_bench, doctest_build→doctor_build_cache,
  kill/parse done, test_runtime_report, tier_budget_check, fan_in_budget, crate_budget, free done
- **docs/attention** : canonical done, docs_reality_audit, contracts done, attention_ledger_parity,
  attention_parity, pm_provenance, profile_parity, db_schema_coverage, data_storage_guard,
  dep_cycles, crate_build_map_parity, deploy_status, capability_snapshot, agentskills_compliance,
  grammar_ssot_parity, pipeline_parity, coverage_gates, mens_scorecard, scaling_audit, speech_runtime_suite,
  watch_run, coolify_eval
Re-run the `coupling=0` scan before each — some may have hidden Tier-2 leakage (route those via the host
in the final step instead of moving).

## Final steps (the headline — needs its own focused pass) — FULLY DE-RISKED (2026-06-30)
Coupling analysis done: the ONLY non-guard deep coupling in run_body + run_body_helpers is tiny —
- `crate::artifact_policy::{gate_isolated_target,ci_nested_target}` (matrix.rs) = `pub use vox_cli_core::artifact_policy` → just rewrite to `vox_cli_core::artifact_policy` (vox-cli-ci already has vox-cli-core). NO move.
- `crate::frontend::pnpm_executable()` (run_body.rs:219, Astro docs arm) → inline `if cfg!(windows) {"pnpm.cmd"} else {"pnpm"}` (frontend.rs stays for its 3 non-ci users).
- `crate::benchmark_telemetry` → DONE, moved to vox-cli-ci (`285754bc5c`).
- `crate::freshness::enforce_for_ci` (run_body.rs:56) → STAYS in vox-cli (moves to ci/mod.rs before delegating).
- `crate::commands::policy` (gate-status) → already covered by the GateStatusWriter seam on VoxCliProviders.

run_body.rs + run_body_helpers/ (16 files) MOVE AS ONE UNIT (helpers ref `super::run_manifest`/`super::gate_status`).
Structure of `run()`: `let root=repo_root(); freshness; let gate_id=cmd.gate_policy_id(); let result: Result = match cmd {...700 lines...}; <gate-status wrap via VoxCliProviders>; result`.
HeavyGuardHost design (put in **vox-cli-ci**, not contracts, so it can take `&CiCmd`):
```rust
pub trait HeavyGuardHost: vox_cli_contracts::GateStatusWriter {
    fn dispatch_heavy(&self, cmd: &CiCmd, root: &Path) -> Option<anyhow::Result<()>>;
}
```
Moved `run(cmd, host: &dyn HeavyGuardHost)`: host-first — `if let Some(r)=host.dispatch_heavy(&cmd,&root){r} else { match cmd {<Tier-1 arms> _=>unreachable!()} }`. **The delicate part = excising the ~24 Tier-2 match arms** (GuiCatalogParity/GuiSurfaceCoverage/GuiSurfaceRegistry/PolicyRegistry{2}/ExecPolicyContract/OperationsVerify+Sync/PrePush{multi-line PrePushOpts}/FmtCheck/EvalMatrix{2}/BuildTimings/PipelineParity/PolicyAllowlistParity/CapabilitySync/CommandSync/ReleaseBuild/RunnerScale+Preflight+Status/CommandCompliance) into `impl HeavyGuardHost for VoxCliProviders` + `_=>unreachable!()`.
**GOTCHA: `run_body_helpers/docs.rs` calls a Tier-2 guard from HELPER code (not just run_body's match)** → that call also routes via host (pass `&dyn HeavyGuardHost` into the helper, or keep that specific guard's logic inline). This is the one spot the "match-split only" mental model misses.
15 Tier-2 guards to impl in dispatch_heavy: build_timings, capability_sync, command_sync, command_compliance, eval_matrix, exec_policy_contract, gui_catalog_parity, gui_surface_coverage, gui_surface_registry, operations_catalog, pipeline_parity, policy_allowlist_parity, policy_registry, pre_push, release_build, runner_scale.

### Original outline (superseded by the above but kept for the step ordering)
- **Step 2** HeavyGuardHost trait in vox-cli-contracts (`&str`-keyed) + impl on VoxCliProviders for the
  13 Tier-2 guards (build_timings, eval_matrix, release_build, runner_scale, gui_catalog_parity,
  gui_surface_coverage, gui_surface_registry, policy_registry, operations_catalog, capability_sync,
  command_sync, exec_policy_contract, scientia_novelty_ledger_contract).
- **Step 3-final** move run_body.rs → vox-cli-ci/src/run.rs as `pub async fn run(cmd, host)`; Tier-2 arms
  call `host.dispatch_heavy(id, root, &cmd)`. **DELETE the freshness call (run_body.rs:56) from the moved
  body** — it stays in vox-cli.
- **Step 4** ci/mod.rs: `crate::freshness::enforce_for_ci(&root)?;` BEFORE `vox_cli_ci::run(cmd, &VoxCliProviders).await`
  (preserves the runner-* exemption, commit 265532e3). Verify `vox ci runner-scale` still skips freshness.
- Gates: clap goldens byte-identical, `cfg(feature` grep on moved files (feature passthrough), freshness re-baseline.

## Gotchas hit
- cmd_enums had `as_cli_str`/`label` as `pub(crate)` → must be `pub` cross-crate.
- vox-cli-ci/lib.rs: `pub mod` must come AFTER the `//!` inner doc (E0753).
- The dep scan MUST include non-`vox_*` crates (sysinfo, owo-colors, tokio, insta, proptest surfaced only on build).
- Pre-existing unrelated red test: `ci_workflow_contract::cross_platform_gate_is_required_three_os_matrix`
  fails because cross-platform-check.yml lacks the literal `ubuntu-latest` — NOT caused by PR-4.
