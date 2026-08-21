# Release Optimization & Verification Implementation Plan (Phase 1a)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every released Vox binary actually build at `[profile.dist]` optimization, prove it is tested at that optimization, and permanently gate the bundle-matrix drift that currently ships a phantom bundle.

**Architecture:** Three independent fixes to existing code, no new crates. (1) `release_build.rs` switches its cargo invocation and output path from `release` to `dist`. (2) A new `[profile.dist-test]` inherits `dist` but restores `panic = "unwind"` so the test harness can run under fat LTO, paired with a subprocess-driven black-box suite that exercises the real `panic = "abort"` binary. (3) A parity test copies the existing `release_binaries_workflow_matrix_matches_ssot` pattern to assert the `bundle-release.yml` matrix equals the `[[bundle]]` id set in `catalog.toml`.

**Tech Stack:** Rust 1.96.0, `cargo`/`cargo-nextest`, `anyhow`, `serde_yaml`, `toml`, GitHub Actions.

**Spec:** [`docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`](../specs/2026-08-20-vox-distribution-system-design.md)

## Global Constraints

- Rust toolchain is pinned to **1.96.0** (`contracts/toolchain/workspace-toolchain.v1.yaml`, `rust-toolchain.toml`). Do not bump it.
- **Never run `cargo fmt --all`** on this workspace — it overflows the Windows `CreateProcess` command-line limit (`os error 206`). Format with `vox run scripts/fmt.vox`, or a single crate with `cargo fmt -p <crate>`.
- **Do not add a workspace crate-to-crate dependency edge.** `vox ci crate-edges` gates the exact edge set. If a task seems to need one, duplicate the helper under ~50 lines with a `// vox:defactored-from <crate> <date>` comment instead. Reading a file from disk in a test is not an edge and is fine.
- **Test-first is binding.** Every new `pub fn` in `crates/*/src/**` needs a `#[test]` in the same file before the commit lands (`skeleton/untested-pub-api`, enforced by the `tdd-guard` pre-commit hook).
- Run `vox ci pre-push --complete` before pushing Rust changes — the default `fast` tier does **not** run clippy.
- Batch commits and push once when the PR is review-ready; request re-review by commenting `@coderabbitai review`, never by re-pushing.
- Existing profile values, copied verbatim from `Cargo.toml`, must not change: `[profile.dist]` is `inherits = "release"`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"`.

---

### Task 1: Build release artifacts at `[profile.dist]`

`[profile.dist]` is defined in `Cargo.toml` but nothing uses it. `release_build.rs` passes `--release` and then reads the binary out of `target/<triple>/release/`. Both must move together — changing only the flag would leave the code reading a path cargo no longer writes to.

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs:150` (the `"--release"` argument) and `crates/vox-cli/src/commands/ci/release_build.rs:174` (the `.join("release")` path segment)
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (existing `#[cfg(test)] mod tests` at the bottom of the same file)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub(crate) const DIST_PROFILE: &str = "dist";` in `crates/vox-cli/src/commands/ci/release_build.rs`. Task 2 and Task 3 both reference this constant by name rather than restating the literal.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
#[test]
fn dist_profile_constant_is_dist() {
    assert_eq!(super::DIST_PROFILE, "dist");
}

