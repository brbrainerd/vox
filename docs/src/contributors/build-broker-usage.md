---
title: "Build broker — cross-agent cargo queue"
category: "Contributors"
---

# Build broker (cross-agent cargo queue)

When several agents / IDE tabs / terminals build in the **same** worktree they
contend on cargo's single `target/` lock (`Blocking waiting for file lock on
build directory`). The build broker turns that opaque block into a fair, visible
queue and records evidence metrics — without a daemon.

## How it works

`crates/vox-cargo-shim` builds a binary literally named `cargo`. Placed on PATH
*ahead of* the rustup proxy, it intercepts `build`/`test`/`check`/`clippy`/`run`/
`bench` inside a vox worktree, takes a fair per-worktree queue ticket
(`.vox/build-queue/<hash>/`), runs the **real** cargo (the rustup proxy, so
`rust-toolchain.toml` / `+toolchain` still work), and appends one metric line per
invocation. Anything else — or any error — falls through to real cargo. It is
never a hard dependency.

Safety: the shim prefers the `$CARGO_HOME`/`~/.cargo/bin` proxy and skips
same-directory sibling copies, and a `VOX_BROKER_DEPTH` guard aborts at depth ≥ 2
so a misconfiguration can never fork-bomb.

## Install (per machine)

```sh
cargo build -p vox-cargo-shim --release
mkdir -p .vox/bin/cargo-shim
cp target/release/cargo[.exe] .vox/bin/cargo-shim/
```

## Activate (per IDE / agent)

Prepend the shim directory to the IDE terminal PATH so IDE-spawned terminals and
agents participate — scoped, **not** a machine-wide PATH change.

- **VS Code / Cursor / Antigravity / Windsurf** (`.vscode/settings.json`, already
  committed for this repo):
  ```jsonc
  "terminal.integrated.env.windows": {
    "PATH": "${workspaceFolder}\\.vox\\bin\\cargo-shim;...;${env:PATH}"
  }
  ```
- **Other hosts / plain shells:** prepend `<repo>/.vox/bin/cargo-shim` to PATH in
  that host's terminal environment.

Reload the window/terminal after changing settings.

## Evidence gate (do we need a coalescing daemon?)

The shim records `queue_wait_ms` and a `would_coalesce` flag per build. The
`vox_build_queue::metrics::summarize_worktree` summary yields a verdict
(`Summary::render`): the deferred coalescing daemon (Layer 1b) is only worth
building if ≥ 10 % of invocations had a coalescing opportunity **and** real queue
waits occurred. Otherwise the daemonless shim is the final form.

See `docs/superpowers/specs/2026-06-18-unified-build-broker-design.md`.
