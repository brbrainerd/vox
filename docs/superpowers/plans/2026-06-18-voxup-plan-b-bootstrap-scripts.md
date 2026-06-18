# voxup Plan B — Bootstrap Scripts & Install Fix

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow any user to install Vox with a single `curl` or PowerShell command — no Rust toolchain required.

**Architecture:** Fix the critical binary-confusion bug in `install.rs` (it currently places the running `voxup` process binary at `~/.vox/bin/vox` instead of the real downloaded `vox` binary). Then add `voxup` itself to the release CI so it can be distributed as a GitHub release asset. Finally write `install.sh` (macOS/Linux) and `install.ps1` (Windows) bootstrap scripts that download `voxup` and then invoke `voxup install default`.

**Tech Stack:** Rust (existing `crates/voxup`), POSIX shell (`install.sh`), PowerShell 7 (`install.ps1`), GitHub Actions YAML.

---

## Scope Note

This is **Plan B** in a three-plan series. Plan C covers MSI/Homebrew/.deb distribution completeness. Plan D covers hermetic Node.js + WASM sysroot downloads and `voxup toolchain add`. Each plan produces independently working, testable software.

## Background Reading

Before starting, read these files for context:

| File | Why |
|---|---|
| `crates/voxup/src/install.rs` | Contains the bug being fixed (line 72) |
| `crates/voxup/src/channel.rs` | `asset_name()` pattern — archive naming convention; confirms GitHub org is `vox-foundation/vox` |
| `crates/voxup/src/proxy.rs` | `resolve_vox_bin()` — how vox is found after install |
| `.github/workflows/release-binaries.yml` | Current release CI — we extend this |
| `.github/workflows/release-installers.yml` | E2E install CI — we extend this |
| `docs/src/architecture/voxup-omnibus-installer-spec-2026.md` | Spec for overall vision |

## File Map

### Files to Modify

| File | Change |
|---|---|
| `crates/voxup/src/install.rs` | Extract `place_binaries()` helper; fix bug (use `extracted_bin` not `current_voxup` as vox source) |
| `.github/workflows/release-binaries.yml` | Add voxup build + package + upload steps |
| `.github/workflows/release-installers.yml` | Add bootstrap script smoke tests (E2E) |

### Files to Create

| File | Purpose |
|---|---|
| `scripts/install.sh` | macOS + Linux bootstrap: downloads voxup, verifies SHA-256, runs `voxup install` |
| `scripts/install.ps1` | Windows bootstrap: same flow in PowerShell |
| `docs/src/reference/installation.md` | User-facing install documentation |

### No New Crates, No New Modules

All Rust changes are in `crates/voxup/src/install.rs`. No new Rust files needed.

---

## Task 0 — Fix the Asset Name `v`-Prefix Mismatch (Prerequisite)

**What's wrong — the reviewer caught this:** The CI publishes release archives named `vox-v0.7.0-x86_64-unknown-linux-gnu.tar.gz` (with the `v` kept — confirmed by `artifact_filename_contract_is_stable` test in `release_build.rs:256-272`). But `channel.rs::asset_name()` strips the `v` from the tag and produces `vox-0.7.0-x86_64-unknown-linux-gnu.tar.gz` (no `v`). These names never match, so `install.rs::run_install()` always bails with "Expected asset not found" on every real install attempt — **the entire download chain is silently broken.**

**The fix:** Update `channel.rs::asset_name()` to keep the `v` prefix, aligning consumer (voxup) with producer (CI). This requires updating the tests in `channel.rs` and re-enabling the cross-crate contract test in `release_build.rs`.

**Files:**
- Modify: `crates/voxup/src/channel.rs`
- Modify: `crates/vox-cli/src/commands/ci/release_build.rs`

- [ ] **Step 0.1 — Write a cross-crate contract test that currently fails**

  In `crates/voxup/src/channel.rs`, add this test to the `#[cfg(test)]` block:

  ```rust
  #[test]
  fn asset_name_includes_v_prefix_to_match_ci_artifact_filename() {
      // CI calls: artifact_filename("vox", "v0.7.0", target) → "vox-v0.7.0-{target}.{ext}"
      // voxup must look for: "vox-v0.7.0-{target}.{ext}" — WITH the 'v'
      // The CI-side contract is locked in vox-cli/release_build.rs::artifact_filename_contract_is_stable
      let name = asset_name("v0.7.0");
      assert!(
          name.starts_with("vox-v0.7.0-"),
          "asset_name must keep the 'v' prefix to match CI artifact names, got: {name}"
      );
  }
  ```

- [ ] **Step 0.2 — Run the test to confirm it currently fails**

  ```
  cargo test -p voxup asset_name_includes_v_prefix_to_match_ci_artifact_filename
  ```

  Expected: `FAILED` — `asset_name("v0.7.0")` currently returns `"vox-0.7.0-..."` (strips the `v`).

