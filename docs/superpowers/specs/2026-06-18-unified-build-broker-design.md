# Unified Build Broker & Cross-Agent Compilation Design

**Status:** design / approved-scope (no code yet)
**Date:** 2026-06-18
**Author:** brainstorm session (Claude Opus 4.8)

## Problem

Multiple coding agents and IDEs (Claude Code tabs, Antigravity, Gemini, plain
terminals, rust-analyzer) operate against the **same** repo checkout
(`C:\Users\Owner\vox`) and contend on its single `target/` directory. Cargo
takes an exclusive advisory lock on the target dir, so concurrent invocations
serialize behind it, surfacing as:

```
cargo test -p vox-orchestrator interrupt_cost_is_single_sourced_from_types_ssot
    Blocking waiting for file lock on build directory
```

Secondary pain: `target/` directories balloon across worktrees and over time.

## What the codebase already solves (do NOT redo)

- **Cross-worktree contention is already solved.** `.cargo/config.toml` sets
  `CARGO_TARGET_DIR = { value = "target", relative = true }` so each git
  worktree gets its own `target/`. Two worktrees never share a lock. A global
  `CARGO_TARGET_DIR` would *reintroduce* contention and cause fingerprint
  thrash (different checkouts → constant rebuilds). **Rejected.**
- **sccache cross-worktree reuse is a measured dead end.**
  `docs/src/ci/shared-compile-cache.md` documents 0% cross-worktree hit rate
  (absolute path prefix in cache keys, even with `--remap-path-prefix`). sccache
  is **out of scope** for this design.
- **Disk-bloat GC is already designed.**
  `docs/superpowers/specs/2026-06-05-target-artifact-gc-design.md` + working
  pure logic in `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs`,
  including incremental-prune (≈65% of bloat). Layer 2 *lands* this; it does not
  redesign it.
- **A build service already exists.** `crates/vox-cli-core/src/build_service.rs`
  has `CargoRequest` (per-invocation target override) gated by
  `artifact_policy.rs`. The broker is built **on** this.

## Reframe

The remaining unsolved problem is **within-one-worktree contention**: rust-analyzer
(continuous `cargo check`) plus N agent tabs all hitting one `target/` in the
main checkout. That is the lock the user actually hits. The design targets that.

## Goals / non-goals

**Goals**
- Eliminate within-worktree lock-blocking for the common cases.
- Agent-agnostic reach (Antigravity/Gemini/terminals participate without each
  being individually wired), achieved at the `cargo`-invocation layer.
- Never become a hard dependency: if the broker is down, builds proceed
  directly.
- Land the already-designed bloat GC.

**Non-goals**
- No distributed/remote build farm (that is the Populi/mesh track).
- No global single target dir. No sccache changes. No cross-machine cache.
- Not replacing `vox ci` gates — the broker sits underneath them.

## Architecture — three layers, cheapest first

### Layer 0 — rust-analyzer carve-out (config only)

rust-analyzer fires `cargo check --message-format=json` on every save and is the
most frequent lock holder. Give it its **own** target dir so it never contends
with manual/agent builds.

