# Working Release Pipeline — Implementation Plan (Phases 0 + 1a)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a `v*` tag produce correct, uniquely-named, genuinely-optimized artifacts that are verified before they publish — starting from a pipeline that currently produces nothing at all.

**Architecture:** Eight tasks in dependency order. Phase 0 repairs the pipeline (a deleted crate breaks every matrix leg; all 16 bundle artifacts collide on one filename; the matrix names a bundle that does not exist). Phase 1a then makes the output actually optimized: remove `panic = "abort"` *before* enabling `[profile.dist]` — because three production paths depend on unwinding — switch all nine `--release` sites, pin the release toolchain, and add a black-box verification job that `publish` structurally depends on.

**Tech Stack:** Rust 1.96.0, `cargo`, `toml`, GitHub Actions, WiX (later phases).

**Spec:** [`docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`](../specs/2026-08-20-vox-distribution-system-design.md)

## Global Constraints

- Rust toolchain is pinned to **1.96.0**. Do not bump it. Never use `dtolnay/rust-toolchain@stable`.
- **Never run `cargo fmt --all`** — it overflows the Windows `CreateProcess` limit (`os error 206`). Use `cargo run -q -p vox-cli -- run scripts/fmt.vox`.
- **`vox` is not on PATH in this worktree.** Every invocation is `cargo run -q -p vox-cli -- <args>`.
- **Verify with `vox ci pre-push --full`, not `--complete`.** `--complete` is static gates only and runs **no tests** (`crates/vox-cli/src/commands/ci/pre_push.rs:8-10`). This plan adds tests.
- **Do not add a workspace crate-to-crate dependency edge.** Duplicate helpers under ~50 lines with `// vox:defactored-from <crate> <date>`. Reading files in tests is not an edge.
- Test-first is binding. Batch commits; open one review-ready PR; re-review via `@coderabbitai review`.
- `[profile.dist]` after Task 4 is: `inherits = "release"`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`. **No `panic` key.**

---

### Task 1: Stop building a crate that no longer exists

`release-binaries.yml:43` passes `--package all`, which sets `want_bootstrap` and shells `cargo build -p vox-bootstrap`. That crate is absent from `crates/` and from `Cargo.lock`. Every matrix leg fails, `if-no-files-found: error` fires, and no release has ever been produced.

**Files:**
- Modify: `crates/vox-cli-ci/src/cmd_enums.rs:9-20` (the `ReleasePackage` enum)
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs:43-51` (`want_bootstrap`), `:62-75` (the build block), `:116-121` (`bootstrap_executable_name`), and its tests at `:200`, `:219-225`, `:255-256`, `:275-281`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (existing `#[cfg(test)] mod tests` at `:194`)

**Interfaces:**
- Consumes: nothing.
- Produces: `ReleasePackage` reduced to `Vox | Mens | All`. Task 2 and Task 6 both reference `ReleasePackage::All` as "every artifact `release_build` knows how to build".

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
/// Every crate name `release_build.rs` passes to `cargo build -p` must be a real
/// workspace crate. `vox-bootstrap` was deleted but `--package all` kept building
/// it, so every release matrix leg failed and no artifact was ever published.
#[test]
fn every_built_package_is_a_real_workspace_crate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = include_str!("release_build.rs");

    // Crate names appear as the literal argument after `"vox-cli",` style entries
    // in build_and_package_binary calls. Scan for any `"vox-<name>"` literal that
    // is used as a package name and assert the directory exists.
    for pkg in ["vox-cli", "vox-ml-cli"] {
        assert!(
            root.join("crates").join(pkg).is_dir(),
            "release_build.rs builds -p {pkg} but crates/{pkg}/ does not exist"
        );
    }
    assert!(
        !src.contains("vox-bootstrap"),
        "release_build.rs still references vox-bootstrap, which was deleted from \
         the workspace (absent from crates/ and Cargo.lock). Building it fails \
         every release matrix leg."
    );
}

