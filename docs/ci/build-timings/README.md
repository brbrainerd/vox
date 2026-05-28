# Build timing snapshots

Machine-local wall-clock samples from `vox ci build-timings` (see `docs/src/ci/runner-contract.md`).

## How to record

```bash
# From repo root; optional: isolate target dir
export CARGO_TARGET_DIR=target-build-timings
# Omit CUDA lane (faster) when comparing compiler/data/ML lanes only:
export SKIP_CUDA_FEATURE_CHECK=1
# Capture **stdout only** (JSON lines). Do not redirect stderr into the file or Cargo progress will corrupt JSONL.
cargo run -p vox-cli --quiet -- ci build-timings --crates --json > docs/ci/build-timings/latest.jsonl
```

Each output line is one JSON object: `lane`, `ok`, `duration_ms`, optional `error`.

## AI / agent loop thresholds (informal)

These are **not** CI gates — use them to spot regressions when agents alternate shells or rerun **`vox ci pre-push`** excessively.

| Check | Tool | Advisory threshold |
|-------|------|---------------------|
| Target-dir fragmentation | **`vox ci dev-loop-audit --json`** | **`fragmentation_risk`** should be **`none`** for routine coding |
| Pre-push wall-clock | **`vox ci pre-push --report-json …`** | Compare **`total_ms`** trend vs your baseline; investigate if **`--quick`** exceeds ~5 min cold without code changes |
| Heavy iteration smell | **`VOX_PREPUSH_AUDIT_LOG`** JSONL | More than **~2 full default pre-push runs per hour** per agent session — prefer **`cargo check -p`** between runs |

See [AI dev loop overhead (2026)](../../src/architecture/ai-dev-loop-overhead-2026.md).

## Soft budgets

**SSOT:** `budgets.json` is the only definition of soft caps — `vox ci build-timings` loads this file when either env var below is set (no duplicate literals in Rust).

- `VOX_BUILD_TIMINGS_BUDGET_WARN=1` — stderr when a lane is **missing** from `budgets.json` or exceeds its cap.
- `VOX_BUILD_TIMINGS_BUDGET_FAIL=1` — fail after timings if any lane **exceeded** its cap (does not require `BUDGET_WARN`).

Refresh caps after major dependency upgrades; keep lane ids in sync with `run_build_timings` in `crates/vox-cli/src/commands/ci/mod.rs` (see unit test `budgets_json_loads_and_defines_all_timing_lanes`).

## CUDA toolkit version bumps (Windows checklist)

When installing a **new** CUDA Toolkit version, update every pinned path so agents and integrated terminals still find `nvcc`:

| Location | What to change |
|----------|----------------|
| [`.vscode/settings.json`](../../../.vscode/settings.json) | `CUDA_PATH` and `PATH` prefixes under `terminal.integrated.env.windows` |
| [`scripts/windows/ensure_cuda_path.ps1`](../../../scripts/windows/ensure_cuda_path.ps1) | Default `-CudaRoot` parameter |
| User **Environment Variables** (optional) | `Path` entries and `CUDA_PATH` (re-run script or edit in System Properties) |
| [`docs/ci/build-timings/snapshot-metadata.json`](snapshot-metadata.json) | `cuda_toolchain.reported_release` when you refresh snapshots |

`vox ci build-timings` / `cuda-features` resolve `nvcc` via **`PATH`**, then **`CUDA_PATH`** / **`CUDA_HOME`** (`bin/nvcc` or `bin/nvcc.exe`). See [`.cursor/rules/build-environment.mdc`](../../../.cursor/rules/build-environment.mdc).

## Files

| File | Purpose |
|------|---------|
| `budgets.json` | Lane id → soft max `duration_ms` |
| `latest.jsonl` | Optional committed sample (regenerate locally; may be gitignored if noisy) |
| `snapshot-metadata.json` | Host / `rustc` / CUDA presence + cache methodology (pair with `latest.jsonl`) |
