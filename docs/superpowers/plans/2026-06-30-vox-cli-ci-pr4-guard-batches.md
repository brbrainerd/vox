# PR-4 continuation: incremental CI guard migration to vox-cli-ci

> Execution plan for the rest of PR-4 (CI extraction). The contracts seam + cmd_enums +
> a first guard batch are LANDED; this is the turnkey recipe for the remaining guards
> and the final dispatcher move. Spec: docs/superpowers/specs/2026-06-30-vox-cli-contracts-ci-extraction.md.

## Landed so far (origin/main)
- `eaa59179d4` — Step 1: cmd_enums (CiCmd + 11 nested enums + ReleasePackage/CompletionGateMode
  dependency-free enum defs) → vox-cli-ci; re-exported via `commands::ci::CiCmd`. clap goldens pass.
- `f47419d6e6` — Step 3a batch 1: 9 leaf guards moved (check_links, doctest_md, canonical_docs,
  contracts_index, free_binary, parse_status, kill_stuck_tests, install_hooks, test_inventory).
- vox-cli-ci deps now include: clap, chrono, vox-bounded-fs, vox-doc-pipeline, vox-jsonschema-util,
  vox-compiler, vox-git, vox-config, owo-colors, sysinfo, tokio (+ insta/proptest dev).

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

## Final steps (the headline — needs its own focused pass)
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
