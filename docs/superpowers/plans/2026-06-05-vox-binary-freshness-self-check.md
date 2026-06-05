# Vox Binary Freshness Self-Check — Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Promotes:** the `status: research` design *Vox Binary Freshness &
Single-Source-of-Truth (2026)* (§3.1 / §3.2 / §4) into a concrete, code-grounded
implementation.

**Goal:** A stale installed `vox.exe` (an older build running outdated guard
logic/allowlists) must never silently mislead a `vox ci *` diagnosis. Make the
binary self-detect staleness by comparing its **embedded build number**
(`env!("VOX_BUILD_NUMBER")`) against the **live** working-tree build number
(`git rev-list --count HEAD`), and:

- **Hard-fail** every `vox ci *` subcommand when the binary is older than the
  tree (escape hatch: `VOX_SKIP_FRESHNESS_CHECK=1`).
- **Surface** the same signal as a non-fatal `vox doctor` check.

**Why build number, not semver:** build number catches *same-version*
staleness (`0.6.0+build.601` vs `0.6.0+build.1917`) that a semver compare misses
— the exact failure mode in the motivating incident.

## Existing-code anchors (verified)

- `crates/vox-build-meta/src/lib.rs` — `emit()` sets `VOX_BUILD_NUMBER =
  git rev-list --count HEAD` and `VOX_GIT_HASH = git rev-parse --short HEAD` at
  build time. Falls back to `"dev"` / `"unknown"` when git is unavailable.
- `crates/vox-cli/build.rs` — calls `vox_build_meta::emit()`, so
  `env!("VOX_BUILD_NUMBER")` is valid inside `vox-cli`.
- `crates/vox-cli/src/lib.rs:88` — `VOX_VERSION` already consumes
  `env!("VOX_BUILD_NUMBER")`. New module lives in the same crate so the same
  `env!` works.
- `crates/vox-cli/src/commands/ci/run_body.rs:55-57` — `pub async fn run(cmd)`
  resolves `let root = repo_root();` once at the top, then matches every
  `CiCmd`. This is the single choke point for **all** `vox ci *` — the hard-fail
  insertion point.
- `crates/vox-cli/src/commands/ci/mod.rs:92` — `repo_root()` →
  `vox_repository::resolve_repo_root_for_ci()` (honors `VOX_REPO_ROOT`, else
  walks up for `AGENTS.md`+`Cargo.toml`, else nearest `.git`).
- `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/mod.rs` —
  `run_checks()` pushes `Check` rows; `vox_ignore.rs` is the pattern to mirror
  for a new check module. `common::Check::{pass,fail,new}` build rows.

## Design decisions

1. **One pure classifier, thin git wrapper.** `classify(embedded, live)` is pure
   over `Option<u64>` and fully unit-testable; `evaluate(repo_root)` wires the
   git subprocess. This keeps tests hermetic (no git, no fixtures).
2. **`"dev"`/non-numeric build numbers → `Unknown` → never fail, never warn.**
   Local dev builds and detached states must not raise false alarms.
3. **`embedded >= live` → Fresh.** A binary built at a *newer* commit than the
   checked-out tree (e.g. you checked out an older commit) is not "stale" in the
   dangerous sense and must not block.
4. **Only `embedded < live` → Stale.** That is the precise, dangerous case: the
   running binary predates the source it is judging.
5. **Scope the cost.** The git call runs only on `vox ci *` (not a hot path) and
   inside `vox doctor`. We do **not** add a git subprocess to every `vox`
   invocation — that would tax hot script/build paths for little gain. (The
   broader "warn on every direct invocation" idea from the design is deliberately
   deferred; `vox ci` is where wrong diagnoses actually happen.)
6. **Escape hatch:** `VOX_SKIP_FRESHNESS_CHECK=1` downgrades the ci hard-fail to
   a printed note, for the rare intentional stale run.

## File changes

- **Create** `crates/vox-cli/src/freshness.rs` — classifier, git wrapper,
  `enforce_for_ci`, `staleness_warning`, unit tests.