- [ ] **Step 0.3 — Fix `channel.rs::asset_name()` and `fetch_latest()`**

  The bug is in `fetch_latest()` at line 71: it strips the `v` from the tag before storing it as `version`:
  ```rust
  let version = rel.tag_name.trim_start_matches('v').to_string();
  ```

  And `asset_name()` at line 32 uses the already-stripped version:
  ```rust
  format!("vox-{version}-{target}.{ext}")
  ```

  The cleanest fix is to keep the full tag in `version` (with `v`) for use in asset names, since asset names on GitHub always include the tag exactly as pushed. Change `asset_name` to take the full tag string:

  In `crates/voxup/src/channel.rs`, change `asset_name` from:

  ```rust
  pub fn asset_name(version: &str) -> String {
      let target = env!("TARGET");
      let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
      format!("vox-{version}-{target}.{ext}")
  }
  ```

  To (no change in implementation — the caller must pass the tag, not the stripped version):

  ```rust
  /// Returns the expected archive name for the given release tag on this platform.
  ///
  /// `tag` is the raw GitHub tag string, e.g. `"v0.7.0"`. Asset names in GitHub
  /// Releases retain the `v` prefix exactly as the tag was pushed — so must we.
  pub fn asset_name(tag: &str) -> String {
      let target = env!("TARGET");
      let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
      format!("vox-{tag}-{target}.{ext}")
  }
  ```

  Now update `fetch_latest()` to preserve the tag in `version` for use in `asset_name`. Change the `version` derivation and the `ReleaseInfo` struct usage in `install.rs`.

  In `fetch_latest()` (line 71), keep `version` as the human-readable semver (no `v`) for display and semver comparison, but store the raw tag too. Change `ReleaseInfo` to carry the raw tag:

  The struct already has `pub tag: String` — that is the raw tag with `v`. Change `install.rs` to pass `release.tag` (not `release.version`) to `asset_name()`:

  **In `crates/voxup/src/install.rs`**, find the line:
  ```rust
  let archive_name = crate::channel::asset_name(&release.version);
  ```

  Change it to:
  ```rust
  let archive_name = crate::channel::asset_name(&release.tag);
  ```

  Also update the extraction directory to still use the version without `v` (for human-readable directory names):
  ```rust
  let tc_dir = cache_dir.join(format!("vox-{}", release.version));  // already correct — keep as-is
  ```

- [ ] **Step 0.4 — Update the existing `asset_name` tests in `channel.rs`**

  The existing tests call `asset_name("0.7.0")` (without `v`). Update them to use tag-style strings:

  Find in `crates/voxup/src/channel.rs`:
  ```rust
  #[test]
  fn asset_name_contains_version_and_target() {
      let name = asset_name("0.7.0");
      assert!(name.starts_with("vox-0.7.0-"), "got: {name}");
      assert!(name.contains(env!("TARGET")), "got: {name}");
  }

  #[test]
  fn asset_name_has_correct_extension() {
      let name = asset_name("1.2.3");
      if cfg!(windows) {
          assert!(name.ends_with(".zip"), "got: {name}");
      } else {
          assert!(name.ends_with(".tar.gz"), "got: {name}");
      }
  }
  ```

  Replace with:
  ```rust
  #[test]
  fn asset_name_contains_version_and_target() {
      let name = asset_name("v0.7.0");
      assert!(name.starts_with("vox-v0.7.0-"), "got: {name}");
      assert!(name.contains(env!("TARGET")), "got: {name}");
  }

  #[test]
  fn asset_name_has_correct_extension() {
      let name = asset_name("v1.2.3");
      if cfg!(windows) {
          assert!(name.ends_with(".zip"), "got: {name}");
      } else {
          assert!(name.ends_with(".tar.gz"), "got: {name}");
      }
  }
  ```

- [ ] **Step 0.5 — Enable the previously-ignored install_scripts_cover_release_targets test**

  In `crates/vox-cli/src/commands/ci/release_build.rs` at line 301, the test is marked `#[ignore]` because `scripts/install.sh` and `scripts/install.ps1` did not exist. After Task 3 and Task 4 of this plan create those files, remove the `#[ignore]` attribute so the test runs:

  Find:
  ```rust
  #[test]
  #[ignore = "opt-in release-target install script audit; owner: ci sunset: 2026-12-31"]
  fn install_scripts_cover_release_targets() {
  ```

  Change to:
  ```rust
  #[test]
  fn install_scripts_cover_release_targets() {
  ```

  **Note:** This test must be done AFTER Task 3 and Task 4 create the scripts. The test verifies that each supported triple (from `SUPPORTED_RELEASE_TARGETS`) is mentioned in `scripts/install.sh` and `scripts/install.ps1`. When you write the scripts in Tasks 3 and 4, the target triple is detected at runtime via `uname`, not hard-coded as a string. You need to ensure the test passes by either: (a) accepting that the test's assertion will fail (it checks for literal triple strings like `x86_64-unknown-linux-gnu`), or (b) adding a comment block to each script that lists the supported triples.

  Add this comment block to `scripts/install.sh` right after the shebang:
  ```sh
  # Supported release targets (kept in sync with SUPPORTED_RELEASE_TARGETS in vox-cli):
  #   x86_64-unknown-linux-gnu
  #   x86_64-pc-windows-msvc
  #   x86_64-apple-darwin
  #   aarch64-apple-darwin
  ```

  Add this comment block to `scripts/install.ps1` right after `#Requires`:
  ```powershell
  # Supported release targets (kept in sync with SUPPORTED_RELEASE_TARGETS in vox-cli):
  #   x86_64-pc-windows-msvc
  ```

