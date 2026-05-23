---
title: "Script surface audit and Vox migration"
description: "SSOT inventory of tracked Python, PowerShell, and shell scripts with owners, essential vs replaceable classification, mapping to vox ci / vox mens equivalents, and noted capability gaps versus container/runtime boundaries."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Script surface audit and Vox migration

This document is the **SSOT** for tracked `.py`, `.ps1`, and `.sh` scripts: purpose, essentiality, replacement `vox` commands, capability gaps, and migration phases.  
Policy for thin CI wrappers: [`scripts/README.md`](../../../scripts/README.md), runner contract [`docs/src/ci/runner-contract.md`](../ci/runner-contract.md), machine inventory [`docs/agents/script-registry.json`](../../agents/script-registry.json).

## Canonical inventory (git-tracked)

| Path | Owner category |
|------|----------------|
| `crates/vox-compiler/src/typeck/checker.py` | **Removed** (empty; real checker is Rust `typeck/checker/`). |
| `patches/aegis-0.9.8/src/test-vectors/gen.py` | Vendor patch maintenance |
| `scripts/extract_mcp_tool_registry.py` | Legacy migration recovery (gated) |
| `infra/containers/entrypoints/populi-entrypoint.sh` | Runtime boundary (container) |
| `infra/containers/entrypoints/vox-entrypoint.sh` | Runtime boundary (container) |
| `scripts/check_codex_ssot.ps1` | CI guard wrapper |
| `scripts/check_codex_ssot.ps1` | CI guard wrapper |
| `scripts/check_cuda_feature_builds.sh` | CI guard wrapper |
| `scripts/check_docs_ssot.ps1` | CI guard wrapper |
| `scripts/check_docs_ssot.sh` | CI guard wrapper |
| `scripts/check_vox_cli_feature_matrix.sh` | CI guard wrapper |
| `scripts/check_vox_cli_no_vox_orchestrator.sh` | CI guard wrapper |
| `scripts/install.ps1` | Bootstrap |
| `scripts/install.sh` | Bootstrap |
| `scripts/mens_release_gate.ps1` | Mens gate wrapper |
| `scripts/mens_release_gate.sh` | Mens gate wrapper |
| `scripts/mens/release_training_gate.ps1` | Legacy gate forwarder |
| `scripts/mens/release_training_gate.sh` | Legacy gate forwarder |
| `scripts/populi/cursor_background_cuda_build.ps1` | Local dev helper |
| `scripts/populi/cursor_background_cuda_build_detached.ps1` | Local dev helper |
| `scripts/populi/cursor_background_train_example.ps1` | Local dev helper |
| `scripts/populi/dogfood_qlora_cuda.ps1` | Operator preset |
| `scripts/populi/mens_gate_safe.ps1` | **Essential** (Windows gate isolation) |
| `scripts/populi/release_ci_full_gate.ps1` | Gate wrapper |
| `scripts/populi/release_training_gate.ps1` | Gate wrapper |
| `scripts/populi/release_training_gate.sh` | Gate wrapper |
| `scripts/populi/vox_continuous_trainer.ps1` | Legacy orchestration |
| `scripts/quality/toestub_scoped.sh` | CI guard wrapper |
| `scripts/run_mens_pipeline.ps1` | Local dev helper |
| `scripts/run_qwen35_qlora_real_4080.ps1` | Operator preset (Qwen 3.5 SSOT; `run_qwen25_*` is deprecated shim) |
| `scripts/telemetry_watch.ps1` | Local dev UX |
| `scripts/toestub_self_apply.ps1` | Quality helper |
| `scripts/toestub_self_apply.sh` | Quality helper |
| `scripts/verify_workspace_manifest.sh` | CI guard wrapper |
| `scripts/windows/ensure_cuda_path.ps1` | **Removed** (Lifted to `vox doctor --fix-cuda-path`) |
| `scripts/windows/run_4080_experiment_cycles.ps1` | Operator batch recipe |
| `scripts/windows/stop_stuck_cargo_tests.ps1` | **Removed** (Lifted to `vox ci kill-stuck-tests`) |
| `tools/jj-checkpoint.ps1` | VCS helper (Jujutsu) |

## Essentiality and justification

### Essential (keep; not substitutable by Vox-the-language)

| Script | Role |
|--------|------|
| [`scripts/install.sh`](../../../scripts/install.sh) / [`install.ps1`](../../../scripts/install.ps1) | Chicken-and-egg bootstrap: download/verify `vox-bootstrap`, no `vox` on PATH yet. |
| [`scripts/populi/mens_gate_safe.ps1`](../../../scripts/populi/mens_gate_safe.ps1) | Until lifted into Rust: isolated `CARGO_TARGET_DIR`, temp `vox.exe`, `-Detach`, log tee — **Windows file-lock / agent timeouts**. |
| [`infra/containers/entrypoints/vox-entrypoint.sh`](../../../infra/containers/entrypoints/vox-entrypoint.sh) | PID1 sidecar: background `populi serve` + `exec` main (container semantics). |
| [`infra/containers/entrypoints/populi-entrypoint.sh`](../../../infra/containers/entrypoints/populi-entrypoint.sh) | Cloud train/serve/agent dispatch: `curl`, HF CLI, traps — **runtime boundary** (see gaps below). |

