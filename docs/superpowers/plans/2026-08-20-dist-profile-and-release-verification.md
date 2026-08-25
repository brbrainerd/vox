# Working Release Pipeline — Implementation Plan (Phases 0 + 1a)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a `v*` tag produce correct, uniquely-named, genuinely-optimized artifacts that are verified before they publish — starting from a pipeline that currently produces nothing at all.

**Architecture:** Nine tasks in dependency order. Phase 0 repairs the pipeline (a deleted crate breaks every matrix leg; all 16 bundle artifacts collide on one filename; the matrix names a bundle that does not exist; the documented install URL 404s). Phase 1a then makes the output optimized: remove `panic = "abort"` *before* enabling `[profile.dist]` — three production paths depend on unwinding — switch the shipped build sites, pin the release toolchain, and add a verification job that `publish` structurally depends on.

**Tech Stack:** Rust 1.96.0, `cargo`, `toml`, `serde_yaml`, GitHub Actions.

**Spec:** [`docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`](../specs/2026-08-20-vox-distribution-system-design.md) (revision 4)

## Global Constraints

- Rust toolchain is pinned to **1.96.0**. Never `dtolnay/rust-toolchain@stable`.
- **Never run `cargo fmt --all`** (Windows `CreateProcess` overflow, `os error 206`). Use `cargo run -q -p vox-cli -- run scripts/fmt.vox`.
- **`vox` is not on PATH in this worktree.** Every invocation is `cargo run -q -p vox-cli -- <args>`.
- **Verify with `vox ci pre-push --full`, not `--complete`** — `--complete` runs no tests (`crates/vox-cli/src/commands/ci/pre_push.rs:8-10`), and this plan is mostly tests.
- **Do not add a workspace crate-to-crate dependency edge.** Reading files in tests is not an edge. `toml` (`crates/vox-cli/Cargo.toml:208`) and `serde_yaml` (`:156`) are already normal dependencies of `vox-cli`.
- **Assert on parsed structures, never on source text.** This repo already depends on `toml`, `serde_yaml`, and `serde_json`. A `!text.contains(...)` assertion fails on comments, passes on reworded bugs, and — when combined with `include_str!` of the test's own file — can never pass at all, because the assertion message becomes part of the corpus being scanned. Where a source scan is genuinely unavoidable, split the needle with `concat!` and keep the literal out of the message.
- `[profile.dist]` after Task 5 is: `inherits = "release"`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`. **No `panic` key.**
- Test-first is binding. **This plan opens the only PR on this branch** — the Phase 1b plan branches off it (see Task 9).

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `docs-astro/public/` + `_redirects` | Route the documented install URL | 1 |
| `crates/vox-cli-ci/src/cmd_enums.rs` | `ReleasePackage` tiers | 2 |
| `crates/vox-cli/src/commands/ci/release_build.rs` | Release builder + its test module (shared with Phase 1b) | 2, 3, 4, 5, 6, 7, 8 |
| `.github/workflows/bundle-release.yml` | Bundle artifacts: naming, matrix, profile | 3, 4, 6 |
| `Cargo.toml` | `[profile.dist]` | 5 |
| `.github/workflows/release-{binaries,installers,gui}.yml`, `Dockerfile` | Shipped-artifact builds | 6, 7, 8 |
| `crates/vox-cli/tests/dist_binary_e2e.rs` | Black-box verification of the shipped binary | 7 |

---

### Task 1: Route the documented install URL

`docs/src/reference/installation.md:18` and both script headers advertise `https://voxlang.org/voxup`. `docs-astro/public/` contains no such file and `_redirects` has no rule. **`curl https://voxlang.org/voxup | sh` pipes a 404 page into a shell.** The spec assigns this to Phase 0; revision 2 of this plan had no task for it.

**Files:**
- Create: `docs-astro/public/voxup`, `docs-astro/public/voxup.ps1`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing later tasks consume.

- [ ] **Step 1: Confirm the gap is still real**

Run: `ls docs-astro/public/ && grep -n voxup docs-astro/public/_redirects || echo "NO ROUTE"`

Expected: `NO ROUTE`, and no `voxup` entry in the directory listing.