/// The SSOT lists what a release ships; nothing previously checked the release
/// builder against it.
#[test]
fn release_build_packages_match_distribution_ssot() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ssot = std::fs::read_to_string(root.join("contracts/distribution/profiles.v1.yaml"))
        .expect("read profiles.v1.yaml");

    // `binaries:` is a flat YAML list of binary names.
    let mut declared: Vec<String> = Vec::new();
    let mut in_list = false;
    for line in ssot.lines() {
        if line.trim_start().starts_with("binaries:") {
            in_list = true;
            continue;
        }
        if in_list {
            match line.trim().strip_prefix("- ") {
                Some(b) => declared.push(b.trim().to_string()),
                None if !line.trim().is_empty() => break,
                None => {}
            }
        }
    }
    declared.sort();
    assert_eq!(
        declared,
        vec!["vox".to_string(), "vox-ml-cli".to_string(), "voxup".to_string()],
        "profiles.v1.yaml binaries: changed; update release_build.rs and this test together"
    );

    // `voxup` is built directly by release-binaries.yml, not by release_build.rs.
    // `vox` comes from the vox-cli crate. Neither vox-bootstrap nor vox-schola
    // may appear anywhere.
    let src = include_str!("release_build.rs");
    assert!(!src.contains("vox-schola"), "vox-schola is retired");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib release_build::tests::every_built_package_is_a_real_workspace_crate -- --nocapture`

Expected: FAIL with `release_build.rs still references vox-bootstrap, which was deleted from the workspace`.

- [ ] **Step 3: Confirm nothing still passes the retired variants**

Run: `grep -rn "package bootstrap\|package both\|--package all" .github/ scripts/ docs/ || echo "none"`

Expected: only `.github/workflows/release-binaries.yml:43` with `--package all`. If any caller passes `bootstrap` or `both`, stop and report — removing the variants would break it.

- [ ] **Step 4: Write the implementation**

In `crates/vox-cli-ci/src/cmd_enums.rs`, reduce the enum:

```rust
/// Release-build target tier (used by [`CiCmd::ReleaseBuild`]); the guard logic lives
/// in vox-cli's `commands::ci::release_build`, which imports this back.
///
/// `Bootstrap` and `Both` were removed: `vox-bootstrap` is retired
/// (contracts/distribution/profiles.v1.yaml) and building it failed every release.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReleasePackage {
    /// Core `vox` CLI only (lean install — no ML/scientia plugins).
    Vox,
    /// `vox-ml-cli` plugin: ML/oratio/speech/populi/train subcommands (heavy: Candle).
    Mens,
    /// Every artifact: vox + every plugin binary. The "full" tier.
    All,
}
```

In `crates/vox-cli/src/commands/ci/release_build.rs`:
- Delete the `want_bootstrap` binding (`:43-51`) and the whole `if want_bootstrap { … }` block (`:62-75`).
- Change `want_vox` to `matches!(package, ReleasePackage::Vox | ReleasePackage::All)`.
- Delete `bootstrap_executable_name` (`:116-121`), its import in the tests `use` block (`:200`), and the assertions at `:219-225`.
- Change the `artifact_filename` test at `:255-256` and the checksum test at `:275-281` to use `vox-ml-cli` in place of `vox-bootstrap`.

Update the workflow comment at `.github/workflows/release-binaries.yml:41` to name only the artifacts that exist:

```yaml
        # `--package all` produces: vox, vox-ml-cli. voxup is built separately below.
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib release_build -- --nocapture`

Expected: PASS, all tests in the module.

- [ ] **Step 6: Prove a release build now completes**

Run: `cargo run -q -p vox-cli -- ci release-build --target x86_64-pc-windows-msvc --version v0.0.0-local --out-dir target/rel-smoke --package all`

Substitute your host triple if not on Windows. Expected: exit 0, and `target/rel-smoke/` contains a `vox-…` and a `vox-ml-cli-…` archive plus `checksums.txt`. **This is the first time this command has succeeded.**

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli-ci/src/cmd_enums.rs crates/vox-cli/src/commands/ci/release_build.rs .github/workflows/release-binaries.yml
git commit -m "fix(release): stop building the retired vox-bootstrap crate

release-binaries.yml passes --package all, which shelled `cargo build -p
vox-bootstrap`. That crate is absent from crates/ and Cargo.lock, so every
matrix leg failed and no release artifact has ever been published. Removes
the Bootstrap/Both tiers and adds a gate binding the release builder to the
distribution SSOT.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Give bundle artifacts unique, versioned names

All 16 matrix cells build `--out bundle.tar.gz` and attach literally `bundle.tar.gz` to the release. They overwrite each other; the survivor carries no bundle id, no target, no version. None appear in `checksums.txt`.

**Files:**
- Modify: `.github/workflows/bundle-release.yml:80-110`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (same tests module)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: the artifact name shape `<bundle>-<tag>-<target>.tar.gz`, matching `release_artifacts::artifact_filename`'s `<name>-<version>-<triple>.<ext>` SSOT.

- [ ] **Step 1: Write the failing test**

Add to the tests module in `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
/// Bundle artifacts must not collide on upload. Every matrix cell previously
/// produced a file literally named `bundle.tar.gz`, so 16 uploads overwrote
/// each other and the survivor identified neither its bundle nor its target.
#[test]
fn bundle_release_artifacts_are_uniquely_named() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wf = std::fs::read_to_string(root.join(".github/workflows/bundle-release.yml"))
        .expect("read bundle-release.yml");

    assert!(
        !wf.contains("--out bundle.tar.gz"),
        "bundle-release.yml builds every matrix cell to the same filename; \
         the 16 release assets collide on upload"
    );
    assert!(
        wf.contains("${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz"),
        "bundle artifacts must be named <bundle>-<tag>-<target>.tar.gz to match \
         the release_artifacts naming SSOT"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib bundle_release_artifacts_are_uniquely_named -- --nocapture`

Expected: FAIL with `bundle-release.yml builds every matrix cell to the same filename`.

- [ ] **Step 3: Write the implementation**

Replace `.github/workflows/bundle-release.yml:80-110` with:

```yaml
      - name: Build bundle tarball
        env:
          VOX_PLUGINS_DIR: ${{ github.workspace }}/bundle-plugins
          BUNDLE_ARTIFACT: ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
        run: |
          cargo run --locked -p vox-cli -- \
            bundle build ${{ matrix.bundle }} \
            --out "$BUNDLE_ARTIFACT"
        shell: bash

      - name: Verify bundle integrity
        env:
          VOX_PLUGINS_DIR: ${{ github.workspace }}/bundle-plugins
          BUNDLE_ARTIFACT: ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
        run: cargo run --locked -p vox-cli -- bundle verify "$BUNDLE_ARTIFACT"
        shell: bash

      - name: Checksum bundle
        env:
          BUNDLE_ARTIFACT: ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
        run: |
          ls -lh "$BUNDLE_ARTIFACT"
          sha256sum "$BUNDLE_ARTIFACT" > "$BUNDLE_ARTIFACT.sha256"
        shell: bash

      - name: Upload bundle artifact
        uses: actions/upload-artifact@v7
        with:
          name: ${{ matrix.bundle }}-${{ matrix.target }}
          path: |
            ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
            ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz.sha256
          retention-days: 30

      - name: Attach to GitHub Release
        if: github.event_name == 'release'
        uses: softprops/action-gh-release@v3
        with:
          files: |
            ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
            ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz.sha256
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

Note `sha256sum` is available on both runner types here: the Linux leg is the Docker fleet image and the Windows leg runs `shell: bash` (Git Bash), which ships it.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib bundle_release_artifacts_are_uniquely_named -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/bundle-release.yml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): bundle artifacts collided on a single filename

