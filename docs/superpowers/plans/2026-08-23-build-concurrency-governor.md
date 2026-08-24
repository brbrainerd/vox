---
title: "Build Failure Classification and Agent Build Throttling — Implementation Plan"
description: "TDD tasks for the format-agnostic build-failure classifier, the agent-scoped CARGO_BUILD_JOBS cap, and the disk/RAM preflight in vox doctor, after the jobserver broker was audited and rejected."
category: "Architecture SSOTs"
---

# Build Failure Classification and Agent Build Throttling — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop agents from editing working code after a build failure they
misread, and start watching the two resources that actually break builds on this
host (free disk, free RAM).

**Architecture:** Three independent pieces, no shared state between them. (1) A
pure function in `vox-cli-ci` that classifies build output as `Real`,
`Corruption`, or `Contention` and returns the diagnostics it found as evidence.
(2) `CARGO_BUILD_JOBS=4` in `.claude/settings.json` `env`, agent-scoped. (3) A
free-disk and free-RAM preflight in `vox doctor`'s build-health group.

**Tech Stack:** Rust 2024, `sysinfo` 0.39 (already a `vox-cli` dependency),
`fs2` 0.4 (already in `Cargo.lock` via `vox-build-queue`). No new workspace
dependencies, no `unsafe`, no new CLI subcommand.

**Spec:** [`docs/superpowers/specs/2026-08-23-build-concurrency-governor-design.md`](../specs/2026-08-23-build-concurrency-governor-design.md)

**Dropped from the previous revision of this plan:** the `vox ci build-broker`
task, the `CARGO_MAKEFLAGS` env task, and the jobserver `vox doctor` task. The
broker was audited and rejected — see the spec's *Rejected: the token broker*.
Do not reintroduce them.

## Global Constraints

- **No new `CiCmd` variant.** Nothing here adds a subcommand, so
  `contracts/reports/gui-surface-coverage.v1.json` and
  `crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt` are
  untouched. If you find yourself adding a variant, stop: it drifts both (the
  first fails `vox ci ssot-drift`, i.e. fast pre-push *and* CI; the second fails
  a test) and each needs its own regeneration step.
- **No `unsafe`.** `contracts/reports/safety-inventory/baseline.v1.json` records
  501 unsafe blocks; adding any drifts it and fails CI. `fs2` and `sysinfo` are
  the safe route to both preflight numbers.
- `docs/src/reference/cli-command-surface.generated.md` derives from the command
  registry, not the clap tree, and is not affected by anything in this plan.
- The `vox-cli → vox-cli-ci` crate edge already exists; no ratchet change.
  Do **not** add an edge from `vox-orchestrator-mcp` to `vox-cli-ci` — that
  would need a user-authorized `exceptions` ledger entry (AGENTS.md
  §Dependency Discipline). Wiring the classifier into the MCP compiler tools is
  a deliberate follow-up, not part of this plan.
- Test-first policy (AGENTS.md): every new `pub fn` gets its test in the same
  file, written first.
- Never run `cargo build`/`check`/`test` on `--workspace` while other agents
  build. Scope every command to `-p vox-cli-ci` or `-p vox-cli`.
- Format with `cargo fmt -p <crate>`; never `cargo fmt --all` (Windows
  `os error 206`).

---

### Task 1: Format-agnostic build-failure classifier

A pure function over build output. The previous implementation tested
`line.starts_with("error[")`, which is `false` for every diagnostic this repo
emits (`--message-format short` puts the path first), so it classified every
real compile error as `Contention`. The corpus below exists to make that
specific regression impossible.

**Files:**
- Create: `crates/vox-cli-ci/src/build_failure.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs` (add `pub mod build_failure;`)

