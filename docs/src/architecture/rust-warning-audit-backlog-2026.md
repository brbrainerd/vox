---
title: "Rust Warning Audit & Remediation Backlog (2026-05-11)"
description: "Closeout ledger from the May 2026 rustc / clippy / rustdoc audit — what was fixed, what is justifiably suppressed, what remains as scoped debt."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Defines warning-debt review cadence and per-crate ownership for the Rust toolchain in this workspace."
---

# Rust Warning Audit & Remediation Backlog (2026-05-11)

## Purpose

Single ledger for the May 2026 Rust warning audit. Records:

1. The **CI-parity baseline** the workspace currently meets.
2. The **broad allowances** still in place (with owners and removal criteria).
3. The **inventory of justified per-item suppressions** (what they're for and when to revisit).
4. The **review cadence** keeping new warnings from drifting back in.

Everything in §3 is intentional and acceptable today; the document exists so that none of it stays acceptable by accident.

## 1. CI-parity baseline (what's verified to pass today)

All three CI gates exit clean (`0`) on the audit branch:

```pwsh
# Workspace clippy under deny-warnings (the CI gate)
cargo clippy --workspace --all-targets -- -D warnings   # ✅ clean

# Workspace cargo check (lib + tests)
cargo check --workspace --all-targets                    # ✅ clean

# Workspace rustdoc under deny-warnings
$env:RUSTDOCFLAGS = "-D warnings"; cargo doc --workspace --no-deps   # ✅ clean
```

The workspace-wide clippy run produces no warnings outside the build-script note `vox-dashboard@0.5.0: vox-dashboard: app.vox compiled to app/src/generated/` (informational, not a lint).

## 2. Broad crate-level suppressions

**As of 2026-05-11 closeout: ZERO crates retain the broad `clippy::all = "allow"` allowance.**

Both previously-allowed crates were cleaned up:

| Crate | Status | What changed |
| --- | --- | --- |
| `vox-orchestrator` | ✅ closed | The previously-documented `dei_shim/research/orchestrator/{pipeline,pipeline_cache}.rs` compile errors were already resolved upstream. Audit removed the `[lints.clippy] all = "allow"` block, replaced with `[lints] workspace = true`, and burned down ~94 latent warnings across `mesh.rs`, `models/registry.rs`, `orchestrator/{vcs_ops,catalog_refresh,persistence/lifecycle}.rs`, `populi_remote.rs`, `usage.rs`, `workspace.rs`, `attention_tracker.rs`, `gate.rs`, `budget/mod.rs`, `planning/{orient,content_blocks}.rs`, `registry_model_resolve.rs`, `session/manager/lifecycle.rs`, `dei_shim/research/{orchestrator/{config,stages},search_policy_feedback}.rs`, plus six test files. Mix of mechanical fixes (sort_by → sort_by_key, manual prefix-strip → strip_prefix, manual clamp → clamp) and idiomatic struct-update rewrites. |
| `vox-orchestrator-mcp` | ✅ closed | Replaced `[lints.clippy] all = "allow"` with `[lints.clippy] all = { level = "warn", priority = -1 }` plus targeted `collapsible_if = "allow"` and `collapsible_match = "allow"` (the only patterns that genuinely don't read better collapsed in this crate's tool-dispatch shape). Burned down all other lints. |

Both crates now compile clean under the workspace-default `clippy::all = "warn"` policy.

## 3. Justified per-item suppressions inventory

All `#![allow(...)]` and `#[allow(...)]` blocks below were inspected during the audit and verified to fall into one of three categories:

- **A — Justified by Rust 2024 mechanics or upstream contract** (no action; revisit only if the upstream pattern changes).
- **B — Justified by serialization / wire-protocol concerns** (revisit when the consumer of the wire shape lands).
- **C — Justified pending a planned refactor** (revisit on the linked spec or sprint).

### 3.1 Crate-root `#![allow(...)]` (file-scope) — Category A

These are stable suppressions tied to language mechanics or test-only conventions.

| Crate / File | Lints allowed | Why (Category A) |
| --- | --- | --- |
| `vox-cli/src/lib.rs` | `clippy::collapsible_if`, `clippy::drop_non_drop` | Idiomatic in this crate; the `&&`-chain rewrite hurts diff-readability for command builders. |
| `vox-db/src/lib.rs` | `clippy::collapsible_if`, `clippy::needless_range_loop`, `clippy::single_char_add_str`, `clippy::redundant_closure`, `clippy::useless_vec` | SQL/migration codepaths; rewriting hurts step-by-step audit clarity. |
| `vox-db-types/src/lib.rs` | `clippy::collapsible_if`, `missing_docs` | DTO crate; docs live in `vox-db`; collapsible-if same as `vox-db`. |
| `vox-codegen/src/codegen_rust/mod.rs` | `clippy::collapsible_if` | Mirror of the parser-side patterns it's lowering. |
| `vox-codegen-ts/src/mod.rs` | `clippy::collapsible_if` | Same. |
| `vox-deploy-codegen/src/lib.rs` | `clippy::collapsible_if` | Same. |
| `vox-container/src/lib.rs` | `clippy::collapsible_if` | Same. |
| `vox-skills/src/lib.rs` | `clippy::collapsible_if` | Same. |
| `vox-openclaw-runtime/src/lib.rs` | `clippy::collapsible_if` | Same. |
| `vox-gamify/src/lib.rs` | `clippy::collapsible_if`, `clippy::type_complexity` | Same; type-complexity from generic event handlers. |
| `vox-package/src/lib.rs` | `clippy::collapsible_if`, `clippy::too_many_arguments`, `clippy::manual_unwrap_or_default` | Manifest builders; argument count is the contract surface. |
| `vox-actor-runtime/src/lib.rs` | `clippy::collapsible_if` | Same. |
| `vox-orchestrator/src/lib.rs` | `clippy::collapsible_if`, `clippy::too_many_arguments`, `clippy::unwrap_or_default`, `clippy::large_enum_variant`, `clippy::let_underscore_future` | Enum variants are wire-shaped; argument counts are contract-shaped; `let _ = future` is intentional fire-and-forget. |
| `vox-compiler/src/typeck/mod.rs` | `clippy::collapsible_if` | Idiomatic in pattern-match-heavy code. |
| `vox-package-types/src/{manifest,lockfile}.rs` | `clippy::new_without_default`, `clippy::should_implement_trait` | `new(x, y)` is the canonical constructor; no `Default` because manifests have no zero-state; `should_implement_trait` triggers on `from_*` constructors that are explicitly NOT `From` impls. |
| `vox-plugin-host/src/{loader,discover,lib}.rs` | `clippy::result_large_err` | `Result<T, PluginError>` where `PluginError` is the contract surface. |
| `vox-plugin-api/src/lib.rs` | `unsafe_code`, `non_local_definitions` | Plugin ABI requires `extern "C"` shapes. |
| `vox-test-harness/src/env_scratch.rs` | `unsafe_code` | Rust 2024: `std::env::{set_var,remove_var}` are `unsafe`; this module is the workspace's serialized env-mutation harness. |
| `vox-populi/src/mens/tensor/mod.rs` | `clippy::module_inception` | `tensor/tensor.rs` is the file split by tensor-kind sub-modules. |
| `vox-populi/src/transport/mod.rs` | `missing_docs` | Generated wire-shapes documented at the field level. |
| `vox-populi/src/mens/mod.rs`, `quota/mod.rs`, `pairing/mod.rs` | `missing_docs` | Same. |
| `vox-cli/src/render.rs`, `v0_validate.rs`, `v0_tsx_validate.rs`, `build_service.rs` | `dead_code` | Staged surfaces awaiting CLI wiring; commented inline. |
| `vox-compiler/src/typeck/policy.rs` | `dead_code` | Policy table consumed via `cfg(feature = ...)` paths. |
| `vox-ml-cli/src/commands/ai/serve/inference.rs` | `dead_code` | Staged inference surface. |
| `vox-ml-cli/src/commands/mens/eval_gate/mod.rs` | `unused_imports` | Re-exports consumed by feature-gated `populi` / `ai::train` call sites. |

### 3.2 Test-file `#![allow(...)]` (file-scope) — Category A

All `crates/*/tests/*.rs` and `crates/vox-integration-tests/tests/*.rs` files that allow `missing_docs` are doing so because they're integration tests where doc coverage isn't expected. All `unsafe_code` allows in tests are Rust 2024 `std::env::{set_var, remove_var}` for serialized env-mutating tests, gated by a serial `Mutex` per the harness pattern.

This is policy, not debt. Listed in the suppression inventory only for completeness.

### 3.3 Per-item `#[allow(...)]` (function/struct-scope) — Category B (DTO / wire-shape)

These allow `dead_code` on wire-facing DTOs whose fields exist for the contract, not for current callers:

| Location | Lint | DTO purpose |
| --- | --- | --- |
| `vox-actor-runtime/src/llm/types.rs::FixtureModelIntentResolvedEvent` | `dead_code` | Aligns with `contracts/telemetry/fixture-model-intent-resolved.v1.schema.json`; emitter integration tracked in ADR-037 (`@subagent` decorator). |
| `vox-actor-runtime/src/llm/types.rs::OrchSubagentDispatchEvent` | `dead_code` | Aligns with `contracts/telemetry/orch-subagent-dispatch.v1.schema.json`; emitter integration tracked in ADR-037. |

Removal: when ADR-037 emit sites are wired, the constructors light up in the orchestrator dispatch path and the allow drops naturally.

### 3.4 Per-item `#[allow(...)]` (function-scope) — Category A (lock-across-await)

| Location | Lint | Justification |
| --- | --- | --- |
| `vox-integration-tests/tests/mcp_project_init_test.rs::vox_project_init_writes_nested_application` | `clippy::await_holding_lock` | Lock intentionally held across awaits to serialize CWD-mutating tests. |
| `vox-populi/tests/dispatch_persistence.rs::verify_dispatch_results_persistence_across_restart` | `clippy::await_holding_lock` | Lock intentionally held across awaits to serialize env-mutating tests. |
| `vox-populi/tests/http_control_plane.rs` (file-level) | `clippy::await_holding_lock` | Same. |
| `vox-publisher/tests/scholarly_zenodo_mock_test.rs` (file-level) | `clippy::await_holding_lock` | Tests serialize env + mock via std mutex; guard held for whole async body. |
| `vox-publisher/tests/scholarly_openreview_mock_test.rs` (file-level) | `clippy::await_holding_lock` | Same. |
| `vox-cli/tests/pm_lifecycle_integration.rs` (file-level) | `clippy::await_holding_lock` | `PM_WORKDIR_GUARD` serializes cwd-sensitive PM tests across awaits. |

Removal: only if the harness is rewritten to use `tokio::sync::Mutex` plus a global serial dispatcher — not planned.

### 3.5 Per-item `#[allow(...)]` — Category A (RPIT trait shape)

| Location | Lint | Justification |
| --- | --- | --- |
| `vox-workflow-runtime/tests/workflow_tracker_tests.rs::is_activity_completed` | `clippy::manual_async_fn` | Trait uses RPIT (`impl Future + Send`), implementor must match signature. |
| `vox-populi/src/mens/hardware/tests.rs` (inner `mod tests`) | `clippy::module_inception` | File is `tests.rs`; inner `mod tests` keeps `cfg(test)` items grouped. |
| `vox-publisher/src/publication_preflight/tests.rs` (inner `mod tests`) | `clippy::module_inception` | Same. |

## 4. Open debt (what to fix next)

All P0 / P1 items from the May 2026 audit are **closed**. Remaining items are P2 / P3.

### 4.1 P2 — Crate-root `#![allow(clippy::collapsible_if)]` consolidation

13 crates carry the same `#![allow(clippy::collapsible_if)]`, plus `vox-orchestrator-mcp` now carries it as a per-crate `[lints.clippy]` override (with `collapsible_match` alongside). Three options for closure:

1. **Keep as-is** (current default). Each crate has signal that the lint is intentional.
2. **Promote to workspace `[lints.clippy]`** in root `Cargo.toml`. Removes 13 lines but loses per-crate signal.
3. **Burn down site-by-site** (collapsing the `if let && if let` patterns to `if let && let && ...` Rust-2024 form, which the audit already did in `vox-search`, `vox-compiler`, etc.).

Recommendation: Option 3 over the next few quarters as files are touched for other reasons. Don't sprint on it.

### 4.2 P3 — `--all-features` test compile errors in `vox-orchestrator`

Running `cargo check -p vox-orchestrator --all-features --tests` surfaces ~15 errors in `crates/vox-orchestrator/src/orchestrator/tests/populi_single_owner.rs` referencing `RemoteTaskEnvelope`, `RemoteTaskResult`, `REMOTE_TASK_ENVELOPE_TYPE`, `REMOTE_TASK_RESULT_TYPE`, `REMOTE_TASK_CANCEL_TYPE`. These symbols are gated behind the `populi-transport` feature but the test does not import the gated module. The default-feature test run is clean.

This is **not** a CI gate and does **not** affect the `-D warnings` parity. Tracked here only so it doesn't surprise a future contributor running `--all-features` locally.

## 5. Review cadence

### 5.1 Per-PR (CI-enforced)

- `cargo clippy --workspace --all-targets -- -D warnings` — already gated; **must stay green**.
- `cargo check --workspace --tests` — already gated.

### 5.2 Pre-merge (recommended for any PR touching `crates/vox-orchestrator/**` or `crates/vox-orchestrator-mcp/**`)

- Run `cargo clippy -p <crate> -- -W clippy::all` locally and inspect the per-crate report. New lint instances introduced under the broad-allow umbrella still need an inline `#[allow(...)]` with rationale, even though they don't fail CI.

### 5.3 Quarterly (during the contributor-experience review)

- Re-run the §1 baseline.
- Diff the §3 inventory against `rg "^#!\[allow\(" crates/` and `rg "^#\[allow\(" crates/` to detect new allows that arrived without an audit entry.
- Reassess the §4 P0/P1 items: if `vox-orchestrator` compile errors are still blocking, re-confirm the upstream owner is making progress.

### 5.4 On crate refactor

- Whenever a P0 or P1 crate's compile/lint blocker resolves, the broad `clippy::all = "allow"` line is removed in the same PR. The Cargo.toml comment block is the checklist.

## 6. Verification commands (cheat sheet)

```pwsh
# Full workspace deny-warnings parity check (matches CI gate)
cargo clippy --workspace --all-targets -- -D warnings   # ✅ clean

# Workspace cargo check (lib + tests)
cargo check --workspace --all-targets                    # ✅ clean

# Workspace rustdoc parity
$env:RUSTDOCFLAGS = "-D warnings"; cargo doc --workspace --no-deps   # ✅ clean

# Identify justified suppressions across the workspace
rg "^#!\[allow\(" crates/   # crate-root / file-root
rg "^#\[allow\("  crates/   # per-item
```

## 7. Cross-references

- `crates/vox-orchestrator/Cargo.toml` — now `[lints] workspace = true`.
- `crates/vox-orchestrator-mcp/Cargo.toml` — explicit `clippy::all = warn` with `collapsible_if`/`collapsible_match` allowed locally.
- `docs/agents/governance.md` — workspace-level lint and structural-limits policy.
- `AGENTS.md §Test-First Policy` — describes how `vox-code-audit` works alongside clippy.
