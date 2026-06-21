# Track B — Release + Nightly Automation Implementation Plan

> **For agentic workers (Gemini 3.5 Flash in Antigravity):** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Read this entire file before starting, then execute **one task at a time**, committing after each.

**Goal:** Make the release pipeline build exactly the binaries the distribution SSOT declares (`vox`, `vox-ml-cli`, `voxup`), support an injected `--version` for nightlies, gate nightly publishes on a green `main`, publish a rolling `nightly` GitHub Release, and surface an "update available" footer in the CLI — each capability landing with its enforcing CI gate.

**Architecture:** The distribution SSOT (`contracts/distribution/profiles.v1.yaml`, key `binaries`) is the single source of truth for *which* binaries ship. `crates/vox-cli/src/commands/ci/release_build.rs` is the builder; today it still references the **deleted** `vox-bootstrap`/`vox-schola` crates and omits `voxup`, so `--package all` is broken. We fix the package set, add a `VOX_VERSION_OVERRIDE` build-time injection path, add a Rust parity test that fails if `ReleasePackage::All` drifts from the SSOT `binaries`, add the `release-nightly.yml` workflow (green-main gate + rolling release), and add a lightweight CLI update-notification footer. The GUI auto-updater (`tauri-plugin-updater`) is **explicitly deferred** (see "Deferred" section) — GUI work is high-risk for this execution target.

**Tech Stack:** Rust (`vox-cli`), GitHub Actions YAML, `gh` CLI, `reqwest` (already a `vox-cli` dep for the footer check), `serde`/`serde_yaml` (already deps).

---

## ⚠️ Gemini-Flash Execution Preamble (READ FIRST — non-negotiable)

You are an implementation agent inside the `vox-foundation/vox` repository in Antigravity. This execution target has documented failure modes (see `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`). The following rules exist **because of those failure modes** — follow them exactly:

1. **Every task ends GREEN + committed.** A kill between tasks must leave a compiling, tested tree. Never split a compile-breaking change across two commits.
2. **Verify-before-use.** Each task that references a symbol/path/signature first has you run an `rg`/read step. The exact signatures are inlined below — but confirm they still match disk before editing. **If disk contradicts this plan, STOP and report. Do NOT invent APIs.**
3. **Self-contained tasks.** Needed code/context is repeated in each task. Do not rely on remembering earlier tasks.
4. **Two-strike circuit breaker.** If a verification command fails twice in a row and you cannot determine why, STOP and write a handoff note. Do NOT loop the same failing action.
5. **One decision per step.** There are no open-ended "design X" steps. If a step feels ambiguous, STOP and report rather than guessing.

### HARD CONSTRAINTS (project policy — violating these fails review)
- **AGENTS.md is normative.** No new `.ps1`/`.sh`/`.py` scripts. The only new non-Rust files allowed by this plan are GitHub Actions YAML workflows.
- **NEVER run `cargo fmt --all`** (banned, CI-enforced). Format ONLY the crate you touched: `cargo fmt -p vox-cli`.
- **On Windows, NEVER pipe `cargo` to `head`/`grep`/`tail`** (it orphans thousands of processes + tens of GB RAM). Redirect to a file: `cargo test -p vox-cli > test-out.txt 2>&1`, then open the file. Scratch files matching `*-out.txt` are git-ignored — never `git add -A` blindly.
- **Integration tests under `tests/`** may name ONLY the crate's public API + dev-dependencies. The parity test in this plan is a **unit test** (`#[cfg(test)] mod tests` inside `release_build.rs`), so it can name `serde_yaml` — but it reads the SSOT via an `include_str!`'d const, not by naming `serde_yaml::Value` in a `tests/` file.
- **Commit after EVERY task** using the exact commit message in that task. Many small commits.
- Commit `Cargo.lock` if a dependency changes (CI runs `--locked`). This plan adds **no** new deps.

### KNOWN-GOOD FACTS (audited 2026-06-19 — trust these, they save a round-trip)
- `crates/vox-bootstrap` and `crates/vox-schola` **do not exist** (deleted). Any `cargo build -p vox-bootstrap` fails with "did not match any packages". This is why `--package all` is currently broken.
- `contracts/distribution/profiles.v1.yaml` top-level key `binaries:` is exactly:
  ```yaml
  binaries:
    - vox
    - vox-ml-cli
    - voxup
  ```
- `crates/vox-cli/src/lib.rs` line ~89 defines `VOX_VERSION` as a `concat!(...)` macro using `env!("CARGO_PKG_VERSION")`, `env!("VOX_BUILD_NUMBER")`, `env!("VOX_GIT_HASH")`.
- `crates/vox-cli/src/commands/ci/release_build.rs` is the builder. `super::cargo_bin()` (defined in `crates/vox-cli/src/commands/ci/mod.rs:98`) returns the cargo binary path.
- `release_artifact_filename(name, version, target)` (imported as `artifact_filename` in tests) produces `{name}-{version}-{target}.{zip|tar.gz}` and keeps the version string verbatim.
- The existing unit test `release_binaries_workflow_matrix_matches_ssot` already enforces the **target** matrix ↔ `SUPPORTED_RELEASE_TARGETS`. You are adding an analogous **package** ↔ `binaries` gate.
- `.github/workflows/release-binaries.yml` line ~42 calls: `cargo run --locked -p vox-cli ci release-build --target ${{ matrix.target }} --version ${{ github.ref_name }} --out-dir dist --package all`. Its line ~40 comment falsely claims `all` produces `vox-bootstrap`/`vox-schola` — that comment is stale and you will correct it.

