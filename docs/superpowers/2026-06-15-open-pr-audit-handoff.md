# Open-PR Adversarial Audit + Runner-Clear Handoff — 2026-06-15

> **SUPERSEDED (2026-06-16):** All four audited PRs (**#321**, **#331**, **#333**, **#334**) and the full **#321–#352** wave are **merged on `main`**. This document is retained as the adversarial audit record only — do **not** treat the "Request Changes" verdicts or "open PR" guidance below as current. For active work, start at [`plans/2026-06-15-config-registry-HANDOFF-STATE.md`](plans/2026-06-15-config-registry-HANDOFF-STATE.md) and [`docs/src/ci/affected-crate-selective-ci.md`](../src/ci/affected-crate-selective-ci.md).

Cross-PR adversarial review of every open pull request, plus the state of the self-hosted CI runner (cleared this session). Authored to replace the CodeRabbit pass (rate-limited, never ran) and to be picked up cold.

## TL;DR (historical — 2026-06-15)

- **Runner fleet: fully cleared.** 0 containers, autoscaler **disabled**, queue drained 125 → ~7 (the rest are runner-less stragglers). Docker engine was wedged (root cause of the pileup); recovered via WSL restart. **Restore instructions below — the autoscaler is OFF until re-enabled.**
- **4 open PRs, all verdict = Request Changes. None is merge-ready.** Two ship real defects: **#321 emits non-compiling Rust** (reproduced) and **#334 can force-kill a developer's live `vox` process**.
- **#331 ⇄ #334 will textually conflict** (both add `vox ci` subcommands to the same two files). **#331/#333/#334 all touch the generated `gui-surface-coverage.v1.json`** (regen-mergeable, not a hand-edit conflict).

## Post-merge status (2026-06-16)

All audited PRs landed on `main` (wave **#321–#352** complete). Adversarial findings disposition:

| PR | Finding (summary) | Status on `main` |
|---|---|---|
| **#321** | Generated `@traced` Rust missing `vox-telemetry` dep (E0433) | **Fixed** — dep emitted in codegen; `traced_fn_compiles` + `traced_fn_span_emitted` tests un-ignored |
| **#321** | Compile test `#[ignore]`-tagged → CI false-green | **Fixed** — `traced_fn_compiles` runs in CI |
| **#321** | Interp tests assert flag only, not span emission | **Partial** — structural span assertion in codegen test; runtime span capture still untested |
| **#321** | Docs claim fully supported codegen path | **Fixed** post-merge |
| **#333** | `config-registry-parity` leaked into GUI coverage without SSOT registration | **Fixed** — registered across command/catalog surfaces (#330, #343) |
| **#331** | Visual-review cache commit step dead after `push:main` removal | **Fixed** — step gated on `merge_group \|\| push` to `main` |
| **#331** | `install-runner-schedule.vox` used `std.env.get` | **Fixed** — uses bare `env.get` |
| **#331** | `ScaleLock` check-then-create not atomic | **Partial** — stale-steal + Task XML `IgnoreNew` remain the real serializer; no `O_EXCL` yet |
| **#331** | `append_history` on dry-run (non-atomic RMW) | **Partial** — intentional: dry-run entries include `"dry_run":true`; file still mutated on dry-run |
| **#334** | No busy/in-use guard → could kill live `vox serve`/`run` | **Fixed** — subcommand guard excludes `serve`, `mcp`, `daemon`, `run` |
| **#334** | Reaper fires on any build failure, not just os error 5 | **Partial** — pre-push hook still reap+retry on any `cargo build` failure (comment says "possible lock") |
| **#334** | pid-recycle TOCTOU between scan and kill | **Fixed** — `kill_pid` re-verifies exe path before signal |
| **#334** | `target_dir` not canonicalized → false-green on junction paths | **Partial** — lowercase path-prefix compare (no `canonicalize`) |
| **#334** | `vox-*` prefix overreach | **Partial** — still matches managed `vox-*` siblings; excludes `*-build` |
| **#334** | Staging copy still hits os error 5 when locked | **Partial** — sidecar-copy reaper pattern; no temp+rename on staging path |

**Also merged (same wave, not in original audit):** #322 LLM egress SSOT, #348 affected-crate selective CI (+ wiring `229febdf`), #340/#345 config GUI codegen (2C.2/2C.3), #350 build-time program, #351 env-var SSOT phases 2–6, #352 orchestrator scope_enforcement fix, and others — see `gh pr list --state merged` for the full #321–#352 table.

---

## Runner clear — what was done & how to restore

| Action | State |
|---|---|
| `VoxCIRunnerScale` scheduled task | **Disabled** (`Disable-ScheduledTask`) — will NOT respawn runners |
| Queued + in-progress GH Actions runs | Cancelled in 3 passes: **125 → ~7** (remaining have no runner; harmless) |
| Docker Linux engine | Was returning HTTP 500 on every call (wedged) → `wsl --shutdown` → **recovered (v29.5.3)** |
| `vox-runner-auto-*` containers | All exited on WSL restart (no restart policy) → **0 remaining** |

**To restore the fleet when ready:** re-enable the autoscaler — `Enable-ScheduledTask -TaskName VoxCIRunnerScale` (PowerShell) — or run one reconcile manually: `cargo run -q -p vox-cli -- ci runner-scale --apply`. The wedged-Docker pileup is exactly what PR #331 (fleet health) addresses; the engine-wedge itself is an environmental Docker Desktop fault, not a fleet bug.

> **Note:** the ~7 residual queued runs belong to the open PRs and will sit until a runner returns or they're re-cancelled. With the fleet down they consume nothing.

---

## United PR audit

### PR #321 — `feat(@traced): full TRACE-D` — 🔴 **Request Changes (DO NOT MERGE)**
Branch `claude/nifty-rubin-9b3fa1` · 27 files · compiler/codegen/telemetry.

The interpreter half of `@traced` is solid and leak-free; the **codegen half emits Rust that does not compile** — the classic half-wired-pipeline false-green this repo is prone to.

| # | file:line | issue | sev |
|---|---|---|---|
| 1 | `vox-codegen/src/codegen_rust/pipeline.rs:198-203` & `:228-232` | Generated `Cargo.toml` (Native + Wasi) declares `tracing` but **not `vox-telemetry`**, yet the emitted `@traced` body calls `vox_telemetry::current_trace_context()`. `vox-actor-runtime` doesn't re-export it → **E0433, generated code won't compile. Reproduced** via `cargo test -p vox-codegen --test emit_compile_harness traced_fn_compiles -- --ignored`. | 🔴 |
| 2 | `vox-codegen/tests/emit_compile_harness.rs:425-429` | The only test that catches #1 is `#[ignore]`-tagged + unchecked in the PR body → **CI stays green on broken codegen**. | 🔴 |
| 3 | `vox-compiler/tests/traced_decorator.rs:18-50` | Interp tests assert only the flag + return value; **no test asserts a span is emitted**. Delete the span block and tests still pass. | 🟡 |
| 4 | `docs/src/reference/ref-decorators.md` | Documents the non-compiling codegen path as "fully supported." | 🟡 |

**Fixes:** add `vox-telemetry` to both generated `Cargo.toml` templates; un-`#[ignore]` the compile test; add a span-emission behavioral assertion; reconcile eval span name (`vox_fn`) vs codegen per-fn span name (the "equivalent tracing" claim is currently false).

### PR #333 — `feat: config guardrails` — 🟡 **Request Changes**
Branch `claude/config-guardrails-remediation` · 29 files · contracts + safe-by-default Rust.

Safe-by-default Rust (circuit_breaker, cost_defense, economy, ingest) is **correct, falls back to safe defaults, and is well-tested.** One blocking SSOT-drift defect:

| # | issue | sev |
|---|---|---|
| 1 | `ci config-registry-parity` is a fully-wired CLI subcommand that **leaked into generated `gui-surface-coverage.v1.json` but was never registered** in `command-registry.yaml` / `catalog.v1.yaml` / `capability-registry.yaml` / `command_registry_handler_needles.rs` → SSOT-drift gate will flag it. | 🟡 |

**Fix:** register `config-registry-parity` across the four SSOT surfaces (or regenerate them) so the generated coverage matches. The `gui-surface-coverage.v1.json` overlap with #331/#334 is a **regen conflict** (re-run the generator post-merge), not a hand-edit.

### PR #331 — `feat(ci-runner): fleet health` — 🟡 **Request Changes** *(this session's work)*
Branch `claude/ci-runner-fleet-health` · 13 files. Phantom reaper, lock, decision log, `runner-status`, concurrency-group fixes, codified schedule.

Core autoscaler logic is correct with genuinely adversarial unit tests. Two items to fix + doc overclaims:

| # | file:line | issue | sev |
|---|---|---|---|
| 1 | `ci.yml` trigger removal → main `ci.yml:~1203-1215` | Dropping `push: branches:[main]` makes the **"Commit visual-review cache + report (main only)"** step (`if: event_name=='push' && ref==main`) **permanently dead** — `merge_group` ≠ `push`. AI-visual-review sha256 cache never re-commits → stale-forever → every merge re-reviews all surfaces via OpenRouter (recurring $). PR comment's "post-merge jobs still fire via merge_group" is false for this `event_name=='push'`-gated step. | 🔴 (my regression) |
| 2 | `scripts/ci/install-runner-schedule.vox:18` | `env_flag` uses `std.env.get(...)`; the repo idiom is bare `env.get(...)`. `std.env` is unverified → likely runtime failure. | 🟡 |
| 3 | `runner_scale.rs:564-590` (`ScaleLock`) | Not atomic (check-then-`File::create`); stale-steal has no ownership token so the prior holder's `Drop` can delete the thief's lock. **Real serializer is the Task XML `IgnoreNew`** — fix the doc comment's "single-instance" overclaim, or use `create_new` (O_EXCL) + PID nonce. | 🟡 |
| 4 | `runner_scale.rs:~809` (`append_history`) | Called on **both** apply and dry-run; non-atomic read-modify-write of `ci-runner-history.jsonl` → concurrent dry-run + apply can corrupt the log. Contradicts the "dry-run never mutates" lock-skip justification. | 🟡 |

**Fixes:** restore a narrowly-scoped trigger for the visual-review cache-commit step (or move it to `event_name != 'pull_request'`); `std.env.get` → `env.get`; soften the lock doc comment or make acquire atomic; gate `append_history` on `!dry_run` (or temp-file+rename).

### PR #334 — `fix(ci): binary-lock resilience` — 🔴 **Request Changes (dangerous as-is)**
Branch `claude/binary-lock-resilience` · 7 files. Free-binary reaper for the Windows `os error 5` self-lock.

Sound core idea (path-scope the reaper to the worktree's own `target/`), good dry-run default + pure tested `should_reap`. But the **kill-selection — the highest-risk surface — is unguarded:**

| # | file:line | issue | sev |
|---|---|---|---|
| 1 | `free_binary.rs:17-37` (`should_reap`) | **No busy/in-use guard** — only `pid == self` is excluded. Reaps any `vox*` under `target/`, including the user's **actively-running** `vox serve`/`vox run` (which lives at `target/debug/vox.exe` in dev). A push firing the hook force-kills it mid-operation. | 🔴 |
| 2 | `install_hooks.rs` (reap step) | Reaper fires automatically with `--apply` on **any** build failure (not just os-error-5). A plain compile error → reap sweep → retry → masks the real error. | 🔴 |
| 3 | `free_binary.rs:40-67` | **pid-recycle TOCTOU**: `scan_locking_pids` then a *second* `System::new_all()` in `kill_pid` re-finds by pid → Windows pid reuse can kill an innocent process. Also O(n²). | 🔴 |
| 4 | `free_binary.rs:33` | `target_dir` not canonicalized vs OS canonical `proc.exe()` → silent **no-op (false-green)** on 8.3/junction/subst paths, or over-match. | 🟡 |
| 5 | `free_binary.rs:37` | `vox-` prefix overreach reaps unrelated `vox-*` binaries (tests, vox-db, vox-gui). | 🟡 |
| 6 | `process_supervision.rs:280-294` | Phase-2 staging copy `target/debug/...-d → ~/.vox/bin` **still hits os-error-5** when the staged copy is locked (no temp+rename). The lock is moved, not eliminated. | 🟡 |

**Fixes (before any merge):** add a busy-guard (env marker `VOX_REAPABLE=1` on detached spawns only, or socket-liveness/parent-pid check); fire the reaper only on a detected `os error 5`/`Access is denied` in cargo stderr (or behind `VOX_PREPUSH_REAP=1`); pass the scan snapshot into the killer to close the pid-recycle window; canonicalize the target dir; narrow the match to the exact relinked binary; temp-file+`MoveFileEx` for the staging copy.

---

## Cross-PR interaction matrix

| Shared artifact | PRs | Nature | Resolution |
|---|---|---|---|
| `crates/vox-cli/src/commands/ci/cmd_enums.rs` | #331 (`RunnerStatus`), #334 (`FreeBinary`) | Both add a `CiCmd` variant (+ #334 a gate-id arm) | Keep both; rebuild for match exhaustiveness |
| `crates/vox-cli/src/commands/ci/run_body.rs` | #331, #334 | Both add a dispatch arm to the same `match` | Mechanical — keep both arms |
| `contracts/reports/gui-surface-coverage.v1.json` | #331, #333, #334 | All append to the same sorted string array | **Regenerate** post-merge; do not hand-merge |
| compiler/codegen/telemetry | #321 only | Disjoint from the rest | Independent |

**Recommended order (once each is fixed):** **#333** (mostly disjoint, one SSOT fix) → **#331** (fix the 2 items) → **#334 rebased on #331** (resolve the cmd_enums/run_body conflicts; #334's additions are small and mechanical) → **#321** independent (anytime after the codegen-dep fix). Re-run the GUI-surface-coverage parity gate after each merge so all new `ci <subcommand>` rows land sorted.

---

## What remains (updated 2026-06-16)

- **Build-time program (#350) and selective CI (#348 + `229febdf`)** — landed; see [`docs/src/ci/affected-crate-selective-ci.md`](../src/ci/affected-crate-selective-ci.md).
- **Config registry Phase 2** — ~80% done; 2C.2/2C.3 merged (#340, #345); burndown + 2D/2E remain — see [`plans/2026-06-15-config-registry-HANDOFF-STATE.md`](plans/2026-06-15-config-registry-HANDOFF-STATE.md).
- **Partial adversarial items** (ScaleLock atomicity, dry-run history mutation, pre-push reap-on-any-failure, staging-copy lock) — acceptable debt unless a regression is observed; track in follow-up issues if needed.
- **Runner fleet:** re-enable autoscaler when ready (`Enable-ScheduledTask -TaskName VoxCIRunnerScale` or `vox ci runner-scale --apply`).

---

*Reviews performed by four parallel adversarial agents (one per PR). Findings with `file:line` were cited from the actual diffs; #321 finding #1 and #334's safety holes were reproduced/traced, not inferred.*