- `.vscode/settings.json`: add
  `"rust-analyzer.cargo.targetDir": "target/rust-analyzer"`
  (a string enables RA's dedicated target dir; relative to workspace root).
- Document the equivalent setting for Antigravity / other rust-analyzer hosts in
  `AGENTS.md` (or a referenced doc) so non-VS-Code IDEs get the same carve-out.

Cost: one config line. Expected to remove the majority of day-to-day blocking on
its own. Independent of Layers 1–2.

### Layer 1 — per-worktree coalescing build broker

A long-lived **per-worktree** daemon that serializes and coalesces cargo
invocations against that worktree's single `target/`.

**Components (each independently testable):**

1. **`cargo` shim** — small exe placed earlier on `PATH` than rustup's cargo.
   - For build/test/check/clippy: connects to the per-worktree daemon, forwards
     `{cwd, argv, env_allowlist}`, streams stdout/stderr/exit back verbatim.
   - For every other subcommand (`fmt`, `add`, `+toolchain …`, etc.) **or** any
     failure to reach the daemon: delegates to the real cargo resolved via
     `rustup which cargo` (preserving toolchain selection and overrides). The
     shim must NEVER shadow rustup behavior on the fallback path.
   - Recursion guard: resolve the real cargo by absolute path; never re-invoke
     itself.

2. **Broker daemon** — new service under
   `crates/vox-cli-core/src/daemon_ipc/`, started by a new
   `vox build-broker serve`, auto-spawned by the shim on first connection miss.
   - **Keyed by worktree path** (canonicalized). One daemon per worktree; each
     owns only its own `target/`.
   - Staged to `~/.vox/bin` before running (per existing
     `process_supervision.rs` pattern) so it never locks `target/debug/*.exe`.
   - State file under `.vox/process-supervision/` (PID, binary path, start time)
     consistent with existing supervision.
   - Maintains a FIFO queue, an in-flight job map, and per-job subscriber lists.
   - Runs real cargo as a child via `build_service.rs::CargoRequest`, with
     `CREATE_NO_WINDOW` on Windows (per the no-flashing-console rule).
   - Tees child output to all subscribers; on child exit, sends framed
     `{exit}` to each.

3. **Exact-match coalescing.**
   - Dedup key = byte-identical `(cwd, argv, env_allowlist)`.
   - A request whose key matches an **in-flight** job attaches to that job's
     subscriber list instead of enqueuing a new cargo run. (No source-hashing —
     too fragile/env-sensitive; exact-match-while-in-flight is correct and
     covers the real case: two tabs running the same `cargo test` seconds apart.)
   - Requests that do not match an in-flight job enqueue normally and run
     one-at-a-time against the target (which is what the cargo lock would do
     anyway, but now visibly queued rather than opaquely blocked).

**IPC contract** lives in `daemon_ipc` as a versioned struct, reusing the
existing newline-delimited JSON `DispatchRequest`/`DispatchResponse` framing
(`Chunk`/`Done`/`Error` payload variants already exist). A new `method` value
(e.g. `"build.cargo"`) carries the shim request; responses stream `Chunk`
(stdout/stderr) then `Done {exit_code}`.

**Error handling**
- Daemon unreachable / IPC error → shim falls back to direct cargo. Hard
  guarantee: the broker can never block a build.
- Daemon crash mid-job → subscribers receive partial output + non-zero exit and
  retry directly.
- Version mismatch (shim vs daemon) → shim refuses the socket, falls back, logs
  a one-line "restart broker" hint.

### Layer 2 — land the existing bloat GC

Implement the already-approved
`docs/superpowers/specs/2026-06-05-target-artifact-gc-design.md`:
- `worktree-target` and `stale-worktree` classes (7-day mtime-walk staleness).
- `--incremental-only` prune (clean `target/*/incremental/`, keep `deps/`).
- Safety gates already specified: current worktree, git-locked trees,
  active-build process scan, uncommitted source.
This is reuse of an existing spec; this design only sequences it after L0/L1.

## Data flow

```
Antigravity / Gemini / Claude tab / terminal
        │ cargo build|test|check|clippy …
        ▼
   cargo shim ──(non-build OR daemon down)──► `rustup which cargo` (direct)
        │ build subcommand, daemon up
        ▼
   per-worktree broker daemon  ──► owns THIS worktree's target/
        ├─ exact-match in-flight? → attach to job, fan out streamed output
        └─ else enqueue → run via build_service::CargoRequest (one at a time)

rust-analyzer ──► target/rust-analyzer  (carved out, never contends)
```

## Testing strategy

- **Shim:** non-build subcommands and the daemon-down path both exec the real
  cargo (resolved via `rustup which cargo`) with argv untouched (golden test).
- **Daemon coalescing:** N concurrent byte-identical requests → exactly 1 cargo
  child ran (verified via a fake-cargo stub on PATH) and all N receive identical
  streamed output + exit code.
- **Distinct keys:** differing argv/cwd/env → distinct jobs, run serially.
- **Fallback:** kill the daemon mid-job → the shim still completes the build
  directly.
- **Binary-lock safety:** daemon runs from `~/.vox/bin`; a rebuild of the broker
  itself does not hit the locked-`vox.exe` failure mode.
- **L0:** assert RA target dir resolves to `target/rust-analyzer` and a manual
  `cargo check` against `target/` does not block an RA check (separate locks).

## Open risks

- Some tools invoke cargo by absolute path and bypass the shim. Mitigation: RA
  (the main such tool) is carved out in L0; remaining bypassers hit the raw lock
  but are rare. Documented, not silently ignored.
- Windows PATH ordering for the shim must precede `~/.cargo/bin`. Setup
  documented in AGENTS.md; verified by a smoke check (`cargo --vox-broker-ping`
  style sentinel, or `where cargo`).