All 16 matrix cells built --out bundle.tar.gz and attached that exact name
to the release, so they overwrote each other and the survivor identified
neither bundle nor target. Names now follow the release_artifacts SSOT and
carry a per-artifact sha256.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Make the bundle matrix gate platform-aware

`bundle-release.yml:36` builds `vox-cloud-only`, absent from `catalog.toml`. `bundle_resolved` returns `ResolveError::UnknownBundle` and the step has no `continue-on-error`, so two jobs fail on every release; `fail-fast: false` hid it.

`vox-ml-metal` must **not** simply be added — the matrix is x86-64 Linux and Windows, and that bundle carries an Apple-Metal plugin. `vox-mobile` is `status = "alpha"`.

**Files:**
- Modify: `.github/workflows/bundle-release.yml:29-38` and the header comment at `:7`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (same tests module)

**Interfaces:**
- Consumes: nothing.
- Produces: `MATRIX_EXCLUDED_BUNDLES`, `catalog_bundle_ids`, `workflow_matrix_bundle_ids` — all private to the tests module.

- [ ] **Step 1: Write the failing test**

```rust
/// Bundles the x86-64 Linux + Windows matrix deliberately does not build.
/// `vox-ml-metal` carries an Apple-Metal plugin; `vox-mobile` is status="alpha"
/// and planned for v0.8. Adding either to the matrix would spawn jobs that
/// cannot succeed on these runners.
const MATRIX_EXCLUDED_BUNDLES: &[&str] = &["vox-ml-metal", "vox-mobile"];

/// Every `id` under a `[[bundle]]` table, minus platform-excluded ones, sorted.
fn catalog_bundle_ids(catalog_toml: &str) -> Vec<String> {
    let v: toml::Value = catalog_toml.parse().expect("catalog.toml must parse");
    let mut ids: Vec<String> = v
        .get("bundle")
        .and_then(|b| b.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("id")?.as_str().map(String::from))
                .filter(|id| !MATRIX_EXCLUDED_BUNDLES.contains(&id.as_str()))
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
fn catalog_bundle_ids_excludes_platform_specific_bundles() {
    let toml_src = "[[bundle]]\nid = \"vox-base\"\n\n[[bundle]]\nid = \"vox-ml-metal\"\n";
    assert_eq!(catalog_bundle_ids(toml_src), vec!["vox-base"]);
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
        "bundle-release.yml `bundle:` matrix must match the buildable [[bundle]] ids \
         in catalog.toml (excluding {MATRIX_EXCLUDED_BUNDLES:?}).\n  \
         only in workflow (phantom — fails UnknownBundle every release): {:?}\n  \
         only in catalog (never built): {:?}",
        actual.iter().filter(|b| !expected.contains(b)).collect::<Vec<_>>(),
        expected.iter().filter(|b| !actual.contains(b)).collect::<Vec<_>>(),
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib bundle_release_matrix_matches_plugin_catalog -- --nocapture`