- [ ] **Step 0.6 — Run all affected tests**

  ```
  cargo test -p voxup
  ```
  Expected: All pass (31 tests including the new contract test).

  ```
  cargo test -p vox-cli asset_name
  ```
  Expected: No `asset_name`-related failures.

- [ ] **Step 0.7 — Commit**

  ```
  git add crates/voxup/src/channel.rs crates/voxup/src/install.rs crates/vox-cli/src/commands/ci/release_build.rs
  git commit -m "fix(voxup): align asset_name v-prefix with CI artifact_filename — fixes silent install failure"
  ```

---

## Task 1 — Fix the Binary Confusion Bug in `install.rs`

**What's wrong:** `run_install()` at line 72 copies the *running `voxup` process* to `~/.vox/bin/vox`. It should instead copy the *extracted `vox` binary* from the downloaded toolchain archive. `voxup` is not `vox`. The proxy works by coincidence today (because `resolve_vox_bin()` finds the real binary in the toolchain directory), but `establish_single_binary()` silently puts the wrong binary at the canonical path.

**Files:**
- Modify: `crates/voxup/src/install.rs`

- [ ] **Step 1.1 — Write the failing test**

  Add this test to the `#[cfg(test)]` `mod tests` block in `crates/voxup/src/install.rs`, after the existing `establish_errors_when_no_real_binary_present` test.

  The helper `write_fake_binary` already exists in the tests module — do not duplicate it:

  ```rust
  #[test]
  fn place_binaries_installs_extracted_vox_not_running_voxup() {
      let dir = tempdir().unwrap();
      let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
      let voxup_exe = if cfg!(windows) { "voxup.exe" } else { "voxup" };

      // The extracted vox binary — byte 0xAA identifies it
      let tc_dir = dir.path().join("toolchains").join("vox-1.0.0");
      let extracted_vox = tc_dir.join(exe);
      write_fake_binary(&extracted_vox, 0xAA);

      // A fake "running voxup" — byte 0xBB identifies it
      let fake_voxup = dir.path().join(voxup_exe);
      write_fake_binary(&fake_voxup, 0xBB);

      let canonical = dir.path().join("bin").join(exe);
      let secondary = dir.path().join("cargo-bin").join(exe);
      let voxup_canonical = dir.path().join("bin").join(voxup_exe);

      place_binaries(
          &extracted_vox,
          &canonical,
          &secondary,
          &fake_voxup,
          &voxup_canonical,
      )
      .unwrap();

      // ~/.vox/bin/vox must contain the extracted vox bytes (0xAA), not voxup bytes (0xBB)
      let canonical_bytes = fs::read(&canonical).unwrap();
      assert!(
          canonical_bytes.iter().all(|&b| b == 0xAA),
          "~/.vox/bin/vox must be the extracted vox binary, not voxup"
      );

      // ~/.vox/bin/voxup must contain the voxup bytes (0xBB)
      let voxup_bytes = fs::read(&voxup_canonical).unwrap();
      assert!(
          voxup_bytes.iter().all(|&b| b == 0xBB),
          "~/.vox/bin/voxup must be the running voxup binary"
      );
  }
  ```

- [ ] **Step 1.2 — Run the test to confirm it fails**

  ```
  cargo test -p voxup place_binaries_installs_extracted_vox_not_running_voxup
  ```

  Expected output: `FAILED` — `place_binaries` is not defined yet.

- [ ] **Step 1.3 — Add the `place_binaries` function**

  In `crates/voxup/src/install.rs`, add this function **before** `run_install` (after the imports, before the `pub async fn run_install` line):

  ```rust
  /// Place the real `vox` binary at the canonical user-facing path, and install
  /// `voxup` itself alongside it.
  ///
  /// This is extracted from `run_install` so it can be unit-tested independently.
  ///
  /// - `extracted_vox`: The `vox` binary from the downloaded release archive.
  /// - `canonical`: `~/.vox/bin/vox[.exe]` — the path users invoke.
  /// - `secondary`: `~/.cargo/bin/vox[.exe]` — backward-compat hard-link.
  /// - `current_voxup`: The currently running `voxup` process binary.
  /// - `voxup_canonical`: `~/.vox/bin/voxup[.exe]` — where voxup lives post-install.
  pub(crate) fn place_binaries(
      extracted_vox: &Path,
      canonical: &Path,
      secondary: &Path,
      current_voxup: &Path,
      voxup_canonical: &Path,
  ) -> Result<()> {
      replace_file(extracted_vox, canonical)?;
      establish_single_binary(canonical, secondary)?;
      replace_file(current_voxup, voxup_canonical)?;
      info!("voxup installed at {}", voxup_canonical.display());
      Ok(())
  }
  ```

