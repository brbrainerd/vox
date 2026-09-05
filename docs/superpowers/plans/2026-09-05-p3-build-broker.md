# P3 — Build Broker Activation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Steps use `- [ ]` checkboxes.
>
> **Read [`2026-09-05-00-INDEX.md`](2026-09-05-00-INDEX.md) first** for file-ownership rules and global constraints.

**Goal:** turn on the machine-wide build broker that already exists, so multiple agents, IDE tabs and sessions cannot saturate one machine.

**Spec:** [`../specs/2026-09-04-distribution-and-plugin-architecture.md`](../specs/2026-09-04-distribution-and-plugin-architecture.md) §16

**You own:** `crates/vox-cargo-shim/`, `crates/vox-build-queue/`, `docs/src/contributors/build-broker-usage.md`, `scripts/broker-*`, `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_broker.rs` *(new)*

## Global constraints

See the index. Non-negotiable everywhere: assert on the artifact never the exit code (`cmd > /tmp/x.log 2>&1; echo $?`); `cargo test -p X` needs `--all-targets` or it can report "0 passed" when tests live in a bin target; guards must run on macOS (no `grep -oP`); never execute a downloaded binary or set `com.apple.quarantine`.

## Starting premise: build nothing new

`vox-cargo-shim` + `vox-build-queue` are a complete, sound build broker: a binary literally named `cargo`, an N-slot cross-process file semaphore, state at `~/.vox/build-broker/` deliberately outside any repo, `metrics.jsonl` + `broker.log` already global and auditable, a `VOX_BROKER_DEPTH` fork-bomb guard, and graceful fallthrough to real cargo.

**It has never run.** `~/.vox/build-broker` does not exist; `which -a cargo` returns only the rustup proxy. Your job is activation, verification and enforcement — not redesign.

---

## Task 1: Cross-platform activation

The usage doc's activation section is **Windows-only**: `terminal.integrated.env.windows.PATH`, `…\.vox\build-broker\bin`, `cargo[.exe]`. Zero mentions of `.osx`/`.linux`, `.zshrc`, `.bashrc`, or `export PATH`. macOS and Linux developers have no documented path at all.

- [ ] Add `scripts/broker-install.vox` (VoxScript, per AGENTS.md "VoxScript-First Glue Code"): builds the shim, installs to `~/.vox/build-broker/bin`, prepends to the login shell profile **ahead of `~/.cargo/bin`**, idempotently.
- [ ] Reuse `voxup`'s profile-detection approach — it already handles zsh/bash/fish and creates a profile on a pristine macOS account.
- [ ] Document `terminal.integrated.env.osx` / `.linux` alongside the existing Windows guidance.
- [ ] Verify by artifact: after running, `which -a cargo` lists the shim first.

## Task 2: A doctor check so it is visible

- [ ] New file `build_broker.rs` (you own it). Checks: is the shim on PATH ahead of `~/.cargo/bin`; does `~/.vox/build-broker/` exist; is the cap sane for this core count.
- [ ] Register with a **one-line append** to `checks_standard/mod.rs` — see the index's shared-file protocol. Do not restructure that file; P6 also appends to it.
- [ ] Every remediation string must name a command that exists in the clap tree.

## Task 3: Enforcement

- [ ] Add a lint asserting no committed script or workflow invokes bare `cargo` in a way that bypasses the shim when the broker is installed. Prove it fails before it passes.
- [ ] Append a short section to `AGENTS.md` (append-only; P1 also appends) telling agents to route builds through the broker and never to run concurrent `cargo` invocations on one workspace.

## Task 4: The container gap — design, then implement one option

The semaphore is a filesystem lock under `~/.vox/build-broker/slots/`. A containerised CI runner has a separate mount namespace and never sees it, so the broker governs host builds only. Measured during spec work: ~10 container cores plus 2 host builds, load 15.4 on 18 cores; the broker would have capped only the host half.

- [ ] Choose: (a) bind-mount `~/.vox/build-broker` into the runner container — one machine, one budget, more correct; or (b) give the container a fixed budget subtracted from the host cap via `VOX_BROKER_MAX_CONCURRENT` — simpler, no host coupling.
- [ ] Implement the chosen one on the broker side. **`scripts/ci-runner-local.sh` is owned by P4** — send the runner-side change as a cross-plan request.

## Task 5: Surface the queue

- [ ] The audit data already exists and is already global. Add a read-only command that renders `broker.log`/`metrics.jsonl` (queue depth, waits, `ahead>0` share) instead of asking users to `tail -f`.
- [ ] Read-only: it must never mutate broker state.

## Task 6: Stop it rotting

`vox-cargo-shim` is workspace-excluded (`Cargo.toml:6`) because its bin must be named `cargo`, so normal workspace CI never builds it.

- [ ] Add a build+test recipe for the excluded crate and hand it to P4 as a cross-plan request for a CI lane.

## Verification
- [ ] Fresh-machine simulation with `VOX_BROKER_HOME` pointed at a temp dir: install, activate, run two concurrent builds, assert `broker.log` shows `ahead>0`.
- [ ] `cargo test -p vox-build-queue --all-targets` with real counts.
- [ ] `vox doctor` reports the broker correctly both when active and when not.

## Cross-plan requests
| To | Request |
|---|---|
| P4 | CI lane: add a required job that runs `vox run scripts/broker-ci.vox` (builds, tests, and clippy-checks the workspace-excluded `crates/vox-cargo-shim` with `--manifest-path`, asserts a real `test result:` line with a non-zero passed count, and confirms the produced `cargo`/`cargo.exe` binary exists) and `vox run scripts/broker-bypass-lint.vox` (exits non-zero on any un-allowlisted broker bypass in `scripts/`, `.github/workflows/`, or `crates/**/*.rs`). |
| P4 | Container budget: the runner supervisor (`scripts/ci-runner-local.sh`, owned by P4) must export `VOX_BROKER_RESERVED_SLOTS` on the host, set to the container's concurrent-build budget, before starting containerised builds — the broker's `flock`-based semaphore can't see across the container's mount namespace, so without this the container's builds run uncounted alongside the host's. |
| P7 | Add a `vox ci build-queue` alias that shells to the `vox-broker` binary this plan ships (the `vox-broker` `[[bin]]` target in `crates/vox-cargo-shim/Cargo.toml`, a read-only viewer over `~/.vox/build-broker/metrics.jsonl` / `broker.log`; see `docs/src/contributors/build-broker-usage.md`). |
| P5 | If `voxup` gains a dev-setup path, call `broker-install.vox` (via `vox run scripts/broker-install.vox`) from it |