### Useful but replaceable

- **CI shims** (`check_*`, `verify_workspace_manifest`, `toestub_scoped`, gate one-liners): canonical behavior is **`vox ci …`**; scripts exist for `cargo run -p vox-cli` ergonomics only.
- **`run_mens_pipeline.ps1`**, **`run_qwen35_qlora_real_4080.ps1`**, **`dogfood_qlora_cuda.ps1`**: operator presets over **`vox mens train`** / **`cargo vox-cuda-release`**.
- **`cursor_background_*.ps1`**, **`telemetry_watch.ps1`**: IDE/logging UX; could become one `vox` subcommand each if pain remains high.

### Legacy or cleanup

- **`vox_continuous_trainer.ps1`**: hard-coded `build_vox.bat`, loop — superseded by **`vox mens corpus …`** + **`vox mens pipeline`**; retain only if actively used, else archive.
- **`toestub_self_apply.*`**: prefer **`vox ci toestub-scoped`** with explicit root and CI-aligned flags.
- **`extract_mcp_tool_registry.py`**: legacy migration tool, disabled by default (`VOX_ALLOW_LEGACY_MCP_EXTRACT=1` + `--allow-legacy`); SSOT is YAML + `vox-mcp-registry/build.rs` (see [`docs/src/reference/mcp-tool-registry-contract.md`](../reference/mcp-tool-registry-contract.md)).
- **`patches/.../gen.py`**: Aegis vector regen only when updating the vendored patch.

## Map to Vox (duplicate vs gap)

### Fully duplicated by `vox ci` (or `vox` mens surface)

| Script pattern | Canonical command |
|----------------|-------------------|
| `check_docs_ssot.*` | `vox ci check-docs-ssot` |
| `check_codex_ssot.ps1` | `vox ci check-codex-ssot` |
| `verify_workspace_manifest.sh` | `vox ci manifest` |
| `check_vox_cli_feature_matrix.sh` | `vox ci feature-matrix` |
| `check_vox_cli_no_vox_orchestrator.sh` | `vox ci no-vox-orchestrator-import` |
| `check_cuda_feature_builds.sh` | `vox ci cuda-features` |
| `quality/toestub_scoped.sh` | `vox ci toestub-scoped [ROOT]` |
| `mens_release_gate.*`, `populi/release_*_gate.*`, `mens/release_*` | `vox ci mens-gate --profile training|ci_full|m1m4` |
| `run_mens_pipeline.ps1` | `vox mens pipeline …` |

