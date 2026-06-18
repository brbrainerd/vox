# Unified Build Broker & Cross-Agent Compilation Design

**Status:** design / approved-scope (no code yet, except L0 which is shipped)
**Date:** 2026-06-18
**Author:** brainstorm session (Claude Opus 4.8); revised after second critique pass

## Problem

Multiple coding agents and IDEs (Claude Code tabs, Antigravity, Gemini, plain
terminals, rust-analyzer) operate against the **same** repo checkout
(`C:\Users\Owner\vox`) and contend on its single `target/` directory. Cargo
takes an exclusive advisory lock on the target dir, so concurrent invocations
serialize, surfacing as:

```
cargo test -p vox-orchestrator interrupt_cost_is_single_sourced_from_types_ssot
    Blocking waiting for file lock on build directory
```

Secondary pain: `target/` directories balloon across worktrees and over time.

## What the codebase already solves (do NOT redo)

- **Cross-worktree contention is already solved.** `.cargo/config.toml` sets
  `CARGO_TARGET_DIR = { value = "target", relative = true }` so each git
  worktree gets its own `target/`. A global `CARGO_TARGET_DIR` would *reintroduce*
  contention and cause fingerprint thrash. **Rejected.**
- **sccache cross-worktree reuse is a measured dead end.**
  `docs/src/ci/shared-compile-cache.md` documents 0% cross-worktree hit rate
  (absolute path prefix in cache keys). sccache is **out of scope**.
- **Disk-bloat GC is already designed.**
  `docs/superpowers/specs/2026-06-05-target-artifact-gc-design.md` + working pure
  logic in `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs`
  (incremental-prune ≈65% of bloat). Layer 2 *lands* this; it does not redesign.
- **A build service already exists.** `crates/vox-cli-core/src/build_service.rs`
  has `CargoRequest` gated by `artifact_policy.rs`. The broker is built **on** it,
  and becomes the single cargo egress (no split-brain second path).

## Reframe

The unsolved problem is **within-one-worktree contention**: rust-analyzer
(continuous `cargo check`) plus N agent tabs all hitting one `target/` in the
main checkout. Two *different* commands against one target **must** serialize —
no tool can parallelize them. The genuinely addressable wins are therefore:
(1) remove RA from the contention set, (2) make the unavoidable serialization
*fair + visible* instead of opaque blocking, (3) give isolatable agents their own
worktree for true parallelism, (4) bound disk growth.

## Goals / non-goals

**Goals**
- Remove rust-analyzer from the lock-contention set.
- Make unavoidable shared-checkout serialization fair, visible, and measured.
- Enable true parallelism for agents that *can* be isolated (own worktree/target).
- Land the already-designed bloat GC, made aware of the RA target dir.
- Never become a hard dependency: if broker tooling is absent/broken, builds
  proceed via real cargo unchanged.

**Non-goals**
- No distributed/remote build farm (Populi/mesh track).
- No global single target dir. No sccache changes. No cross-machine cache.
- No long-lived daemon **in this scope** — see Layer 1 evidence-gate.

## Architecture — layers, cheapest first

### Layer 0 — rust-analyzer carve-out (SHIPPED)

RA fires `cargo check` on every save and is the most frequent lock holder.
`"rust-analyzer.cargo.targetDir": "target/rust-analyzer"` is set in: repo
`.vscode/settings.json` (covers any fork opening this repo) and the global user
settings of VS Code, Cursor, Antigravity, and Windsurf. RA now compiles into a
dedicated dir and never takes the `target/` lock.

**Known cost (reconciled with L2):** `target/rust-analyzer` is a *second* full
dependency tree (≈2× dep disk) and incurs a one-time cold check build. This is an
accepted trade (disk for lock-freedom). **L2 GC MUST treat `target/rust-analyzer`
as a first-class target tree and must NOT prune it while an IDE/RA process is
attached** (extend the active-build process scan to include rust-analyzer).

### Layer 1 — daemonless fair-queue cargo shim (+ evidence gate)

A small `cargo` shim that serializes shared-target builds *fairly and visibly*,
without a long-lived daemon. The coalescing daemon is deferred until measurement
proves it is needed.