**Interfaces:**
- Consumes: nothing.
- Produces, at `vox_cli_ci::build_failure`:
  `pub enum BuildFailureKind { Contention, Corruption, Real }`,
  `pub struct BuildFailure { pub kind: BuildFailureKind, pub diagnostics: Vec<String> }`,
  `pub fn classify_build_failure(output: &str, truncated: bool) -> BuildFailure`.
  No other task in this plan consumes them.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli-ci/src/build_failure.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn kind(output: &str) -> BuildFailureKind {
        classify_build_failure(output, false).kind
    }

    /// The regression that motivated this rewrite. This repo builds with
    /// `--message-format short`, where the path comes first and the previous
    /// `starts_with("error[")` predicate returned false — classifying every
    /// real compile error as `Contention`, i.e. "retry, don't edit code".
    #[test]
    fn short_format_diagnostics_are_real() {
        let out = "crates\\vox-gui\\src\\commands\\mic.rs:169:51: error[E0610]: \
                   `{integer}` is a primitive type and therefore doesn't have fields\n\
                   error: could not compile `vox-gui` (lib) due to 1 previous error";
        let f = classify_build_failure(out, false);
        assert_eq!(f.kind, BuildFailureKind::Real);
        assert_eq!(f.diagnostics.len(), 1, "{:?}", f.diagnostics);
    }

    #[test]
    fn full_format_diagnostics_are_real() {
        // Coded, and uncoded-with-location-arrow: both must be found.
        let coded = "error[E0308]: mismatched types\n --> src/lib.rs:3:9";
        let uncoded = "error: expected one of `,` or `}`, found `;`\n --> src/lib.rs:7:1";
        assert_eq!(kind(coded), BuildFailureKind::Real);
        assert_eq!(kind(uncoded), BuildFailureKind::Real);
    }

    #[test]
    fn json_format_diagnostics_are_real() {
        let out = r#"{"reason":"compiler-message","message":{"level":"error","rendered":"boom"}}"#;
        assert_eq!(kind(out), BuildFailureKind::Real);
    }

    /// Real beats contention. A mixed run — crate A a genuine error, crate B a
    /// linker failure — previously returned `Contention` because contention was
    /// evaluated first.
    #[test]
    fn a_real_error_outranks_a_linker_failure() {
        let out = "src/a.rs:1:1: error[E0425]: cannot find value `nope` in this scope\n\
                   error: linking with `lld-link` failed: exit code: 1\n\
                   error: could not compile `b` (lib)";
        assert_eq!(kind(out), BuildFailureKind::Real);
    }

    /// Zero diagnostics on a failed build is the only contention signal we
    /// need. `cc failed` / `Permission denied` / `failed to run custom build
    /// command` are deliberately NOT markers — each also names a real bug, and
    /// a diagnostic-free build script failure lands here anyway.
    #[test]
    fn no_diagnostics_is_contention_and_says_so() {
        let out = "error: could not compile `glob` (lib)\n\
                   error: failed to run custom build command for `zstd-sys v2.0.16`";
        let f = classify_build_failure(out, false);
        assert_eq!(f.kind, BuildFailureKind::Contention);
        assert!(f.diagnostics.is_empty(), "evidence must show zero: {:?}", f.diagnostics);
    }

    /// Corruption markers must be anchored to diagnostic lines. A test *named*
    /// after E0460 previously classified the whole run as `Corruption`.
    #[test]
    fn corruption_markers_are_anchored_to_diagnostics() {
        let real = "test metadata::e0460_stale_rlib_is_corruption ... ok\n\
                    src/a.rs:2:2: error[E0308]: mismatched types";
        assert_eq!(kind(real), BuildFailureKind::Real);

        let corrupt = "src/lib.rs:1:1: error[E0460]: found possibly newer version of crate `windows`";
        assert_eq!(kind(corrupt), BuildFailureKind::Corruption);

        let stub = "error: only metadata stub found for `rlib` dependency `std`\n --> src/lib.rs:1:1";
        assert_eq!(kind(stub), BuildFailureKind::Corruption);
    }

    /// ENOSPC is the one proven cause in the corpus, and it poisons the target
    /// directory for the *next* build. It must be reachable on a cargo error
    /// line, not only on a source diagnostic.
    #[test]
    fn enospc_is_corruption() {
        let out = "error: couldn't create a temp dir: There is not enough space \
                   on the disk. (os error 112)";
        assert_eq!(kind(out), BuildFailureKind::Corruption);
        // ...but the same string inside test output is not.
        assert_eq!(
            kind("test disk::reports_os_error_112 ... ok"),
            BuildFailureKind::Contention,
        );
    }

    /// A truncated capture is indistinguishable from a no-diagnostic build.
    /// `Real` is the safe direction: it costs an investigation, where a wrong
    /// `Contention` costs an edit to working code.
    #[test]
    fn truncated_capture_without_diagnostics_is_real() {
        let out = "error: could not compile `glob` (lib)";
        assert_eq!(classify_build_failure(out, true).kind, BuildFailureKind::Real);
        assert_eq!(classify_build_failure(out, false).kind, BuildFailureKind::Contention);
    }

    #[test]
    fn empty_output_is_contention_with_no_evidence() {
        let f = classify_build_failure("", false);
        assert_eq!(f.kind, BuildFailureKind::Contention);
        assert!(f.diagnostics.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli-ci --lib build_failure`
Expected: FAIL to compile — `classify_build_failure`, `BuildFailure`, and
`BuildFailureKind` are not defined.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/vox-cli-ci/src/build_failure.rs`, above the test module:

```rust
//! Classifies a failed build's output, and returns the evidence for the verdict.
//!
//! Two failure classes on this host are not code defects, and both have been
//! misdiagnosed as such: a compiler that exits nonzero emitting no diagnostic
//! (memory pressure — the host idles at 1.6 GB free with a 1,193 MB peak rustc
//! working set), and stale or truncated artifacts left behind by an earlier
//! disk-full run, whose errors surface in the *next* build.
//!
//! The rule that matters: **`Real` outranks `Contention`**, and "real" is
//! decided by whether the output contains source diagnostics in *any* cargo
//! message format. A previous version tested `line.starts_with("error[")`,
//! which is false for `--message-format short` — the format this repo uses —
//! and so excused every genuine compile error as contention.

/// What a failed build's output actually indicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFailureKind {
    /// No diagnostics at all: the compiler died without saying why. Retry on a
    /// quiet host; check free RAM. Do not edit code.
    Contention,
    /// Stale or truncated artifacts, usually from an earlier disk-full run.
    /// Check free disk, then `cargo clean -p` the named crates.
    Corruption,
    /// A genuine compile error. Behave normally.
    Real,
}

/// A verdict plus the diagnostics that produced it.
///
/// The evidence is the point: a caller prints "classified as contention: 0
/// source diagnostics found" and a human overrules a wrong verdict at a glance.
/// A bare enum makes a misclassification invisible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFailure {
    pub kind: BuildFailureKind,
    pub diagnostics: Vec<String>,
}

/// Markers of stale or truncated artifacts. Only honored on diagnostic lines.
const CORRUPTION_MARKERS: &[&str] = &[
    "E0460",
    "only metadata stub found for",
    "found invalid metadata files for crate",
];

/// ENOSPC. Honored on any line that is itself an error line.
const DISK_FULL_MARKERS: &[&str] = &["os error 112", "not enough space on the disk"];

/// Cargo's own summary lines. They name no source location and carry no error
/// code, so they are not evidence of a code defect.
const CARGO_SUMMARIES: &[&str] = &[
    "could not compile",
    "build failed",
    "aborting due to",
    "failed to run custom build command",
    "linking with",
    "process didn't exit successfully",
];

/// Classify the output of a failed build.
///
/// `truncated` says whether the capture was cut short. A truncated capture with
/// no diagnostics classifies `Real`, because it is indistinguishable from a
/// build whose diagnostics were simply not captured.
pub fn classify_build_failure(output: &str, truncated: bool) -> BuildFailure {
    let diagnostics = source_diagnostics(output);

    if truncated && diagnostics.is_empty() {
        return BuildFailure { kind: BuildFailureKind::Real, diagnostics };
    }

    let disk_full = output
        .lines()
        .any(|l| is_error_line(l) && DISK_FULL_MARKERS.iter().any(|m| l.contains(m)));
    let stale_artifacts = diagnostics
        .iter()
        .any(|d| CORRUPTION_MARKERS.iter().any(|m| d.contains(m)));
    if disk_full || stale_artifacts {
        return BuildFailure { kind: BuildFailureKind::Corruption, diagnostics };
    }

    // Real before contention: one genuine error anywhere outranks contention
    // anywhere. Contention is the fallback and needs no marker list.
    let kind = if diagnostics.is_empty() {
        BuildFailureKind::Contention
    } else {
        BuildFailureKind::Real
    };
    BuildFailure { kind, diagnostics }
}

/// Every source diagnostic in the output, in any cargo message format.
fn source_diagnostics(output: &str) -> Vec<String> {
    let lines: Vec<&str> = output.lines().collect();
    let mut found = Vec::new();

    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim_end();
        let t = line.trim_start();
        if t.is_empty() {
            continue;
        }

        // `--message-format json`.
        let json = t.starts_with('{') && t.contains("\"level\":\"error\"");
        // Any format, coded: `error[E0610]`, with or without a path prefix.
        let coded = t.contains("error[E");
        // `--message-format short`: `path:line:col: error…`.
        let short = is_short_format_diagnostic(t);
        // Full format, uncoded: `error: msg` with a `-->` location beneath it.
        let full_uncoded = t.starts_with("error: ")
            && !CARGO_SUMMARIES
                .iter()
                .any(|s| t["error: ".len()..].starts_with(s))
            && lines[i + 1..]
                .iter()
                .find(|n| !n.trim().is_empty())
                .is_some_and(|n| n.trim_start().starts_with("--> "));

        if json || coded || short || full_uncoded {
            found.push(line.to_string());
        }
    }
    found
}

/// `path:line:col: error…` — the short format's shape.
fn is_short_format_diagnostic(t: &str) -> bool {
    let Some(idx) = t.find(": error") else {
        return false;
    };
    // The prefix must end in `:<digits>:<digits>`; that is what separates a
    // diagnostic from prose that happens to contain the word "error".
    let mut parts = t[..idx].rsplit(':');
    let col = parts.next().unwrap_or_default();
    let line = parts.next().unwrap_or_default();
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    numeric(col) && numeric(line)
}

/// True for a line that is itself an error report, as opposed to prose or test
/// output that merely quotes one. Anchoring on this is what stops a test *named*
/// after an error code from tripping the corruption markers.
fn is_error_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("error") || is_short_format_diagnostic(t)
}
```

Add to `crates/vox-cli-ci/src/lib.rs`, alongside the other `pub mod` lines
(keep them alphabetical):

```rust
pub mod build_failure;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli-ci --lib build_failure`
Expected: PASS, 9 tests.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p vox-cli-ci --all-targets -- -D warnings
cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/build_failure.rs crates/vox-cli-ci/src/lib.rs
git commit -m "feat(ci): format-agnostic build failure classifier with evidence"
```

---

### Task 2: Cap agent build parallelism with `CARGO_BUILD_JOBS`

One line, agent-scoped, no shared state. This replaces the rejected broker
entirely: nothing to start, nothing to keep alive, nothing to leak.

**Files:**
- Modify: `.claude/settings.json` (add a top-level `env` key)
- Modify: `docs/src/architecture/build-time-log.md` (record the value and its basis)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing any other task reads.

- [ ] **Step 1: Add the env var**

`.claude/settings.json` currently has exactly one top-level key, `hooks`. Add
`env` beside it:

```json
{
  "env": {
    "CARGO_BUILD_JOBS": "4"
  },
  "hooks": { }
}
```

(Keep the existing `hooks` object exactly as it is — the snippet above elides
its contents.)

**Deliberately no SessionStart hook.** There is nothing to start. Note for
anyone tempted to add one later: hooks belong in the **inner**
`SessionStart[0].hooks` array, not the outer one, and `vox ci …` commands are
freshness-gated — a stale installed `vox` turns a SessionStart hook into a
silent no-op.

- [ ] **Step 2: Verify it takes effect**

In a fresh agent shell:

```bash
echo "$CARGO_BUILD_JOBS"
cargo check -p vox-bounded-fs
```

Expected: prints `4`, and the build succeeds. A human shell outside the harness
prints nothing and is unaffected — that is the intent.

- [ ] **Step 3: Record the basis for the number**

Add to `docs/src/architecture/build-time-log.md` a short section stating:
agent shells cap at `CARGO_BUILD_JOBS=4`; the basis is memory, not cores
(15.7 GB physical / 1.6 GB free at idle / 1,193 MB peak single-rustc working
set → 4 × 1.2 GB ≈ 4.8 GB, inside the reclaimable headroom); a core-derived 12
would imply ~14 GB of resident compilers on a 15.7 GB host; re-derive as
`available_bytes / peak_rustc_working_set`, never from `nproc`. Link the spec.

- [ ] **Step 4: Commit**

```bash
git add .claude/settings.json docs/src/architecture/build-time-log.md
git commit -m "perf(build): cap agent cargo jobs at 4 (memory-derived)"
```

---

### Task 3: Free-disk and free-RAM preflight in `vox doctor`

ENOSPC is the only proven cause in the failure corpus and nothing watches it. A
full disk does not merely fail a build; it poisons the target directory so the
*next* build reports a metadata error, which is how a disk problem gets
misdiagnosed as a compiler problem.

**Files:**
- Modify: `crates/vox-cli/Cargo.toml` (add `fs2`)
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs`

**Interfaces:**
- Consumes: `vox_cli_ci::repo_root()` (already re-exported as
  `crate::commands::ci::repo_root`). **Do not** expect a root parameter at the
  call site: `linker_health` is invoked at `build_health.rs:622` inside
  `run(auto_heal, checks)`, which has no root in scope.
- Produces: `pub(crate) fn resource_severity(free: u64, warn_at: u64, fail_at: u64) -> &'static str`
  and `pub(crate) async fn resource_preflight(checks: &mut Vec<Check>)`, plus two
  new entries in `KNOWN_DIAGNOSIS_IDS` and a new `DiagCheckKind::Resources`.

- [ ] **Step 1: Write the failing test**

Append to the `mod tests` block at the bottom of `build_health.rs`:

```rust
    #[test]
    fn resource_severity_brackets_the_thresholds() {
        const GB: u64 = 1024 * 1024 * 1024;
        // fail_at 5 GB, warn_at 20 GB — the disk thresholds.
        assert_eq!(resource_severity(GB, 20 * GB, 5 * GB), "error");
        assert_eq!(resource_severity(5 * GB, 20 * GB, 5 * GB), "error", "boundary is inclusive");
        assert_eq!(resource_severity(10 * GB, 20 * GB, 5 * GB), "warn");
        assert_eq!(resource_severity(100 * GB, 20 * GB, 5 * GB), "ok");
    }

    /// The corpus failure was `os error 112` with 916 MB free of 476 GB. That
    /// must be an error-severity check, not a warning.
    #[test]
    fn the_observed_enospc_free_space_is_an_error() {
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * MB;
        assert_eq!(resource_severity(916 * MB, 20 * GB, 5 * GB), "error");
    }

    /// This host idles at 1.6 GB free of 15.7 GB with zero rustc running, so
    /// the RAM check is expected to warn at rest. That is the finding.
    #[test]
    fn the_measured_idle_ram_warns() {
        const MB: u64 = 1024 * 1024;
        assert_eq!(resource_severity(1600 * MB, 2048 * MB, 1024 * MB), "warn");
    }

    #[tokio::test]
    async fn resource_preflight_emits_both_checks() {
        let mut checks = Vec::new();
        resource_preflight(&mut checks).await;
        assert_eq!(checks.len(), 2, "one disk check, one memory check");
        assert!(checks.iter().any(|c| c.name.contains("disk")), "{checks:?}");
        assert!(checks.iter().any(|c| c.name.contains("memory")), "{checks:?}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib build_health`
Expected: FAIL to compile — `resource_severity` and `resource_preflight` are not
defined. The existing `every_known_id_maps_to_a_check_kind` test still passes at
this point; Step 3 keeps it passing.

- [ ] **Step 3: Write minimal implementation**

Add `fs2` to `crates/vox-cli/Cargo.toml` under `[dependencies]` (already in
`Cargo.lock` via `vox-build-queue`, so this adds no new supply-chain surface):

```toml
fs2 = "0.4"
```

Add the two ids to `KNOWN_DIAGNOSIS_IDS` in `build_health.rs` (the list at
line 17):

```rust
    "build.disk_low",
    "build.memory_low",
```

Add a variant to `DiagCheckKind` (line 431):

```rust
    Resources,
```

Add the mapping arm to `check_kind_for_diag` (line 442), before the `_ => None`:

```rust
        "build.disk_low" | "build.memory_low" => Some(DiagCheckKind::Resources),
```

Add the dispatch arm to `run_check_for_diag` (line 461):

```rust
        DiagCheckKind::Resources => resource_preflight(checks).await,
```

Append the implementation after `linker_health` (which ends around line 525):

```rust
const GIB: u64 = 1024 * 1024 * 1024;

/// Disk: below 5 GiB a build can hit ENOSPC and poison the target directory;
/// below 20 GiB a full `cargo build --workspace` is not comfortably survivable.
const DISK_FAIL_AT: u64 = 5 * GIB;
const DISK_WARN_AT: u64 = 20 * GIB;

/// Memory: peak single-rustc working set measured at 1,193 MB on this host, so
/// below 1 GiB free even one compiler is over budget.
const MEM_FAIL_AT: u64 = GIB;
const MEM_WARN_AT: u64 = 2 * GIB;

/// Severity for a free-bytes reading. Pure, so the thresholds are testable
/// without depending on whatever the host happens to have free right now.
///
/// Boundaries are inclusive at the failing end: exactly `fail_at` free is not
/// "enough", it is the last reading before there is none.
pub(crate) fn resource_severity(free: u64, warn_at: u64, fail_at: u64) -> &'static str {
    if free <= fail_at {
        "error"
    } else if free <= warn_at {
        "warn"
    } else {
        "ok"
    }
}

/// Free disk on the target-directory volume, and free physical RAM.
///
/// ENOSPC is the one *proven* cause of build failure in the 2026-08-23 corpus
/// (`os error 112`, 916 MB free of 476 GB) and nothing watched it. Its damage
/// outlives the run: cargo leaves truncated `.rmeta` files, and the next build
/// reports a metadata error whose real cause was the previous full disk.
pub(crate) async fn resource_preflight(checks: &mut Vec<Check>) {
    let root = vox_cli_ci::repo_root();
    // The target dir may not exist yet in a fresh worktree; the volume is the
    // same either way, which is all `available_space` needs.
    let probe = if root.join("target").is_dir() {
        root.join("target")
    } else {
        root.clone()
    };

    match fs2::available_space(&probe) {
        Ok(free) => {
            let sev = resource_severity(free, DISK_WARN_AT, DISK_FAIL_AT);
            let detail = format!("{:.1} GiB free on {}", free as f64 / GIB as f64, probe.display());
            checks.push(Check::new(
                "build: disk space",
                sev == "ok",
                if sev == "ok" {
                    detail
                } else {
                    diag(
                        "build.disk_low",
                        sev,
                        &format!(
                            "{detail} — a full disk truncates .rmeta files and the NEXT build \
                             reports a metadata error instead"
                        ),
                        "cargo clean, or vox ci workspace-artifacts --prune",
                        false,
                    )
                },
            ));
        }
        Err(e) => checks.push(Check::fail(
            "build: disk space",
            diag(
                "build.disk_low",
                "warn",
                &format!("could not read free space on {}: {e}", probe.display()),
                "check the path is on a mounted volume",
                false,
            ),
        )),
    }

    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let free = sys.available_memory();
    let jobs = std::env::var("CARGO_BUILD_JOBS").unwrap_or_else(|_| "unset".to_string());
    let sev = resource_severity(free, MEM_WARN_AT, MEM_FAIL_AT);
    // Peak single-rustc working set here is 1,193 MB, so report the ratio: it
    // is what makes CARGO_BUILD_JOBS a judgeable number rather than a guess.
    let detail = format!(
        "{:.1} GiB free, CARGO_BUILD_JOBS={jobs} (peak rustc working set ~1.2 GiB)",
        free as f64 / GIB as f64
    );
    checks.push(Check::new(
        "build: memory headroom",
        sev == "ok",
        if sev == "ok" {
            detail
        } else {
            diag(
                "build.memory_low",
                sev,
                &format!("{detail} — low memory makes rustc exit nonzero with no diagnostic"),
                "close other builds, or lower CARGO_BUILD_JOBS in .claude/settings.json",
                false,
            )
        },
    ));
}
```

Register it in `run` (line 617), immediately after `linker_health(checks).await;`:

```rust
    resource_preflight(checks).await;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib build_health`
Expected: PASS, including the pre-existing
`every_known_id_maps_to_a_check_kind` test (which now covers the two new ids)
and the 4 new tests.

- [ ] **Step 5: Verify end to end**

```bash
cargo run -q -p vox-cli -- doctor 2>&1 | grep -Ei "disk space|memory headroom"
```

Expected: two rows. On this host the memory row is expected to warn at rest —
1.6 GB free of 15.7 GB is the measured idle state, and surfacing it is the
point of the check.

- [ ] **Step 6: Lint, format, commit**

```bash
cargo clippy -p vox-cli --all-targets -- -D warnings
cargo fmt -p vox-cli
git add crates/vox-cli/Cargo.toml Cargo.lock crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/build_health.rs
git commit -m "feat(doctor): free-disk and free-RAM preflight for builds"
```

- [ ] **Step 7: Confirm no SSOT drift**

```bash
cargo run -q -p vox-cli -- ci ssot-drift
```

Expected: clean. No `CiCmd` variant was added, so
`contracts/reports/gui-surface-coverage.v1.json` and
`crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt` are
unchanged; no `unsafe` was added, so
`contracts/reports/safety-inventory/baseline.v1.json` is unchanged. If this step
reports drift, something in the plan was exceeded — fix the source, do not
regenerate a baseline.

---

## Self-Review

**Spec coverage.** §1 Failure classifier → Task 1, including every named
required test case (short/full/JSON formats, mixed-run ordering, anchored
corruption markers, truncation guard, zero-diagnostic evidence). §2
`CARGO_BUILD_JOBS = 4` → Task 2. §3 Disk and RAM preflight → Task 3, with the
thresholds the spec states (disk fail 5 GB / warn 20 GB; RAM fail 1 GB / warn
2 GB). *Rejected: the token broker* needs no task by construction — the check is
that no task here creates a semaphore, a resident process, or a
`CARGO_MAKEFLAGS` value. Appendices A and B are historical record, not work.

**No task exists for the broker, `CARGO_MAKEFLAGS`, or a jobserver doctor
check.** That is deliberate and is the main structural change from the previous
revision of this plan.

**Plan-conformance blockers, all resolved:** Task 3 takes no root parameter and
calls `vox_cli_ci::repo_root()` instead, because the `linker_health` call site
at `build_health.rs:622` has no root in scope. No `CiCmd` variant is added, so
neither `gui-surface-coverage.v1.json` nor
`command_catalog_paths_baseline.txt` needs regeneration (Step 7 confirms). No
`unsafe` is added, so the safety inventory stays at 501.
`cli-command-surface.generated.md` derives from the command registry, not the
clap tree, and is unaffected. The `vox-cli → vox-cli-ci` edge already exists.

**Type consistency.** `BuildFailure`/`BuildFailureKind`/`classify_build_failure`
(Task 1) are consumed by no other task, as stated. `resource_severity` takes
`(free, warn_at, fail_at)` in that order in both its tests and its call sites.
`Check::new`/`fail` and the `diag(id, severity, root_cause, fix, auto_healable)`
helper match the existing signatures in `build_health.rs` and `common.rs`. Free
bytes are `u64` throughout.

**Known follow-up, out of scope:** nothing consumes `classify_build_failure`
yet. The natural consumer is the agent-facing cargo-check path at
`crates/vox-orchestrator-mcp/src/compiler_tools.rs:216` and `:958` — the same
code whose `--message-format=short` output the old classifier was blind to — but
wiring it there needs a `vox-orchestrator-mcp → vox-cli-ci` crate edge, which is
a user-authorized ledger decision. Land the tested pure function first.
