---
title: "Local CI parity (pre-push)"
description: "Fast default `git push` hook via `vox ci pre-push`; full static gate with `--complete`; emergency `--no-verify` policy."
category: "Contributors"
status: "current"
last_updated: "2026-05-27"
training_eligible: true
schema_type: "TechArticle"
---

# Local CI parity (pre-push)

`vox ci pre-push` is the **`git push` hook** target (`cargo run -q -p vox-cli -- ci install-hooks`).
It runs **before** the remote receives objects.

> **Canonical tier table:** `docs/superpowers/specs/2026-05-27-test-suite-perf-and-gate-tiers-design.md §4`
> **Budget thresholds:** `contracts/budgets/test-tier-budgets.v1.yaml`

## Profiles

| Profile | Flags | What runs | Target wall-clock (post Phase 2+3) |
| -------- | ----- | ----------- | ------------------- |
| **Fast** (default) | _(none)_, or **`--quick`** | `cargo fmt --check`, **`vox ci line-endings`**, **`vox ci ssot-drift`** (includes **`contracts-index`**, **`docs-reality-audit verify`**, registry parity, …), **scoped** **`vox-doc-pipeline --lint-only`** + **`vox ci doctest-md --strict`** on changed `docs/src/**/*.md` (excludes **`docs/src/archive/`**), **`vox-drift-check`**. No workspace clippy / doc-inventory / scoped TOESTUB. | **≤60s** (arch-check cached). |
| **Complete** | **`--complete`** | Everything in **fast**, plus **full-tree** doc lint + doctest under **`docs/src/`**, **`vox ci doc-inventory verify`**, workspace **`cargo clippy … -D warnings`**, scoped TOESTUB on changed `crates/<pkg>`. Matches the historical pre-merge static gate (without integration tests). | **≤180s** typical. |
| **Full** | **`--full`** | **`--complete`** plus **`cargo nextest run --workspace --profile ci --no-fail-fast`** (slow `#[ignore]` tests excluded). | **≤120s** (slow excluded). |
| **Full+cov** | **`--full --with-coverage`** | **`--full`** but uses **`cargo llvm-cov nextest`** + emits lcov/HTML report under `target/llvm-cov/`. | **≤260s**. |
| **Full+since** | **`--full --since <ref>`** | **`--full`** nextest step runs only for packages changed since `<ref>` + their reverse-deps. Falls back to workspace when > 20 packages impacted. | **3–20s** for 1–3 crate edits. |
| **Full+cov+since** | **`--full --with-coverage --since <ref>`** | Combination of the above two. | **3–30s** typical. |

**Legacy:** **`--quick`** is an alias for the default **fast** profile (it conflicts with **`--complete`** / **`--full`**).

**Progress:** During slow subprocess steps, stderr prints a **heartbeat every ~3s** (`still running <step> (Xs elapsed)`) so a push never looks hung.

**Telemetry:** **`--report-json <path>`** emits per-step durations — **`contracts/reports/pre-push-report.v1.schema.json`** (`schema_version` **3** adds `with_coverage` and extended profile values; v2 added `profile`). Env **`VOX_PREPUSH_AUDIT_LOG`** appends one JSON line per successful run (not **`--dry-run`**).

### Extended `--full` flags

| Flag | Effect |
| ---- | ------ |
| **`--include-slow`** | Also runs the slow `#[ignore]` partition (arch-check smoke, scientia timeout, codegen bundle check). Adds ~3–5 min. CI always sets this. |
| **`--with-coverage`** | Substitutes `cargo llvm-cov nextest` for plain nextest and appends `cargo llvm-cov report`. Requires `cargo-llvm-cov` on PATH. |
| **`--since <ref>`** | Narrows nextest to packages touched since `<ref>` (default: `origin/main`). Falls back to `--workspace` if > `VOX_PREPUSH_SINCE_FALLBACK_THRESHOLD` (default 20) packages impacted. |
| **`--enforce-budgets`** | After a successful real run (not `--dry-run`), compares total elapsed against `contracts/budgets/test-tier-budgets.v1.yaml`. Warns at `warn_ms` (1.2× baseline); fails at `fail_ms` (1.5× baseline). No-op if budgets file is absent. |