**Distribution (scoped, NOT global PATH):**
- The shim lives at `.vox/bin/cargo-shim.exe` (built by `vox build-broker install`).
- It is put on PATH **only for IDE-spawned processes**, by prepending its dir to
  `terminal.integrated.env.windows.PATH` in the repo `.vscode/settings.json` —
  the same mechanism already used there for CUDA. This confines blast radius to
  vox IDE terminals/agents; the machine-wide `cargo` is untouched.
- `AGENTS.md` documents the equivalent PATH prepend for non-VS-Code hosts.

**Shim behavior:**
1. Resolve **real cargo** = first `cargo` on PATH whose canonical path ≠ the
   shim's own path (this is the rustup *proxy* at `~/.cargo/bin/cargo.exe`,
   preserving `rust-toolchain.toml` / `+toolchain`). Resolve once, cache in a
   sidecar file; never call `rustup which` (that bypasses the proxy).
2. If the subcommand is not in {`build`,`test`,`check`,`clippy`,`run`,`bench`}
   **or** cwd is not inside a registered vox worktree → `exec` real cargo
   immediately, argv untouched.
3. Otherwise: acquire a **fair FIFO queue lock** scoped to the worktree
   (canonicalized worktree root, found by walking up to the dir containing
   `.cargo/config.toml`). Lock impl = a lockfile under
   `.vox/build-queue/<worktree-hash>.lock` with an ordered wait list so waiters
   run in arrival order (cargo's native lock is not guaranteed fair). While
   waiting, print `vox-broker: queued (position N) behind <pid/cmd>` to stderr.
4. Run real cargo as a child with the caller's **full environment passed through**
   minus a volatile denylist (`PROMPT`, `TERM`, session/TTY ids); set
   `CARGO_TERM_COLOR=always` so piped output keeps color. On Windows spawn with
   `CREATE_NO_WINDOW`. Stream stdout/stderr through unmodified; propagate the
   exact exit code.
5. On shim termination (Ctrl-C / disconnect), forward the signal to the child
   and release the queue lock (no orphaned builds).
6. Any error in queue/lock logic → fall back to direct `exec` of real cargo. The
   shim can never block a build.

**Measurement (built in from day one — this is the evidence gate):**
- The shim appends one JSON line per invocation to
  `.vox/build-queue/metrics.jsonl`: `{ts, worktree, subcmd, queue_wait_ms,
  ran_ms, argv_hash, env_hash, would_coalesce}`.
- `would_coalesce` = true iff, at enqueue time, another *in-flight or queued*
  invocation had an identical `(argv_hash, env_hash, cwd)`. This counts the
  coalescing opportunity **without** building the daemon.
- `vox build-broker stats` summarizes: p50/p95 `queue_wait_ms`, invocation
  count, and coalesce-opportunity rate.

**Evidence gate for the daemon (Layer 1b, deferred):** build the long-lived
coalescing daemon ONLY if, after a representative usage window, the metrics show
a material coalesce-opportunity rate (target threshold: ≥10% of invocations had
`would_coalesce=true`) AND non-trivial `queue_wait_ms`. Otherwise the daemonless
shim is the final form and 1b is cancelled. If built, 1b reuses the existing
`daemon_ipc` stdio-JSON framing, is keyed per-worktree, staged to `~/.vox/bin`
(never locks `target/debug`), single-instance via the same lockfile, and carries
a version handshake against the working-tree commit (refuse + fall back on
mismatch, log a one-line restage hint).

**Single egress:** `build_service.rs::CargoRequest` is updated to invoke cargo
via the shim resolution path when running inside a vox worktree, so `vox ci` and
programmatic builds share the same queue rather than forming a second path.

### Layer 1c — per-agent worktree isolation (true parallelism)

For agents that are NOT pinned to the main checkout, real parallelism comes from
isolation, which the repo already supports (`vox-targets/<hash>/` allowed lane in
`artifact_policy.rs`, plus existing worktree tooling). Scope here is a thin
convenience: `vox build-broker worktree <name>` creates/【reuses】a worktree with
its own `target/`, so two agents in two worktrees build in genuine parallel with
zero lock contention. Agents pinned to the main checkout (Antigravity/Gemini)
fall back to the L1 fair queue. Disk cost of N worktrees is bounded by L2.

### Layer 2 — land the existing bloat GC (RA-aware)

Implement `docs/superpowers/specs/2026-06-05-target-artifact-gc-design.md`:
`worktree-target` + `stale-worktree` classes (7-day mtime-walk), `--incremental-only`
prune (`target/*/incremental/`, keep `deps/`), existing safety gates (current
worktree, git-locked, active-build process scan, uncommitted source). **Addition
required by L0:** the active-build scan must include `rust-analyzer` and the GC
must classify `target/rust-analyzer` correctly (treat as live target while RA is
attached; eligible for incremental-prune only when no RA/build process is active).

## Data flow

```
Antigravity / Gemini / Claude tab / terminal  (IDE-scoped PATH)
        │ cargo build|test|check|clippy …
        ▼
   cargo shim ──(non-build subcmd, OR outside worktree, OR error)──► real cargo (rustup proxy), exec
        │ build subcmd, inside worktree
        ▼
   fair FIFO queue lock (.vox/build-queue/<wt>.lock)  ── prints queue position
        │ acquired
        ▼
   real cargo child (full env passthrough, CARGO_TERM_COLOR=always, CREATE_NO_WINDOW)
        │ + append metrics.jsonl  (queue_wait_ms, would_coalesce, …)
        ▼  release lock on exit / signal

rust-analyzer ──► target/rust-analyzer  (L0, never contends)
isolatable agents ──► own worktree/target  (L1c, true parallel)
```

## Testing strategy

- **Shim passthrough:** non-build subcommands, and cwd outside any worktree, and
  the error-fallback path, all `exec` real cargo (resolved as first non-shim
  cargo on PATH) with argv untouched — golden tests via a fake-cargo stub.
- **Real-cargo resolution:** with the shim dir prepended to PATH, resolution
  finds the rustup proxy, not the shim (no self-recursion); cached on second call.
- **Fair queue:** 3 concurrent *different* invocations acquire the lock in
  arrival order; each prints its queue position; total = serial sum (cannot
  parallelize same target — asserted, documents the inherent limit).
- **Env fidelity:** child observes the caller's full env minus the denylist;
  `argv_hash`/`env_hash` change when `RUSTFLAGS` changes (guards fingerprint
  thrash); `would_coalesce` true only for byte-identical `(argv,env,cwd)`.
- **Cancellation:** SIGINT/Ctrl-C to the shim kills the child and releases the
  lock; no orphaned cargo (process scan after).
- **Metrics:** N invocations append N well-formed JSONL lines; `vox build-broker
  stats` reports correct p50 and coalesce-opportunity rate.
- **Single egress:** `build_service::CargoRequest` inside a worktree routes
  through the queue (observed via the lockfile/metrics).
- **L1c:** `vox build-broker worktree foo` yields an isolated `target/` and two
  worktree builds run concurrently without queue contention.
- **L2 RA-awareness:** GC refuses to prune `target/rust-analyzer` while a
  rust-analyzer process is detected; incremental-prune proceeds when idle.

## Open risks

- Tools invoking cargo by absolute path bypass the shim. Mitigation: RA (the main
  one) is carved out in L0; remaining bypassers hit cargo's native lock — correct,
  just without the fair-queue UX. Documented, not silently ignored.
- IDE-scoped PATH covers IDE-spawned terminals/agents; a cargo run from a plain
  OS shell outside any IDE won't use the shim (acceptable — it still uses cargo's
  native lock; no regression).
- Coalescing opportunity may prove negligible → the daemon (1b) is intentionally
  never built. This is a success, not a gap: the metric decides.

## Success criteria

- L0: a manual `cargo check` against `target/` does not block while RA is
  checking (separate locks) — observable, no blocking message.
- L1: zero machine-wide cargo regressions; queued builds show a position message
  instead of opaque blocking; `metrics.jsonl` populated.
- Decision output: after a usage window, `vox build-broker stats` yields a
  go/no-go number for the 1b daemon.
- L2: `target/` + `target/rust-analyzer` bloat bounded by GC without ever pruning
  a live RA target.