**Vox language note:** These are **host CLI** capabilities (Rust `vox-cli`), not features of the `.vox` language. A future “Vox scripts” layer should call the same primitives via a small **host ABI** (see [Boundary policy](#boundary-policy-keep-vs-migrate)).

### Partially duplicated (orchestration / UX gap)

| Need | Today | Gap |
|------|--------|-----|
| Windows-safe mens gate | `mens_gate_safe.ps1` | **Done in Rust:** `vox ci mens-gate --windows-isolated-runner` (+ `--gate-build-target-dir`, `--gate-log-file`). PS1 is thin delegate + **`-Detach`** only. |
| Live training tails | `telemetry_watch.ps1` | **Done:** `vox mens watch-telemetry` (alias `watch`; default 3s poll). PS1 delegates. |
| CUDA release build + log | `cursor_background_cuda_build*.ps1` | **Done:** `vox ci cuda-release-build` (tee under `mens/runs/logs`); PS1 delegates. |
| Full-repo TOESTUB | `toestub_self_apply.*` | **Done:** `vox ci toestub-self-apply`; shell scripts delegate. |
| Cloud container train | `populi-entrypoint.sh` | **Train:** `vox mens train`. **Serve:** `vox mens serve` + **`vox-schola`** copied in [`infra/containers/Dockerfile.populi`](../../../infra/containers/Dockerfile.populi). **Agent:** still explicit unsupported in entrypoint (use cloud dispatch). |

### Not a Vox-language duplicate (keep at boundary)

- OS env mutation (`vox doctor --fix-cuda-path`).
- Process kill (`vox ci kill-stuck-tests`).
- JJ workflow (`tools/jj-checkpoint.ps1`).
- Vendor crypto vector gen (`patch gen.py`).

## Ranked capability gaps (low K-complexity first)

1. ~~**Lift Windows mens-gate workaround into Rust**~~ — shipped: `--windows-isolated-runner` / `--gate-log-file` / `--gate-build-target-dir`.
2. ~~**`vox mens watch-telemetry`**~~ — shipped (alias `watch`).
3. ~~**TOESTUB self-apply**~~ — shipped: `vox ci toestub-self-apply`.
4. **Docker entrypoint** — train + serve paths updated in `docker/populi-entrypoint.sh` + `Dockerfile.populi` (`vox-schola` CPU build in slim builder). **Agent** still unsupported in-container (cloud dispatch).
5. **Bootstrap remains `vox-bootstrap`** — do not grow compiler “standard library” for HTTPS install.

## Administrative OS mutations
Administrative OS tasks are implemented as native `vox` CLI primitives rather than shell scripts or language built-ins, preserving boundary security and eliminating "blue code" (PowerShell dependency).
- `vox doctor --fix-cuda-path`
- `vox ci kill-stuck-tests`

## Phase 1 cleanups (done)

- Removed empty `crates/vox-compiler/src/typeck/checker.py` (doc inventory regenerated).
- Fix [`scripts/populi/dogfood_qlora_cuda.ps1`](../../../scripts/populi/dogfood_qlora_cuda.ps1) -> use **`vox mens train`** (not `vox populi train`).
- Align [`infra/containers/entrypoints/populi-entrypoint.sh`](../../../infra/containers/entrypoints/populi-entrypoint.sh) **train** branch to **`vox mens train`**; document **serve/agent** limitations in this doc.
- Mark **`vox_continuous_trainer.ps1`** as deprecated in-script; prefer **`vox mens corpus`** + **`vox mens pipeline`**.
- Correct **[`scripts/README.md`](../../../scripts/README.md)** canonical train line to match **`vox mens train`** (matches `run_qwen35_qlora_real_4080.ps1`).
- Extend [`docs/agents/script-registry.json`](../../agents/script-registry.json) with missing tracked scripts.

## Phase 2 (implemented in `vox-cli`)

### `vox ci mens-gate` (Windows)

- `--windows-isolated-runner` — `cargo build -p vox-cli` to OS temp `…/vox-targets/<repo-hash>/mens-gate-safe` by default (or `--gate-build-target-dir`), copy `vox.exe` to `%TEMP%`, set `VOX_MENS_GATE_INNER=1`, re-run gate steps (see [`matrix.rs`](../../../crates/vox-cli/src/commands/ci/run_body_helpers/matrix.rs)).
- `--gate-log-file <path>` — tee child stdout/stderr (isolated runner only).
- **Detach** for IDE timeouts remains in [`scripts/populi/mens_gate_safe.ps1`](../../../scripts/populi/mens_gate_safe.ps1) (`Start-Process`); non-detach path calls `vox` with the flags above.

### `vox mens watch-telemetry` (alias `watch`)

- Default paths { `target/dogfood/train.err.log`, `target/dogfood/telemetry.jsonl`; `--interval-ms` (default 3000).
- See [`watch_telemetry.rs`](../../../crates/vox-cli/src/commands/mens/watch_telemetry.rs).

### `vox ci cuda-release-build`

- Teeing release build with `gpu,mens-candle-cuda`; see [`cuda_release_build.rs`](../../../crates/vox-cli/src/commands/ci/run_body_helpers/cuda_release_build.rs).

### `vox ci toestub-self-apply`

- Release-builds `vox-toestub` then runs full-repo `toestub` binary (replaces ad-hoc `cargo`-only scripts).

## Boundary policy (keep vs migrate)

| Layer | Owns | Do not move into Vox language core |
|-------|------|-------------------------------------|
| **Bootstrap** | `vox-bootstrap`, `install.*` | HTTPS, manifest parse, archive extract |
| **CLI** | `vox`, `vox ci`, `vox mens`, `vox schola` | Policy guards, nested `cargo`, training orchestration |
| **Container / OS** | entrypoints, `ensure_cuda_path`, stuck-test killer | PID1, `curl` provider APIs, registry env writes |
| **Future Vox scripts** | `.vox` + host | Narrow `host::*` ABI: `process`, `env`, `fs`, optional gated `http_fetch` — **deny-by-default** in sandbox |

Goal: **one Rust CLI** + **minimal POSIX glue** where the OS requires it — not a POSIX shell inside the language.

## Acceptance metrics

| Metric | Target |
|--------|--------|
| Wrapper script reduction | ≥ **50%** of `scripts/check_*.sh` / twin `.ps1` removable from *default* docs/CI once callers use `vox ci …` directly |
| Canonical command parity | Every **non-essential** script row in `script-registry.json` has `replacement` = single `vox …` or `vox-bootstrap` line |
| Workflow stability | No CI job regression: same profiles for `mens-gate`, SSOT checks, manifest, feature matrix |
| Docker train | `VOX_JOB_KIND=train` invokes **`vox mens train`** with HF data dir and output dir |
| Dead paths | Zero empty or misleading “checker” files next to Rust modules |

---

**Maintenance:** When adding scripts, update [`docs/agents/script-registry.json`](../../agents/script-registry.json) and this inventory table in the same PR.