#[test]
fn release_build_source_uses_dist_profile_not_release() {
    // Guards the F1 regression: shipping thin-LTO binaries because the build
    // invocation said `--release` while Cargo.toml defined `[profile.dist]`.
    //
    // NOTE: this reads its own file, so the needles below MUST stay escaped as
    // `\"--release\"` and `.join(\"release\")`. Written that way the source
    // bytes are `"` `\` `"` `-` `-` ... , which does not contain the needle —
    // so the test does not match itself. Rewriting either needle as a raw
    // string (`r#"--release"#`) would make this test fail against itself.
    let src = include_str!("release_build.rs");
    assert!(
        !src.contains("\"--release\""),
        "release_build.rs must not pass `--release`; use `--profile dist` so \
         [profile.dist] (fat LTO, codegen-units=1, strip=symbols, panic=abort) applies"
    );
    assert!(
        !src.contains(".join(\"release\")"),
        "release_build.rs must read built binaries from target/<triple>/dist/, \
         not target/<triple>/release/"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib release_build::tests -- --nocapture`

Expected: FAIL — `dist_profile_constant_is_dist` fails to compile with `cannot find value 'DIST_PROFILE' in module 'super'`, and `release_build_source_uses_dist_profile_not_release` fails both assertions.

- [ ] **Step 3: Write minimal implementation**

Near the top of `crates/vox-cli/src/commands/ci/release_build.rs`, just below the `pub use crate::utils::install_policy::SUPPORTED_RELEASE_TARGETS;` line, add:

```rust
/// Cargo profile used for every shipped artifact.
///
/// `[profile.dist]` in the workspace `Cargo.toml` sets `lto = "fat"`,
/// `codegen-units = 1`, `strip = "symbols"`, and `panic = "abort"`. Building
/// with plain `--release` silently ships thin-LTO, unstripped, unwinding
/// binaries — see spec finding F1.
pub(crate) const DIST_PROFILE: &str = "dist";
```

In `build_and_package_binary`, replace the `"--release",` element of the `cmd.args([...])` call with two elements:

```rust
    cmd.current_dir(repo_root).args([
        "build",
        "-p",
        package_name,
        "--profile",
        DIST_PROFILE,
        "--locked",
        "--target",
        target,
    ]);
```

Then replace the output path lookup so it reads from the `dist` directory:

```rust
    let built_binary = repo_root
        .join("target")
        .join(target)
        .join(DIST_PROFILE)
        .join(built_bin_name);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib release_build::tests -- --nocapture`

Expected: PASS — all tests in the module green, including the two new ones.

- [ ] **Step 5: Verify a real artifact actually builds and is stripped**

Run: `cargo run -q -p vox-cli -- ci release-build --target x86_64-pc-windows-msvc --version v0.0.0-local --out-dir target/dist-smoke --package vox`

Expected: exits 0, and `target/dist-smoke/` contains a `vox-v0.0.0-local-x86_64-pc-windows-msvc.zip`. Substitute your own host triple if not on Windows. This is slow (fat LTO, `codegen-units = 1`) — 10–25 minutes cold is normal and is the point.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): build shipped artifacts with --profile dist, not --release

[profile.dist] (fat LTO, codegen-units=1, strip=symbols, panic=abort) was
defined in Cargo.toml but never used; every released binary was thin-LTO.
Adds a source-level guard test so it cannot regress.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Add `[profile.dist-test]` so the suite can run under fat LTO

`panic = "abort"` makes `[profile.dist]` untestable by the normal harness — `libtest` needs unwinding to catch panics, so `#[should_panic]` tests abort the process instead of passing. A sibling profile that keeps every other `dist` setting but restores unwinding gives fat-LTO and `codegen-units = 1` coverage. Task 3 covers the `panic = "abort"` half.

**Files:**
- Modify: `Cargo.toml` (the profile section, immediately after the existing `[profile.dist]` block)
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (same `#[cfg(test)] mod tests` block)

**Interfaces:**
- Consumes: `DIST_PROFILE` from Task 1.
- Produces: a `dist-test` cargo profile, referenced by name in the CI workflow added in Task 3 Step 5.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
#[test]
fn dist_test_profile_inherits_dist_but_unwinds() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"),
    )
    .expect("read workspace Cargo.toml");

    let v: toml::Value = manifest.parse().expect("workspace Cargo.toml must parse");
    let p = v
        .get("profile")
        .and_then(|p| p.get("dist-test"))
        .expect("[profile.dist-test] must exist so the suite can run under fat LTO");

    assert_eq!(
        p.get("inherits").and_then(|x| x.as_str()),
        Some("dist"),
        "dist-test must inherit dist so it tracks any future dist change"
    );
    assert_eq!(
        p.get("panic").and_then(|x| x.as_str()),
        Some("unwind"),
        "dist-test must restore unwinding; libtest cannot catch panics under abort"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib dist_test_profile_inherits_dist_but_unwinds -- --nocapture`

Expected: FAIL with `[profile.dist-test] must exist so the suite can run under fat LTO`.

- [ ] **Step 3: Write minimal implementation**

In the workspace root `Cargo.toml`, directly after the existing `[profile.dist]` block, add:

```toml
# Test lane for the shipped optimization level. Inherits everything from
# `dist` (fat LTO, codegen-units = 1, strip = "symbols") but restores
# unwinding, because libtest relies on catching panics: under
# `panic = "abort"` a `#[should_panic]` test aborts the process instead of
# passing, so the harness cannot run at all. The `panic = "abort"` half of
# `dist` is covered separately by the black-box subprocess suite in
# crates/vox-cli/tests/dist_binary_e2e.rs.
[profile.dist-test]
inherits = "dist"
panic = "unwind"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib dist_test_profile_inherits_dist_but_unwinds -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "feat(release): add [profile.dist-test] for ship-optimization testing

Inherits dist (fat LTO, codegen-units=1) but restores panic=unwind so
libtest can run; panic=abort is covered by the black-box suite instead.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Black-box E2E suite against the real `dist` binary

This is the only lane that exercises `panic = "abort"` as users receive it. It drives the built binary as a subprocess, so no test harness is compiled into it.

**Files:**
- Create: `crates/vox-cli/tests/dist_binary_e2e.rs`
- Create: `.github/workflows/dist-verify.yml`

**Interfaces:**
- Consumes: `DIST_PROFILE` (Task 1) — the binary path this suite probes is `target/<profile>/vox`, with `dist` as the profile. The `[profile.dist-test]` name (Task 2) is used by the workflow in Step 5.
- Produces: nothing later tasks consume.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/tests/dist_binary_e2e.rs`:

```rust
//! Black-box verification of the shipped `dist`-profile binary.
//!
//! Runs the binary as a subprocess, so nothing here depends on a test harness
//! being linked into it. This is the ONLY lane that exercises
//! `panic = "abort"` exactly as users receive it — see spec finding F2.
//!
//! Skips (rather than fails) when the dist binary is absent, so a normal
//! `cargo test` run does not force a 20-minute fat-LTO build. CI builds it
//! first; see .github/workflows/dist-verify.yml.

use std::path::PathBuf;
use std::process::Command;

fn dist_binary() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/dist")
        .join(exe);
    p.exists().then_some(p)
}

/// Returns (stdout, stderr, exit_code), or `None` if the dist binary is absent.
fn run_dist(args: &[&str]) -> Option<(String, String, i32)> {
    let bin = dist_binary()?;
    let out = Command::new(&bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn {} {:?}: {e}", bin.display(), args));
    Some((
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    ))
}

#[test]
fn dist_binary_reports_semver_version() {
    let Some((stdout, _, code)) = run_dist(&["--version"]) else {
        eprintln!("SKIP: target/dist/vox not built");
        return;
    };
    assert_eq!(code, 0, "`vox --version` must exit 0");
    let has_semver = stdout
        .split_whitespace()
        .any(|t| t.trim_start_matches('v').split('.').count() >= 3);
    assert!(has_semver, "`vox --version` must print a semver, got: {stdout:?}");
}

#[test]
fn dist_binary_help_exits_zero() {
    let Some((stdout, _, code)) = run_dist(&["--help"]) else {
        eprintln!("SKIP: target/dist/vox not built");
        return;
    };
    assert_eq!(code, 0, "`vox --help` must exit 0");
    assert!(stdout.contains("vox"), "help output must mention vox");
}

#[test]
fn dist_binary_rejects_unknown_subcommand_without_aborting() {
    // Under panic = "abort" a panicking error path terminates via SIGABRT
    // (134 on Unix) or STATUS_BREAKPOINT on Windows, instead of returning a
    // clean non-zero exit. This asserts the error path is a real Result, not
    // a panic — a distinction that ONLY shows up in an abort build.
    let Some((_, _, code)) = run_dist(&["definitely-not-a-real-subcommand"]) else {
        eprintln!("SKIP: target/dist/vox not built");
        return;
    };
    assert_ne!(code, 0, "unknown subcommand must fail");
    assert!(
        (0..=99).contains(&code),
        "unknown subcommand must exit with a normal error code, not an abort; got {code}"
    );
}

#[test]
fn dist_binary_compiles_and_runs_a_golden_program() {
    let Some(bin) = dist_binary() else {
        eprintln!("SKIP: target/dist/vox not built");
        return;
    };
    let dir = std::env::temp_dir().join("vox-dist-e2e");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let src = dir.join("hello.vox");
    std::fs::write(&src, "fn main() {\n    print(\"dist-ok\")\n}\n").expect("write hello.vox");

    let out = Command::new(&bin)
        .args(["run", "--interp"])
        .arg(&src)
        .output()
        .expect("spawn vox run");

    assert!(
        out.status.success(),
        "`vox run --interp hello.vox` failed: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("dist-ok"),
        "golden program output missing"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

First confirm the suite compiles and skips cleanly with no dist binary present:

Run: `cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: PASS with four `SKIP: target/dist/vox not built` lines. That is the correct pre-build state — it proves the harness and skip logic work.

Now build the dist binary so the tests actually execute:

Run: `cargo build -p vox-cli --profile dist --features heavy-retrieval`

Then re-run: `cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: FAIL if any real defect exists in the abort-profile binary. If all four pass immediately, that is a valid outcome — the tests are regression guards for a build configuration that has never been exercised, and their value is standing coverage, not a red-to-green transition.

- [ ] **Step 3: Write minimal implementation**

No production code change is required if Step 2's tests pass against the built binary. If `dist_binary_rejects_unknown_subcommand_without_aborting` fails with an abort-range exit code, the cause is a `panic!`/`unwrap()` on the argument-parsing error path in `crates/vox-cli/src/cli_dispatch/mod.rs`; convert that specific call site to return `anyhow::Result` rather than panicking, and re-run.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: PASS, four tests, no SKIP lines.

- [ ] **Step 5: Add the CI lane**

Create `.github/workflows/dist-verify.yml`:

```yaml
# Verifies the SHIPPED optimization level, not the dev one.
#
# Two lanes, both required, because neither alone is sufficient (spec F2):
#   - dist-test profile: fat LTO + codegen-units=1 with panic=unwind, so
#     libtest can run the suite at all.
#   - black-box: drives the real panic=abort binary as a subprocess.
name: dist-verify

on:
  push:
    tags:
      - "v*"
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: dist-verify-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

jobs:
  dist-verify:
    name: dist verification
    runs-on: [self-hosted, linux, x64, docker]
    # Fat LTO with codegen-units=1 across the workspace is slow by design.
    timeout-minutes: 180
    steps:
      - uses: actions/checkout@v7

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Run suite under dist-test (fat LTO, codegen-units=1)
        run: cargo test -p vox-cli --profile dist-test --locked

      - name: Build the real dist binary (panic=abort)
        run: cargo build -p vox-cli --profile dist --locked --features heavy-retrieval

      - name: Black-box E2E against the dist binary
        run: cargo test -p vox-cli --test dist_binary_e2e --locked -- --nocapture
```

Verify the workflow parses before committing:

Run: `cargo run -q -p vox-cli -- ci workflow-concurrency-guard`

Expected: exits 0. The `concurrency:` block above satisfies the guard's requirement for `cancel-in-progress: true`. The `runs-on` is self-hosted, so `vox ci runner-policy-check` needs no exception entry.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/tests/dist_binary_e2e.rs .github/workflows/dist-verify.yml
git commit -m "test(release): verify the shipped dist-profile binary end to end

Adds a black-box subprocess suite that exercises panic=abort as users get
it, plus a dist-verify workflow running both the dist-test profile suite
and the black-box lane on every v* tag.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Gate the bundle-matrix drift

`bundle-release.yml` builds `vox-cloud-only`, which no longer exists in `catalog.toml`, and never builds the real `vox-ml-metal` or `vox-mobile`. This copies the shape of the existing `release_binaries_workflow_matrix_matches_ssot` test in the same file, which already reads a workflow YAML and diffs it against an SSOT.

**Files:**
- Modify: `.github/workflows/bundle-release.yml:29-38` (the `bundle:` matrix list)
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (same `#[cfg(test)] mod tests` block)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `fn catalog_bundle_ids(catalog_toml: &str) -> Vec<String>` and `fn workflow_matrix_bundle_ids(yml: &str) -> Vec<String>`, both private to the `tests` module. No later task consumes them.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
/// Every `id` under a `[[bundle]]` table in the plugin catalog, sorted.
fn catalog_bundle_ids(catalog_toml: &str) -> Vec<String> {
    let v: toml::Value = catalog_toml.parse().expect("catalog.toml must parse");
    let mut ids: Vec<String> = v
        .get("bundle")
        .and_then(|b| b.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("id")?.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

/// The `bundle:` matrix entries in bundle-release.yml, sorted.
fn workflow_matrix_bundle_ids(yml: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut in_bundle_list = false;
    for line in yml.lines() {
        let t = line.trim();
        if t == "bundle:" {
            in_bundle_list = true;
            continue;
        }
        if in_bundle_list {
            match t.strip_prefix("- ") {
                Some(id) => ids.push(id.trim().to_string()),
                // First non-item line ends the list (e.g. `target:`).
                None if !t.is_empty() => break,
                None => {}
            }
        }
    }
    ids.sort();
    ids
}

#[test]
fn workflow_matrix_bundle_ids_parses_a_simple_list() {
    let yml = "        bundle:\n          - vox-base\n          - vox-dev\n        target:\n          - x86_64\n";
    assert_eq!(workflow_matrix_bundle_ids(yml), vec!["vox-base", "vox-dev"]);
}

#[test]
fn catalog_bundle_ids_reads_bundle_tables() {
    let toml_src = "[[bundle]]\nid = \"vox-base\"\n\n[[bundle]]\nid = \"vox-dev\"\n";
    assert_eq!(catalog_bundle_ids(toml_src), vec!["vox-base", "vox-dev"]);
}

#[test]
fn bundle_release_matrix_matches_plugin_catalog() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog = std::fs::read_to_string(root.join("crates/vox-plugin-catalog/catalog.toml"))
        .expect("read catalog.toml");
    let wf = std::fs::read_to_string(root.join(".github/workflows/bundle-release.yml"))
        .expect("read bundle-release.yml");

    let expected = catalog_bundle_ids(&catalog);
    let actual = workflow_matrix_bundle_ids(&wf);

    assert_eq!(
        actual, expected,
        "bundle-release.yml `bundle:` matrix must exactly match the [[bundle]] \
         ids in crates/vox-plugin-catalog/catalog.toml.\n  \
         only in workflow (phantom, wastes matrix jobs): {:?}\n  \
         only in catalog (never built, never shipped): {:?}",
        actual.iter().filter(|b| !expected.contains(b)).collect::<Vec<_>>(),
        expected.iter().filter(|b| !actual.contains(b)).collect::<Vec<_>>(),
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib bundle_release_matrix_matches_plugin_catalog -- --nocapture`

Expected: FAIL, reporting `only in workflow (phantom...): ["vox-cloud-only"]` and `only in catalog (never built...): ["vox-ml-metal", "vox-mobile"]`.

- [ ] **Step 3: Write minimal implementation**

In `.github/workflows/bundle-release.yml`, replace the `bundle:` matrix list with the catalog's actual nine ids, in sorted order to match the test:

```yaml
        bundle:
          - vox-base
          - vox-dev
          - vox-edge
          - vox-fullstack
          - vox-mesh
          - vox-ml
          - vox-ml-metal
          - vox-mobile
          - vox-server
```

Also update the comment at the top of the file, which currently claims a stale count:

```yaml
# Matrix: 9 bundles x 2 platforms = 18 jobs per run.
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib -- bundle_release --nocapture`

Expected: PASS, all three new tests green.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/bundle-release.yml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): bundle matrix built a phantom and omitted two real bundles

bundle-release.yml built vox-cloud-only, which was removed from
catalog.toml and survives only in superseded plan docs, while never
building vox-ml-metal or vox-mobile. Adds a parity test so the matrix and
the catalog cannot drift again.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Verify the whole phase and push

**Files:** none created or modified.

**Interfaces:**
- Consumes: everything from Tasks 1–4.
- Produces: nothing.

- [ ] **Step 1: Format**

Run: `vox run scripts/fmt.vox`

Expected: exits 0. Do **not** use `cargo fmt --all` (see Global Constraints).

- [ ] **Step 2: Run the complete local gate tier**

Run: `vox ci pre-push --complete`

Expected: exits 0. This tier includes clippy, which the default `fast` tier skips.

- [ ] **Step 3: Confirm no crate-dependency edge was added**

Run: `cargo run -q -p vox-cli -- ci crate-edges`

Expected: exits 0. All four tasks read files from disk in tests only; none adds a workspace edge.

- [ ] **Step 4: Commit any formatting drift and push**

```bash
git add -A
git commit -m "style: rustfmt after dist-profile release verification work

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" || echo "nothing to commit"
git push -u origin claude/vox-distribution-system-f7e4c0
```

- [ ] **Step 5: Open the PR once, review-ready**

```bash
gh pr create --title "fix(release): ship binaries at [profile.dist] and verify them" --body "$(cat <<'PRBODY'
Implements Phase 1a of the Vox distribution system design.

**F1 — every shipped binary was thin-LTO.** `[profile.dist]` (fat LTO,
`codegen-units=1`, `strip=symbols`, `panic=abort`) was defined in
`Cargo.toml` but `release_build.rs` passed `--release` and read from
`target/<triple>/release/`. Both now use `dist`.

**F2 — nothing was tested at ship optimization.** Release smoke was
`--help`/`--version` only, and `panic="abort"` makes the normal harness
unrunnable under `dist`. Adds `[profile.dist-test]` (inherits `dist`,
restores unwinding) plus a black-box subprocess suite that does exercise
`panic="abort"`, both wired into a new `dist-verify` workflow.

**F3 — bundle matrix drift.** `bundle-release.yml` built `vox-cloud-only`,
which no longer exists in `catalog.toml`, and never built `vox-ml-metal`
or `vox-mobile`. Fixed and gated by a parity test.

Spec: `docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
PRBODY
)"
```

Expected: a PR URL. Do not re-push to trigger re-review — comment `@coderabbitai review` instead.

---

## Follow-on plans

- **Phase 1b** — extend `contracts/distribution/profiles.v1.yaml` with `offline_payload` (URL, sha256, SPDX licence, size, per-OS applicability) and add `vox ci gen-installer-manifests` emitting the WiX feature tree, macOS `<choices-outline>`, and `.deb` control fields. Delivers nothing standalone; Phase 2 consumes it.
- **Phase 2** — the installers themselves. Blocked on a Windows signing certificate and a Linux signing decision.
- **Phase 3** — nightly channel, git-cliff as the single changelog source, matrix expansion, build provenance. Unblocked.
- **Phase 4** — GUI auto-update and the managed-install handoff. Blocked on a Tauri updater keypair.
