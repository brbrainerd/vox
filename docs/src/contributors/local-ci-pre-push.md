---
title: "Local CI parity (pre-push)"
description: "Fast default `git push` hook via `vox ci pre-push`; full static gate with `--complete`; emergency `--no-verify` policy."
category: "Contributors"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
schema_type: "TechArticle"
---

# Local CI parity (pre-push)

`vox ci pre-push` is the **`git push` hook** target (`cargo run -q -p vox-cli -- ci install-hooks`).
It runs **before** the remote receives objects.

## Profiles

| Profile | Flags | What runs | Typical wall-clock |
| -------- | ----- | ----------- | ------------------- |
| **Fast** (default) | _(none)_, or **`--quick`** | `cargo fmt --check`, **`vox ci line-endings`**, **`vox ci ssot-drift`** (includes **`contracts-index`**, **`docs-reality-audit verify`**, registry parity, …), **scoped** **`vox-doc-pipeline --lint-only`** + **`vox ci doctest-md --strict`** on changed `docs/src/**/*.md` (excludes **`docs/src/archive/`**), **`vox-drift-check`**. No workspace clippy / doc-inventory / scoped TOESTUB. | Often **under ~1–3 min** (depends on diff size and cold Cargo). |
| **Complete** | **`--complete`** | Everything in **fast**, plus **full-tree** doc lint + doctest under **`docs/src/`**, **`vox ci doc-inventory verify`**, workspace **`cargo clippy … -D warnings`**, scoped TOESTUB on changed `crates/<pkg>`. Matches the historical pre-merge static gate (without integration tests). | **~2–8 min** typical. |
| **Full** | **`--full`** | **`--complete`** plus **`cargo nextest run --workspace --profile ci --no-fail-fast`**. | **~10–25+ min** typical. |

**Legacy:** **`--quick`** is an alias for the default **fast** profile (it conflicts with **`--complete`** / **`--full`**).

**Progress:** During slow subprocess steps, stderr prints a **heartbeat every ~3s** (`still running <step> (Xs elapsed)`) so a push never looks hung.

**Telemetry:** **`--report-json <path>`** emits per-step durations — **`contracts/reports/pre-push-report.v1.schema.json`** (`schema_version` **2** adds **`profile`**: `fast` \| `complete` \| `full`). Env **`VOX_PREPUSH_AUDIT_LOG`** appends one JSON line per successful run (not **`--dry-run`**).

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

- **Automated:** `cargo test -p vox-cli pre_push_dry_run` — asserts **`--dry-run`** step lists for fast / **`--complete`** / **`--full`**, report schema **v2**, and **`--act`** workflow flags.
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