Expected: FAIL, reporting `only in workflow (phantom …): ["vox-cloud-only"]`.

- [ ] **Step 3: Write the implementation**

In `.github/workflows/bundle-release.yml`, replace the `bundle:` list:

```yaml
        bundle:
          - vox-base
          - vox-dev
          - vox-edge
          - vox-fullstack
          - vox-mesh
          - vox-ml
          - vox-server
```

And correct the header comment at `:7`:

```yaml
# Matrix: 7 bundles x 2 platforms = 14 jobs per run.
# vox-ml-metal (Apple Metal) and vox-mobile (alpha, v0.8) are excluded — see
# MATRIX_EXCLUDED_BUNDLES in crates/vox-cli/src/commands/ci/release_build.rs.
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib -- bundle_release catalog_bundle workflow_matrix --nocapture`

Expected: PASS, four tests.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/bundle-release.yml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): matrix built a phantom bundle on every release

vox-cloud-only was removed from catalog.toml but stayed in the matrix, so
bundle_resolved returned UnknownBundle and two jobs failed on every release
(hidden by fail-fast: false). Adds a platform-aware parity gate.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Remove `panic = "abort"` from `[profile.dist]` before enabling it

Three production paths in the shipped binary depend on unwinding. Task 5 enables `[profile.dist]`, so this must land first or Task 5 ships the regression.

**Files:**
- Modify: `Cargo.toml` (`[profile.dist]`, around `:424-429`)
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (same tests module)

**Interfaces:**
- Consumes: nothing.
- Produces: a `dist` profile safe for Task 5 to enable.

- [ ] **Step 1: Write the failing test**

```rust
/// `[profile.dist]` must not set panic = "abort". Three non-test paths in the
/// shipped binary rely on unwinding:
///   - vox-actor-runtime/src/supervisor.rs:30,52 — spawn_supervised matches on
///     JoinError::is_panic(); under abort a panicking task kills the process and
///     every caller silently loses supervision.
///   - vox-vcs/src/jj_actor.rs:196,282 — the `guarded!` macro catch_unwinds
///     block_on so a panicking jj-lib call returns Err(Unavailable) instead of
///     killing the actor loop. `jj` is a default feature of vox-orchestrator,
///     which vox-cli takes with defaults, so this ships.
///   - vox-search/src/memory_cache.rs:87 — resume_unwind on a spawn_blocking panic.
#[test]
fn dist_profile_does_not_abort_on_panic() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"),
    )
    .expect("read workspace Cargo.toml");
    let v: toml::Value = manifest.parse().expect("workspace Cargo.toml must parse");
    let p = v
        .get("profile")
        .and_then(|p| p.get("dist"))
        .expect("[profile.dist] must exist");

    assert!(
        p.get("panic").is_none(),
        "[profile.dist] must not set `panic`; abort breaks catch_unwind-based \
         panic containment in supervisor.rs and jj_actor.rs"
    );
    // The optimization settings are the point of the profile — keep them.
    assert_eq!(p.get("lto").and_then(|x| x.as_str()), Some("fat"));
    assert_eq!(p.get("codegen-units").and_then(|x| x.as_integer()), Some(1));
    assert_eq!(p.get("strip").and_then(|x| x.as_str()), Some("symbols"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib dist_profile_does_not_abort_on_panic -- --nocapture`

Expected: FAIL with `[profile.dist] must not set 'panic'`.

- [ ] **Step 3: Confirm the three call sites are real before changing anything**

Run: `grep -rn "catch_unwind\|resume_unwind\|is_panic()" --include=*.rs crates/vox-actor-runtime/src crates/vox-vcs/src crates/vox-search/src`

Expected: hits in `supervisor.rs`, `jj_actor.rs`, and `memory_cache.rs`, none of them inside `#[cfg(test)]`. If they have moved, update the test's doc comment to the new locations before proceeding.

- [ ] **Step 4: Write the implementation**