- **Modify** `crates/vox-cli/src/lib.rs` — declare `pub mod freshness;`.
- **Modify** `crates/vox-cli/src/commands/ci/run_body.rs` — call
  `crate::freshness::enforce_for_ci(&root)?;` immediately after `let root =`.
- **Create**
  `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/freshness.rs`
  — `run(checks)` pushing one `Check` row.
- **Modify**
  `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/mod.rs` —
  declare the module and call it from `run_checks`.

---

## Task 1: Freshness module (`crates/vox-cli/src/freshness.rs`)

- [ ] `EMBEDDED_BUILD_NUMBER = env!("VOX_BUILD_NUMBER")`, `EMBEDDED_GIT_HASH =
  env!("VOX_GIT_HASH")`, `SKIP_ENV = "VOX_SKIP_FRESHNESS_CHECK"`.
- [ ] `enum Freshness { Fresh, Stale { embedded, live }, Unknown(&'static str) }`.
- [ ] `parse_build_number(&str) -> Option<u64>` (rejects `"dev"`/non-numeric).
- [ ] `classify(Option<u64>, Option<u64>) -> Freshness` (pure; rule 3 + 4).
- [ ] `live_build_number(&Path) -> Option<u64>` — `git -C <root> rev-list
  --count HEAD` with `// vox-arch-check: allow git-exec`.
- [ ] `evaluate(&Path) -> Freshness`.
- [ ] `skip_requested() -> bool` (env non-empty).
- [ ] `staleness_warning(&Path) -> Option<String>` — message incl. embedded vs
  live + the `cargo install --path crates/vox-cli --force` remedy.
- [ ] `enforce_for_ci(&Path) -> anyhow::Result<()>` — `Err` on `Stale` unless
  `skip_requested()` (then `eprintln!` a note); `Ok` otherwise.
- [ ] Unit tests: `parse_build_number`, all `classify` arms,
  `enforce_for_ci` honoring the skip env.

## Task 2: Wire `vox-cli` lib + ci hard-fail

- [ ] `crates/vox-cli/src/lib.rs`: add `pub mod freshness;`.
- [ ] `run_body.rs`: `crate::freshness::enforce_for_ci(&root)?;` after `let root
  = repo_root();`.

## Task 3: Doctor check

- [ ] New `checks_standard/freshness.rs`: `pub fn run(checks)` →
  `Check::pass`/`fail`/`new` from `evaluate`. `Unknown` is a non-blocking pass
  ("dev build / git unavailable").
- [ ] `checks_standard/mod.rs`: `mod freshness;` + `freshness::run(&mut checks);`
  in `run_checks`.

## Task 4: Verify

- [ ] `cargo build -p vox-cli`.
- [ ] `cargo test -p vox-cli freshness`.
- [ ] `cargo clippy -p vox-cli` clean on touched files.
- [ ] `cargo fmt -p vox-cli`.

## Task 5: Canonical-binary SSOT doctor check (§3.2, scoped)

Declare `~/.cargo/bin/<vox>` canonical and detect PATH-shadowing divergence.

- [x] `freshness.rs`: `vox_binary_name()`, `canonical_install_path()`,
  `build_number_from_version_line()`, `distinct_build_numbers()` (+ unit tests).
- [x] New `checks_standard/binary_ssot.rs`: enumerate every `vox` on `PATH` plus
  `~/.cargo/bin` and `~/.vox/bin`, ask each `--version`, fail when build numbers
  disagree — listing each path, marking the canonical, recommending a refresh of
  the canonical install.
- [x] Wire into `run_checks`.

## Out of scope (tracked, not built here)

- §3.2 install-machinery: voxup forwarder / `vox self-install` so one real
  binary backs both locations (riskier; the Task 5 doctor check *detects* the
  divergence — this would *prevent* it).
- §3.3 version-bump runbook tie-in / `gui-version-sync` freshness assertion.
- "Warn on every direct invocation" (perf-sensitive; deferred per decision 5,
  and confirmed by the maintainer — keep it off the per-`vox`-script path).