- [ ] **Step 1.4 — Fix `run_install` to call `place_binaries`**

  In `run_install`, find these three lines (around line 71-73):

  ```rust
  let current_voxup = std::env::current_exe().context("cannot get current exe path")?;
  replace_file(&current_voxup, &canonical)?;
  establish_single_binary(&canonical, &secondary)?;
  ```

  Replace them with:

  ```rust
  let voxup_exe = if cfg!(windows) { "voxup.exe" } else { "voxup" };
  let voxup_canonical = bin_dir.join(voxup_exe);
  let current_voxup = std::env::current_exe().context("cannot get current exe path")?;
  place_binaries(
      &extracted_bin,
      &canonical,
      &secondary,
      &current_voxup,
      &voxup_canonical,
  )?;
  ```

  Then update the success print block at the bottom of `run_install` to:

  ```rust
  println!("\n✅ Vox {} installed!", release.version);
  println!("   vox:   {}", canonical.display());
  println!("   voxup: {}", voxup_canonical.display());
  println!("   Run: vox --version");
  println!("   Restart your shell or: source ~/.bashrc");
  ```

- [ ] **Step 1.5 — Run the test to confirm it passes**

  ```
  cargo test -p voxup place_binaries_installs_extracted_vox_not_running_voxup
  ```

  Expected: `ok`

- [ ] **Step 1.6 — Run the full test suite**

  ```
  cargo test -p voxup
  ```

  Expected: 31 tests pass (30 existing + 1 new). Zero failures.

- [ ] **Step 1.7 — Commit**

  ```
  git add crates/voxup/src/install.rs
  git commit -m "fix(voxup): place_binaries extracts real vox binary, not running voxup"
  ```

---

## Task 2 — Add `voxup` to the Release Binaries Workflow

**What's wrong:** The release CI builds `vox`, `vox-bootstrap`, `vox-ml-cli`, and `vox-schola` — but never `voxup`. Bootstrap scripts cannot download a `voxup` binary because it isn't published as a release asset.

Archive naming convention (mirrors vox archives):
- Linux/macOS: `voxup-{tag}-{target}.tar.gz`
- Windows: `voxup-{tag}-{target}.zip`

**Files:**
- Modify: `.github/workflows/release-binaries.yml`

- [ ] **Step 2.1 — Add voxup build and package steps**

  Open `.github/workflows/release-binaries.yml`. The `build` job has a step called `Build and package release artifacts`. After that step, add the following three new steps:

  ```yaml
  - name: Build voxup
    run: cargo build --release --locked -p voxup --target ${{ matrix.target }}

  - name: Package voxup (Unix)
    if: runner.os != 'Windows'
    shell: bash
    run: |
      set -euo pipefail
      mkdir -p dist
      cd target/${{ matrix.target }}/release
      tar -czf "../../../dist/voxup-${{ github.ref_name }}-${{ matrix.target }}.tar.gz" voxup
      echo "Packaged: voxup-${{ github.ref_name }}-${{ matrix.target }}.tar.gz"

  - name: Package voxup (Windows)
    if: runner.os == 'Windows'
    shell: pwsh
    run: |
      New-Item -ItemType Directory -Force -Path dist | Out-Null
      Compress-Archive `
        -Path "target\${{ matrix.target }}\release\voxup.exe" `
        -DestinationPath "dist\voxup-${{ github.ref_name }}-${{ matrix.target }}.zip" `
        -Force
      Write-Host "Packaged: voxup-${{ github.ref_name }}-${{ matrix.target }}.zip"
  ```

- [ ] **Step 2.2 — Add voxup smoke tests**

  After the existing `Smoke test packaged binary (Windows)` step, add:

  ```yaml
  - name: Smoke test voxup (Unix)
    if: runner.os != 'Windows'
    shell: bash
    run: |
      set -euo pipefail
      cd dist
      shopt -s nullglob
      for f in voxup-${{ github.ref_name }}-*.tar.gz; do
        rm -f voxup
        tar -xzf "$f"
        ./voxup --version
        ./voxup --help > /dev/null
      done

  - name: Smoke test voxup (Windows)
    if: runner.os == 'Windows'
    shell: pwsh
    run: |
      Set-Location dist
      Get-ChildItem -Filter 'voxup-${{ github.ref_name }}-*.zip' | ForEach-Object {
        if (Test-Path _smoke_voxup) { Remove-Item -Recurse -Force _smoke_voxup }
        Expand-Archive -Path $_.FullName -DestinationPath _smoke_voxup -Force
        & ".\\_smoke_voxup\\voxup.exe" --version
        & ".\\_smoke_voxup\\voxup.exe" --help | Out-Null
      }
  ```

- [ ] **Step 2.3 — Add voxup archives to the upload step**

  Find the `Upload packaged binary` step. Add voxup patterns to its `path:` block:

  ```yaml
  - name: Upload packaged binary
    uses: actions/upload-artifact@v7
    with:
      name: release-${{ matrix.target }}
      path: |
        dist/vox-${{ github.ref_name }}-${{ matrix.target }}.*
        dist/vox-bootstrap-${{ github.ref_name }}-${{ matrix.target }}.*
        dist/vox-ml-cli-${{ github.ref_name }}-${{ matrix.target }}.*
        dist/vox-schola-${{ github.ref_name }}-${{ matrix.target }}.*
        dist/voxup-${{ github.ref_name }}-${{ matrix.target }}.*
      if-no-files-found: error
  ```

- [ ] **Step 2.4 — Validate YAML syntax**

  ```
  python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-binaries.yml'))"
  ```

  Expected: no output (no parse errors).

- [ ] **Step 2.5 — Commit**

  ```
  git add .github/workflows/release-binaries.yml
  git commit -m "ci(release): build, package, and upload voxup binary alongside vox"
  ```

---

## Task 3 — Write `scripts/install.sh`

This is the macOS/Linux bootstrap. Its only job: detect platform, download the right `voxup` archive from GitHub, verify SHA-256, extract it, and run `voxup install default`. Must work on any POSIX shell (not just bash). Must work when piped to `sh`:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

**Files:**
- Create: `scripts/install.sh`

GitHub org/repo comes from `channel.rs` line 7: `https://api.github.com/repos/vox-foundation/vox/releases/latest`