In the workspace root `Cargo.toml`, replace the `[profile.dist]` block:

```toml
# Shipped-artifact profile. Fat LTO + codegen-units=1 for runtime speed,
# symbols stripped for size.
#
# Deliberately does NOT set `panic = "abort"`. The workspace relies on
# unwinding in three non-test paths: vox-actor-runtime's spawn_supervised
# (JoinError::is_panic), vox-vcs's `guarded!` macro (catch_unwind around
# jj-lib, which ships via vox-orchestrator's default features), and
# vox-search's memory_cache (resume_unwind). Abort would turn each of those
# containment points into a process kill.
[profile.dist]
inherits = "release"
lto = "fat"
codegen-units = 1
strip = "symbols"
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib dist_profile_does_not_abort_on_panic -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): drop panic=abort from [profile.dist] before enabling it

Three shipped code paths rely on unwinding for panic containment
(spawn_supervised, the jj_actor guarded! macro, memory_cache). Enabling
[profile.dist] with panic=abort would have silently converted each into a
process kill.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Build every shipped artifact at `[profile.dist]`

Nine sites pass `--release`. Fixing only `release_build.rs` leaves voxup, the GUI sidecar, the MSI/deb inputs, and every bundle tarball on thin LTO.

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs:150` and `:171-174`
- Modify: `.github/workflows/release-binaries.yml:46`; `.github/workflows/release-installers.yml:23, 91, 119`; `.github/workflows/release-gui.yml:90, 91, 104`; `.github/workflows/bundle-release.yml` (the `cargo run --locked` lines from Task 2 and remaining `--release` build steps)
- Modify: `Dockerfile:21`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (same tests module)

**Interfaces:**
- Consumes: the panic-free `[profile.dist]` from Task 4.
- Produces: `pub(crate) const DIST_PROFILE: &str = "dist";`. Task 6 uses `target/<profile>/` paths derived from it.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn dist_profile_constant_is_dist() {
    assert_eq!(super::DIST_PROFILE, "dist");
}

