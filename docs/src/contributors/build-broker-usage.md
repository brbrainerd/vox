---
title: "Build broker — machine-wide cargo coordination"
category: "Contributors"
---

# Build broker (machine-wide cargo coordination)

When many agents / IDE tabs / git hooks build across many worktrees on one
machine, they (a) pile up on cargo's `target` and global package-cache locks and
(b) saturate CPU/RAM/IO. The build broker is a `cargo`-named shim that puts every
build through a **machine-wide concurrency cap** so at most _N_ cargo builds run
at once, turning a 13-way pileup into an orderly, observable queue — without a
daemon.

## How it works

`crates/vox-cargo-shim` builds a binary literally named `cargo`. Placed on PATH
ahead of the rustup proxy, it intercepts `build`/`test`/`check`/`clippy`/`run`/
`bench`, acquires one of _N_ global slots (a cross-process file-lock semaphore),
runs the **real** cargo (the rustup proxy, so `rust-toolchain.toml` / `+toolchain`
still work), records a metric, and releases the slot. Everything else — or any
error — falls straight through to real cargo. It is never a hard dependency.

State lives **outside any repo** at `~/.vox/build-broker/` (overridable via
`VOX_BROKER_HOME`) so concurrent agents' `git clean`/checkout can't wipe it:
- `slots/slot_*` — the N-slot semaphore.
- `inflight/*` — in-flight build identities (for coalescing metrics).
- `metrics.jsonl` — one record per build (machine-readable).
- `broker.log` — one human-readable line per build (every worktree, one place).

Safety: prefers the `~/.cargo/bin` proxy and skips sibling shim copies; a hard
`VOX_BROKER_DEPTH` guard aborts at depth ≥ 2 so a misconfiguration can never
fork-bomb.

## Tunables (env)

- `VOX_BROKER_MAX_CONCURRENT` — max simultaneous builds machine-wide. Default
  ≈ logical-cores / 3, clamped to [2, 8]. Lower it when the machine thrashes.
- `VOX_BROKER_HOME` — relocate broker state (tests / isolation).
- `VOX_BROKER_DEBUG` — print resolution and exit without building.

## Install (per machine, git-proof)

```sh
cargo build -p vox-cargo-shim --release
mkdir -p ~/.vox/build-broker/bin
cp target/release/cargo[.exe] ~/.vox/build-broker/bin/
```

## Activate (per environment)

Prepend the shim dir to PATH **ahead of `~/.cargo/bin`** so cargo invocations are
intercepted:
- **Persistent user scope:** prepend `…\.vox\build-broker\bin` to the user `PATH`
  env var (covers all future-launched shells/agents).
- **VS Code / Cursor / Antigravity / Windsurf:** add the same dir to
  `terminal.integrated.env.windows.PATH` for live, already-running IDEs (applies
  to newly spawned terminals — reload the window or open a new terminal).

A running process keeps its launch-time environment, so already-running agents
pick up the shim only on a **new terminal / window reload**.

## Observe (verify it's working)

```sh
# live tail of every build the broker handles, across all worktrees
tail -f ~/.vox/build-broker/broker.log

# how many builds queued behind the cap (ahead>0 = contention absorbed)
grep -v 'ahead=0' ~/.vox/build-broker/broker.log | wc -l
```

A line reads e.g.:
```
<ts> test   wait=  4200ms ran= 18000ms ahead=3 cap=4 coalesce=false exit=0 <worktree>
```
`ahead>0` / `wait>0` means the cap absorbed contention that would otherwise have
blocked opaquely on cargo's locks or thrashed the machine.

## Evidence gate (coalescing daemon)

`metrics.jsonl` records `would_coalesce` per build. A coalescing daemon (Layer 1b)
is only worth building if a material share of builds are true duplicates; until
the data shows that, the daemonless cap is the final form. See
`docs/superpowers/specs/2026-06-18-unified-build-broker-design.md`.