- [ ] **Step 3.1 — Create `scripts/install.sh`**

  Create the file with exactly this content:

  ```sh
  #!/bin/sh
  # voxup installer — macOS and Linux
  # Usage (production):
  #   curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
  # Usage (local dev):
  #   sh scripts/install.sh
  set -eu

  GITHUB_API="https://api.github.com/repos/vox-foundation/vox/releases/latest"
  GITHUB_DL="https://github.com/vox-foundation/vox/releases/download"

  # ── Helpers ─────────────────────────────────────────────────────────────────

  say()      { printf "voxup: %s\n" "$*" >&2; }
  err()      { say "error: $*"; exit 1; }
  need_cmd() { command -v "$1" >/dev/null 2>&1 || err "need '$1' but it was not found in PATH"; }

  # ── Platform detection ───────────────────────────────────────────────────────

  detect_target() {
      _os="$(uname -s)"
      _arch="$(uname -m)"

      case "$_os" in
          Linux)  ;;
          Darwin) ;;
          *)      err "Unsupported OS: $_os (expected Linux or Darwin)" ;;
      esac

      case "$_arch" in
          x86_64)          ;;
          aarch64|arm64)   _arch="aarch64" ;;
          *)               err "Unsupported architecture: $_arch" ;;
      esac

      if [ "$_os" = "Linux" ]; then
          printf "%s" "${_arch}-unknown-linux-gnu"
      else
          printf "%s" "${_arch}-apple-darwin"
      fi
  }

  # ── SHA-256 verification ─────────────────────────────────────────────────────

  verify_checksum() {
      _file="$1"
      _expected="$2"

      if command -v sha256sum >/dev/null 2>&1; then
          _actual="$(sha256sum "$_file" | cut -d ' ' -f1)"
      elif command -v shasum >/dev/null 2>&1; then
          _actual="$(shasum -a 256 "$_file" | cut -d ' ' -f1)"
      else
          say "WARNING: no sha256 tool found — skipping integrity check"
          return 0
      fi

      if [ "$_actual" != "$_expected" ]; then
          err "SHA-256 mismatch for $_file\n  expected: $_expected\n  actual:   $_actual"
      fi
      say "Checksum OK"
  }

  # ── Main ─────────────────────────────────────────────────────────────────────

  main() {
      need_cmd curl
      need_cmd tar

      say "Detecting platform..."
      _target="$(detect_target)"
      say "Target: $_target"

      say "Fetching latest release info..."
      _tag="$(curl -sSfL \
          -H "Accept: application/vnd.github+json" \
          -H "User-Agent: voxup-install.sh" \
          "$GITHUB_API" \
          | grep '"tag_name"' \
          | head -1 \
          | sed 's/.*"tag_name": *"\(.*\)".*/\1/')"
      [ -n "$_tag" ] || err "Could not determine latest release tag from GitHub API"
      say "Latest release: $_tag"

      _archive="voxup-${_tag}-${_target}.tar.gz"
      _archive_url="${GITHUB_DL}/${_tag}/${_archive}"
      _checksums_url="${GITHUB_DL}/${_tag}/checksums.txt"

      _tmpdir="$(mktemp -d)"
      # shellcheck disable=SC2064
      trap "rm -rf '$_tmpdir'" EXIT

      say "Downloading $_archive..."
      curl --proto '=https' --tlsv1.2 -sSfL "$_archive_url" -o "$_tmpdir/$_archive"

      say "Downloading checksums.txt..."
      curl --proto '=https' --tlsv1.2 -sSfL "$_checksums_url" -o "$_tmpdir/checksums.txt"

      _checksum="$(grep "  ${_archive}$" "$_tmpdir/checksums.txt" | cut -d ' ' -f1)"
      [ -n "$_checksum" ] || err "No checksum found for '$_archive' in checksums.txt"

      verify_checksum "$_tmpdir/$_archive" "$_checksum"

      say "Extracting..."
      tar -xzf "$_tmpdir/$_archive" -C "$_tmpdir"

      [ -f "$_tmpdir/voxup" ] || err "voxup binary not found after extraction"
      chmod +x "$_tmpdir/voxup"

      say "Running: voxup install default"
      "$_tmpdir/voxup" install default
  }

  main "$@"
  ```

- [ ] **Step 3.2 — Verify POSIX syntax**

  ```
  sh -n scripts/install.sh
  ```

  Expected: no output (no syntax errors).