**Diagnostics:** **`vox ci dev-loop-audit`** surfaces **`CARGO_TARGET_DIR`** fragmentation that causes redundant compiles across terminals ([runner-contract §Cargo incremental cache](../ci/runner-contract.md#cargo-incremental-cache-troubleshooting-ai-multi-terminal)).

### CI vs local

- **Fast** pre-push **does not** scan all archived research Markdown locally; **GitHub `docs-quality` / merge gates still enforce full-doc behavior**.
- Before merging doc-heavy or registry-risky changes, run **`vox ci pre-push --complete`** (or rely on CI).

## Not in fast pre-push (run before risky edits)

The GitHub merge gate still runs additional steps that **fast** `vox ci pre-push` skips locally. Before changing **`contracts/operations/catalog.v1.yaml`**, command registry rows, or `crates/vox-cli/src/lib.rs` dispatch, also run locally:

- **`cargo run -p vox-cli -- ci command-compliance`**
- **`cargo run -p vox-cli -- ci operations-verify`** when the operations catalog or MCP/capability projections change
- **`cargo run -p vox-cli -- ci command-sync`** (verify generated CLI reference docs)
- **`cargo run -p vox-cli -- ci dep-sprawl`** / **`cargo run -p vox-arch-check`** when dependency graphs move

Use **`vox ci ssot-drift`** for an aggregate check if you want one heavy command instead of piecing the above together.

## Install the git hook (one-time)

```bash
cargo run -q -p vox-cli -- ci install-hooks
```

This writes `.git/hooks/pre-push` as a thin delegate to **`vox ci pre-push`** (fast profile by default). See [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md).

## Bypass (emergency only)

**`git push --no-verify`** skips the hook. Use **only** for emergencies or when fixing the hook itself — **CI still runs**. After pushing with **`--no-verify`**, run **`vox ci pre-push --complete`** (or **`--full`**) locally as soon as possible and fix any failures before the next merge.

## Tuning the diff base

Scoped doc/doctest steps use **`git diff --name-only $BASE...HEAD`**. Default **`BASE`** is **`origin/main`**. Override with **`VOX_PREPUSH_BASE=<ref>`** (e.g. **`VOX_PREPUSH_BASE=HEAD~1`**).

Scoped TOESTUB ( **`--complete`** / **`--full`** ) uses the same base.

## `--act` mode (GH-hosted exception workflows)

When **`--act`** is set, `vox ci pre-push` additionally runs workflows that target **`ubuntu-latest`** inside Docker via [nektos/act](https://github.com/nektos/act). Composable with any profile (**`--complete --act`**, etc.).

**Workflows covered:** `docs-quality.yml`, `link_checker.yml`, `ts-emit-noemit.yml`.

**Configuration:** [`.actrc`](../../../.actrc) at the repo root.

## Verification (smoke)

- **Automated:** `cargo test -p vox-cli pre_push_dry_run` — asserts **`--dry-run`** step lists for fast / **`--complete`** / **`--full`**, report schema **v3** (`with_coverage` + extended profile enum), **`--act`** workflow flags, and **`--enforce-budgets`** flag acceptance.
- **Inspect planned steps:** `vox ci pre-push --dry-run` prints the exact subprocess sequence without executing them.
- **`git push --dry-run`** still runs the real **`pre-push`** hook (unless **`--no-verify`**); use **`vox ci pre-push --dry-run`** to preview work without hook side effects.

### Installing `act`

`act` must be on **`PATH`**, or available as **`gh act`**. Docker must be running.

#### Windows

| Method | Command | Notes |
| ------ | ------- | ----- |
| **WinGet** | `winget install nektos.act` | |
| **Scoop** | `scoop install act` | |
| **Chocolatey** | `choco install act-cli` | Administrator |
| **GitHub CLI extension** | `gh extension install nektos/gh-act` | Invoke as **`gh act`** |

Verify:

```powershell
act --version
docker version
```