- [ ] **Step 2: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
/// The install command we document must resolve. `docs/src/reference/installation.md`
/// and both script headers advertise https://voxlang.org/voxup ; if nothing is
/// served there, `curl … | sh` pipes a 404 page into a shell.
#[test]
fn documented_install_urls_are_served() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (advertised, served) in [("voxup", "docs-astro/public/voxup"),
                                 ("voxup.ps1", "docs-astro/public/voxup.ps1")] {
        assert!(
            root.join(served).is_file(),
            "https://voxlang.org/{advertised} is documented but {served} does not exist"
        );
    }
    // The served copies must not drift from the canonical scripts.
    for (served, canonical) in [("docs-astro/public/voxup", "scripts/install.sh"),
                                ("docs-astro/public/voxup.ps1", "scripts/install.ps1")] {
        let a = std::fs::read_to_string(root.join(served)).expect("read served copy");
        let b = std::fs::read_to_string(root.join(canonical)).expect("read canonical script");
        assert_eq!(
            a, b,
            "{served} has drifted from {canonical}; regenerate it in the same commit"
        );
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib documented_install_urls_are_served -- --nocapture`

Expected: FAIL with `docs-astro/public/voxup does not exist`.

- [ ] **Step 4: Write the implementation**

```bash
cp scripts/install.sh docs-astro/public/voxup
cp scripts/install.ps1 docs-astro/public/voxup.ps1
```

Astro copies `public/` verbatim to the site root, so `https://voxlang.org/voxup` serves the script with no `_redirects` rule needed. Add a note at the top of `scripts/install.sh` and `scripts/install.ps1` recording that `docs-astro/public/` holds published copies kept in sync by `documented_install_urls_are_served`.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib documented_install_urls_are_served -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add docs-astro/public/voxup docs-astro/public/voxup.ps1 scripts/install.sh scripts/install.ps1 crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(docs): serve the install scripts at the URL we document

installation.md and both script headers advertise https://voxlang.org/voxup,
but docs-astro/public/ had no such file and _redirects no rule — the
documented curl|sh command piped a 404 page into a shell.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Stop building a crate that no longer exists

`release-binaries.yml:43` passes `--package all`, which sets `want_bootstrap` and shells `cargo build -p vox-bootstrap`. That crate is absent from `crates/` and from `Cargo.lock`. Every matrix leg fails and no release has ever been produced.

**Files:**
- Modify: `crates/vox-cli-ci/src/cmd_enums.rs:9-20`
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs` — `want_bootstrap` (`:43-51`), the build block (`:62-75`), `bootstrap_executable_name` (`:116-121`), and its tests at `:200`, `:219-225`, `:255-256`, `:275-281`
- Modify: `.github/workflows/release-binaries.yml` — comment `:41`, smoke loops `:93-107` and `:119-133`, upload globs `:167`, `:169`
- Modify: `docs/src/reference/cli.md:243`, `docs/src/ci/binary-release-contract.md`, `docs/src/ci/workflow-enumeration.md:20`, `docs/src/reference/ref-installation.md:28,42`
- Modify: `docs/agents/script-registry.json:161,168`, `scripts/quality/audit-dependency-layers.vox:22`, `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs:385,427`
- Modify: `AGENTS.md` §Retired Surfaces
- Test: `crates/vox-cli/src/commands/ci/release_build.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `ReleasePackage` reduced to `Vox | Mens | All`, and `pub(crate) const RELEASE_PACKAGES: &[&str] = &["vox-cli", "vox-ml-cli"];` — the single source the builder and its test both read.

- [ ] **Step 1: Sweep for every reference before touching anything**

Run:

```bash
grep -rn "vox-bootstrap\|vox-schola\|ReleasePackage::Bootstrap\|ReleasePackage::Both" .github/ scripts/ docs/src/ docs/agents/ crates/vox-cli/src crates/vox-cli-ci/src AGENTS.md || echo none
```

Expected: hits in every file listed above. Record the list — Step 5 must clear all of them. Revision 2 of this plan used a narrower pattern (`"package bootstrap\|package both"`) and missed six locations, including `docs/src/reference/cli.md:243`, which spells it `--package vox|bootstrap|both`.

- [ ] **Step 2: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
/// Every crate the release builder shells out to must exist. `vox-bootstrap`
/// was deleted from the workspace but `--package all` kept building it, so
/// every release matrix leg failed and no artifact was ever published.
///
/// Reads `RELEASE_PACKAGES` — the same constant `run()` uses — rather than a
/// hardcoded list, so adding a package to the builder cannot bypass this.
#[test]
fn every_release_package_exists_in_the_workspace() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).expect("read Cargo.lock");
    for pkg in super::RELEASE_PACKAGES {
        assert!(
            root.join("crates").join(pkg).is_dir(),
            "release_build shells `cargo build -p {pkg}` but crates/{pkg}/ does not exist"
        );
        assert!(
            lock.contains(&format!("name = \"{pkg}\"")),
            "release_build shells `cargo build -p {pkg}`, absent from Cargo.lock"
        );
    }
}

