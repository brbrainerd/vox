# Distribution & Release Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Get Vox to the point where cutting a public release is a one-command, low-risk act — every distribution channel fed from one version, every manifest generated, and every gate that would catch a bad release already running.

**Architecture:** Three layers. Layer 1 turns the version work already landed into enforced CI gates, so drift fails a PR instead of a release. Layer 2 generates each channel's manifest from the release artifact rather than by hand. Layer 3 is the release itself and the cleanups it unblocks — none of which can proceed until a stable tag exists, so they are sequenced last and marked explicitly.

**Tech Stack:** Rust 1.96.0 (pinned), `vox-cli-ci` gates, GitHub Actions, `git-cliff`, `cargo-hakari`, Homebrew, winget, crates.io.

**Spec:** No separate spec. This continues `docs/superpowers/plans/2026-09-04-macos-compatibility.md` (12 of 14 tasks landed in PR #477) and implements the distribution findings from the 2026-09-04 session. Facts were measured on this machine on 2026-09-04; re-verify any that look stale.

## Global Constraints

- **No public release without explicit approval.** Several tasks below are *blocked* on one. They say so and must not be worked around.
- **Toolchain:** Rust `1.96.0` from `rust-toolchain.toml`. `dtolnay/rust-toolchain@stable` sets `RUSTUP_TOOLCHAIN` and **overrides the pin** — use `@master` + `toolchain: "1.96.0"` on any artifact-producing lane.
- **Version SSOT:** `[workspace.package] version` in the root `Cargo.toml`. A bump touches **127 lines across 117 files** — never hand-edit; use `cargo run -q -p vox-cli-ci --example ssot_probe -- <version> --write`.
- **Installer parity:** `scripts/install.sh` ≡ `docs-astro/public/voxup`, `scripts/install.ps1` ≡ `docs-astro/public/voxup.ps1`, byte-for-byte.
- **Repo conventions:** `actions/checkout@v7`; `timeout-minutes` on every job; `shell: bash` and `set -euo pipefail` in multi-line `run:` blocks.
- **Never verify through a pipe.** `cmd | tail` exits with `tail`'s status; that masked two real build failures in one session. Use `cmd > /tmp/x.log 2>&1; echo $?`.
- **Assert on the artifact, not the exit code.** After an install, `ls` what landed. After a test, confirm the run count is non-zero. Every defect in the prior plan was something reporting success without anyone checking.

## Name availability — measured 2026-09-04

| Registry | `vox` | `voxlang` | Notes |
|---|---|---|---|
| crates.io | **TAKEN** (`0.10.0-rc.7`, unrelated) | free | `vox-cli`, `voxup` also free |
| npm | **TAKEN** (unrelated) | free | `@vox/*` scope appears unclaimed |
| Homebrew | **TAKEN** — the VOX music player *cask* | free | why the formula is `voxlang` |
| winget | not checked | not checked | needs `Publisher.Package` form |

---

### Task 1: Gate version drift on every PR

`crates/vox-cli-ci/src/version_ssot.rs` exists and is tested, but **nothing runs it in CI**. The 127-line bump is only safe if drift is caught before a release, not during one.

**Files:**
- Modify: `.github/workflows/cross-platform-check.yml` (add a step to `standalone-installables`)
- Modify: `crates/vox-cli-ci/examples/ssot_probe.rs`

**Interfaces:**
- Consumes: `vox_cli_ci::version_ssot::{workspace_version, path_dependency_versions, npm_versions, workspace_hack_pin, major_minor, drift}`.
- Produces: a non-zero exit from `ssot_probe` when any declaration disagrees — the gate later tasks rely on.

- [ ] **Step 1: Confirm the probe currently exits 0 on a clean tree**

```bash
cd /Users/brbrainerd/dev/vox
cargo run -q -p vox-cli-ci --example ssot_probe > /tmp/ssot.log 2>&1; echo "exit=$?"
cat /tmp/ssot.log
```

Expected: `exit=0`, `✅ no drift`, `workspace-hack pins: all 127 agree (0.6)`.

- [ ] **Step 2: Prove it fails on real drift**

```bash
cp Cargo.toml /tmp/Cargo.toml.bak
sed -i '' 's|vox-secrets                = { path = "crates/vox-secrets", version = "0.6.0" }|vox-secrets                = { path = "crates/vox-secrets", version = "0.5.0" }|' Cargo.toml
cargo run -q -p vox-cli-ci --example ssot_probe > /tmp/ssot2.log 2>&1; echo "exit=$?"
cat /tmp/ssot2.log
cp /tmp/Cargo.toml.bak Cargo.toml
```

Expected: non-zero exit and a `DRIFT` line naming `vox-secrets`. **If it exits 0, the probe's exit path is wrong — fix it before continuing**; a gate that cannot fail is worse than no gate.

- [ ] **Step 3: Add the CI step**

In `.github/workflows/cross-platform-check.yml`, inside the `standalone-installables` job, after the `cargo check (standalone, per-crate)` step:

```yaml
      # The version is restated in 127 places — 9 workspace path deps, 5 npm
      # declarations, and a hakari-generated workspace-hack pin in all 113 member
      # crates. A bump that misses any of them does not resolve at all
      # ("candidate versions found which didn't match"), and nothing caught that
      # before a release.
      - name: Version SSOT (no drift)
        shell: bash
        run: |
          set -euo pipefail
          cargo run -q -p vox-cli-ci --example ssot_probe
```

- [ ] **Step 4: Validate and commit**

```bash
uv run --no-project --with pyyaml python -c "import yaml;yaml.safe_load(open('.github/workflows/cross-platform-check.yml'));print('ok')"
act --list --workflows .github/workflows/cross-platform-check.yml > /dev/null 2>&1; echo "act=$?"
git add .github/workflows/cross-platform-check.yml crates/vox-cli-ci/examples/ssot_probe.rs
git commit -m "Gate version drift in CI"
```

---

### Task 2: Enforce conventional commits

`release-prepare.yml` derives the next version with `git cliff --bumped-version`, which reads conventional commits. **Only ~14 of the last 30 commits comply** — including none written during the 2026-09-04 session. Under-compliance silently produces patch bumps for feature work.

**Files:**
- Create: `.github/workflows/commit-lint.yml`
- Modify: `CONTRIBUTING.md`

**Interfaces:**
- Consumes: nothing.
- Produces: a PR-blocking check that every commit subject matches the conventional form `git-cliff` parses.

- [ ] **Step 1: Measure current compliance**

```bash
git log --format=%s -50 | grep -cE '^(feat|fix|chore|docs|refactor|test|ci|build|perf|style|revert)(\(.+\))?!?: ' 
git log --format=%s -50 | grep -vE '^(feat|fix|chore|docs|refactor|test|ci|build|perf|style|revert)(\(.+\))?!?: ' | head -10
```

Record both numbers in the commit body. This is the baseline the gate must not break retroactively.

- [ ] **Step 2: Check whether a linter already exists**

```bash
ls crates/vox-cli-ci/src/commit_lint.rs
grep -rn "commit_lint\|CommitLint" crates/vox-cli-ci/src/cmd_enums.rs | head -5
```

`crates/vox-cli-ci/src/commit_lint.rs` exists. **Read it before writing anything** — if it already enforces the conventional form, this task is only about wiring it to CI, and Steps 3–4 shrink to adding the workflow.

- [ ] **Step 3: Write the workflow**

Create `.github/workflows/commit-lint.yml`:

```yaml
# Conventional commits are not a style preference here: release-prepare derives
# the next semver from them via `git cliff --bumped-version`. A non-conventional
# subject is invisible to git-cliff, so a feature lands as a patch bump.
name: commit-lint

on:
  pull_request:
  merge_group:

concurrency:
  group: commit-lint-${{ github.ref }}
  cancel-in-progress: true

jobs:
  lint:
    name: Conventional commit subjects
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0

      - name: Check subjects on this PR only
        shell: bash
        run: |
          set -euo pipefail
          base="${{ github.event.pull_request.base.sha || github.event.merge_group.base_sha }}"
          bad=0
          while IFS= read -r subject; do
            [ -z "$subject" ] && continue
            if ! printf '%s' "$subject" | grep -qE '^(feat|fix|chore|docs|refactor|test|ci|build|perf|style|revert)(\(.+\))?!?: '; then
              echo "::error::not a conventional subject: $subject"
              bad=1
            fi
          done < <(git log --format=%s "$base..HEAD")
          exit "$bad"
```

Scoping to the PR's own commits is deliberate: history predates the rule, and a gate that fails on other people's old commits gets disabled.

- [ ] **Step 4: Verify and commit**

```bash
uv run --no-project --with pyyaml python -c "import yaml;yaml.safe_load(open('.github/workflows/commit-lint.yml'));print('ok')"
actionlint .github/workflows/commit-lint.yml && echo "actionlint clean"
git add .github/workflows/commit-lint.yml CONTRIBUTING.md
git commit -m "ci: require conventional commit subjects on PRs"
```

Note the commit message above is itself conventional — the gate applies to this task's own commit.

---

### Task 3: Generate the Homebrew formula in the release lane

`crates/vox-cli-ci/src/package_manifests.rs` can render the formula from a release's `checksums.txt`, but the release workflow still contains `echo "Simulating Homebrew Tap update..."`. The published tap (`vox-foundation/homebrew-vox`) is therefore hand-maintained and goes stale on the next release.

**Files:**
- Create: `crates/vox-cli-ci/examples/render_formula.rs`
- Modify: `.github/workflows/release-installers.yml` (the `publish-macos-brew` job)

**Interfaces:**
- Consumes: `vox_cli_ci::package_manifests::{resolve_assets, render_homebrew_formula}`.
- Produces: `Formula/voxlang.rb` content on stdout, given a tag and a checksums file.

- [ ] **Step 1: Write the renderer entry point**

Create `crates/vox-cli-ci/examples/render_formula.rs`:

```rust
//! Render Formula/voxlang.rb for a release. Reads checksums.txt, writes stdout.
//!
//! Usage: render_formula <tag> <path-to-checksums.txt>
fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(tag), Some(checksums_path)) = (args.next(), args.next()) else {
        eprintln!("usage: render_formula <tag> <checksums.txt>");
        std::process::exit(2);
    };
    let checksums = std::fs::read_to_string(&checksums_path)
        .unwrap_or_else(|e| { eprintln!("reading {checksums_path}: {e}"); std::process::exit(1) });
    match vox_cli_ci::package_manifests::resolve_assets(&tag, &checksums) {
        Ok(assets) => print!("{}", vox_cli_ci::package_manifests::render_homebrew_formula(&assets)),
        Err(missing) => {
            for m in missing {
                eprintln!("::error::release {tag} is missing asset {m}");
            }
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 2: Verify it reproduces the published formula**

```bash
curl -sSfL "https://github.com/vox-foundation/vox/releases/download/v0.6.0-rc.4748/checksums.txt" -o /tmp/cks.txt
cargo run -q -p vox-cli-ci --example render_formula -- v0.6.0-rc.4748 /tmp/cks.txt > /tmp/gen.rb 2>/tmp/gen.err
echo "exit=$?"; head -3 /tmp/gen.err
diff /tmp/gen.rb Formula/voxlang.rb && echo "IDENTICAL" || echo "differs — reconcile before wiring CI"
```

If it differs, the generator is the source of truth: update `Formula/voxlang.rb` from the generated output, not the reverse. Confirm `brew style` still passes on the result.

- [ ] **Step 3: Prove it fails loudly on an incomplete release**

```bash
grep -v "x86_64-apple-darwin" /tmp/cks.txt > /tmp/cks_partial.txt
cargo run -q -p vox-cli-ci --example render_formula -- v0.6.0-rc.4748 /tmp/cks_partial.txt > /dev/null 2>/tmp/partial.err
echo "exit=$?"; cat /tmp/partial.err
```

Expected: non-zero, naming `vox-v0.6.0-rc.4748-x86_64-apple-darwin.tar.gz`. A release missing a target must not silently publish a formula for the others.

- [ ] **Step 4: Replace the placeholder**

In `.github/workflows/release-installers.yml`, replace `echo "Simulating Homebrew Tap update..."` and its comment with:

```yaml
      - name: Render the formula from this release
        shell: bash
        run: |
          set -euo pipefail
          curl --proto '=https' --tlsv1.2 -sSfL \
            "https://github.com/vox-foundation/vox/releases/download/${GITHUB_REF_NAME}/checksums.txt" \
            -o /tmp/checksums.txt
          cargo run -q -p vox-cli-ci --example render_formula -- \
            "${GITHUB_REF_NAME}" /tmp/checksums.txt > /tmp/voxlang.rb
          test -s /tmp/voxlang.rb

      - name: Publish to the tap
        env:
          TAP_TOKEN: ${{ secrets.TAP_TOKEN }}
        shell: bash
        run: |
          set -euo pipefail
          if [ -z "${TAP_TOKEN:-}" ]; then
            echo "::warning::TAP_TOKEN not set — formula rendered but not published"
            exit 0
          fi
          git clone --depth 1 "https://x-access-token:${TAP_TOKEN}@github.com/vox-foundation/homebrew-vox.git" /tmp/tap
          cp /tmp/voxlang.rb /tmp/tap/Formula/voxlang.rb
          cd /tmp/tap
          git config user.name "vox-release-bot"
          git config user.email "noreply@voxlang.org"
          git add Formula/voxlang.rb
          git diff --cached --quiet && { echo "formula unchanged"; exit 0; }
          git commit -m "voxlang ${GITHUB_REF_NAME}"
          git push
```

The `TAP_TOKEN` guard means the lane degrades to "rendered, not published" rather than failing a release when the secret is absent.

- [ ] **Step 5: Validate and commit**

```bash
uv run --no-project --with pyyaml python -c "import yaml;yaml.safe_load(open('.github/workflows/release-installers.yml'));print('ok')"
actionlint .github/workflows/release-installers.yml && echo "actionlint clean"
git add crates/vox-cli-ci/examples/render_formula.rs .github/workflows/release-installers.yml Formula/voxlang.rb
git commit -m "ci: render and publish the Homebrew formula from release checksums"
```

**A `TAP_TOKEN` repository secret is required before this publishes.** It needs `contents: write` on `vox-foundation/homebrew-vox` only — a fine-grained PAT scoped to that one repo, not the org. Ask the maintainer to add it; do not create or paste tokens.

---

### Task 4: Generate a winget manifest

winget has no manifest at all. Its format is three YAML files under `manifests/<first-letter>/<Publisher>/<Package>/<version>/`, and the installer manifest needs a SHA-256 per architecture — the same `checksums.txt` data the formula uses.

**Files:**
- Modify: `crates/vox-cli-ci/src/package_manifests.rs`
- Create: `crates/vox-cli-ci/examples/render_winget.rs`

**Interfaces:**
- Consumes: `ReleaseAssets` from Task 3's module.
- Produces: `render_winget_installer(&ReleaseAssets, publisher_id) -> String`.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-cli-ci/src/package_manifests.rs`'s `mod tests`:

```rust
#[test]
fn winget_installer_manifest_pins_the_windows_asset() {
    let a = resolve_assets("v0.6.0-rc.4748", FIXTURE_WITH_WINDOWS).unwrap();
    let m = render_winget_installer(&a, "VoxFoundation.Vox");
    assert!(m.contains("PackageIdentifier: VoxFoundation.Vox"));
    assert!(m.contains("PackageVersion: 0.6.0-rc.4748"));
    assert!(m.contains("Architecture: x64"));
    assert!(m.contains(&a.windows_x64.as_ref().unwrap().sha256.to_uppercase()),
        "winget requires an uppercase InstallerSha256");
    assert!(m.contains("ManifestType: installer"));
}
```

Add the fixture beside `FIXTURE`, including the Windows row:

```rust
const FIXTURE_WITH_WINDOWS: &str = "\
91060c1f32ddc1b03b67a41bf824506d8619ab184f6d18a030087d491fa0a456  vox-v0.6.0-rc.4748-aarch64-apple-darwin.tar.gz
da632656969b441b5b37c047366535a948a432468ad82699de5e6ab7202f5659  vox-v0.6.0-rc.4748-x86_64-apple-darwin.tar.gz
9f939b9f5ed0b98663aabdbac50513e309c23b67f82bd14aca8376aeb543fcd8  vox-v0.6.0-rc.4748-x86_64-unknown-linux-gnu.tar.gz
3333333333333333333333333333333333333333333333333333333333333333  vox-v0.6.0-rc.4748-x86_64-pc-windows-msvc.zip
";
```

- [ ] **Step 2: Run it and confirm it fails**

```bash
cargo test -p vox-cli-ci winget_installer_manifest_pins_the_windows_asset > /tmp/w.log 2>&1; echo "exit=$?"
grep -E "^error|test result" /tmp/w.log | head -3
```

Expected: compile error — `render_winget_installer` and `windows_x64` do not exist yet.

- [ ] **Step 3: Add the Windows asset and the renderer**

Add `pub windows_x64: Option<Asset>` to `ReleaseAssets`, resolved from `vox-{tag}-x86_64-pc-windows-msvc.zip`. It is `Option` because the three Unix assets are required and Windows is resolved separately — a macOS-only point release should still render a formula.

```rust
/// Render winget's installer manifest.
///
/// winget requires `InstallerSha256` in UPPERCASE hex; a lowercase digest is
/// rejected by `winget validate` with a schema error that does not mention case.
pub fn render_winget_installer(a: &ReleaseAssets, package_id: &str) -> String {
    let w = a.windows_x64.as_ref().expect("windows asset required for a winget manifest");
    format!(
        "# GENERATED by `vox ci package-manifests` — do not hand-edit.\n\
         PackageIdentifier: {package_id}\n\
         PackageVersion: {version}\n\
         InstallerType: zip\n\
         Installers:\n\
         \x20 - Architecture: x64\n\
         \x20   InstallerUrl: {base}/{tag}/{file}\n\
         \x20   InstallerSha256: {sha}\n\
         \x20   NestedInstallerType: portable\n\
         \x20   NestedInstallerFiles:\n\
         \x20     - RelativeFilePath: vox.exe\n\
         ManifestType: installer\n\
         ManifestVersion: 1.6.0\n",
        version = a.version(),
        base = DOWNLOAD_BASE,
        tag = a.tag,
        file = w.filename,
        sha = w.sha256.to_uppercase(),
    )
}
```

- [ ] **Step 4: Run the test and the full module suite**

```bash
cargo test -p vox-cli-ci package_manifests > /tmp/w2.log 2>&1; echo "exit=$?"
grep "test result" /tmp/w2.log
```

Expected: all pass, run count > 0.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli-ci/src/package_manifests.rs
git commit -m "feat: render a winget installer manifest from release checksums"
```

**Submission to `microsoft/winget-pkgs` is blocked on a stable release** — winget rejects prereleases. Task 9 covers it.

---

### Task 5: Make the crates publishable, without publishing

crates.io has `vox` taken by an unrelated crate at `0.10.0-rc.7`; `vox-cli`, `voxup` and `voxlang` are free. Publishing ~113 workspace crates has ordering and metadata requirements that will not be discovered at release time without a dry run.

**Files:**
- Modify: `crates/vox-cli/Cargo.toml`, `crates/voxup/Cargo.toml` (metadata only)
- Create: `.github/workflows/publish-dry-run.yml`

**Interfaces:**
- Consumes: nothing.
- Produces: a CI lane proving `cargo publish --dry-run` succeeds for the publishable crates.

- [ ] **Step 1: Find what is actually publishable**

```bash
cd /Users/brbrainerd/dev/vox
grep -l 'publish = false' crates/*/Cargo.toml | wc -l
for f in crates/*/Cargo.toml; do
  grep -q 'publish = false' "$f" || echo "$f"
done | wc -l
```

Record both counts. A crate without `publish = false` is a crate you are promising to publish; if that number is ~113, decide deliberately which are public API and mark the rest `publish = false` **before** any release.

- [ ] **Step 2: Dry-run the leaf binary**

```bash
cargo publish --dry-run -p vox-cli --allow-dirty > /tmp/pub.log 2>&1; echo "exit=$?"
grep -E "^(error|warning)" /tmp/pub.log | head -20
```

Expected failures to triage, not fix blindly: missing `description`/`license`/`repository`, and path dependencies that are not yet on crates.io. The second class is why publish order matters — every `vox-*` dependency must exist on crates.io first.

- [ ] **Step 3: Add the metadata each publishable crate needs**

For any crate the dry run flags, add to its `[package]`:

```toml
description = "<one sentence, not starting with the crate name>"
repository = "https://github.com/vox-foundation/vox"
license = "Apache-2.0"
```

`license` and `repository` can inherit with `.workspace = true` if the workspace declares them; check `[workspace.package]` first and prefer inheritance so this never drifts.

- [ ] **Step 4: Add the dry-run lane**

Create `.github/workflows/publish-dry-run.yml`:

```yaml
# Proves the crates would publish, without publishing. Publish order matters:
# every vox-* path dependency must already exist on crates.io, so a first
# release publishes leaves-first. Discovering that during a release is too late.
name: publish-dry-run

on:
  pull_request:
    paths: ['**/Cargo.toml']
  workflow_dispatch:

jobs:
  dry-run:
    name: cargo publish --dry-run
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"
      - name: Dry-run the publishable leaves
        shell: bash
        run: |
          set -euo pipefail
          cargo publish --dry-run -p vox-cli --allow-dirty
```

- [ ] **Step 5: Verify and commit**

```bash
uv run --no-project --with pyyaml python -c "import yaml;yaml.safe_load(open('.github/workflows/publish-dry-run.yml'));print('ok')"
git add crates/*/Cargo.toml .github/workflows/publish-dry-run.yml
git commit -m "ci: prove the crates publish without publishing them"
```

**Do not run `cargo publish` for real.** crates.io publication is irreversible — versions cannot be deleted, only yanked.

---

### Task 6: Stop `vox run --sandbox` claiming isolation it does not provide

`crates/vox-cli/src/commands/runtime/run/sandbox.rs:186-199` — the
`#[cfg(not(any(target_os = "linux", target_os = "windows")))]` arm prints a warning, sets `VOX_SANDBOX=1` as an "informational hint", and returns `Ok(())`. Linux gets Landlock, Windows gets Job Objects, **macOS gets nothing** — and the command still succeeds, so an operator believes their script is sandboxed when it is not.

This is the only security-relevant item in this plan. It is independent of every other task.

**Files:**
- Modify: `crates/vox-cli/src/commands/runtime/run/sandbox.rs:186-199`
- Test: same file, inline `#[cfg(test)]`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Confirm the behavior**

```bash
sed -n '180,205p' crates/vox-cli/src/commands/runtime/run/sandbox.rs
grep -rn "sandbox" crates/vox-cli/src/lib.rs | head -5
```

Verify the arm returns `Ok(())` and that `--sandbox` is a user-facing flag. If a `--sandbox-best-effort` style flag already exists, wire to it instead of adding one.

- [ ] **Step 2: Write the failing test**

```rust
#[cfg(test)]
mod sandbox_honesty_tests {
    /// On a platform with no sandbox implementation, requesting one must fail
    /// rather than succeed silently. `--sandbox` that returns Ok() while
    /// providing no isolation is worse than an unsupported error: the operator
    /// believes the guarantee holds.
    #[test]
    fn unsupported_platform_is_an_error_unless_best_effort() {
        assert!(super::sandbox_verdict(false, false).is_err());
        assert!(super::sandbox_verdict(false, true).is_ok());
        assert!(super::sandbox_verdict(true, false).is_ok());
    }
}
```

- [ ] **Step 3: Run it and confirm it fails**

```bash
cargo test -p vox-cli --lib sandbox_honesty_tests > /tmp/s.log 2>&1; echo "exit=$?"
grep -E "^error|test result" /tmp/s.log | head -3
```

Expected: compile error — `sandbox_verdict` does not exist.

- [ ] **Step 4: Extract the pure decision and use it**

```rust
/// `Ok(())` when the requested sandbox can be honored, `Err` when it cannot.
///
/// `supported` is whether this build has a real implementation (Landlock on
/// Linux, Job Objects on Windows). `best_effort` is the operator explicitly
/// accepting no isolation.
pub(crate) fn sandbox_verdict(supported: bool, best_effort: bool) -> Result<(), String> {
    if supported || best_effort {
        Ok(())
    } else {
        Err("--sandbox is not implemented on this platform (no Landlock or Job \
             Objects equivalent). Re-run with --sandbox-best-effort to proceed \
             with NO isolation, or run on Linux/Windows for a real sandbox."
            .to_string())
    }
}
```

Call it from the `cfg(not(...))` arm and return the error instead of `Ok(())`. Add the `--sandbox-best-effort` flag beside `--sandbox` in the CLI definition.

- [ ] **Step 5: Verify both paths on this machine**

```bash
cargo test -p vox-cli --lib sandbox_honesty_tests > /tmp/s2.log 2>&1; echo "exit=$?"
cargo build -p vox-cli > /tmp/sb.log 2>&1; echo "build=$?"
echo 'fn main() {}' > /tmp/probe.vox
./target/debug/vox run --sandbox /tmp/probe.vox > /tmp/s3.log 2>&1; echo "sandbox exit=$?"; tail -2 /tmp/s3.log
./target/debug/vox run --sandbox-best-effort /tmp/probe.vox > /tmp/s4.log 2>&1; echo "best-effort exit=$?"
```

Expected: `--sandbox` now exits non-zero on macOS with the actionable message; `--sandbox-best-effort` behaves as `--sandbox` did.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/runtime/run/sandbox.rs
git commit -m "fix: --sandbox fails on platforms with no sandbox implementation"
```

---

### Task 7: Decide the ACI shell backend for POSIX hosts

**Blocked on an owner decision — do not guess.** `crates/vox-orchestrator-mcp/src/aci/envelope.rs:47` returns `"powershell"` on every platform, including the no-args case, and `envelope.rs:55` does the same when args carry no `backend` key. `contracts/aci/agent-computer-interface.v1.yaml` defines only `contract_first`, `powershell`, `nushell` — there is no POSIX adapter, so a correct macOS default cannot be invented in code.

- [ ] **Step 1: Identify the owner**

```bash
grep -rn "aci" .github/CODEOWNERS 2>/dev/null || echo "no CODEOWNERS entry"
git log --format='%an' -20 -- contracts/aci/ | sort | uniq -c | sort -rn | head -3
```

- [ ] **Step 2: Put the three options to them**

- **(A)** Default to `contract_first` when unspecified. The SSOT already describes it as shell-agnostic. No contract change. *Recommended.*
- **(B)** Add a `posix` adapter to the YAML SSOT, both JSON schemas, and the normalizer. Most correct, largest blast radius.
- **(C)** Keep `powershell`, document it as a telemetry label rather than an execution target, and add a comment at the call site so it is not re-filed.

- [ ] **Step 3 (option A only): Write the failing test**

```rust
#[test]
fn aci_shell_backend_defaults_to_contract_first_without_args() {
    let v = super::aci_shell_backend_for_tool("vox_run_shell", None);
    assert_eq!(v, serde_json::Value::String("contract_first".into()));
}
```

- [ ] **Step 4 (A): Change both default sites**

`envelope.rs:47` (no args) **and** `envelope.rs:55` (`unwrap_or("powershell")`). Changing only the first leaves `{"cmd":"ls"}` mislabeled — a half-applied fix. Add `"contract_first" => "contract_first"` to the normalizer match.

- [ ] **Step 5 (A): Update the test that encodes the old default**

`aci_shell_backend_for_run_shell_default_pwsh` at `envelope.rs:168-176` asserts `"powershell"` for exactly this case. It **will** fail — that is a certainty. Rename it to `aci_shell_backend_for_run_shell_defaults_to_contract_first`, change the assertion, and say so in the commit body.

- [ ] **Step 6: Verify and commit**

```bash
cargo test -p vox-orchestrator-mcp > /tmp/aci.log 2>&1; echo "exit=$?"; grep "test result" /tmp/aci.log
git add crates/vox-orchestrator-mcp/src/aci/envelope.rs
git commit -m "fix: default the ACI shell backend to contract_first when unspecified"
```

---

### Task 8: Run the local CI lane once, for real

The `act` + colima path has only ever been dry-run (`act -n`). Until a live run happens, the local lane cannot be relied on and its documented timings are guesses.

**Files:**
- Modify: `docs/src/ci/alternatives-and-local-mirroring.md` (the macOS paragraph)

- [ ] **Step 1: Confirm the daemon is sized and Rosetta is active**

```bash
colima list
docker info --format 'CPUs={{.NCPU}} Mem={{.MemTotal}}'
time docker run --rm --platform linux/amd64 alpine uname -m
```

Expected `x86_64`, quickly. If it prints `aarch64` the architecture pin is not applying; if it takes minutes, Rosetta is off — restart with `colima start --vm-type vz --vz-rosetta`.

- [ ] **Step 2: Execute one job end to end**

```bash
time act --workflows .github/workflows/cross-platform-check.yml \
     --job standalone-installables > /tmp/act-live.log 2>&1; echo "exit=$?"
tail -20 /tmp/act-live.log
```

Only the `ubuntu-latest` matrix leg runs; `.actrc` deliberately refuses to map `macos-latest`. Expect a ~1 GB image pull on first run.

- [ ] **Step 3: Record what actually happened**

Replace the speculative wording in the macOS paragraph with the measured wall-clock time and image size. **If the run failed, document the failure and its workaround rather than deleting the paragraph** — a known-broken lane documented beats an untested lane implied to work.

- [ ] **Step 4: Commit**

```bash
git add docs/src/ci/alternatives-and-local-mirroring.md
git commit -m "docs: record measured act + colima results for the macOS lane"
```

---

### Task 9: Cut the first stable release

**Blocked on explicit approval. Do not perform any step here without it.**

Every tag to date is `v*-rc.<commit-count>`, and the release action auto-marks semver prereleases. That is why `/releases/latest` returns 404, and why both installers carry a list-and-sort workaround. A single non-prerelease `vX.Y.Z` tag retires all of it.

- [ ] **Step 1: Confirm the gates from Tasks 1–5 are green**

```bash
gh pr checks --watch
cargo run -q -p vox-cli-ci --example ssot_probe
```

- [ ] **Step 2: Compute and apply the version**

Run `release-prepare` (`workflow_dispatch`), review the PR it opens, merge it. The bump touches 127 lines; do not hand-edit.

- [ ] **Step 3: Tag**

```bash
git tag v0.7.0 && git push origin v0.7.0
```

No `-rc.` suffix. That is the point.

- [ ] **Step 4: Verify the endpoint that was 404ing**

```bash
curl -sS -o /dev/null -w "%{http_code}\n" https://api.github.com/repos/vox-foundation/vox/releases/latest
```

Expected: `200`.

---

### Task 10: Retire the installer workaround

**Blocked on Task 9.** Once `/releases/latest` returns 200, both installers can drop the list-and-sort logic — retiring the endpoint 404, the `created_at`-vs-`published_at` ordering assumption, and the draft-visibility edge case in one change.

**Files:**
- Modify: `scripts/install.sh`, `docs-astro/public/voxup`
- Modify: `scripts/install.ps1`, `docs-astro/public/voxup.ps1`
- Modify: `crates/voxup/src/channel.rs`

- [ ] **Step 1: Verify the endpoint first**

```bash
curl -sSfL -H "Accept: application/vnd.github+json" \
  https://api.github.com/repos/vox-foundation/vox/releases/latest | head -3
```

**If this 404s, stop** — Task 9 has not landed and the workaround is still load-bearing.

- [ ] **Step 2: Restore the simple lookup in the shell installer**

In `scripts/install.sh`, restore `GITHUB_API=".../releases/latest"` and the direct `tag_name` parse, keeping the `tr ',' '\n'` normalization (it is correct for both response shapes and costs nothing).

- [ ] **Step 3: Restore it in the Rust installer**

In `crates/voxup/src/channel.rs`, replace the list-and-filter in `fetch_latest` with a single `GET` to `/releases/latest`. Keep the `published_at` ordering test as a regression guard against reintroducing the bug.

- [ ] **Step 4: Sync the mirrors and verify end to end**

```bash
cp scripts/install.sh docs-astro/public/voxup
cp scripts/install.ps1 docs-astro/public/voxup.ps1
diff -q scripts/install.sh docs-astro/public/voxup && diff -q scripts/install.ps1 docs-astro/public/voxup.ps1 && echo "mirrors in sync"
sh -n scripts/install.sh && echo "syntax ok"
rm -rf /tmp/voxhome && mkdir -p /tmp/voxhome
HOME=/tmp/voxhome sh scripts/install.sh > /tmp/inst.log 2>&1; echo "install exit=$?"
/tmp/voxhome/.vox/bin/vox --version
```

Both the shell script and the binary resolve the release independently — a fix to one is not a fix to the other. That is how the first attempt shipped broken; the end-to-end install is the only check that catches it.

- [ ] **Step 5: Commit**

```bash
git add scripts/install.sh scripts/install.ps1 docs-astro/public/voxup docs-astro/public/voxup.ps1 crates/voxup/src/channel.rs
git commit -m "fix: use /releases/latest now that a stable release exists"
```

---

### Task 11: Submit to homebrew-core and winget

**Blocked on Task 9.** Both registries reject prereleases.

- [ ] **Step 1: Check homebrew-core's acceptance criteria**

Read the current [Acceptable Formulae](https://docs.brew.sh/Acceptable-Formulae) rules. The notability bar (maintained, versioned, not a personal project) is the gate — assess honestly before spending effort, and record the assessment.

- [ ] **Step 2: Submit the formula**

```bash
brew bump-formula-pr --help
```

Acceptance means users get `brew install voxlang` with **no tap and no `brew trust`**. When it lands, delete the trust instructions from `Formula/README.md` and the tap README — they are marked as temporary in both files precisely so they are removed rather than inherited.

- [ ] **Step 3: Submit the winget manifest**

Generate with Task 4's renderer, validate locally with `winget validate`, then open a PR against `microsoft/winget-pkgs` under `manifests/v/VoxFoundation/Vox/<version>/`.

- [ ] **Step 4: Record the outcome**

Update `docs/src/reference/installation.md` with whichever channels actually accepted. Do not document a channel as available before it is.

---

## Deferred, with reasons

- **npm publication of the Rust CLI: do not do it.** `voxlang` is free on npm and `vox` is taken, but shipping a Rust binary through npm means a postinstall script that downloads a platform binary — the exact pattern that makes `npm install` a supply-chain risk, and it duplicates what Homebrew, winget and `voxup` already do correctly. The `@vox/*` packages under `clients/` are genuinely TypeScript and *should* be on npm; that is a different decision and belongs with the Axis/TypeScript workstream.
- **Apple Developer Program ($99/yr): declined.** Measured 2026-09-04: `curl` does not set `com.apple.quarantine`, and the ad-hoc linker-signed binary runs fine via both `voxup` and Homebrew. Notarization would only serve browser downloads from the Releases page. If it is ever bought, the hardened runtime requires `com.apple.security.cs.disable-library-validation` or plugin `dlopen` breaks.
- **`vox-mesh-policy` / `vox-scaling-policy` duplication.** Near-identical copies that both had to absorb the same six `WorkerDonationPolicy` fields. Deduping removes one of three places a future field must be added. Own plan.
- **`vox doctor --probe` full-suite cost.** The probe runs every check, including a `cargo` compile probe, against a `HEALTHCHECK --timeout=5s`. Task 6 of the previous plan fixed *which* checks gate the verdict; making the probe run only those is a separate change spanning `checks_standard/*`.

## Self-Review

**Coverage.** Every outstanding item is a task or an explicit Deferred entry with a reason. Landed already and therefore absent: the macOS bring-up fixes, the plugin artifact guard, doctor remediations, the `build.1` shallow-clone defect, the 127-line version SSOT, the published Homebrew tap, and `release-prepare`.

**Blocked tasks are marked, not worked around.** Tasks 9, 10 and 11 depend on a public release the user has explicitly withheld; Task 7 depends on an owner decision. Each says so in its first line, and Task 10 opens with a check that stops the executor if Task 9 has not landed.

**Placeholder scan.** No `TBD`/`TODO`/"similar to Task N". Task 5 Step 3 says "for any crate the dry run flags" — that is deliberately data-driven, and Step 2 produces the list.

**Type consistency.** `ReleaseAssets` gains `windows_x64: Option<Asset>` in Task 4 and is consumed by name in Tasks 3 and 4 only. `sandbox_verdict(supported, best_effort) -> Result<(), String>` is defined and used within Task 6. `ssot_probe`'s contract (exit non-zero on drift, `--write` to apply) is established in Task 1 and relied on by Task 9 Step 1.

**Known risk.** Task 5 Step 1 may reveal that ~113 crates are nominally publishable. Deciding which are public API is a design decision this plan surfaces but does not make — it must be made before, not during, a release.