---

## File Structure (decomposition lock-in)

| File | Responsibility | Action |
|------|----------------|--------|
| `crates/vox-cli/src/commands/ci/release_build.rs` | Build + package release artifacts; `ReleasePackage` enum | **Modify** — remove dead `Bootstrap`/`Both` + `vox-bootstrap` path; add `voxup`; make `All` = SSOT set; add `VOX_VERSION_OVERRIDE` forwarding; add parity test |
| `crates/vox-cli/src/lib.rs` | `VOX_VERSION` constant | **Modify** — add `option_env!("VOX_VERSION_OVERRIDE")` arm |
| `crates/vox-cli/src/commands/updates.rs` | CLI "update available" footer check | **Create** — read latest GitHub release tag, compare, print footer |
| `crates/vox-cli/src/commands/mod.rs` (or the existing module index) | Register `updates` module | **Modify** — `pub mod updates;` |
| `.github/workflows/release-binaries.yml` | Tagged-release build matrix | **Modify** — fix stale comment only (package set now correct in Rust) |
| `.github/workflows/release-nightly.yml` | Scheduled nightly: green-main gate + rolling release | **Create** |
| `docs/superpowers/antigravity-handoff-ledger.md` | Handoff ledger | **Append** AGH-#### entry at the end (Phase 6) |

---

## Phase 1 — Fix the release package set [SEQUENTIAL]

> **Why:** `--package all` currently tries to build the deleted `vox-bootstrap` crate → the entire release pipeline is broken. We delete the dead variants, add `voxup`, and make `All` equal the SSOT set (`vox`, `vox-ml-cli`, `voxup`). This is the keystone — do it first and all in one atomic task so the tree never references a missing crate.

### Task 1: Replace the `ReleasePackage` set and build dispatch

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs`

- [ ] **Step 1: Verify current state on disk**

Run: `rg -n "ReleasePackage|vox-bootstrap|bootstrap_executable_name|want_bootstrap|want_mens|want_vox" crates/vox-cli/src/commands/ci/release_build.rs`

Expected: you see `enum ReleasePackage { Vox, Bootstrap, Both, Mens, All }`, `want_bootstrap`, `bootstrap_executable_name`, and several `vox-bootstrap` literals. If the enum no longer has `Bootstrap`/`Both`, this task is already partly done — STOP and report what differs.

- [ ] **Step 2: Replace the enum definition**

In `crates/vox-cli/src/commands/ci/release_build.rs`, replace the entire enum block:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReleasePackage {
    /// Core `vox` CLI only (lean install — no ML/scientia plugins).
    Vox,
    /// Standalone `vox-bootstrap` installer used by `scripts/install.{sh,ps1}`.
    Bootstrap,
    /// `vox` core + `vox-bootstrap` (legacy "Both" tier — pre-plugin packaging).
    Both,
    /// `vox-ml-cli` plugin: ML/oratio/speech/populi/train subcommands (heavy: Candle).
    Mens,
    /// Every artifact: vox + bootstrap + every plugin binary. The "full" tier.
    All,
}
```

with:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReleasePackage {
    /// Core `vox` CLI only (lean install — no ML/scientia plugins).
    Vox,
    /// `vox-ml-cli` plugin: ML/oratio/speech/populi/train subcommands (heavy: Candle).
    Mens,
    /// `voxup` toolchain multiplexer + hermetic installer.
    Voxup,
    /// Every shipped binary: `vox`, `vox-ml-cli`, `voxup`.
    /// MUST equal `contracts/distribution/profiles.v1.yaml` `binaries` (enforced by
    /// `all_package_matches_distribution_ssot` below).
    All,
}
```

- [ ] **Step 3: Replace the `want_*` flags and build dispatch in `run`**

Replace this block (currently lines ~53–102):

```rust
    let mut checksum_lines = Vec::new();
    let want_vox = matches!(
        package,
        ReleasePackage::Vox | ReleasePackage::Both | ReleasePackage::All
    );
    let want_bootstrap = matches!(
        package,
        ReleasePackage::Bootstrap | ReleasePackage::Both | ReleasePackage::All
    );
    let want_mens = matches!(package, ReleasePackage::Mens | ReleasePackage::All);

    if want_vox {
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-cli",
            executable_name(target),
            "vox",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
    if want_bootstrap {
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-bootstrap",
            bootstrap_executable_name(target),
            "vox-bootstrap",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
    if want_mens {
        let mens_bin = plugin_executable_name(target, "vox-ml-cli");
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-ml-cli",
            &mens_bin,
            "vox-ml-cli",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
```

with:

```rust
    let mut checksum_lines = Vec::new();
    let want_vox = matches!(package, ReleasePackage::Vox | ReleasePackage::All);
    let want_mens = matches!(package, ReleasePackage::Mens | ReleasePackage::All);
    let want_voxup = matches!(package, ReleasePackage::Voxup | ReleasePackage::All);

    if want_vox {
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-cli",
            executable_name(target),
            "vox",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
    if want_mens {
        let mens_bin = plugin_executable_name(target, "vox-ml-cli");
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "vox-ml-cli",
            &mens_bin,
            "vox-ml-cli",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
    if want_voxup {
        let voxup_bin = plugin_executable_name(target, "voxup");
        let artifact_name = build_and_package_binary(
            repo_root,
            out_dir_abs.as_path(),
            target,
            artifact_version,
            "voxup",
            &voxup_bin,
            "voxup",
        )?;
        let digest = sha256_file(&out_dir_abs.join(&artifact_name))?;
        checksum_lines.push(checksum_line(&digest, &artifact_name));
    }
```

- [ ] **Step 4: Delete the now-unused `bootstrap_executable_name` function**

Remove this entire function (currently lines ~130–136):

```rust
fn bootstrap_executable_name(target: &str) -> &'static str {
    if is_windows_target(target) {
        "vox-bootstrap.exe"
    } else {
        "vox-bootstrap"
    }
}
```

- [ ] **Step 5: Fix the test module — remove bootstrap references**

In the `#[cfg(test)] mod tests` block:

(a) Change the import (currently lines ~213–216):
```rust
    use super::{
        bootstrap_executable_name, checksum_line, executable_name, plugin_executable_name,
        validate_release_target,
    };
```
to:
```rust
    use super::{
        checksum_line, executable_name, plugin_executable_name, validate_release_target,
    };
```

(b) In `executable_name_matches_target_family`, delete the two `bootstrap_executable_name(...)` assertions:
```rust
        assert_eq!(
            bootstrap_executable_name("x86_64-pc-windows-msvc"),
            "vox-bootstrap.exe"
        );
        assert_eq!(
            bootstrap_executable_name("x86_64-unknown-linux-gnu"),
            "vox-bootstrap"
        );
```
(Leave the `plugin_executable_name(...)` assertions for `vox-ml-cli` intact, and the `executable_name(...)` assertions intact.)

(c) In `artifact_filename_contract_is_stable`, replace the trailing `vox-bootstrap` assertion:
```rust
        assert_eq!(
            artifact_filename("vox-bootstrap", "v1.2.3", "x86_64-unknown-linux-gnu"),
            "vox-bootstrap-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"
        );
```
with a `voxup` one:
```rust
        assert_eq!(
            artifact_filename("voxup", "v1.2.3", "x86_64-unknown-linux-gnu"),
            "voxup-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"
        );
```

(d) In `checksum_manifest_supports_multiple_entries`, replace the `vox-bootstrap` filename literals with `voxup` (both the input and the expected output):
```rust
    #[test]
    fn checksum_manifest_supports_multiple_entries() {
        let all = [
            checksum_line("aaa", "vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"),
            checksum_line("bbb", "voxup-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"),
        ]
        .join("");
        assert_eq!(
            all,
            "aaa  vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz\nbbb  voxup-v1.2.3-x86_64-unknown-linux-gnu.tar.gz\n"
        );
    }
```

- [ ] **Step 6: Format**

Run: `cargo fmt -p vox-cli`

- [ ] **Step 7: Verify it compiles and tests pass**

Run: `cargo test -p vox-cli --lib commands::ci::release_build > test-out.txt 2>&1`
Then open `test-out.txt`.
Expected: compiles clean; all `release_build` unit tests PASS; **no** reference to `vox-bootstrap` remains. If you see "cannot find function `bootstrap_executable_name`" you missed a test reference — fix it.

- [ ] **Step 8: Confirm no stray bootstrap references remain in the file**

Run: `rg -n "bootstrap|Both|schola" crates/vox-cli/src/commands/ci/release_build.rs`
Expected: **no output** (exit 1). If anything prints, remove it before committing.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): release-build package set = SSOT binaries (drop dead bootstrap, add voxup)"
```

---

## Phase 2 — Version override injection [SEQUENTIAL]

> **Why:** Nightly builds need a synthetic version (`0.6.0-nightly.YYYYMMDD+sha`) baked into the binary. We add a compile-time `VOX_VERSION_OVERRIDE` env hook to `VOX_VERSION`, and forward `--version` into that env when the builder spawns `cargo build`.

### Task 2: Add `VOX_VERSION_OVERRIDE` to the version constant

**Files:**
- Modify: `crates/vox-cli/src/lib.rs`

- [ ] **Step 1: Verify current state**

Run: `rg -n "pub const VOX_VERSION" crates/vox-cli/src/lib.rs`
Expected: one hit, a `concat!(...)` definition near line 89. Read the 12 lines after it.

- [ ] **Step 2: Replace the constant**

Replace:
```rust
/// Build version string: `0.x.y+build.N (githash)`
pub const VOX_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+build.",
    env!("VOX_BUILD_NUMBER"),
    " (",
    env!("VOX_GIT_HASH"),
    ")",
);
```
with:
```rust
/// Build version string. When `VOX_VERSION_OVERRIDE` is set at compile time
/// (release/nightly CI injects it), it wins verbatim; otherwise we synthesize
/// `0.x.y+build.N (githash)` from the build env.
pub const VOX_VERSION: &str = match option_env!("VOX_VERSION_OVERRIDE") {
    Some(v) => v,
    None => concat!(
        env!("CARGO_PKG_VERSION"),
        "+build.",
        env!("VOX_BUILD_NUMBER"),
        " (",
        env!("VOX_GIT_HASH"),
        ")",
    ),
};
```

- [ ] **Step 3: Format**

Run: `cargo fmt -p vox-cli`

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p vox-cli > build-out.txt 2>&1`
Then open `build-out.txt`. Expected: exit 0, no errors. (A `const match` on `Option<&str>` is valid in const context on this toolchain — if the compiler rejects it, STOP and report the exact error.)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/lib.rs
git commit -m "feat(release): VOX_VERSION honors compile-time VOX_VERSION_OVERRIDE"
```

### Task 3: Forward `--version` as `VOX_VERSION_OVERRIDE` into the build

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs`

- [ ] **Step 1: Verify the build spawn site**

Run: `rg -n "fn build_and_package_binary|Command::new\(super::cargo_bin\(\)\)|cmd.args\(\[" crates/vox-cli/src/commands/ci/release_build.rs`
Expected: `build_and_package_binary` defines `cmd` then calls `cmd.current_dir(repo_root).args([...])`. Note it does **not** currently receive `artifact_version` — confirm the signature.

- [ ] **Step 2: Thread `artifact_version` into the env**

`build_and_package_binary` already takes `artifact_version: &str` (it's used for the artifact filename). Add an env injection on the `cmd` right after the `--features heavy-retrieval` block and before `cmd.status()`. Locate:

```rust
    if package_name == "vox-cli" {
        cmd.args(["--features", "heavy-retrieval"]);
    }
    let status = cmd
        .status()
        .with_context(|| format!("spawn cargo build for {package_name} release artifact"))?;
```

Replace with:

```rust
    if package_name == "vox-cli" {
        cmd.args(["--features", "heavy-retrieval"]);
    }
    // Bake the resolved release/nightly version into the binary (read by
    // `VOX_VERSION` via `option_env!`). Without this, `vox --version` on a
    // nightly artifact would print the workspace dev version, not the tag.
    cmd.env("VOX_VERSION_OVERRIDE", artifact_version);
    let status = cmd
        .status()
        .with_context(|| format!("spawn cargo build for {package_name} release artifact"))?;
```

- [ ] **Step 3: Format**

Run: `cargo fmt -p vox-cli`

- [ ] **Step 4: Verify it compiles and tests still pass**

Run: `cargo test -p vox-cli --lib commands::ci::release_build > test-out.txt 2>&1`
Then open `test-out.txt`. Expected: compiles clean, all release_build unit tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "feat(release): forward release-build --version into VOX_VERSION_OVERRIDE"
```

---

## Phase 3 — SSOT ↔ package parity gate [SEQUENTIAL]

> **Why:** The locked program decision (Q5) is that each capability ships with its enforcing gate. This adds a unit test that fails CI if `ReleasePackage::All`'s built set ever drifts from `contracts/distribution/profiles.v1.yaml` `binaries`. It mirrors the existing `release_binaries_workflow_matrix_matches_ssot` target gate.

### Task 4: Expose the `All` package's binary set as testable data

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs`

- [ ] **Step 1: Add a const listing the `All` set, next to the enum**

Immediately **after** the `ReleasePackage` enum definition (from Task 1), add:

```rust
/// The package names `ReleasePackage::All` builds, in archive-name form.
/// This is the parity anchor checked against the distribution SSOT
/// (`contracts/distribution/profiles.v1.yaml` `binaries`).
pub const ALL_RELEASE_BINARIES: &[&str] = &["vox", "vox-ml-cli", "voxup"];
```

- [ ] **Step 2: Add the SSOT embed + parity test**

At the top of the `#[cfg(test)] mod tests` block (just after the existing `use` lines), add the embed const and a parsing-free check. Add this test function inside the `mod tests` block:

```rust
    /// The distribution SSOT, embedded so the parity test needs no file IO at runtime.
    const PROFILES_YAML: &str =
        include_str!("../../../../../contracts/distribution/profiles.v1.yaml");

    #[derive(serde::Deserialize)]
    struct ProfilesBinaries {
        binaries: Vec<String>,
    }

    #[test]
    fn all_package_matches_distribution_ssot() {
        use std::collections::BTreeSet;

        let parsed: ProfilesBinaries =
            serde_yaml::from_str(PROFILES_YAML).expect("distribution SSOT must parse");

        let from_ssot: BTreeSet<String> = parsed.binaries.into_iter().collect();
        let from_code: BTreeSet<String> = super::ALL_RELEASE_BINARIES
            .iter()
            .map(|s| s.to_string())
            .collect();

        assert_eq!(
            from_code, from_ssot,
            "ReleasePackage::All ({:?}) must equal contracts/distribution/profiles.v1.yaml `binaries` ({:?}). \
             If you added/removed a shipped binary, update BOTH the SSOT and ALL_RELEASE_BINARIES + the build dispatch in run().",
            from_code, from_ssot
        );
    }
```

> **Verify the `include_str!` depth before trusting it.** From `crates/vox-cli/src/commands/ci/release_build.rs`, the path to repo root is **five** `../`: `src` → `vox-cli` → `crates` → (root). Wait — count precisely in Step 3 and fix if needed.

- [ ] **Step 3: Verify the `include_str!` path depth**

The file is at `crates/vox-cli/src/commands/ci/release_build.rs`. Directory chain from the file: `ci/` → `commands/` → `src/` → `vox-cli/` → `crates/` → repo root. That is **five** parent hops to reach repo root, then `contracts/...`. So the correct literal is:

```rust
    const PROFILES_YAML: &str =
        include_str!("../../../../../contracts/distribution/profiles.v1.yaml");
```

Confirm by running: `cargo test -p vox-cli --lib all_package_matches_distribution_ssot > test-out.txt 2>&1`
Open `test-out.txt`. If you see `couldn't read ... No such file or directory`, the depth is wrong — adjust the number of `../` until it resolves (try 4 or 6), then re-run. Do NOT proceed until it compiles.

- [ ] **Step 4: Verify the test PASSES (green parity)**

Run: `cargo test -p vox-cli --lib all_package_matches_distribution_ssot > test-out.txt 2>&1`
Open `test-out.txt`. Expected: `test ... all_package_matches_distribution_ssot ... ok`, 1 passed. If it FAILS, the SSOT and `ALL_RELEASE_BINARIES` disagree — reconcile them (the SSOT is `vox`, `vox-ml-cli`, `voxup`).

- [ ] **Step 5: Prove the gate bites (temporary RED, then revert)**

Temporarily edit `ALL_RELEASE_BINARIES` to `&["vox"]`. Run the test again into `test-out.txt`. Expected: it FAILS with the parity message. **Revert** back to `&["vox", "vox-ml-cli", "voxup"]` and re-run to confirm green. (This proves the gate is real, not vacuous. Do not commit the temporary edit.)

- [ ] **Step 6: Format**

Run: `cargo fmt -p vox-cli`

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "test(release): gate ReleasePackage::All against distribution SSOT binaries"
```

---

## Phase 4 — Nightly release workflow [SEQUENTIAL]

> **Why:** A scheduled workflow that (1) refuses to publish if `main`'s HEAD isn't green, (2) builds the SSOT binary set for all targets with an injected nightly version, (3) smoke-tests, and (4) replaces a rolling `nightly` pre-release. Per `docs/superpowers/specs/2026-06-17-nightly-release-pipeline-design.md`.

### Task 5: Create `release-nightly.yml`

**Files:**
- Create: `.github/workflows/release-nightly.yml`

- [ ] **Step 1: Read the existing release workflow for matrix + runner conventions**

Run: `rg -n "target:|runs_on:|runs-on:|actions/checkout|dtolnay/rust-toolchain|release-build|upload" .github/workflows/release-binaries.yml`
Confirm the four targets and their runners (Linux self-hosted; Windows `windows-latest`; macOS x64 + arm64 `macos-latest`). The nightly workflow uses GitHub-hosted `ubuntu-latest` for Linux (per the spec's "GitHub-Hosted Linux Runner" note) for scheduled reliability.

- [ ] **Step 2: Write the workflow**

Create `.github/workflows/release-nightly.yml` with exactly:

```yaml
name: Nightly Release

on:
  schedule:
    # 04:00 UTC daily
    - cron: "0 4 * * *"
  workflow_dispatch: {}

permissions:
  contents: write

concurrency:
  group: nightly-release
  cancel-in-progress: true

jobs:
  gate:
    name: Verify main is green
    runs-on: ubuntu-latest
    outputs:
      sha: ${{ steps.resolve.outputs.sha }}
      nightly_version: ${{ steps.resolve.outputs.nightly_version }}
    steps:
      - uses: actions/checkout@v4
        with:
          ref: main
          fetch-depth: 1
      - name: Resolve HEAD + nightly version
        id: resolve
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          set -euo pipefail
          sha="$(git rev-parse HEAD)"
          short="${sha:0:7}"
          workspace_version="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
          nightly_version="${workspace_version}-nightly.$(date -u +%Y%m%d)+${short}"
          echo "sha=${sha}" >> "$GITHUB_OUTPUT"
          echo "nightly_version=${nightly_version}" >> "$GITHUB_OUTPUT"
          echo "Resolved nightly version: ${nightly_version}"
      - name: Require green combined status on HEAD
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          set -euo pipefail
          sha="${{ steps.resolve.outputs.sha }}"
          state="$(gh api repos/${{ github.repository }}/commits/${sha}/status -q '.state')"
          echo "Combined status for ${sha}: ${state}"
          if [ "${state}" != "success" ]; then
            echo "::error::main@${sha} is '${state}', not 'success' — aborting nightly."
            exit 1
          fi

  build:
    name: Build ${{ matrix.target }}
    needs: gate
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            runs_on: '["ubuntu-latest"]'
          - target: x86_64-pc-windows-msvc
            runs_on: '["windows-latest"]'
          - target: x86_64-apple-darwin
            runs_on: '["macos-latest"]'
          - target: aarch64-apple-darwin
            runs_on: '["macos-latest"]'
    runs-on: ${{ fromJson(matrix.runs_on) }}
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ needs.gate.outputs.sha }}
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build + package nightly artifacts (vox, vox-ml-cli, voxup)
        run: cargo run --locked -p vox-cli ci release-build --target ${{ matrix.target }} --version ${{ needs.gate.outputs.nightly_version }} --out-dir dist --package all
      - name: Smoke test vox (Unix)
        if: runner.os != 'Windows'
        run: |
          set -euo pipefail
          ver="$(./target/${{ matrix.target }}/release/vox --version)"
          echo "vox --version => ${ver}"
          echo "${ver}" | grep -q "nightly"
      - name: Smoke test vox (Windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $ver = & "./target/${{ matrix.target }}/release/vox.exe" --version
          Write-Host "vox --version => $ver"
          if ($ver -notmatch "nightly") { exit 1 }
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: nightly-${{ matrix.target }}
          path: dist/*
          retention-days: 3

  publish:
    name: Publish rolling nightly release
    needs: [gate, build]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ needs.gate.outputs.sha }}
      - name: Download all build artifacts
        uses: actions/download-artifact@v4
        with:
          path: staged
      - name: Flatten + merge checksums
        run: |
          set -euo pipefail
          mkdir -p dist
          # Move every archive into dist/, concatenating per-target checksums.txt.
          : > dist/checksums.txt
          find staged -type f | while read -r f; do
            base="$(basename "$f")"
            if [ "$base" = "checksums.txt" ]; then
              cat "$f" >> dist/checksums.txt
            else
              cp "$f" "dist/${base}"
            fi
          done
          echo "Staged release assets:"
          ls -la dist
      - name: Replace rolling nightly release
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          set -euo pipefail
          sha="${{ needs.gate.outputs.sha }}"
          gh release delete nightly --cleanup-tag --yes || true
          gh release create nightly \
            --prerelease \
            --target "${sha}" \
            --title "Vox Nightly (${{ needs.gate.outputs.nightly_version }})" \
            --notes "Automated nightly for main@${sha} ($(date -u +'%Y-%m-%d %H:%M UTC')). Pre-release; not for production." \
            dist/*
```

- [ ] **Step 3: Lint the YAML for basic validity**

Run: `rg -n "name:|on:|jobs:|runs-on:|needs:" .github/workflows/release-nightly.yml | head -30`
Confirm three jobs (`gate`, `build`, `publish`) and that `build`/`publish` declare `needs:`. There is no Rust compiler check for workflow YAML; correctness is verified when the workflow first runs (or via `workflow_dispatch`).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release-nightly.yml
git commit -m "feat(ci): nightly release workflow (green-main gate + rolling pre-release)"
```

### Task 6: Correct the stale comment in `release-binaries.yml`

**Files:**
- Modify: `.github/workflows/release-binaries.yml`

- [ ] **Step 1: Find the stale comment**

Run: `rg -n "bootstrap|schola|core \+ bootstrap|--package all" .github/workflows/release-binaries.yml`
Expected: a step name like "Build and package release artifacts (core + bootstrap + plugins)" and a comment "`--package all` produces: vox, vox-bootstrap, vox-ml-cli, vox-schola".

- [ ] **Step 2: Replace both with accurate text**

Change the step name to:
```yaml
      - name: Build and package release artifacts (vox + vox-ml-cli + voxup)
```
and the comment to:
```yaml
        # `--package all` produces: vox, vox-ml-cli, voxup (the distribution SSOT `binaries` set).
```
Leave the actual `run:` command unchanged (it already passes `--package all`).

- [ ] **Step 3: Verify no bootstrap/schola references remain**

Run: `rg -n "bootstrap|schola" .github/workflows/release-binaries.yml`
Expected: **no output** (exit 1).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release-binaries.yml
git commit -m "docs(ci): correct release-binaries package comment (no bootstrap/schola)"
```

---

## Phase 5 — CLI update-notification footer [PARALLEL-SAFE]

> **Why:** Users on an old build should learn a newer release exists. This is a **best-effort, non-blocking** check: query the latest GitHub release tag, compare to the running version, print a one-line footer if newer. It must NEVER fail a command, slow startup, or run in CI/non-interactive contexts. The GUI auto-updater is deferred (see "Deferred").
>
> **This phase touches only new files + one module-registration line, so it is PARALLEL-SAFE with Phases 1–4 IF executed by a separate subagent. If executing inline, do it after Phase 3 so `VOX_VERSION` already honors the override.**

### Task 7: Create the update-check module (test-first)

**Files:**
- Create: `crates/vox-cli/src/commands/updates.rs`
- Test: same file (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Verify how sibling command modules are registered**

Run: `rg -n "pub mod |^mod " crates/vox-cli/src/commands/mod.rs | head -40`
Confirm the module-list style (e.g. `pub mod diagnostics;`). You will add `pub mod updates;` in Task 8. Also confirm `reqwest` is a dependency: `rg -n "^reqwest" crates/vox-cli/Cargo.toml` — if absent, STOP and report (the footer needs it; do not add a new dep without confirming the version line).

- [ ] **Step 2: Write the failing test for version comparison**

Create `crates/vox-cli/src/commands/updates.rs` with ONLY the pure comparison logic + tests first:

```rust
//! Best-effort "update available" footer for interactive CLI sessions.
//!
//! Non-blocking and failure-silent: a network error, a parse failure, or a
//! non-interactive/CI environment all result in NO output and NO error. This
//! must never change a command's exit code or perceptibly slow startup.

/// Returns `Some(latest)` when `latest` is a strictly newer semver than
/// `current`, else `None`. Both inputs may carry a leading `v` and/or build
/// metadata; we compare on the leading `MAJOR.MINOR.PATCH` only (pre-release
/// and `+build` suffixes are ignored for the "newer?" decision).
pub fn newer_version<'a>(current: &str, latest: &'a str) -> Option<&'a str> {
    let cur = parse_triplet(current)?;
    let lat = parse_triplet(latest)?;
    if lat > cur { Some(latest) } else { None }
}

fn parse_triplet(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    // Cut at the first non-version delimiter so "0.6.0-nightly.x+sha (hash)" works.
    let core = s
        .split(|c: char| c == '-' || c == '+' || c == ' ')
        .next()
        .unwrap_or(s);
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_newer_patch() {
        assert_eq!(newer_version("0.6.0", "0.6.1"), Some("0.6.1"));
    }

    #[test]
    fn ignores_equal_and_older() {
        assert_eq!(newer_version("0.6.1", "0.6.1"), None);
        assert_eq!(newer_version("0.6.2", "0.6.1"), None);
    }

    #[test]
    fn strips_v_prefix_and_build_metadata() {
        assert_eq!(
            newer_version("0.6.0+build.7 (abc1234)", "v0.7.0"),
            Some("v0.7.0")
        );
    }

    #[test]
    fn garbage_is_silent_none() {
        assert_eq!(newer_version("not-a-version", "0.6.1"), None);
        assert_eq!(newer_version("0.6.0", "garbage"), None);
    }
}
```

- [ ] **Step 3: Run the tests (they should fail to compile until the module is registered, but can be tested directly)**

The module isn't registered yet, so test it via the crate once Task 8 lands. For now, verify it at least parses by compiling the crate after Task 8. **Skip running until Step in Task 8.** (We keep registration as its own task so each task ends green.)

Actually run the unit tests now by registering temporarily is unnecessary — proceed to Task 8 which registers the module, then run.

- [ ] **Step 4: Commit the pure logic**

```bash
git add crates/vox-cli/src/commands/updates.rs
git commit -m "feat(cli): update-check version comparison (pure, tested)"
```

### Task 8: Register the module and run its tests

**Files:**
- Modify: `crates/vox-cli/src/commands/mod.rs`

- [ ] **Step 1: Add the module declaration**

In `crates/vox-cli/src/commands/mod.rs`, add (in alphabetical position among the `pub mod` lines):
```rust
pub mod updates;
```

- [ ] **Step 2: Format**

Run: `cargo fmt -p vox-cli`

- [ ] **Step 3: Run the update-check tests**

Run: `cargo test -p vox-cli --lib commands::updates > test-out.txt 2>&1`
Open `test-out.txt`. Expected: 4 tests in `commands::updates::tests` PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/mod.rs
git commit -m "feat(cli): register updates module"
```

### Task 9: Add the network fetch + footer printer (failure-silent)

**Files:**
- Modify: `crates/vox-cli/src/commands/updates.rs`

- [ ] **Step 1: Confirm reqwest blocking vs async usage in the crate**

Run: `rg -n "reqwest::blocking|reqwest::Client|\.await" crates/vox-cli/src/commands/ci/ | head -10`
Decide: if the CLI footer is called from a sync context, use `reqwest::blocking`; if async, use the async client. **If `reqwest::blocking` is not available** (feature not enabled), use the async `reqwest::Client` inside a short-lived `tokio` runtime, OR — simplest and lowest-risk — gate the whole fetch behind a 1.5s timeout and return `None` on any error. Confirm which features `reqwest` has: `rg -n "^reqwest" crates/vox-cli/Cargo.toml`. **If unsure, STOP and report the reqwest line** rather than guessing the feature set.

- [ ] **Step 2: Add the fetch + footer functions**

Append to `crates/vox-cli/src/commands/updates.rs` (this example uses the async client with a hard timeout; adapt to the confirmed reqwest features from Step 1):

```rust
use crate::VOX_VERSION;

const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/vox-foundation/vox/releases/latest";

/// True when the footer should be suppressed: CI, non-interactive, or an
/// explicit opt-out. Keeps the check invisible in scripts and pipelines.
fn suppressed() -> bool {
    std::env::var_os("CI").is_some()
        || std::env::var_os("VOX_NO_UPDATE_CHECK").is_some()
        || !atty_stdout()
}

#[cfg(unix)]
fn atty_stdout() -> bool {
    // SAFETY: isatty on a valid fd is always sound.
    unsafe { libc::isatty(1) == 1 }
}

#[cfg(not(unix))]
fn atty_stdout() -> bool {
    // On Windows, default to "interactive" unless CI/opt-out caught it above.
    true
}

/// Fetch the latest release tag, returning `None` on ANY failure.
async fn fetch_latest_tag() -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Rel {
        tag_name: String,
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .ok()?;
    let rel: Rel = client
        .get(LATEST_RELEASE_API)
        .header("User-Agent", concat!("vox/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    Some(rel.tag_name)
}

/// Print a one-line footer to stderr if a newer release exists. Never errors,
/// never blocks longer than the fetch timeout, silent in CI/non-interactive.
pub async fn maybe_print_update_footer() {
    if suppressed() {
        return;
    }
    let Some(latest) = fetch_latest_tag().await else {
        return;
    };
    if let Some(newer) = newer_version(VOX_VERSION, &latest) {
        eprintln!("\nA new Vox release is available: {newer} (you have {VOX_VERSION}). Run `voxup update`.");
    }
}
```

> **`libc` is already a `cfg(unix)` dependency pattern in the workspace** (voxup uses it). Confirm `vox-cli` has `libc` for unix before relying on `atty_stdout`: `rg -n "^libc|libc =" crates/vox-cli/Cargo.toml`. If absent on `vox-cli`, replace `atty_stdout()` with the `std::io::IsTerminal` trait (stable since Rust 1.70): `use std::io::IsTerminal; std::io::stdout().is_terminal()` — this is simpler and cross-platform. **Prefer `IsTerminal`** unless it's unavailable on the pinned toolchain (1.96 has it). Use it and drop the `libc`/`cfg` blocks entirely.

- [ ] **Step 3: Simplify to `IsTerminal` (recommended path)**

Per the note, replace the `atty_stdout` cfg blocks with:
```rust
fn atty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}
```
and delete the two `#[cfg(...)] fn atty_stdout` definitions.

- [ ] **Step 4: Format + compile**

Run: `cargo fmt -p vox-cli`
Run: `cargo build -p vox-cli > build-out.txt 2>&1`
Open `build-out.txt`. Expected: exit 0. If `reqwest::Client`/`.json()` fail to resolve, the `json` feature may be off — STOP and report the `reqwest` Cargo line rather than adding features blindly.

- [ ] **Step 5: Run all updates tests (pure logic still green)**

Run: `cargo test -p vox-cli --lib commands::updates > test-out.txt 2>&1`
Open `test-out.txt`. Expected: the 4 pure tests still PASS. (The network functions aren't unit-tested — they're failure-silent by design.)

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/updates.rs
git commit -m "feat(cli): failure-silent update-available footer (best-effort, opt-out via VOX_NO_UPDATE_CHECK)"
```

> **Wiring the footer into the actual CLI entrypoint is intentionally NOT in this task.** Calling `maybe_print_update_footer().await` from the top-level command dispatcher touches the hot startup path and the async runtime setup, which varies by command. That wiring is a **one-line follow-up** the human will place after choosing the call site (likely after a successful top-level command in the interactive lane). Leave a note in the handoff (Phase 6). The module is complete, tested, and ready.

---

## Phase 6 — Handoff ledger [PARALLEL-SAFE]

### Task 10: Append the AGH-#### ledger entry

**Files:**
- Modify (append): `docs/superpowers/antigravity-handoff-ledger.md`

- [ ] **Step 1: Find the next AGH id**

Run: `rg -n "# --- AGH-[0-9]+" docs/superpowers/antigravity-handoff-ledger.md | tail -3`
The next id is the highest seen + 1 (e.g. if AGH-0018 is last, use AGH-0019).

- [ ] **Step 2: Append the entry**

Append to the end of the file (replace `NNNN` with the resolved id, and fill the real commit SHAs + test counts from your run):

````markdown

---

```yaml
# --- AGH-NNNN ---
id: AGH-NNNN
date: "2026-06-19"
plan: "docs/superpowers/plans/2026-06-19-track-b-release-nightly-automation.md"
subsystem: "Track B — release + nightly automation (install/release/publish program)"
target: "Gemini 3.5 Flash (Antigravity)"
delivered:
  - "release_build.rs: ReleasePackage = {Vox, Mens, Voxup, All}; dead bootstrap/schola removed; All builds vox+vox-ml-cli+voxup"
  - "VOX_VERSION honors VOX_VERSION_OVERRIDE; release-build forwards --version into it"
  - "all_package_matches_distribution_ssot parity gate (ReleasePackage::All == SSOT binaries)"
  - ".github/workflows/release-nightly.yml (green-main gate + rolling nightly pre-release)"
  - "release-binaries.yml stale bootstrap/schola comment corrected"
  - "crates/vox-cli/src/commands/updates.rs: failure-silent update-available footer (pure logic tested)"
outcome: "GREEN | partial | failed"   # set honestly
verification: "cargo test -p vox-cli --lib commands::ci::release_build + commands::updates -> N passed"
errors_encountered: []
agent_deviations: []
followups:
  - "Wire maybe_print_update_footer() into the interactive CLI dispatcher (one line; human chooses call site)."
  - "GUI auto-updater (tauri-plugin-updater) deferred — own plan."
  - "release-nightly.yml unproven until first scheduled/dispatch run."
commits: []   # fill with the SHAs from this track
```
````

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/antigravity-handoff-ledger.md
git commit -m "docs(ledger): AGH-NNNN — Track B release + nightly automation"
```

---

## Deferred (NOT in this plan — do not attempt)

- **GUI auto-updater (`tauri-plugin-updater` + toast).** The INDEX lists "GUI update notification" under Track B, but GUI work is high-risk for this execution target (per the limitations doc) and the Tauri updater needs signing keys + an update manifest endpoint that depend on Phase 4's nightly release existing first. This gets its **own** plan after the nightly release is proven live.
- **Wiring the CLI footer into the dispatcher.** Module is built + tested; the single call-site insertion is a human decision (hot startup path).

---

## Definition of Done (whole track)

- [ ] `cargo test -p vox-cli --lib commands::ci::release_build > test-out.txt 2>&1` → all PASS, including `all_package_matches_distribution_ssot`.
- [ ] `cargo test -p vox-cli --lib commands::updates > test-out.txt 2>&1` → 4 PASS.
- [ ] `cargo build -p vox-cli > build-out.txt 2>&1` → exit 0.
- [ ] `rg -n "bootstrap|schola" crates/vox-cli/src/commands/ci/release_build.rs .github/workflows/release-binaries.yml` → no output.
- [ ] `cargo fmt -p vox-cli` leaves no diff.
- [ ] `.github/workflows/release-nightly.yml` exists with `gate`, `build`, `publish` jobs.
- [ ] Every task committed with its exact message. AGH ledger entry appended.
- [ ] Report: final test counts, any disk-vs-plan contradiction, the resolved `include_str!` depth, the `reqwest` feature decision, and the deferred follow-ups.

---

## Self-Review (author's check against the spec/INDEX)

**Spec coverage (INDEX Track B):**
- "Build release-nightly.yml (green-main gate, rolling nightly)" → Phase 4 ✅
- "Unify the release matrix on the SSOT binaries" → Phase 1 (package set) + Phase 3 (parity gate) ✅
- "Add update notification (CLI footer + GUI)" → CLI footer Phase 5 ✅; GUI **explicitly deferred** with rationale ✅
- "Gates: nightly-green-before-release" → Phase 4 `gate` job ✅; "release-artifact ↔ SSOT-binaries parity" → Phase 3 test ✅
- Spec premise "vox-bootstrap/vox-schola retired" → confirmed on disk; plan removes the still-present dead code (a correction the spec implied but never landed) ✅

**Placeholder scan:** No "TBD"/"handle edge cases"/"similar to". Every code step has full code. The two genuinely human decisions (footer call-site, GUI updater) are explicitly carved out as Deferred, not left as vague steps.

**Type consistency:** `ReleasePackage::{Vox,Mens,Voxup,All}` used consistently across enum (Task 1), dispatch (Task 1), and `ALL_RELEASE_BINARIES`/parity test (Task 4). `newer_version`/`parse_triplet`/`maybe_print_update_footer`/`fetch_latest_tag` names consistent across Tasks 7 and 9. `VOX_VERSION_OVERRIDE` consistent across lib.rs (Task 2) and release_build.rs (Task 3).