/// `--package all` must still parse, and the retired tiers must not.
#[test]
fn release_package_value_enum_matches_the_shipped_tiers() {
    use clap::ValueEnum;
    let names: Vec<String> = vox_cli_ci::cmd_enums::ReleasePackage::value_variants()
        .iter()
        .filter_map(|v| v.to_possible_value().map(|p| p.get_name().to_string()))
        .collect();
    assert_eq!(
        names,
        vec!["vox".to_string(), "mens".to_string(), "all".to_string()],
        "ReleasePackage tiers changed; `bootstrap` and `both` built a deleted crate"
    );
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-cli --lib -- release_package every_release_package --nocapture`

Expected: FAIL to compile — `cannot find value 'RELEASE_PACKAGES' in module 'super'`.

- [ ] **Step 4: Reduce the enum**

In `crates/vox-cli-ci/src/cmd_enums.rs`:

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

- [ ] **Step 5: Update the builder and clear every stale reference**

In `crates/vox-cli/src/commands/ci/release_build.rs`, below the `pub use … SUPPORTED_RELEASE_TARGETS;` line:

```rust
/// Crates the release builder shells `cargo build -p` for. Asserted against the
/// workspace by `every_release_package_exists_in_the_workspace`.
pub(crate) const RELEASE_PACKAGES: &[&str] = &["vox-cli", "vox-ml-cli"];
```

Then delete the `want_bootstrap` binding and its whole `if want_bootstrap { … }` block; change `want_vox` to `matches!(package, ReleasePackage::Vox | ReleasePackage::All)`; delete `bootstrap_executable_name`, its `use` import in the tests block, and the assertions at `:219-225`; and retarget the `artifact_filename` (`:255-256`) and checksum (`:275-281`) tests at `vox-ml-cli`.

In `.github/workflows/release-binaries.yml`: rewrite the comment at `:41` to `# --package all produces: vox, vox-ml-cli. voxup is built separately below.`, and **delete** the dead smoke loops (`:93-107`, `:119-133`) and upload globs (`:167`, `:169`) for `vox-bootstrap-*` and `vox-schola-*`.

Then clear the stale documentation. None of it is generated, so no regeneration command applies — but two of these files are `DOCS_SSOT_FILES` (`crates/vox-cli-ci/src/constants.rs:12`) read by `check_install_policy_surfaces` inside `ssot-drift`:

- `docs/src/reference/cli.md:243` — `[--package vox|bootstrap|both]` → `[--package vox|mens|all]`; drop the bootstrap/both tier rows at `:33,35`.
- `docs/src/ci/binary-release-contract.md` — bootstrap archive names (`:41-42`), extracted-file rows (`:52-53`), the `vox-bootstrap --help` smoke (`:81`), `cargo test -p vox-bootstrap` (`:107`), plus the frontmatter description (`:3`) and prose at `:17,33,87,92`.
- `docs/src/ci/workflow-enumeration.md:20` — still claims `--package both`.
- `docs/src/reference/ref-installation.md:28,42` — the standalone `vox-bootstrap` download narrative.
- `docs/agents/script-registry.json:161,168` and `scripts/quality/audit-dependency-layers.vox:22` — stale `vox-bootstrap` entries.
- `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs:385,427` — allowlist entries for a directory that does not exist.
- `AGENTS.md` §Retired Surfaces — add a `vox-bootstrap` → `voxup` / `scripts/install.{sh,ps1}` row.

No deprecation marker is needed. That marker tracks vestigial code in an *active migration*; `vox-bootstrap` is already deleted, so these are dangling references and outright removal is correct.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib release_build -- --nocapture`

Expected: PASS.

- [ ] **Step 7: Confirm the docs gate is satisfied**

Run: `cargo run -q -p vox-cli -- ci command-compliance`

Expected: exit 0. `check_install_policy_surfaces` (`validators.rs:765-825`) reads `binary-release-contract.md`.

`vox ci command-sync` and `command_catalog_paths_baseline.txt` are **not** affected — both are registry-derived and record command *paths*, never `ValueEnum` variants. Do not go hunting for a regeneration step.

- [ ] **Step 8: Prove a release build now completes**

Run: `cargo run -q -p vox-cli -- ci release-build --target x86_64-pc-windows-msvc --version v0.0.0-local --out-dir target/rel-smoke --package all`

Substitute your host triple if not on Windows. Expected: exit 0, with `vox-…` and `vox-ml-cli-…` archives plus `checksums.txt`. **This is the first time this command has succeeded.**

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "fix(release): stop building the retired vox-bootstrap crate

release-binaries.yml passes --package all, which shelled `cargo build -p
vox-bootstrap`. That crate is absent from crates/ and Cargo.lock, so every
matrix leg failed and no release artifact has ever been published. Removes the
Bootstrap/Both tiers, clears eight stale references, and binds the builder's
package list to the workspace with a test.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Give bundle artifacts unique, versioned names

All 16 matrix cells build `--out bundle.tar.gz` and attach that literal name to the release. They overwrite each other, and none appears in `checksums.txt`.

**Files:**
- Modify: `.github/workflows/bundle-release.yml:80-112`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: artifact name shape `<bundle>-<tag>-<target>.tar.gz`, matching the `release_artifacts::artifact_filename` SSOT.

- [ ] **Step 1: Write the failing test**

Parse the YAML — do not grep it. A substring assertion is satisfied by one surviving occurrence anywhere, including a comment, while the actual `--out` reverts to a fixed name.

```rust
/// Bundle artifacts must be parameterised by BOTH matrix axes, or the 16
/// uploads collide and the survivor identifies neither bundle nor target.
#[test]
fn every_bundle_artifact_name_is_matrix_unique() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join(".github/workflows/bundle-release.yml"))
        .expect("read bundle-release.yml");
    let v: serde_yaml::Value = serde_yaml::from_str(&text).expect("workflow must be valid YAML");

    let steps = v["jobs"]["build-bundles"]["steps"]
        .as_sequence()
        .expect("build-bundles must have steps");

    let mut checked = 0usize;
    for step in steps {
        let blob = serde_yaml::to_string(step).expect("re-serialise step");
        // Steps that name an artifact: the build (--out) and the release attach (files:).
        let names_artifact = blob.contains("--out") || !step["with"]["files"].is_null();
        if !names_artifact {
            continue;
        }
        checked += 1;
        assert!(
            blob.contains("matrix.bundle") && blob.contains("matrix.target"),
            "bundle artifact name is not parameterised by both matrix axes:\n{blob}"
        );
    }
    assert!(checked >= 2, "expected at least a build step and an attach step, saw {checked}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib every_bundle_artifact_name_is_matrix_unique -- --nocapture`

Expected: FAIL — the build step names `bundle.tar.gz` with neither matrix axis.

- [ ] **Step 3: Write the implementation**

Replace `.github/workflows/bundle-release.yml` **lines 80–112**. The range matters: the `Attach to GitHub Release` step runs through `:112`, and replacing only `:80-110` leaves an orphaned `env:`/`GITHUB_TOKEN` pair after the new block's own `env:` — a **duplicate mapping key, and the workflow will not load at all.**

```yaml
      - name: Build bundle tarball
        env:
          VOX_PLUGINS_DIR: ${{ github.workspace }}/bundle-plugins
          BUNDLE_ARTIFACT: ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
        # `cargo run --release` here builds the bundler TOOL, not a shipped
        # artifact — the tarball contents come from `bundle build`. Keep it on
        # --release: fat LTO would add three ~30-minute links per matrix cell
        # for zero change to what ships.
        run: |
          cargo run --release --locked -p vox-cli -- \
            bundle build ${{ matrix.bundle }} \
            --out "$BUNDLE_ARTIFACT"
        shell: bash

      - name: Verify bundle integrity
        env:
          VOX_PLUGINS_DIR: ${{ github.workspace }}/bundle-plugins
          BUNDLE_ARTIFACT: ${{ matrix.bundle }}-${{ github.ref_name }}-${{ matrix.target }}.tar.gz
        run: cargo run --release --locked -p vox-cli -- bundle verify "$BUNDLE_ARTIFACT"
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

`sha256sum` is available on both legs — the Linux leg is the Docker fleet image, and the Windows leg runs `shell: bash` (Git Bash, which ships GNU coreutils).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib every_bundle_artifact_name_is_matrix_unique -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Confirm the workflow still parses**

Run: `cargo run -q -p vox-cli -- ci workflow-concurrency-guard --strict`

Expected: exit 0. `--strict` matters — the guard is advisory by default and exits 0 regardless.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/bundle-release.yml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): bundle artifacts collided on a single filename

All 16 matrix cells built --out bundle.tar.gz and attached that exact name, so
they overwrote each other and the survivor identified neither bundle nor
target. Names now follow the release_artifacts SSOT and carry a sha256.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Make the bundle matrix gate platform-aware

`bundle-release.yml:36` builds `vox-cloud-only`, absent from `catalog.toml`. `bundle_resolved` returns `ResolveError::UnknownBundle` and the step has no `continue-on-error`, so two jobs fail on every release; `fail-fast: false` hid it.

**Files:**
- Modify: `.github/workflows/bundle-release.yml:29-38` and the header comment at `:7`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `MATRIX_EXCLUDED_BUNDLES` and two helpers, private to the tests module.

- [ ] **Step 1: Write the failing test**

```rust
/// Bundles the x86-64 Linux + Windows matrix deliberately does not build.
/// `vox-ml-metal` carries an Apple-Metal plugin; `vox-mobile` is status="alpha",
/// planned v0.8. Adding either would spawn jobs that cannot succeed here.
const MATRIX_EXCLUDED_BUNDLES: &[&str] = &["vox-ml-metal", "vox-mobile"];

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

/// Read the matrix from parsed YAML. A hand-rolled line scanner gives wrong
/// answers on comments, quoted ids, flow style, anchors, and any second
/// `bundle:` key elsewhere in the file.
fn workflow_matrix_bundle_ids(yml: &str) -> Vec<String> {
    let v: serde_yaml::Value = serde_yaml::from_str(yml).expect("workflow must be valid YAML");
    let mut ids: Vec<String> = v["jobs"]["build-bundles"]["strategy"]["matrix"]["bundle"]
        .as_sequence()
        .expect("matrix.bundle must be a list")
        .iter()
        .map(|x| x.as_str().expect("bundle id must be a string").to_string())
        .collect();
    ids.sort();
    ids
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
        "bundle-release.yml matrix must match the buildable [[bundle]] ids in \
         catalog.toml (excluding {MATRIX_EXCLUDED_BUNDLES:?}).\n  \
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

Run: `cargo test -p vox-cli --lib -- bundle_release catalog_bundle --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/bundle-release.yml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(release): matrix built a phantom bundle on every release

vox-cloud-only was removed from catalog.toml but stayed in the matrix, so
bundle_resolved returned UnknownBundle and two jobs failed on every release
(hidden by fail-fast: false). Adds a platform-aware parity gate that parses
the workflow YAML rather than scanning it.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Remove `panic = "abort"` from `[profile.dist]` before enabling it

Three production paths depend on unwinding. Task 6 enables `[profile.dist]`, so this must land first or Task 6 ships the regression.

**Files:**
- Modify: `Cargo.toml` (`[profile.dist]`, `:424-429`)
- Test: `crates/vox-cli/src/commands/ci/release_build.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: a `dist` profile safe for Task 6 to enable.

- [ ] **Step 1: Confirm the three call sites are real**

Run: `grep -rn "catch_unwind\|resume_unwind\|is_panic()" --include=*.rs crates/vox-actor-runtime/src crates/vox-vcs/src crates/vox-search/src`

Expected: hits in `supervisor.rs`, `jj_actor.rs`, `memory_cache.rs`, none inside `#[cfg(test)]`. If they have moved, update the test's doc comment to the new locations.

- [ ] **Step 2: Write the failing test**

```rust
/// `[profile.dist]` must not abort on panic. Three non-test paths in the shipped
/// binary rely on unwinding:
///   - vox-actor-runtime/src/supervisor.rs:30,52 — spawn_supervised matches on
///     JoinError::is_panic(); under abort a panicking task kills the process and
///     every caller silently loses supervision.
///   - vox-vcs/src/jj_actor.rs:196,282 — the `guarded!` macro catch_unwinds
///     block_on so a panicking jj-lib call returns Err(Unavailable) rather than
///     killing the actor loop. `jj` is a default feature of vox-orchestrator,
///     which vox-cli takes with defaults, so this ships.
///   - vox-search/src/memory_cache.rs:88 — resume_unwind on a spawn_blocking panic.
///
/// Also guards the two other routes abort could arrive by: inheritance from
/// [profile.release], and a global rustflag.
#[test]
fn dist_profile_does_not_abort_on_panic() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    let v: toml::Value = manifest.parse().expect("workspace Cargo.toml must parse");

    let dist = v["profile"].get("dist").expect("[profile.dist] must exist");
    assert!(
        dist.get("panic").is_none(),
        "[profile.dist] must not set `panic`; abort breaks catch_unwind-based \
         panic containment in supervisor.rs and jj_actor.rs"
    );
    assert!(
        v["profile"]["release"].get("panic").is_none(),
        "[profile.release] sets `panic`; [profile.dist] inherits from it"
    );

    // The optimization settings ARE the point of the profile — keep them.
    assert_eq!(dist.get("lto").and_then(|x| x.as_str()), Some("fat"));
    assert_eq!(dist.get("codegen-units").and_then(|x| x.as_integer()), Some(1));
    assert_eq!(dist.get("strip").and_then(|x| x.as_str()), Some("symbols"));

    let cargo_cfg = std::fs::read_to_string(root.join(".cargo/config.toml")).unwrap_or_default();
    assert!(
        !cargo_cfg.replace(' ', "").contains("panic=abort"),
        ".cargo/config.toml sets panic=abort globally, bypassing the profile"
    );
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib dist_profile_does_not_abort_on_panic -- --nocapture`

Expected: FAIL with `[profile.dist] must not set 'panic'`.

- [ ] **Step 4: Write the implementation**

```toml
# Shipped-artifact profile. Fat LTO + codegen-units=1 for runtime speed,
# symbols stripped for size.
#
# Deliberately does NOT set `panic = "abort"`. The workspace relies on unwinding
# in three non-test paths: vox-actor-runtime's spawn_supervised
# (JoinError::is_panic), vox-vcs's `guarded!` macro (catch_unwind around jj-lib,
# which ships via vox-orchestrator's default features), and vox-search's
# memory_cache (resume_unwind). Abort would turn each containment point into a
# process kill.
#
# `strip = "symbols"` does cost symbolicated backtraces on Linux/macOS (release
# strips only debuginfo). Nothing in the workspace captures a Backtrace, sets a
# panic hook, or asserts on backtrace content, so the only loss is
# RUST_BACKTRACE=1 quality in a user-reported panic. Switch to
# `strip = "debuginfo"` if that becomes a support burden.
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

Three shipped paths rely on unwinding for panic containment (spawn_supervised,
the jj_actor guarded! macro, memory_cache). Enabling [profile.dist] with
panic=abort would have silently converted each into a process kill.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Build every shipped artifact at `[profile.dist]`

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs:150`, `:171-174`
- Modify: `.github/workflows/release-binaries.yml` `:46`, `:51`, `:62`, timeout `:19`
- Modify: `.github/workflows/release-installers.yml` `:64`, `:66`, `:82`, `:91`, `:93`
- Modify: `.github/workflows/release-gui.yml` `:90-91`, `:96-100`, `:104`, `:111`, `:117`
- Modify: `.github/workflows/bundle-release.yml:67`
- Modify: `Dockerfile:21`, `:23`, `:32`
- Modify: timeouts in `bundle-release.yml:25`, `release-gui.yml:21`, `ci.yml` docker smoke, `docker-eval.yml`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs`

**Interfaces:**
- Consumes: the panic-free `[profile.dist]` from Task 5.
- Produces: `pub(crate) const DIST_PROFILE: &str = "dist";` and `pub(crate) fn built_binary_path(repo_root: &Path, target: &str, bin: &str) -> PathBuf`.

**Do NOT change `crates/vox-gui/tauri.conf.json`.** Its `externalBin` value `../../target/release/vox` is a **staging convention, not a build output** — every workflow already copies into it. Seven consumers read it (`build.rs` autobuild, `vox doctor`'s sidecar check, `gui-build.vox`, and four workflows), and changing it makes `crates/vox-gui/build.rs:56` build into `target/release/` then look in `target/dist/`, panicking on every fresh worktree. Change only the copy **source**.

**Two build sites deliberately stay on `--release`:** `release-installers.yml:23` and `:119` build `voxup` solely to run `--help` and an install E2E inside `timeout-minutes: 30` jobs. Fat LTO would turn 2-minute builds into ~30-minute ones and time both out, for zero shipped bytes.

- [ ] **Step 1: Write the failing test**

Assert the **positive** — absence of `--release` is not presence of `--profile dist` (`cargo build -p vox-cli` with no flag passes an absence check while shipping a debug binary). Also assert the path fallout, which is the likeliest failure mode: switch the flag, forget a `target/release/` read, and you get a green test with a workflow that dies at the next step.

```rust
/// Build steps that produce a SHIPPED artifact must use --profile dist, and no
/// shipped-artifact workflow may still read from target/release/.
///
/// Comments are stripped before scanning: release-installers.yml documents the
/// old command in prose, and a blanket file-level `contains` assertion is
/// unsatisfiable because of it.
#[test]
fn shipped_build_steps_use_the_dist_profile() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let flag = concat!("--", "release");

    // (file, build lines that legitimately stay on release, target/release paths
    //  that legitimately remain)
    let shipped: &[(&str, &[&str], &[&str])] = &[
        (".github/workflows/release-binaries.yml", &[], &[]),
        // voxup is built here only to smoke `--help` and run an install E2E;
        // fat LTO would blow the 30-minute job budget for zero shipped bytes.
        // Because it stays on --release, its output stays at target/release/.
        (
            ".github/workflows/release-installers.yml",
            &["-p voxup"],
            &["target/release/voxup"],
        ),
        // The Tauri sidecar STAGING destination is target/release/vox-<triple>
        // and must not move — tauri.conf.json's externalBin is read by seven
        // consumers. Only the copy SOURCE moves to dist. `bundle` is Tauri's
        // own output dir, unrelated to the cargo profile.
        (
            ".github/workflows/release-gui.yml",
            &[],
            &["target/release/vox-", "target/release/bundle"],
        ),
        // `cargo run … -- bundle` builds the bundler tool, not a shipped artifact.
        (".github/workflows/bundle-release.yml", &["cargo run"], &[]),
        ("Dockerfile", &[], &[]),
    ];

    for (rel, allowed_flags, allowed_paths) in shipped {
        let text = std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("read {rel}: {e}"));
        for (i, line) in text.lines().enumerate() {
            let code = line.split('#').next().unwrap_or("");
            if !code.contains("cargo build") && !code.contains("cargo run") {
                continue;
            }
            if allowed_flags.iter().any(|a| code.contains(a)) {
                continue;
            }
            assert!(
                !code.contains(flag),
                "{rel}:{} builds a shipped artifact with the release profile:\n  {}",
                i + 1,
                code.trim()
            );
            assert!(
                code.contains("--profile dist"),
                "{rel}:{} builds a shipped artifact without --profile dist:\n  {}",
                i + 1,
                code.trim()
            );
        }
        // Path fallout: switching the flag relocates output, so a surviving
        // `target/release` read is a job that dies at the next step.
        for (i, line) in text.lines().enumerate() {
            let code = line.split('#').next().unwrap_or("");
            if !code.contains("target/release") {
                continue;
            }
            if allowed_paths.iter().any(|a| code.contains(a)) {
                continue;
            }
            panic!(
                "{rel}:{} still reads target/release/ after the profile switch:\n  {}",
                i + 1,
                code.trim()
            );
        }
    }
}

