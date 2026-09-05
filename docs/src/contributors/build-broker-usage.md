---
title: "Build broker — machine-wide cargo coordination"
description: "How the vox-cargo-shim build broker caps concurrent cargo builds machine-wide, its tunables, install steps, and how to inspect its audit log with vox-broker."
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
- `VOX_BROKER_RESERVED_SLOTS` — slots subtracted from the host cap for a build
  domain the file-lock semaphore cannot see, chiefly a containerised CI runner
  sharing this host's CPU/RAM but not its mount namespace (the broker's
  `flock`-based semaphore is invisible across that boundary, so a container's
  builds would otherwise run uncounted alongside the host's). Effective cap is
  `max(1, base_cap - reserved)` — it never drops to 0, since a cap of 0 slots
  would spin forever. Applies after `VOX_BROKER_MAX_CONCURRENT` too: an
  explicit override is still reduced by the reservation. A non-numeric or
  negative value is ignored (treated as 0). Default: 0 (no reservation).
- `VOX_BROKER_HOME` — relocate broker state (tests / isolation).
- `VOX_BROKER_DEBUG` — print resolution and exit without building.

## Install (per machine, git-proof)

The installer is dry-run by default — it prints exactly what it would do and
changes nothing until you pass `--apply`. Prepending a binary literally named
`cargo` onto PATH is a machine-wide change, so always look at the plan first:

```sh
vox run scripts/broker-install.vox              # dry run: prints the plan, touches nothing
vox run scripts/broker-install.vox -- --apply   # builds, installs, and edits your shell profile
```

`--apply` builds `vox-cargo-shim` in release mode, copies every binary it
produces into `${VOX_BROKER_HOME:-$HOME/.vox/build-broker}/bin/` (preserving
the exec bit), installs a copy of the installer itself alongside it (so a
later remediation step has an absolute, checkout-free path to re-run), and
idempotently prepends that `bin/` dir to `PATH` **ahead of `~/.cargo/bin`** in
your detected login-shell profile (zsh → `.zshrc`; bash → `.bash_profile` on
macOS / `.bashrc` on Linux; fish → `.config/fish/config.fish`; anything else →
`.profile`). The PATH block is delimited by fixed marker comments
(`# >>> vox build broker >>>` / `# <<< vox build broker <<<`) so re-running
`--apply` replaces the block in place instead of duplicating it. The profile
is written via a temp file + atomic `mv` and is never truncated — if the
detected profile doesn't exist yet, the installer creates it and says so.

Useful flags:

- `--profile <path>` — write the PATH block to this file instead of the
  auto-detected profile. Mainly for testing in an isolated temp file.
- `--no-profile` — install the binaries but make no profile edit (you'll add
  the directory to PATH yourself).
- `--help` — usage.

**Manual fallback**, if you'd rather not run the script (or the crate's set
of `[[bin]]` targets has grown since this doc was written — enumerate them
from `crates/vox-cargo-shim/Cargo.toml` rather than assuming just `cargo`):

```sh
cargo build --release --manifest-path crates/vox-cargo-shim/Cargo.toml
mkdir -p ~/.vox/build-broker/bin
cp target/release/cargo[.exe] ~/.vox/build-broker/bin/
```

(`-p vox-cargo-shim` does not resolve here — the crate is excluded from the
main workspace so its `cargo`-named binary can't be entangled in the
workspace's own cargo resolution — hence `--manifest-path`.)

## Activate (per environment)

Prepend the shim dir to PATH **ahead of `~/.cargo/bin`** so cargo invocations are
intercepted. `vox run scripts/broker-install.vox -- --apply` does the
POSIX-shell-profile route below for you; the IDE / Windows routes still need
manual setup:

- **POSIX shells (macOS / Linux):** `vox run scripts/broker-install.vox -- --apply` adds
  an `export PATH="…/.vox/build-broker/bin:$PATH"` block to your `.zshrc`,
  `.bash_profile`/`.bashrc`, or `.config/fish/config.fish` (fish uses
  `set -gx PATH … $PATH`, not `export`). This covers all future-launched
  shells and agents.
- **Persistent user scope (Windows):** prepend `…\.vox\build-broker\bin` to
  the user `PATH` env var (covers all future-launched shells/agents).
- **VS Code / Cursor / Antigravity / Windsurf:** add the same dir to
  `terminal.integrated.env.osx.PATH`, `terminal.integrated.env.linux.PATH`, or
  `terminal.integrated.env.windows.PATH` (pick the one matching your OS) for
  live, already-running IDEs (applies to newly spawned terminals — reload the
  window or open a new terminal).

A running process keeps its launch-time environment, so already-running agents
pick up the shim only on a **new terminal / window reload**.

## Observe (verify it's working)

Verifying activation is a two-step check, because `which -a cargo` only reads
the **current process's** PATH — it proves nothing about a profile edit it
never re-read, and will keep showing only the rustup proxy no matter how
correct the installer was:

```sh
# 1. The edit: confirm the profile actually has the marker block and the bin path.
grep -A1 '>>> vox build broker >>>' ~/.zshrc   # or your detected profile

# 2. The effect: a FRESH login shell re-reads the profile, so check there —
#    not in the shell you're already sitting in.
sh -lc 'command -v cargo'
zsh -lc 'which -a cargo'      # first hit should be …/.vox/build-broker/bin/cargo
```

Once activated, `vox-broker` (installed alongside the shim, on the same PATH
entry) reads the broker's own audit files for you — no more `tail -f` plus a
`grep`:

```sh
vox-broker stats   # summary over metrics.jsonl: p50/p95 queue wait, coalesce rate,
                    # and the coalescing-daemon go/no-go verdict
vox-broker log     # last 20 lines of broker.log (add -n N for a different count)
vox-broker status  # effective cap, any VOX_BROKER_RESERVED_SLOTS reservation,
                    # a sampled count of busy slots, and the broker home path
```

`vox-broker` is strictly read-only: it never creates, deletes, or modifies any
broker state (including the broker home directory itself), and its slot count
in `status` is an explicit sample, not a snapshot — another process can take
or release a slot between the read and your acting on it. Before the broker has
ever run, every subcommand says so plainly and exits 0, rather than printing a
row of zeros.

A `broker.log` line reads e.g.:

```text
<ts> test   wait=  4200ms ran= 18000ms ahead=3 cap=4 coalesce=false exit=0 <worktree>
```

`ahead>0` / `wait>0` means the cap absorbed contention that would otherwise have
blocked opaquely on cargo's locks or thrashed the machine. For anyone who wants
the raw files instead: `~/.vox/build-broker/broker.log` (human-readable,
one line per build, every worktree) and `~/.vox/build-broker/metrics.jsonl`
(machine-readable, one JSON record per build) — both overridable via
`VOX_BROKER_HOME`.

## Evidence gate (coalescing daemon)

`metrics.jsonl` records `would_coalesce` per build. A coalescing daemon (Layer 1b)
is only worth building if a material share of builds are true duplicates; until
the data shows that, the daemonless cap is the final form. See
`docs/superpowers/specs/2026-06-18-unified-build-broker-design.md`.
