# P2 — Toolchain SSOT & Enforcement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Steps use `- [ ]` checkboxes.
>
> **Read [`2026-09-05-00-INDEX.md`](2026-09-05-00-INDEX.md) first** for file-ownership rules and global constraints.

**Goal:** one place states the Rust/Node/pnpm versions; the other eight restatements are generated; and a bump is one command with checks that run on a developer's laptop.

**Spec:** [`../specs/2026-09-04-distribution-and-plugin-architecture.md`](../specs/2026-09-04-distribution-and-plugin-architecture.md) §12

**You own:** `rust-toolchain.toml`, `contracts/toolchain/`, `Dockerfile`, `Dockerfile.ci-runner`, `infra/ci-runner/`, `.github/actions/` *(new)*, `crates/vox-cli-ci/src/version_ssot.rs`, `crates/vox-cli-ci/examples/ssot_probe.rs`, root `Cargo.toml`

## Global constraints

See the index. Non-negotiable everywhere: assert on the artifact never the exit code (`cmd > /tmp/x.log 2>&1; echo $?`); `cargo test -p X` needs `--all-targets` or it can report "0 passed" when tests live in a bin target; guards must run on macOS (no `grep -oP`); never execute a downloaded binary or set `com.apple.quarantine`.

## Correct starting premise

An earlier audit claimed CI compiles on floating stable while releases compile on the pin. **That is false** and you must not "fix" it. Measured:

```
info: note that the toolchain '1.96.0-...' is currently in use (overridden by ...)
[Build] info: syncing channel updates for 1.96.0-...
```

`rust-toolchain.toml` wins for every in-repo cargo invocation. `@stable`'s real cost is that 45 sites install a toolchain that compiles nothing, with components for the wrong version.

The genuine defects: the pin is a `.0` and **1.96.1 exists**; cache keys omit the toolchain entirely; the version is restated in 8 live places while the existing guard checks 2, uses GNU-only `grep -oP`, and lives in `ci.yml`, which never completes.

---

## Task 1: Choose the target version on evidence

- [ ] Install both candidates: `rustup toolchain install 1.96.1 1.98.1 --profile minimal --component rustfmt,clippy`
- [ ] `cargo +1.98.1 check --workspace --all-targets > /tmp/tc-1981.log 2>&1; echo $?` — record error count, not just exit.
- [ ] If 1.98.1 is clean, take it (latest stable, non-`.0`). If not, fall back to 1.96.1 and record exactly what failed.
- [ ] MSRV (`rust-version`) is a **floor** and may stay at `1.96` while the toolchain moves. Do not raise it without a reason.
- [ ] Commit the decision with the log excerpt as evidence.

## Task 2: Extend `ssot_probe` to the toolchain rows

`ssot_probe` already rewrites 13 restatements of the *Vox* version with a verified 127-line bump. Reuse it; do not build a parallel mechanism.

Rows to generate from `contracts/toolchain/workspace-toolchain.v1.yaml`:

| # | Path | Form |
|---|---|---|
| 1 | `rust-toolchain.toml` | `channel = "X"` |
| 2 | root `Cargo.toml` | `rust-version = "maj.min"` |
| 3 | `Dockerfile` | `FROM rust:X-slim-bookworm` |
| 4 | `Dockerfile.ci-runner` | `ARG RUST_VERSION=X` |
| 5 | `infra/ci-runner/Dockerfile` | `ARG RUST_VERSION=X` |
| 6 | `contracts/distribution/profiles.v1.yaml` | `rust_version: "X"` — **P7 owns this file**; emit a cross-plan request instead |
| 7 | `contracts/channels/stable.toml` | `min_rust = "X"` |
| 8 | `crates/voxup/src/profiles.rs` fixture | **P5 owns**; cross-plan request |

- [ ] Write a failing test: drift in any owned row is detected and reported with the file and line.
- [ ] Beware the substring bug already fixed once in `version_ssot.rs`: `line.find("version")` matched inside the path `vox-versioning`. Use the key-anchored `toml_version_key_end` helper.
- [ ] `--write` rewrites; no-arg reports drift and exits non-zero.

## Task 3: The composite action — kill 45 restatements

- [ ] Create `.github/actions/setup-rust/action.yml`: reads the SSOT, installs exactly that toolchain with requested components, configures cache with a key that **includes the toolchain version**.
- [ ] Parse the YAML **without** `grep -oP` — it must work on macOS and under `act`.
- [ ] Test it locally with `act` before handing to P4.
- [ ] **Do not edit `.github/workflows/`** — P4 owns those and adopts the action itself.

## Task 4: Enforcement rules, each with a runnable check

- [ ] **No `.0` pins.** Validator rejects `versions.rust` matching `^\d+\.\d+\.0$`. Test both directions.
- [ ] **No hand-written toolchain versions.** Lint fails any Dockerfile `ARG RUST_VERSION=`/`FROM rust:` disagreeing with the SSOT. *(The workflow half of this lint belongs to P4.)*
- [ ] **Portable guards.** Delete the `grep -oP` guard; the replacement must pass on macOS.
- [ ] Prove each lint fails before it passes: introduce the violation, capture output, revert, re-run.

## Verification
- [ ] `cargo test -p vox-cli-ci --all-targets > /tmp/p2.log 2>&1; echo $?` with real counts.
- [ ] Bump end-to-end on a scratch branch; `cargo check --workspace` still exit 0; `git diff --stat` matches the expected row count.
- [ ] Every new guard runs clean on macOS.

## Cross-plan requests
| To | Request |
|---|---|
| P4 | Adopt `./.github/actions/setup-rust` at all 45 sites; add the workflow-side lint |
| P5 | Let `ssot_probe` own the `voxup/src/profiles.rs` version fixture |
| P7 | Let `ssot_probe` own `rust_version` in `profiles.v1.yaml` |