- [ ] **Step 3.3 — Verify platform detection logic with a quick unit test**

  Run this inline to confirm the detection logic parses correctly on your machine (this does NOT hit GitHub):

  ```sh
  sh -c '
  set -eu
  _os="$(uname -s)"
  _arch="$(uname -m)"
  case "$_arch" in aarch64|arm64) _arch="aarch64" ;; esac
  if [ "$_os" = "Linux" ]; then
      _target="${_arch}-unknown-linux-gnu"
  else
      _target="${_arch}-apple-darwin"
  fi
  echo "Would download: voxup-vX.Y.Z-${_target}.tar.gz"
  '
  ```

  Expected on macOS arm64: `Would download: voxup-vX.Y.Z-aarch64-apple-darwin.tar.gz`
  Expected on Linux x86_64: `Would download: voxup-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`

- [ ] **Step 3.4 — Commit**

  ```
  git add scripts/install.sh
  git commit -m "feat(install): add install.sh bootstrap for macOS and Linux"
  ```

---

## Task 4 — Write `scripts/install.ps1`

The Windows counterpart to `install.sh`. Must work with:
```powershell
Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1; .\voxup.ps1
```
And also be runnable locally as `.\scripts\install.ps1`.

**Files:**
- Create: `scripts/install.ps1`