/// The builder writes to and reads from target/<triple>/<profile>/.
#[test]
fn built_binary_path_uses_the_dist_profile() {
    let p = super::built_binary_path(std::path::Path::new("/repo"), "x86_64-unknown-linux-gnu", "vox");
    assert!(
        p.ends_with("target/x86_64-unknown-linux-gnu/dist/vox"),
        "built artifacts must be read from the dist profile dir, got {}",
        p.display()
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli --lib -- shipped_build_steps built_binary_path --nocapture`

Expected: FAIL to compile (`built_binary_path` undefined), then FAIL naming the first workflow still on the release profile.

- [ ] **Step 3: Change the builder**

Below the `pub use … SUPPORTED_RELEASE_TARGETS;` line:

```rust
/// Cargo profile used for every shipped artifact.
///
/// `[profile.dist]` sets `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`.
/// Plain `--release` is thin-LTO and keeps debuginfo — see spec finding F6.
pub(crate) const DIST_PROFILE: &str = "dist";

/// Where cargo writes a `--target <triple> --profile dist` binary.
pub(crate) fn built_binary_path(repo_root: &Path, target: &str, bin: &str) -> PathBuf {
    repo_root.join("target").join(target).join(DIST_PROFILE).join(bin)
}
```

In `build_and_package_binary`, replace the `"--release",` element with `"--profile", DIST_PROFILE,`, and replace the output path lookup with `built_binary_path(repo_root, target, built_bin_name)`.

- [ ] **Step 4: Change every shipped build site and its read paths**

Switching the flag relocates output to `target[/<triple>]/dist/`. Every read path must move in the same edit:

| File | Change |
|---|---|
| `release-binaries.yml:46` | `cargo build --profile dist --locked -p voxup --target …` |
| `release-binaries.yml:51` | `cd target/${{ matrix.target }}/dist` |
| `release-binaries.yml:62` | `-Path "target\…\dist\voxup.exe"` |
| `release-installers.yml:64` | `cargo wix --no-build --profile dist --package vox-cli` |
| `release-installers.yml:66` | comment: `target/release/vox.exe` → `target/dist/vox.exe` |
| `release-installers.yml:82` | `cargo deb -p vox-cli --profile dist` |
| `release-installers.yml:91` | `cargo build --profile dist --locked -p vox-cli` |
| `release-installers.yml:93` | `tar -czvf … -C target/dist vox` |
| `release-gui.yml:90,91,104` | `cargo build -p vox-cli --profile dist --target …` |
| `release-gui.yml:96-100,111,117` | copy **source** → `target/<triple>/dist/vox[.exe]`; **destination stays `target/release/vox-<triple>`** |
| `bundle-release.yml:67` | `cargo build --profile dist --locked -p vox-cli --target …` |
| `Dockerfile:21` | `cargo build --profile dist -j 1 --locked …` |
| `Dockerfile:23` | **delete the `&& strip /app/target/release/vox`** — `strip = "symbols"` already did it |
| `Dockerfile:32` | `COPY --from=builder /app/target/dist/vox` |

`cargo-wix` and `cargo-deb` both default to `target/release/`: [`crates/vox-cli/wix/main.wxs:31-41`](../../../crates/vox-cli/wix/main.wxs) documents that `CargoTargetBinDir` defaults to `target\release\`, and `cargo-deb` runs its own `--release`. Neither has a hardcoded path in `Cargo.toml`, so the `--profile` flag is the entire fix.

- [ ] **Step 5: Raise every timeout that now pays for fat LTO**

Fat LTO with `codegen-units = 1` is 5–20× the link time of thin LTO. Four jobs besides `release-binaries` gain one:

| File | Job | From | To |
|---|---|---|---|
| `release-binaries.yml:19` | `build` | 60 | 180 |
| `bundle-release.yml:25` | `build-bundles` | 60 | 180 |
| `release-gui.yml:21` | `build-tauri` | 90 | 240 |
| `ci.yml` (docker smoke, ~`:1610`) | `docker-vox-image-smoke` | 30 | 180 |
| `docker-eval.yml` | image build | 45 | 180 |

The two Docker jobs build `Dockerfile` with `-j 1`, so they pay the fat-LTO cost twice (default and mesh images). Verify each current value before editing.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib -- shipped_build_steps built_binary_path --nocapture`

Expected: PASS.

- [ ] **Step 7: Prove the artifact is really optimized**

Run: `cargo run -q -p vox-cli -- ci release-build --target x86_64-pc-windows-msvc --version v0.0.0-local --out-dir target/dist-smoke --package vox`

Substitute your host triple if not on Windows. Expected: exit 0, archive present. **Budget 90–180 minutes cold on a 4-core machine** — `codegen-units = 1` applies to the whole dependency rebuild, not just the final link.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "fix(release): build all shipped artifacts with --profile dist

[profile.dist] was defined but referenced by nothing; every shipped binary was
thin-LTO with debuginfo. Covers release_build.rs, voxup, the GUI sidecar,
MSI/deb inputs, bundle tarballs, and the daily public Docker image, moving all
read paths to target/dist and raising five timeouts for the fat-LTO cost.
tauri.conf.json is deliberately unchanged: its externalBin is a staging
convention, and repointing it panics vox-gui's build.rs in fresh worktrees.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Verify the dist binary, and make `publish` depend on it

Tag pushes **cannot** be gated by GitHub required checks — those apply to branch and PR refs. The only reliable ordering mechanism is `needs:` inside one workflow.

**Files:**
- Create: `crates/vox-cli/tests/dist_binary_e2e.rs`
- Modify: `.github/workflows/release-binaries.yml` (add `dist-verify`; change `publish`'s `needs:`)
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (the gate assertion)

**Interfaces:**
- Consumes: `DIST_PROFILE` (Task 6).
- Produces: the `VOX_DIST_BIN` contract — when set, a missing binary is a hard failure.

- [ ] **Step 1: Write the gate test first**

The load-bearing claim of this task is *"publish cannot run before verification."* Nothing in revision 2 asserted it — deleting the job, or dropping it from `needs:`, left every test green.

```rust
/// A tag push cannot be gated by GitHub required checks, so the ordering between
/// verification and publication must be structural. Deleting the dist-verify job
/// or dropping it from `needs:` would silently publish unverified artifacts.
#[test]
fn publish_job_is_gated_on_dist_verification() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join(".github/workflows/release-binaries.yml"))
        .expect("read release-binaries.yml");
    let v: serde_yaml::Value = serde_yaml::from_str(&text).expect("workflow must be valid YAML");

    assert!(
        !v["jobs"]["dist-verify"].is_null(),
        "the dist-verify job was removed; artifacts would publish unverified"
    );
    let needs = v["jobs"]["publish"]["needs"]
        .as_sequence()
        .expect("publish must declare a needs: list");
    let names: Vec<&str> = needs.iter().filter_map(|n| n.as_str()).collect();
    assert!(
        names.contains(&"dist-verify"),
        "publish no longer needs dist-verify (needs: {names:?}); a tag would \
         publish while verification was still running, or after it failed"
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-cli --lib publish_job_is_gated_on_dist_verification -- --nocapture`

Expected: FAIL with `the dist-verify job was removed`.

- [ ] **Step 3: Write the black-box suite**

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

/// Proves the binary was actually built at [profile.dist]. Every other test here
/// would pass identically against a debug build; this one would not.
#[cfg(target_os = "linux")]
#[test]
fn dist_binary_is_stripped_of_symbols() {
    let Some(bin) = dist_binary() else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    let bytes = std::fs::read(&bin).expect("read dist binary");
    assert!(
        !bytes.windows(7).any(|w| w == b".symtab"),
        "dist binary retains a symbol table — it was not built at [profile.dist] \
         (strip = \"symbols\")"
    );
}

#[test]
fn dist_binary_version_matches_the_crate() {
    let Some((stdout, _, code)) = run_dist(&["--version"]) else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    assert_eq!(code, 0, "`vox --version` must exit 0");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "`vox --version` must report {}, got {stdout:?}",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn dist_binary_rejects_unknown_subcommand_cleanly() {
    let Some((_, _, code)) = run_dist(&["definitely-not-a-real-subcommand"]) else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    // clap's parse-error path exits 2. A process killed by SIGABRT yields no
    // exit code at all, which `run_dist` surfaces as -1.
    assert!(
        code > 0 && code < 100,
        "unknown subcommand must exit with a normal error code; got {code} \
         (-1 means killed by a signal, i.e. an abort)"
    );
}

#[test]
fn dist_binary_compiles_and_runs_a_golden_program() {
    let Some(bin) = dist_binary() else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    // A unique dir per run: two concurrent fleet jobs would otherwise race on
    // the same hello.vox, and a fixed path is never cleaned up.
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("hello.vox");
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

- [ ] **Step 4: Prove the hard-failure contract**

Run: `cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: PASS with SKIP lines — the local no-binary state.

Now the property that matters:

Run (bash): `VOX_DIST_BIN=/nonexistent cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`
Run (PowerShell): `$env:VOX_DIST_BIN="/nonexistent"; cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: **FAIL** with `VOX_DIST_BIN=/nonexistent does not exist`. This proves CI cannot silently no-op. Unset the variable afterwards.

- [ ] **Step 5: Run against the real binary**

Run: `cargo build -p vox-cli --profile dist --features heavy-retrieval` then `cargo test -p vox-cli --test dist_binary_e2e -- --nocapture`

Expected: PASS, no SKIP lines. If `dist_binary_rejects_unknown_subcommand_cleanly` reports `-1`, a parse-error path is panicking; convert that call site in `crates/vox-cli/src/cli_dispatch/mod.rs` to return `anyhow::Result`.

- [ ] **Step 6: Add the gating job**

```yaml
  # Verifies the SHIPPED optimization level (spec F8) and GATES publish on it.
  #
  # A job here, not a standalone workflow: tag pushes cannot be gated by GitHub
  # required checks (those apply to branch/PR refs), so two independent
  # `push: tags` workflows have no ordering and the release would publish while
  # verification ran, and publish unaffected if it failed.
  #
  # One lane only: `cargo test --profile dist` across the workspace would
  # fat-LTO-link each of the 80+ integration targets against a 1656-package
  # graph, exceeding the 14 GB runner budget (runner_scale.rs MEM_PER_RUNNER,
  # whose own test cites a measured ~12 GB peak at THIN LTO).
  #
  # No rust-toolchain step: the fleet image ships the pinned 1.96.0 toolchain
  # and rust-toolchain.toml is authoritative.
  dist-verify:
    name: dist verification (fat LTO)
    runs-on: [self-hosted, linux, x64]
    # ~90-180m dep compile at codegen-units=1 + a serial fat-LTO link + the e2e
    # test target. CARGO_BUILD_JOBS caps the dep phase for RSS headroom.
    timeout-minutes: 240
    permissions:
      contents: read
    env:
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

Change `publish` to `needs: [build, dist-verify]`.

- [ ] **Step 7: Verify the workflow gates**

```bash
cargo run -q -p vox-cli -- ci workflow-concurrency-guard --strict
```

```bash
cargo run -q -p vox-cli -- ci runner-policy-check --strict
```

```bash
cargo nextest run -p vox-cli sccache_workflow_guard
```

All exit 0. `release-binaries.yml` has no top-level `concurrency:` block but is registered in `docs/src/ci/concurrency-exceptions.md`, so the guard passes; `runs-on: [self-hosted, linux, x64]` needs no hosted-runner exception.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/tests/dist_binary_e2e.rs .github/workflows/release-binaries.yml crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "test(release): gate publish on black-box verification of the dist binary

Adds a subprocess suite against the real shipped artifact and makes publish
declare needs: [build, dist-verify], with a test asserting that gate exists.
VOX_DIST_BIN makes a wrong artifact path a hard failure rather than a silent
skip, and a .symtab check proves the binary was actually built at
[profile.dist] rather than merely starting.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Pin the release toolchain

**Files:**
- Modify: `.github/workflows/release-binaries.yml:35`, `release-installers.yml:20,58,74,89`, `bundle-release.yml:50`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs`

**Interfaces:** none.

- [ ] **Step 1: Write the failing test**

Assert the pin **matches** `rust-toolchain.toml`. Revision 2's test derived the expected version and then never used it, so a wrong pin would pass.

```rust
/// Release workflows must not float the toolchain. Building shipped artifacts on
/// `@stable` means users get binaries from a compiler no CI gate ever ran, and
/// each new stable silently imports its lint wave.
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

    let floating = concat!("rust-toolchain@", "stable");
    for rel in [
        ".github/workflows/release-binaries.yml",
        ".github/workflows/release-installers.yml",
        ".github/workflows/bundle-release.yml",
    ] {
        let text = std::fs::read_to_string(root.join(rel)).expect("read workflow");
        assert!(
            !text.contains(floating),
            "{rel} floats the toolchain; pin it to {want} (rust-toolchain.toml)"
        );
        if text.contains("dtolnay/rust-toolchain") {
            assert!(
                text.contains(&format!("toolchain: \"{want}\"")),
                "{rel} installs a toolchain other than rust-toolchain.toml's {want}"
            );
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib release_workflows_pin_the_toolchain -- --nocapture`

Expected: FAIL naming `release-binaries.yml`.

- [ ] **Step 3: Write the implementation**

In each of the three workflows, replace every `- uses: dtolnay/rust-toolchain@stable` with:

```yaml
      # Pinned, not @stable: shipped artifacts must be built by the same compiler
      # CI gates use (rust-toolchain.toml). @stable also imports each new
      # release's lint wave — see AGENTS.md §Perennial Bug Patterns.
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"
```

Where the step also passes `targets:`, keep that input — it works identically on `@master`. `release-gui.yml` is deliberately excluded: its toolchain steps carry documented cross-target handling; leave them and their comments alone.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib release_workflows_pin_the_toolchain -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(ci): pin the toolchain in release workflows

Shipped artifacts were built on dtolnay/rust-toolchain@stable while the repo
pins 1.96.0 — a compiler no CI gate ever ran. The test now asserts the pin
matches rust-toolchain.toml rather than merely rejecting @stable.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: Verify the phase and open one PR

- [ ] **Step 1: Format**

Run: `cargo run -q -p vox-cli -- run scripts/fmt.vox`

Expected: exit 0. Never `cargo fmt --all`.

- [ ] **Step 2: Run the full local gate tier**

Run: `cargo run -q -p vox-cli -- ci pre-push --full`

Expected: exit 0. `--full` is required — `--complete` runs no tests.

- [ ] **Step 3: Confirm no crate edge was added**

Run: `cargo run -q -p vox-cli -- ci crate-edges`

Expected: exit 0.

- [ ] **Step 4: Confirm the SSOT gates still pass**

```bash
cargo test -p voxup --test distribution_parity --locked
```

```bash
cargo run -q -p vox-cli -- ci command-compliance
```

Both exit 0. The first proves the distribution SSOT still agrees after Task 2; the second reads `binary-release-contract.md`, which Task 2 edited.

- [ ] **Step 5: Push and open the PR**

```bash
git add -A
git commit -m "style: rustfmt after release pipeline repair

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" || echo "nothing to commit"
git push -u origin claude/vox-distribution-system-f7e4c0
```

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
- `https://voxlang.org/voxup` — the install URL we document — was unrouted, so
  the documented `curl | sh` piped a 404 page into a shell.

**Phase 1a — the output was never optimized.**
- `[profile.dist]` was referenced by nothing; the shipped build sites used
  `--release` (thin LTO, debuginfo retained).
- `panic = "abort"` is removed from `dist` first: `spawn_supervised`, the
  `jj_actor` `guarded!` macro, and `memory_cache` all rely on unwinding, so
  enabling the profile as written would have converted three panic-containment
  points into process kills.
- `publish` now declares `needs: [build, dist-verify]`. Tag pushes cannot be
  gated by required checks, so ordering has to be structural.
- Release workflows pin 1.96.0 instead of floating `@stable`.

`crates/vox-gui/tauri.conf.json` is deliberately unchanged — its `externalBin`
is a staging convention, and repointing it panics `vox-gui`'s `build.rs` in
every fresh worktree.

Spec: `docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
PRBODY
)"
```

**This is the only PR on this branch.** The Phase 1b plan branches off this head and targets it with `--base`; do not push Phase 1b commits here, because `.coderabbit.yaml` sets `auto_incremental_review: false` and they would land in an already-reviewed PR. Request re-review with `@coderabbitai review`, never by re-pushing.

---

## Follow-on plans

- **Phase 1b** — [`2026-08-20-distribution-security-floor.md`](2026-08-20-distribution-security-floor.md). Branches off this PR's head.
- **Phase 1c** — `vox ci gen-installer-manifests` and its five registration points; installer naming via the `release_artifacts` SSOT; a behavioural `detect_target` test to replace the comment-grep guard at `release_build.rs:287-307`.
- **Phase 2** — installers (greenfield), signing repair, feature tree, hardware gating, uninstall. Blocked on signing certificates.
- **Phase 3** — nightly channel, git-cliff changelog SSOT, matrix expansion, SBOM made blocking, provenance, container tagging, crates.io.
- **Phase 4** — GUI updater, GUI release orchestration, managed-install refusal signal, `vox upgrade`'s openclaw sidecar, model pull, full clean-room matrix.