/// No workflow that produces a SHIPPED artifact may build with `--release`;
/// [profile.release] is thin-LTO and keeps debuginfo. Guards all nine sites.
///
/// NOTE: this test names the flag only inside a `concat!` so the assertion
/// message cannot match the files it scans.
#[test]
fn shipped_artifacts_build_with_dist_profile() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let needle = concat!("--", "release");

    let shipped = [
        ".github/workflows/release-binaries.yml",
        ".github/workflows/release-installers.yml",
        ".github/workflows/release-gui.yml",
        ".github/workflows/bundle-release.yml",
        "Dockerfile",
    ];
    for rel in shipped {
        let text = std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("read {rel}: {e}"));
        assert!(
            !text.contains(needle),
            "{rel} builds a shipped artifact with the release profile; use \
             `--profile dist` so fat LTO + codegen-units=1 + strip apply"
        );
    }

    let src = include_str!("release_build.rs");
    assert!(
        !src.contains(concat!("\"--", "release\"")),
        "release_build.rs must pass --profile dist"
    );
    assert!(
        !src.contains(concat!(".join(\"", "release\")")),
        "release_build.rs must read artifacts from target/<triple>/dist/"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib shipped_artifacts_build_with_dist_profile -- --nocapture`

Expected: FAIL, naming the first workflow still using the release profile.

- [ ] **Step 3: Write the implementation**

In `crates/vox-cli/src/commands/ci/release_build.rs`, below the `pub use … SUPPORTED_RELEASE_TARGETS;` line at `:12`:

```rust
/// Cargo profile used for every shipped artifact.
///
/// `[profile.dist]` sets `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`.
/// Plain `--release` is thin-LTO and keeps debuginfo — see spec finding F6.
pub(crate) const DIST_PROFILE: &str = "dist";
```

In `build_and_package_binary`, replace the `"--release",` array element with `"--profile", DIST_PROFILE,`, and change the output path segment from `.join("release")` to `.join(DIST_PROFILE)`.

Then, in each workflow, replace `--release` with `--profile dist` on every `cargo build` / `cargo run` line that produces a shipped artifact:
- `release-binaries.yml:46` (voxup)
- `release-installers.yml:23, 91, 119`
- `release-gui.yml:90, 91, 104` (the CLI sidecar)
- `bundle-release.yml` — the `cargo run --locked -p vox-cli` lines already dropped `--release` in Task 2; confirm no `--release` remains
- `Dockerfile:21`

**Two path consequences to handle in the same edit.** Any step that afterwards reads `target/release/…` must read `target/dist/…`. In particular, `release-gui.yml` stages the CLI as a Tauri `externalBin` sidecar, and `crates/vox-gui/tauri.conf.json:32-34` hardcodes `"../../target/release/vox"` — update that to `"../../target/dist/vox"` and check `scripts/gui-build.vox` for the same assumption.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib shipped_artifacts_build_with_dist_profile -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Prove the artifact is really optimized**

Run: `cargo run -q -p vox-cli -- ci release-build --target x86_64-pc-windows-msvc --version v0.0.0-local --out-dir target/dist-smoke --package vox`

Expected: exit 0, archive present. Slow by design — fat LTO with `codegen-units = 1` is 5–20× the link time of thin LTO and holds multi-GB peak RSS in one `rustc` process. Budget 30–60 minutes cold on a 4-core machine.

- [ ] **Step 6: Raise the release build timeout**

`--package all` now builds two binaries at fat LTO. Check the current `timeout-minutes` on `release-binaries.yml`'s `build` job (it was 60) and raise it to `180`, or the first real tag times out.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "fix(release): build all shipped artifacts with --profile dist

[profile.dist] was defined but referenced by nothing; all nine build sites
passed --release, so every shipped binary was thin-LTO with debuginfo. Covers
release_build.rs, voxup, the GUI sidecar, MSI/deb inputs, bundle tarballs, and
the daily public Docker image.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Verify the dist binary, and make `publish` depend on it

Tag pushes **cannot** be gated by GitHub required checks — those apply to branch and PR refs only. The sole reliable ordering mechanism is `needs:` inside one workflow, so verification is a job in `release-binaries.yml`, not a standalone workflow.

**Files:**
- Create: `crates/vox-cli/tests/dist_binary_e2e.rs`
- Modify: `.github/workflows/release-binaries.yml` (add a `dist-verify` job; change `publish`'s `needs:`)

**Interfaces:**
- Consumes: `DIST_PROFILE` (Task 5).
- Produces: the `VOX_DIST_BIN` environment contract — when set, a missing binary is a hard failure.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/tests/dist_binary_e2e.rs`:

```rust
//! Black-box verification of the shipped `dist`-profile binary.
//!
//! Runs the binary as a subprocess, so nothing here needs a test harness linked
//! into it. `cargo test --profile dist` cannot serve this purpose: cargo ignores
//! the `panic` setting for test targets, and a full fat-LTO test lane would
//! fat-LTO-link each of the 80+ integration targets against a 1656-package graph
//! and OOM the 14 GB runner. See spec finding F8.

use std::path::PathBuf;
use std::process::Command;

/// Locate the dist binary.
///
/// CI sets `VOX_DIST_BIN` to the exact artifact under verification. When it is
/// set, a missing binary is a HARD FAILURE — silently skipping would make the
/// whole verification lane a no-op that reports green.
fn dist_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VOX_DIST_BIN") {
        let p = PathBuf::from(p);
        assert!(
            p.is_file(),
            "VOX_DIST_BIN={} does not exist — the verification lane would \
             otherwise silently pass without testing anything",
            p.display()
        );
        return Some(p);
    }
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/dist")
        .join(exe);
    p.exists().then_some(p)
}

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
        eprintln!("SKIP: target/dist/vox not built and VOX_DIST_BIN unset");
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
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    assert_eq!(code, 0, "`vox --help` must exit 0");
    assert!(stdout.contains("vox"), "help output must mention vox");
}

#[test]
fn dist_binary_rejects_unknown_subcommand_cleanly() {
    let Some((_, _, code)) = run_dist(&["definitely-not-a-real-subcommand"]) else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    // clap's parse-error path exits 2. An abort would surface as 134 (SIGABRT)
    // on Unix or a large status on Windows.
    assert_ne!(code, 0, "unknown subcommand must fail");
    assert!(
        (0..=99).contains(&code),
        "unknown subcommand must exit with a normal error code, not an abort; got {code}"
    );
}

#[test]
fn dist_binary_compiles_and_runs_a_golden_program() {
    let Some(bin) = dist_binary() else {
        eprintln!("SKIP: dist binary unavailable");
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

- [ ] **Step 2: Run the suite with no binary, then with one**

Run: `cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: PASS with four SKIP lines — this confirms the harness compiles and the skip path works locally.

Now prove the hard-failure contract, which is the property that matters:

Run: `VOX_DIST_BIN=/nonexistent cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`
(PowerShell: `$env:VOX_DIST_BIN="/nonexistent"; cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`)

Expected: **FAIL** with `VOX_DIST_BIN=/nonexistent does not exist`. This is the failing test — it proves CI cannot silently no-op.

- [ ] **Step 3: Run against the real binary**

Run: `cargo build -p vox-cli --profile dist --features heavy-retrieval` then
`cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: PASS, four tests, no SKIP lines. If `dist_binary_rejects_unknown_subcommand_cleanly` reports an abort-range code, a parse-error path is panicking; convert that call site in `crates/vox-cli/src/cli_dispatch/mod.rs` to return `anyhow::Result`.

- [ ] **Step 4: Add the gating job**

Add to `.github/workflows/release-binaries.yml`:

```yaml
  # Verifies the SHIPPED optimization level (spec F8) and GATES publish on it.
  #
  # This is a job here, not a standalone workflow, because tag pushes cannot be
  # gated by GitHub required checks — those apply to branch/PR refs only. Two
  # independent `push: tags` workflows have no ordering, so the release would
  # publish while verification ran, and publish unaffected if it failed.
  #
  # One lane only: `cargo test --profile dist` across the workspace would
  # fat-LTO-link each of the 80+ integration targets against a 1656-package
  # graph, exceeding the 14 GB runner budget (runner_scale.rs MEM_PER_RUNNER,
  # whose own test cites a measured ~12 GB peak at THIN LTO).
  #
  # No rust-toolchain step: the fleet image ships the pinned 1.96.0 toolchain
  # and rust-toolchain.toml is authoritative. `@stable` would install an unused
  # second toolchain and import each new release's lint wave.
  dist-verify:
    name: dist verification (fat LTO)
    runs-on: [self-hosted, linux, x64]
    # ~30-45m dep compile + ~20-40m serial fat-LTO link (codegen-units=1, so the
    # 4 vCPUs do not help) + ~15-25m for the e2e test target. p95 ~75-100m.
    timeout-minutes: 120
    permissions:
      contents: read
    env:
      # The fat-LTO link is serial anyway; capping the dep phase at 2 concurrent
      # rustc processes keeps peak RSS under the 14 GB container cap.
      CARGO_BUILD_JOBS: "2"
    steps:
      - uses: actions/checkout@v7

      - name: Build the real dist binary
        # Feature set MUST match release_build.rs::build_and_package_binary.
        run: cargo build -p vox-cli --profile dist --locked --features heavy-retrieval

      - name: Black-box E2E against the dist binary
        env:
          VOX_DIST_BIN: ${{ github.workspace }}/target/dist/vox
        run: cargo test -p vox-cli --test dist_binary_e2e --locked -- --nocapture
```

Then change the `publish` job's dependency to `needs: [build, dist-verify]`.

- [ ] **Step 5: Verify the workflow passes its gates**

Run each and expect exit 0:

```bash
cargo run -q -p vox-cli -- ci workflow-concurrency-guard --strict
```

```bash
cargo run -q -p vox-cli -- ci runner-policy-check --strict
```

```bash
cargo nextest run -p vox-cli sccache_workflow_guard
```

The `--strict` flags matter: `workflow-concurrency-guard` is advisory by default and exits 0 regardless.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/tests/dist_binary_e2e.rs .github/workflows/release-binaries.yml
git commit -m "test(release): gate publish on black-box verification of the dist binary

Adds a subprocess suite against the real shipped artifact and makes publish
declare needs: [build, dist-verify]. VOX_DIST_BIN makes a wrong artifact path
a hard failure rather than a silent skip, so the lane cannot no-op green.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Pin the release toolchain

Every release workflow uses `dtolnay/rust-toolchain@stable` while the repo pins 1.96.0, so shipped artifacts are built on a different compiler than CI gates and each new stable imports its lint wave.

**Files:**
- Modify: `.github/workflows/release-binaries.yml:35`; `.github/workflows/release-installers.yml:20, 58, 74, 89`; `.github/workflows/bundle-release.yml:50`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (same tests module)

**Interfaces:** none.

- [ ] **Step 1: Write the failing test**

```rust
/// Release workflows must not float the toolchain. Building shipped artifacts on
/// `@stable` while the repo pins 1.96.0 means users get binaries from a compiler
/// no CI gate ever ran, and each new stable silently imports its lint wave.
#[test]
fn release_workflows_pin_the_toolchain() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pinned = std::fs::read_to_string(root.join("rust-toolchain.toml"))
        .expect("read rust-toolchain.toml");
    let want = pinned
        .lines()
        .find_map(|l| l.trim().strip_prefix("channel = "))
        .map(|v| v.trim().trim_matches('"').to_string())
        .expect("rust-toolchain.toml must declare a channel");

    for rel in [
        ".github/workflows/release-binaries.yml",
        ".github/workflows/release-installers.yml",
        ".github/workflows/bundle-release.yml",
    ] {
        let text = std::fs::read_to_string(root.join(rel)).expect("read workflow");
        assert!(
            !text.contains("rust-toolchain@stable"),
            "{rel} floats the toolchain with @stable; pin it to {want} \
             (rust-toolchain.toml) or drop the step — the fleet image already ships it"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib release_workflows_pin_the_toolchain -- --nocapture`

Expected: FAIL naming `release-binaries.yml`.

- [ ] **Step 3: Write the implementation**

In each of the three workflows, replace every

```yaml
      - uses: dtolnay/rust-toolchain@stable
```

with

```yaml
      # Pinned, not @stable: shipped artifacts must be built by the same compiler
      # CI gates use (rust-toolchain.toml). @stable also imports each new
      # release's lint wave — see AGENTS.md §Perennial Bug Patterns.
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"
```

Where the step also passes `targets:`, keep that input. Note `release-gui.yml` is intentionally excluded — its toolchain steps carry documented cross-target handling; leave them and their comments alone.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib release_workflows_pin_the_toolchain -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(ci): pin the toolchain in release workflows

Shipped artifacts were built on dtolnay/rust-toolchain@stable while the repo
pins 1.96.0 — a compiler no CI gate ever ran.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Verify the whole phase and open one PR

**Files:** none created or modified beyond formatting.

- [ ] **Step 1: Format**

Run: `cargo run -q -p vox-cli -- run scripts/fmt.vox`

Expected: exit 0. Never `cargo fmt --all`.

- [ ] **Step 2: Run the full local gate tier**

Run: `cargo run -q -p vox-cli -- ci pre-push --full`

Expected: exit 0. `--full` is required — `--complete` runs no tests, and this phase is almost entirely tests.

- [ ] **Step 3: Confirm no crate edge was added**

Run: `cargo run -q -p vox-cli -- ci crate-edges`

Expected: exit 0.

- [ ] **Step 4: Confirm the SSOT gates still pass**

Run: `cargo test -p voxup --test distribution_parity --locked`

Expected: exit 0. Task 1 changed what the release builder produces; this proves the distribution SSOT still agrees.

- [ ] **Step 5: Commit any formatting drift and push**

```bash
git add -A
git commit -m "style: rustfmt after release pipeline repair

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" || echo "nothing to commit"
git push -u origin claude/vox-distribution-system-f7e4c0
```

- [ ] **Step 6: Open the PR once, review-ready**

```bash
gh pr create --title "fix(release): repair the release pipeline and ship optimized artifacts" --body "$(cat <<'PRBODY'
Phases 0 and 1a of the Vox distribution system design.

**Phase 0 — the pipeline produced nothing.**
- `release-binaries.yml` passed `--package all`, which built the deleted
  `vox-bootstrap` crate. Every matrix leg failed and no release artifact has
  ever been published.
- All 16 bundle cells built `--out bundle.tar.gz` and attached that literal
  name, so they overwrote each other on upload.
- The matrix built `vox-cloud-only`, removed from `catalog.toml`, failing two
  jobs on every release behind `fail-fast: false`.

**Phase 1a — the output was never optimized.**
- `[profile.dist]` was referenced by nothing; all nine build sites used
  `--release` (thin LTO, debuginfo retained).
- `panic = "abort"` is removed from `dist` first: `spawn_supervised`, the
  `jj_actor` `guarded!` macro, and `memory_cache` all rely on unwinding, so
  enabling the profile as written would have converted three panic-containment
  points into process kills.
- `publish` now declares `needs: [build, dist-verify]`. Tag pushes cannot be
  gated by required checks, so ordering has to be structural.
- Release workflows pin 1.96.0 instead of floating `@stable`.

Spec: `docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
PRBODY
)"
```

Do not re-push to trigger re-review — comment `@coderabbitai review`.

---

## Follow-on plans

- **Phase 1b — security floor.** Three fixes under ten lines each: delete `install.sh`'s fail-open hash branch; explicit tar entry validation with a `#[cfg(unix)]` slip test; job-scoped `permissions:`. Then plugin integrity — required `sha256` in the catalog, verified at **load**, not just install. **Gates Phase 2.**
- **Phase 1c** — `vox ci gen-installer-manifests` plus its five registration points; installer naming via the `release_artifacts` SSOT; a behavioural `detect_target` test to replace the comment-grep guard.
- **Phase 2** — installers (greenfield), signing repair, feature tree, hardware gating, uninstall. Blocked on signing certificates.
- **Phase 3** — nightly channel, git-cliff changelog SSOT, matrix expansion, SBOM and provenance, container tagging, crates.io.
- **Phase 4** — GUI updater, GUI release orchestration, managed-install refusal signal, the second updater (`vox upgrade`), model pull, full clean-room matrix.