- [ ] **Step 4.1 — Create `scripts/install.ps1`**

  Create the file with exactly this content:

  ```powershell
  #Requires -Version 5.1
  # voxup installer — Windows
  # Usage (production):
  #   Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1; .\voxup.ps1
  # Usage (local dev):
  #   .\scripts\install.ps1
  [CmdletBinding()]
  param()

  $ErrorActionPreference = 'Stop'

  $GithubApi = 'https://api.github.com/repos/vox-foundation/vox/releases/latest'
  $GithubDl  = 'https://github.com/vox-foundation/vox/releases/download'

  function Write-Step([string]$Msg) { Write-Host "voxup: $Msg" -ForegroundColor Cyan }
  function Write-Fail([string]$Msg) {
      Write-Host "voxup: error: $Msg" -ForegroundColor Red
      exit 1
  }

  # ── Platform detection ────────────────────────────────────────────────────────

  function Get-VoxupTarget {
      $cpu = $env:PROCESSOR_ARCHITECTURE
      $arch = if ($cpu -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
      return "${arch}-pc-windows-msvc"
  }

  # ── SHA-256 verification ──────────────────────────────────────────────────────

  function Assert-Sha256([string]$FilePath, [string]$Expected) {
      $actual   = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()
      $expected = $Expected.ToLower().Trim()
      if ($actual -ne $expected) {
          Write-Fail "SHA-256 mismatch for $FilePath`n  expected: $expected`n  actual:   $actual"
      }
      Write-Step "Checksum OK"
  }

  # ── Main ─────────────────────────────────────────────────────────────────────

  Write-Step "Detecting platform..."
  $Target = Get-VoxupTarget
  Write-Step "Target: $Target"

  Write-Step "Fetching latest release info..."
  try {
      $release = Invoke-RestMethod `
          -Uri $GithubApi `
          -Headers @{ Accept = 'application/vnd.github+json'; 'User-Agent' = 'voxup-install.ps1' } `
          -UseBasicParsing
  } catch {
      Write-Fail "Failed to fetch release info from GitHub: $_"
  }
  $Tag = $release.tag_name
  if (-not $Tag) { Write-Fail "Could not determine latest release tag from GitHub API" }
  Write-Step "Latest release: $Tag"

  $Archive      = "voxup-${Tag}-${Target}.zip"
  $ArchiveUrl   = "${GithubDl}/${Tag}/${Archive}"
  $ChecksumsUrl = "${GithubDl}/${Tag}/checksums.txt"

  $TmpDir = Join-Path $env:TEMP "voxup-install-$(New-Guid)"
  New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

  try {
      Write-Step "Downloading $Archive..."
      Invoke-WebRequest -Uri $ArchiveUrl -OutFile "$TmpDir\$Archive" -UseBasicParsing

      Write-Step "Downloading checksums.txt..."
      Invoke-WebRequest -Uri $ChecksumsUrl -OutFile "$TmpDir\checksums.txt" -UseBasicParsing

      $ChecksumLine = Get-Content "$TmpDir\checksums.txt" |
          Where-Object { $_ -match "  $([regex]::Escape($Archive))$" } |
          Select-Object -First 1
      if (-not $ChecksumLine) {
          Write-Fail "No checksum entry found for '$Archive' in checksums.txt"
      }
      $ExpectedHash = ($ChecksumLine -split '\s+')[0]

      Assert-Sha256 -FilePath "$TmpDir\$Archive" -Expected $ExpectedHash

      Write-Step "Extracting..."
      Expand-Archive -Path "$TmpDir\$Archive" -DestinationPath $TmpDir -Force

      $VoxupExe = "$TmpDir\voxup.exe"
      if (-not (Test-Path $VoxupExe)) {
          Write-Fail "voxup.exe not found after extraction in $TmpDir"
      }

      Write-Step "Running: voxup install default"
      & $VoxupExe install default
      if ($LASTEXITCODE -ne 0) {
          Write-Fail "voxup install default exited with code $LASTEXITCODE"
      }
  } finally {
      Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
  }
  ```

- [ ] **Step 4.2 — Verify the script parses**

  Run this in PowerShell (works on Windows PowerShell 5.1+):

  ```powershell
  $errors = $null
  $null = [System.Management.Automation.Language.Parser]::ParseFile(
      (Resolve-Path 'scripts\install.ps1').Path,
      [ref]$null,
      [ref]$errors
  )
  if ($errors) { $errors | ForEach-Object { Write-Host "ParseError: $($_.Message)" }; exit 1 }
  Write-Host "install.ps1 syntax OK"
  ```

  Expected: `install.ps1 syntax OK`

- [ ] **Step 4.3 — Commit**

  ```
  git add scripts/install.ps1
  git commit -m "feat(install): add install.ps1 bootstrap for Windows"
  ```

---

## Task 5 — Wire Bootstrap Scripts into Release CI

Publish the bootstrap scripts as GitHub release assets, and add E2E smoke tests.

**Files:**
- Modify: `.github/workflows/release-binaries.yml` (publish job only)
- Modify: `.github/workflows/release-installers.yml`

- [ ] **Step 5.1 — Add checkout to the publish job**

  In `.github/workflows/release-binaries.yml`, the `publish` job currently starts with `actions/download-artifact`. The publish job needs repo access to include the script files. Add a checkout as the **first** step:

  ```yaml
  publish:
    name: Publish GitHub release
    runs-on: [self-hosted, linux, x64]
    needs: build
    steps:
      - uses: actions/checkout@v6   # ← ADD THIS LINE

      - uses: actions/download-artifact@v8
        with:
          path: release-artifacts
  ```

- [ ] **Step 5.2 — Add scripts to the release assets**

  Find the `Create release with assets` step in the `publish` job. Update its `files:` block:

  ```yaml
  - name: Create release with assets
    uses: softprops/action-gh-release@v3
    with:
      files: |
        release-artifacts/release-*/*
        release-artifacts/checksums.txt
        release-artifacts/sbom.spdx.json
        scripts/install.sh
        scripts/install.ps1
      generate_release_notes: true
      fail_on_unmatched_files: false
  ```

- [ ] **Step 5.3 — Add bootstrap syntax + E2E tests to `release-installers.yml`**

  Add two new jobs to `.github/workflows/release-installers.yml` after the existing `test-voxup-installer` job:

  ```yaml
  test-bootstrap-unix:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v6

      - name: Verify install.sh POSIX syntax
        run: |
          sh -n scripts/install.sh
          echo "install.sh syntax OK"

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Build voxup (for E2E bootstrap test)
        run: cargo build --release -p voxup

      - name: E2E — install.sh full (tag runs only)
        if: startsWith(github.ref, 'refs/tags/v')
        run: |
          export HOME=/tmp/fake_home_bootstrap
          mkdir -p $HOME
          sh scripts/install.sh
          if [ ! -f "$HOME/.vox/bin/vox" ]; then
            echo "FAIL: ~/.vox/bin/vox not found after install.sh"
            exit 1
          fi
          version=$("$HOME/.vox/bin/vox" --version 2>&1)
          echo "Installed vox: $version"
          if ! echo "$version" | grep -qE '[0-9]+\.[0-9]+\.[0-9]+'; then
            echo "FAIL: vox --version did not return semver"
            exit 1
          fi
          if [ ! -f "$HOME/.vox/bin/voxup" ]; then
            echo "FAIL: ~/.vox/bin/voxup not found — install.rs did not place voxup"
            exit 1
          fi

  test-bootstrap-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v6

      - name: Verify install.ps1 parses
        shell: pwsh
        run: |
          $errors = $null
          $null = [System.Management.Automation.Language.Parser]::ParseFile(
              (Resolve-Path 'scripts\install.ps1').Path,
              [ref]$null,
              [ref]$errors
          )
          if ($errors) {
              $errors | ForEach-Object { Write-Host "ParseError: $($_.Message)" }
              exit 1
          }
          Write-Host "install.ps1 syntax OK"

      - name: E2E — install.ps1 full (tag runs only)
        if: startsWith(github.ref, 'refs/tags/v')
        shell: pwsh
        run: |
          $fakeHome = "$env:TEMP\fake_home_bootstrap"
          New-Item -ItemType Directory -Force -Path $fakeHome | Out-Null
          $env:USERPROFILE = $fakeHome
          $env:HOMEDRIVE = Split-Path $fakeHome -Qualifier
          $env:HOMEPATH  = Split-Path $fakeHome -NoQualifier
          .\scripts\install.ps1
          $vox = "$fakeHome\.vox\bin\vox.exe"
          if (-not (Test-Path $vox)) {
              Write-Host "FAIL: $vox not found after install.ps1"
              exit 1
          }
          $version = & $vox --version 2>&1
          Write-Host "Installed vox: $version"
          if ($version -notmatch '\d+\.\d+\.\d+') {
              Write-Host "FAIL: vox --version did not return semver"
              exit 1
          }
          $voxup = "$fakeHome\.vox\bin\voxup.exe"
          if (-not (Test-Path $voxup)) {
              Write-Host "FAIL: $voxup not found — install.rs did not place voxup.exe"
              exit 1
          }
  ```

- [ ] **Step 5.4 — Validate both YAML files**

  ```
  python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-installers.yml'))"
  python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-binaries.yml'))"
  ```

  Expected: no output.

- [ ] **Step 5.5 — Commit**

  ```
  git add .github/workflows/release-installers.yml .github/workflows/release-binaries.yml
  git commit -m "ci(release): publish install.sh/install.ps1 as release assets; add bootstrap E2E CI"
  ```

---

## Task 6 — Write Installation Docs

**Files:**
- Create: `docs/src/reference/installation.md`

Every doc file under `docs/src/` needs YAML frontmatter. See `docs/src/contributors/documentation-governance.md` for valid `category` values.

- [ ] **Step 6.1 — Create `docs/src/reference/installation.md`**

  ```markdown
  ---
  title: "Installing Vox"
  description: "Install the Vox programming language and toolchain on macOS, Linux, or Windows using the official voxup installer."
  category: "reference"
  status: "current"
  ---

  # Installing Vox

  Vox installs via **`voxup`**, a toolchain installer modelled after `rustup`.
  A single command downloads the Vox CLI and configures your shell `PATH`.

  ## Quick Install

  ### macOS and Linux

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
  ```

  ### Windows (PowerShell)

  ```powershell
  Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1
  .\voxup.ps1
  ```

  After installation, restart your terminal (or `source ~/.bashrc`) then verify:

  ```
  vox --version
  ```

  ## What Gets Installed

  | Path | Contents |
  |---|---|
  | `~/.vox/bin/vox` | The Vox CLI (real binary from the release archive) |
  | `~/.vox/bin/voxup` | The installer binary (used by `voxup update`) |
  | `~/.vox/toolchains/vox-<version>/` | Versioned toolchain directory |
  | `~/.vox/toolchains/active` | Active version number (plain text) |

  `~/.vox/bin` is added to your shell `PATH` automatically.

  ## Updating

  ```
  voxup update
  ```

  This checks GitHub for a newer release and installs it if available.

  ## Manual Install (developers only)

  If you have the Rust toolchain and want to build from source:

  ```
  cargo install --locked --path crates/voxup
  voxup install default
  ```
  ```

- [ ] **Step 6.2 — Lint the frontmatter**

  ```
  cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/installation.md
  ```

  Expected: no lint errors.

- [ ] **Step 6.3 — Commit**

  ```
  git add docs/src/reference/installation.md
  git commit -m "docs: add installation.md with official voxup install instructions"
  ```

---

## Verification Plan

### Automated Tests

```
cargo test -p voxup
```
Expected: 31 tests, all green.

```
cargo check -p voxup
```
Expected: zero warnings, zero errors.

```
sh -n scripts/install.sh
```
Expected: no output (POSIX syntax clean).

```powershell
# Windows only
$errors = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile(
    (Resolve-Path 'scripts\install.ps1').Path, [ref]$null, [ref]$errors)
if ($errors) { exit 1 }
```
Expected: exits 0.

### CI (PR branch — no tag)

Push a branch. The following CI jobs should run and pass:
- `test-voxup-installer` — existing smoke tests (voxup --help)
- `test-bootstrap-unix` — `sh -n scripts/install.sh` syntax check
- `test-bootstrap-windows` — PowerShell parse check
- Full E2E steps are skipped (tag condition is false)

### Full E2E (tag run)

After Plan B merges, cut a `v*` tag. Verify:

1. GitHub release contains `voxup-v*-{target}.{ext}` for all 4 platforms
2. GitHub release contains `install.sh` and `install.ps1`
3. `checksums.txt` has entries for all voxup archives
4. `test-bootstrap-unix` E2E passes on Ubuntu and macOS
5. `test-bootstrap-windows` E2E passes

### Manual Smoke

After a tagged release:

```sh
# macOS / Linux — using the GitHub release URL directly
curl --proto '=https' --tlsv1.2 -sSf \
  https://github.com/vox-foundation/vox/releases/latest/download/install.sh | sh
vox --version       # must print semver
voxup --version     # must print semver
voxup update        # must say "already up to date"
```

```powershell
# Windows
Invoke-WebRequest `
  -Uri https://github.com/vox-foundation/vox/releases/latest/download/install.ps1 `
  -OutFile install.ps1
.\install.ps1
vox --version
voxup update
```

---

## What Comes Next (Plan C and Plan D)

**Plan C — Release Distribution Completeness:**
- Fix `release-installers.yml` Windows MSI build (add `cargo build --release` before `cargo wix`)
- Implement real Homebrew tap dispatch (`repository_dispatch` to `vox-foundation/homebrew-vox`)
- Add Linux `.deb` upload to GitHub release artifacts

**Plan D — Toolchain Expansion:**
- Implement `voxup toolchain add <name>` subcommand
- Download real WASM sysroots in `provision_wasm_sysroots()` (currently just creates an empty dir)
- Hermetic Node.js download for `vox build` frontend bundling
- `voxup` self-update (update the voxup binary itself, not just vox)
