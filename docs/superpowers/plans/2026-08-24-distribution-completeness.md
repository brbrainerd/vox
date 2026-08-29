# Distribution Completeness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four remaining distribution gaps — the Tauri desktop GUI never bundles anything, the two GPU plugins are undeliverable, the `.deb` release fix is unverified, and there is no Homebrew formula audit — while eliminating every hand-pinned version/hash in the chain so a future version bump touches nothing.

**Architecture:** Config and CI fixes for the GUI bundler and its signing step; a new fail-closed-but-dynamic install path in `vox-cli` that lets first-party GPU plugins derive their version and checksum from the same release they ship in, instead of pinning either in committed source; new CI jobs that build, package, and upload those plugins as release assets; a widened `version-tag-guard` that admits prerelease tags without weakening its real check; and three real-signal verification tasks (a WSL2 `.deb` install, a WSL2 Homebrew formula audit, and a live tag push) that prove each fix against the actual GitHub Release API, not workflow status.

**Tech Stack:** Rust (`vox-cli`, `vox-plugin-*` crates), GitHub Actions YAML, Tauri v2 config, WSL2/Ubuntu 24.04, Linuxbrew.

**Spec:** `docs/superpowers/specs/2026-08-24-distribution-completeness-design.md`

## Global Constraints

- No hardcoded version or hash anywhere in the chain — every value is either inherited (`version.workspace = true`), derived at build/install time, or guarded by a test that fails on drift. (Spec: "No hardcoded versions or hashes anywhere in the chain.")
- The `install_from_catalog` change is on a fail-closed security path: an archive must still be refused unless its computed SHA-256 matches one obtained from the release. Third-party `github:OWNER/REPO` sources keep the pinned-hash model, completely untouched. (Spec Risks.)
- CUDA build attempts must never block Metal, the core `vox`/`vox-ml-cli`/`voxup` release, or `publish`. (Spec Phase 3, Structural problem B.)
- Verification tags are `v0.6.0-rc.<VOX_BUILD_NUMBER>` (prerelease, never `latest`); the real `v0.6.0` is cut only after Phases 1–3 are proven. (Spec Phase 0.)
- Reuse existing patterns rather than inventing new ones: `// vox:defactored-from <crate> <date>` for duplicated helpers under ~50 lines (AGENTS.md Dependency Discipline), `current_target_triple()`/`plugin_artifact_filename()` from `vox-plugin-types` for triple-key and cdylib-name derivation (already exist, already tested).

---

### Task 1: Widen `version-tag-guard.yml` to admit prereleases without weakening it

**Files:**
- Modify: `.github/workflows/version-tag-guard.yml:44-71`

**Interfaces:**
- Consumes: nothing from later tasks (first task)
- Produces: any tag of the form `v<core>-<anything>` now passes this guard when `<core>` matches `Cargo.toml`'s workspace version, which every later verification task's tag push depends on

- [ ] **Step 1: Read the current comparison logic to confirm the exact lines to change**

Run: `sed -n '44,71p' .github/workflows/version-tag-guard.yml`

Expected: the `tag_version="${tag#v}"` line and the `if [ "${tag_version}" != "${cargo_version}" ]` block, unchanged from what this plan was written against.

- [ ] **Step 2: Change the tag-version extraction to strip prerelease/build metadata**

In `.github/workflows/version-tag-guard.yml`, change:

```bash
          tag="${GITHUB_REF_NAME}"          # e.g. v0.7.0
          tag_version="${tag#v}"            # strip leading v -> 0.7.0
```

to:

```bash
          tag="${GITHUB_REF_NAME}"          # e.g. v0.7.0, v0.6.0-rc.4735
          tag_core="${tag#v}"               # strip leading v -> 0.7.0, 0.6.0-rc.4735
          # Strip semver prerelease/build metadata so v0.6.0-rc.4735 and
          # v0.6.0-nightly.4735 compare equal to Cargo.toml's plain "0.6.0" --
          # this only WIDENS what passes; a real mismatch (v0.7.0 tagged
          # against a 0.6.0 Cargo.toml) still fails, prerelease suffix or not.
          tag_version="${tag_core%%-*}"      # 0.6.0-rc.4735 -> 0.6.0
          tag_version="${tag_version%%+*}"   # 0.6.0+build.1 -> 0.6.0 (if ever tagged with build metadata)
```

- [ ] **Step 3: Update the echo line that reports what was compared**

Change:

```bash
          echo "git tag        : ${tag} (version ${tag_version})"
```

to:

```bash
          echo "git tag        : ${tag} (core version ${tag_version})"
```

- [ ] **Step 4: Verify the new extraction logic locally with representative inputs**

Run this exact snippet in bash to confirm the extraction behaves correctly for every tag shape this plan will use, before trusting it inside CI:

```bash
for tag in v0.6.0 v0.6.0-rc.4735 v0.6.0-nightly.4735 v0.7.0 v0.6.0+build.9; do
  tag_core="${tag#v}"
  tag_version="${tag_core%%-*}"
  tag_version="${tag_version%%+*}"
  echo "$tag -> $tag_version"
done
```

Expected output:

```
v0.6.0 -> 0.6.0
v0.6.0-rc.4735 -> 0.6.0
v0.6.0-nightly.4735 -> 0.6.0
v0.7.0 -> 0.7.0
v0.6.0+build.9 -> 0.6.0
```

If any line differs, the extraction is wrong — fix Step 2 before proceeding, since every later verification task's tag push depends on this passing.

- [ ] **Step 5: Confirm the file is still valid YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/version-tag-guard.yml', encoding='utf-8')); print('OK')"`

Expected: `OK`

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/version-tag-guard.yml
git commit -m "fix(ci): let version-tag-guard admit prerelease tags

Only v<workspace-version> could ever pass -- v0.6.0-rc.1 failed exactly
as hard as a real drift. This made every verification tag push produce
a permanently-red guard, and made nightly tags structurally impossible
(no dated/numbered tag can pass a guard that requires an exact match).

Strip -prerelease and +build metadata before comparing. A real mismatch
(v0.7.0 tagged against a 0.6.0 Cargo.toml) still fails."
```

---

### Task 2: Turn on the Tauri bundler and stop hardcoding its version

**Files:**
- Modify: `crates/vox-gui/tauri.conf.json`

**Interfaces:**
- Consumes: nothing
- Produces: a `bundle.active: true` config that `tauri-action` (Task 3's workflow) will actually produce installers from

- [ ] **Step 1: Read the current bundle block to confirm exact content**

Run: `cat crates/vox-gui/tauri.conf.json`

Expected: `bundle` contains only `icon` and `externalBin`, and the top-level object has a hardcoded `"version": "0.6.0"`.

- [ ] **Step 2: Add `active` and `targets`, remove the hardcoded version**

Replace the full file content with:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Vox",
  "identifier": "org.vox-foundation.gui",
  "build": {
    "beforeDevCommand": "",
    "beforeBuildCommand": "",
    "frontendDist": "ui/dist"
  },
  "app": {
    "windows": [
      {
        "title": "Axis",
        "width": 1280,
        "height": 800,
        "label": "main"
      }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' asset: http://asset.localhost data: blob:; font-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "dmg", "appimage", "deb"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "externalBin": [
      "../../target/release/vox"
    ]
  }
}
```

The `"version"` key is removed, not renamed: `crates/vox-gui/Cargo.toml` already has `version.workspace = true`, and Tauri's config schema falls back to the building crate's `Cargo.toml` `[package] version` when the config omits `version` — so this is the same SSOT the rest of the workspace already uses, with no new mechanism to build. Task 5's verification step confirms this fallback actually produces `0.6.0`-versioned installers; if it doesn't, that step fails loudly rather than shipping an unversioned app silently.

`targets` is per-OS in practice — Tauri only builds the targets valid for the host it runs on (`msi` only assembles on Windows, `dmg` only on macOS, `appimage`/`deb` only on Linux), so listing all four here is correct and matches `release-gui.yml`'s three-OS matrix; each OS's `tauri-action` run silently skips the targets it can't build.

- [ ] **Step 3: Confirm the file is still valid JSON**

Run: `python -c "import json; json.load(open('crates/vox-gui/tauri.conf.json', encoding='utf-8')); print('OK')"`

Expected: `OK`

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/tauri.conf.json
git commit -m "fix(gui): turn on the Tauri bundler

bundle.active defaults to false in Tauri v2. Every recorded
release-gui.yml run (6/6) built the bare executable, bundled nothing,
and tauri-action correctly reported 'No artifacts were found' -- this
alone explains every historical failure on every platform.

Also drops the hardcoded \"version\": \"0.6.0\" -- vox-gui's Cargo.toml
already inherits the workspace version, and Tauri's config falls back
to the crate's own Cargo.toml version when the config omits it. One
less place a version bump has to be remembered."
```

---

### Task 3: Fix the Windows signing step's nonexistent path

**Files:**
- Modify: `.github/workflows/release-gui.yml`

**Interfaces:**
- Consumes: Task 2's `bundle.active: true` (this step only has anything to sign once Task 2 lands)
- Produces: a signing step that points at where `tauri-action` actually places the MSI, and that skips cleanly instead of failing when Azure secrets are absent

- [ ] **Step 1: Read the current signing step**

Run: `grep -n "Sign Windows Installer" -A 12 .github/workflows/release-gui.yml`

Expected: `files-folder: ./crates/vox-gui/src-tauri/target/release/bundle/msi` — a path under a `src-tauri/` directory that does not exist in this repo.

- [ ] **Step 2: Confirm the real bundle output path**

`.cargo/config.toml` pins `CARGO_TARGET_DIR = "target"` (relative to the workspace root), and Tauri's config lives at `crates/vox-gui/tauri.conf.json` with no `src-tauri` indirection — so the real MSI bundle path, once Task 2's `active: true` takes effect, is `target/release/bundle/msi` at the workspace root, not under `crates/vox-gui/`.

Run: `grep -n "CARGO_TARGET_DIR" .cargo/config.toml`

Expected: confirms `CARGO_TARGET_DIR = { value = "target", relative = true }` resolves to the workspace root.

- [ ] **Step 3: Fix the path and gate the whole step on the signing secret existing**

Change:

```yaml
      - name: Sign Windows Installer
        if: runner.os == 'Windows'
        uses: azure/trusted-signing-action@v2
```

to:

```yaml
      - name: Sign Windows Installer
        # Also gated on the signing secret existing: an absent
        # AZURE_CODE_SIGNING_ACCOUNT means signing was never configured for
        # this repo (or this fork), and an unsigned MSI is a
        # worse-but-shippable outcome -- a hard failure here would take the
        # whole GUI release down over a missing credential, not a bug.
        if: runner.os == 'Windows' && env.AZURE_CODE_SIGNING_ACCOUNT != ''
        env:
          AZURE_CODE_SIGNING_ACCOUNT: ${{ secrets.AZURE_CODE_SIGNING_ACCOUNT }}
        uses: azure/trusted-signing-action@v2
```

And change:

```yaml
          files-folder: ./crates/vox-gui/src-tauri/target/release/bundle/msi
```

to:

```yaml
          # target/ is the workspace root's CARGO_TARGET_DIR (.cargo/config.toml)
          # -- there is no src-tauri/ directory in this repo.
          files-folder: ./target/release/bundle/msi
```

- [ ] **Step 4: Confirm the file is still valid YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/release-gui.yml', encoding='utf-8')); print('OK')"`

Expected: `OK`

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release-gui.yml
git commit -m "fix(ci): correct the Windows GUI signing path, skip cleanly without a cert

files-folder pointed at crates/vox-gui/src-tauri/target/..., a directory
that does not exist in this repo (no src-tauri/ indirection; Tauri config
lives at crates/vox-gui/tauri.conf.json) and CARGO_TARGET_DIR is pinned
to the workspace root. This was latent -- it could never fire because the
build died before signing -- and would have failed immediately once the
bundler (previous commit) started producing an MSI.

Also gates the whole step on AZURE_CODE_SIGNING_ACCOUNT being set: an
unsigned installer is a worse-but-shippable outcome; failing the entire
release over a missing signing credential is not."
```

---

### Task 4: Stop hardcoding the GPU plugin versions — inherit and drift-gate them

**Files:**
- Modify: `crates/vox-plugin-mens-candle-cuda/Plugin.toml`
- Modify: `crates/vox-plugin-mens-candle-cuda/Cargo.toml` (add `toml` dev-dependency)
- Create: `crates/vox-plugin-mens-candle-cuda/tests/plugin_toml_version_matches_crate.rs`
- Modify: `crates/vox-plugin-mens-candle-metal/Plugin.toml`
- Modify: `crates/vox-plugin-mens-candle-metal/Cargo.toml` (add `toml` dev-dependency)
- Create: `crates/vox-plugin-mens-candle-metal/tests/plugin_toml_version_matches_crate.rs`

**Interfaces:**
- Consumes: nothing
- Produces: `Plugin.toml`'s `version` field, now `0.6.0`, which Task 8 (release packaging) and Task 6 (install path) both rely on matching the workspace version

- [ ] **Step 1: Write the failing test (CUDA)**

`Plugin.toml` is not Cargo-parsed, so nothing enforces it staying in sync with the crate's own `version.workspace = true`. This test makes that drift loud instead of silent: any future workspace version bump that forgets to touch `Plugin.toml` fails CI here, with a message naming exactly what to fix.

Create `crates/vox-plugin-mens-candle-cuda/tests/plugin_toml_version_matches_crate.rs`:

```rust
//! Plugin.toml is hand-maintained, not Cargo-generated, so nothing else
//! enforces it staying in sync with the crate's own `version.workspace =
//! true`. This is the drift gate: a workspace version bump that forgets to
//! touch Plugin.toml fails here, loudly, instead of shipping a plugin whose
//! declared version disagrees with the release it ships in.

#[test]
fn plugin_toml_version_matches_crate_version() {
    let manifest = include_str!("../Plugin.toml");
    let parsed: toml::Value = manifest.parse().expect("Plugin.toml must be valid TOML");
    let declared = parsed["plugin"]["version"]
        .as_str()
        .expect("Plugin.toml must have [plugin] version as a string");
    assert_eq!(
        declared,
        env!("CARGO_PKG_VERSION"),
        "Plugin.toml's version ({declared}) does not match this crate's \
         Cargo.toml version ({}). Update Plugin.toml's [plugin] version to \
         match -- it is not derived automatically.",
        env!("CARGO_PKG_VERSION")
    );
}
```

- [ ] **Step 2: Add the `toml` dev-dependency (CUDA)**

`toml = "0.8"` is already a workspace dependency (`Cargo.toml:210`) — this adds it as a dev-dependency for the plugin crate, no new external crate is introduced.

In `crates/vox-plugin-mens-candle-cuda/Cargo.toml`, change:

```toml
[dev-dependencies]
tempfile = { workspace = true }
```

to:

```toml
[dev-dependencies]
tempfile = { workspace = true }
toml = { workspace = true }
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-plugin-mens-candle-cuda --test plugin_toml_version_matches_crate -- --nocapture`

Expected: FAIL — `Plugin.toml's version (0.1.0) does not match this crate's Cargo.toml version (0.6.0)`

- [ ] **Step 4: Fix `Plugin.toml` to pass**

In `crates/vox-plugin-mens-candle-cuda/Plugin.toml`, change:

```toml
version = "0.1.0"
```

to:

```toml
version = "0.6.0"
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-plugin-mens-candle-cuda --test plugin_toml_version_matches_crate -- --nocapture`

Expected: PASS

- [ ] **Step 6: Repeat Steps 1–5 for Metal**

Create `crates/vox-plugin-mens-candle-metal/tests/plugin_toml_version_matches_crate.rs` with identical content to Step 1's file (same test, different crate — `include_str!("../Plugin.toml")` and `env!("CARGO_PKG_VERSION")` resolve per-crate automatically).

Add the same `toml = { workspace = true }` line to `crates/vox-plugin-mens-candle-metal/Cargo.toml`'s `[dev-dependencies]`.

Run: `cargo test -p vox-plugin-mens-candle-metal --test plugin_toml_version_matches_crate -- --nocapture`

Expected: FAIL — `Plugin.toml's version (0.1.0) does not match ... (0.6.0)`

Change `crates/vox-plugin-mens-candle-metal/Plugin.toml`'s `version = "0.1.0"` to `version = "0.6.0"`.

Run the same test again.

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-plugin-mens-candle-cuda/Plugin.toml crates/vox-plugin-mens-candle-cuda/Cargo.toml crates/vox-plugin-mens-candle-cuda/tests/plugin_toml_version_matches_crate.rs crates/vox-plugin-mens-candle-metal/Plugin.toml crates/vox-plugin-mens-candle-metal/Cargo.toml crates/vox-plugin-mens-candle-metal/tests/plugin_toml_version_matches_crate.rs
git commit -m "fix(plugins): sync Plugin.toml version to the workspace, add a drift gate

Both plugin crates already inherit version.workspace = true; only the
hand-written Plugin.toml files still hardcoded 0.1.0, a stale duplicate
of a number Cargo already knows. Bumped to 0.6.0 and added a test in
each crate that fails loudly on any future drift between Plugin.toml's
version and the crate's own CARGO_PKG_VERSION -- a version bump that
forgets to touch Plugin.toml now fails CI instead of shipping quietly
wrong."
```

---

### Task 5: Add a dynamic, checksums.txt-verified install path for first-party plugins

**Files:**
- Modify: `crates/vox-cli/src/commands/plugin/install.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks directly, but the asset naming it constructs (`{id}-v{version}-{triple}.zip`) must match what Task 8's packaging step produces, and the `checksums.txt` format it parses must match what `release-binaries.yml`'s `publish` job already generates (`sha256sum` output: `<hash>  <filename>`)
- Produces: `install_first_party_plugin(id, triple, yes, allow_unverified) -> Result<()>` and `parse_checksums(text: &str) -> HashMap<String, String>`, called by the rewired `install_from_catalog` (this task) — no other task calls these directly, but Task 7's catalog repoint is what routes traffic into this path

This is the security-sensitive task in this plan. It changes a fail-closed
verification path. The invariant that must hold after this task: an
archive is refused unless its computed SHA-256 matches a hash obtained
from the release itself — this task only changes *where* that hash comes
from (a fetched `checksums.txt` instead of a catalog-pinned string), and
only for the one first-party repo constant, never for third-party
`github:OWNER/REPO` sources.

- [ ] **Step 1: Write the failing test for `parse_checksums`**

This is the same parsing shape `voxup::download::parse_checksums` already uses and tests — duplicated per AGENTS.md's defactor policy (under ~50 lines, and a `vox-cli -> voxup` crate edge is not authorized), not reused via a new dependency.

In `crates/vox-cli/src/commands/plugin/install.rs`, find the `#[cfg(test)] mod tests {` block (it already exists — this crate already has plugin-install tests). Add, inside that module:

```rust
    #[test]
    fn parse_checksums_extracts_name_to_hash_pairs() {
        let text = "\
aaaa000000000000000000000000000000000000000000000000000000001111  mens-candle-metal-v0.6.0-macos-aarch64.zip
bbbb000000000000000000000000000000000000000000000000000000002222  checksums.txt
";
        let got = parse_checksums(text);
        assert_eq!(
            got.get("mens-candle-metal-v0.6.0-macos-aarch64.zip").map(String::as_str),
            Some("aaaa000000000000000000000000000000000000000000000000000000001111")
        );
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn parse_checksums_skips_short_and_blank_lines() {
        let text = "\n   \nnothash  file.zip\n";
        let got = parse_checksums(text);
        assert!(got.is_empty(), "malformed/short hash lines must be skipped, got {got:?}");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli --lib plugin::install::tests::parse_checksums -- --nocapture`

Expected: FAIL — `cannot find function 'parse_checksums' in this scope`

- [ ] **Step 3: Implement `parse_checksums`**

Add this function to `crates/vox-cli/src/commands/plugin/install.rs`, near `verify_plugin_archive` (same file, module-level, not inside `mod tests`):

```rust
/// Parse a `checksums.txt` body (as generated by `release-binaries.yml`'s
/// `publish` job: `sha256sum` output, `<hash>  <filename>` per line) into
/// filename -> lowercase hex sha256.
///
// vox:defactored-from voxup 2026-08-24 (voxup::download::parse_checksums, ~13 lines)
fn parse_checksums(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let (hash, rest) = line.split_once("  ")?;
            let name = rest.trim().to_string();
            let hash = hash.trim().to_lowercase();
            if hash.len() != 64 {
                return None;
            }
            Some((name, hash))
        })
        .collect()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-cli --lib plugin::install::tests::parse_checksums -- --nocapture`

Expected: PASS, both tests

- [ ] **Step 5: Write the failing test for the first-party install URL/checksum-URL construction**

The network fetch itself isn't unit-testable without a mock server (out of scope for this crate's existing test style — `install.rs`'s other network paths aren't mocked either), so this test covers the pure, deterministic part: given a version and triple, what URLs get built. Add:

```rust
    #[test]
    fn first_party_plugin_urls_are_built_from_the_running_binary_version() {
        let (asset_url, checksums_url) =
            first_party_plugin_urls("mens-candle-metal", "0.6.0", "macos-aarch64");
        assert_eq!(
            asset_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0/mens-candle-metal-v0.6.0-macos-aarch64.zip"
        );
        assert_eq!(
            checksums_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0/checksums.txt"
        );
    }
```

- [ ] **Step 6: Run the test to verify it fails**

Run: `cargo test -p vox-cli --lib plugin::install::tests::first_party_plugin_urls -- --nocapture`

Expected: FAIL — `cannot find function 'first_party_plugin_urls' in this scope`

- [ ] **Step 7: Implement the first-party install path**

Add near the top of `install.rs` (module-level consts) and near `install_from_catalog`:

```rust
/// The repo whose own GitHub Release first-party plugins ship as assets of.
/// Not a `github:` catalog source string (no plugin is written this way in
/// catalog.toml) -- it is the trust root `install_from_catalog` compares a
/// `github:` source against to decide whether it may use the dynamic,
/// checksums.txt-verified path below instead of requiring a pinned
/// version+sha256.
const FIRST_PARTY_PLUGIN_REPO: &str = "vox-foundation/vox";

/// Build the asset URL and the checksums.txt URL for a first-party plugin
/// at `version` for `triple`. Pure and deterministic -- no network, no
/// filesystem -- so it's tested directly rather than through a live fetch.
fn first_party_plugin_urls(id: &str, version: &str, triple: &str) -> (String, String) {
    let base = format!("https://github.com/{FIRST_PARTY_PLUGIN_REPO}/releases/download/v{version}");
    let asset_url = format!("{base}/{id}-v{version}-{triple}.zip");
    let checksums_url = format!("{base}/checksums.txt");
    (asset_url, checksums_url)
}

/// Fetch `checksums.txt` from `checksums_url` and return the hash recorded
/// for `asset_name`.
///
/// Fail-closed: a network error, a malformed checksums.txt, or a missing
/// entry for `asset_name` is always an `Err`, never a silent fallback to
/// "unverified" -- that decision belongs solely to the caller's explicit
/// `allow_unverified` flag, checked before this function is ever called.
async fn fetch_first_party_checksum(checksums_url: &str, asset_name: &str) -> Result<String> {
    let client = vox_http_client::client();
    let text = client
        .get(checksums_url)
        .send()
        .await
        .with_context(|| format!("GET {checksums_url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error fetching {checksums_url}"))?
        .text()
        .await
        .context("reading checksums.txt")?;

    parse_checksums(&text).remove(asset_name).with_context(|| {
        format!("no checksums.txt entry found for {asset_name} at {checksums_url}")
    })
}

/// Install a first-party plugin shipped as a release asset of vox's own
/// repo. Unlike third-party `github:` sources, neither `version` nor
/// `sha256` is pinned in catalog.toml: the plugin ships in the SAME
/// release as this running binary (Phase 0's tag-alignment decision), so
/// its version is this binary's own `CARGO_PKG_VERSION`, and its expected
/// hash is read from that release's `checksums.txt` -- the same mechanism
/// `voxup` already uses to verify the `vox` binary itself. This stays
/// within the trust boundary the running binary already sits in; it does
/// not widen it to arbitrary `github:` sources (see catalog.rs's
/// FIRST_PARTY_PLUGIN_REPO check in `install_from_catalog`, and the
/// spec's Risks section).
async fn install_first_party_plugin(
    id: &str,
    triple: &str,
    yes: bool,
    allow_unverified: bool,
) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let asset_name = format!("{id}-v{version}-{triple}.zip");
    let (asset_url, checksums_url) = first_party_plugin_urls(id, version, triple);

    let expected_sha256 = if allow_unverified {
        None
    } else {
        Some(fetch_first_party_checksum(&checksums_url, &asset_name).await?)
    };

    install_from_url(&asset_url, yes, expected_sha256.as_deref(), allow_unverified).await
}
```

Then rewire `install_from_catalog`'s `github:` branch. Change:

```rust
    } else if let Some(gh) = source.strip_prefix("github:") {
        // Pinned, not `latest`: the bytes behind a floating asset change, so no
        // recorded hash could ever match it.
        let triple = vox_plugin_host::current_target_triple_key();
        let version = entry.version.as_deref().with_context(|| {
            format!(
                "plugin '{id}' has a github: source but no pinned `version` in \
                 catalog.toml; an unpinned release asset cannot be checksummed"
            )
        })?;
        let url = format!(
            "https://github.com/{gh}/releases/download/v{version}/{id}-v{version}-{triple}.zip"
        );
        install_from_url(&url, yes, entry.sha256.as_deref(), allow_unverified).await
    } else {
```

to:

```rust
    } else if let Some(gh) = source.strip_prefix("github:") {
        let triple = vox_plugin_host::current_target_triple_key();

        // First-party plugins shipped inside vox's own release derive both
        // the version and the checksum dynamically (see
        // install_first_party_plugin) instead of requiring either to be
        // pinned by hand in catalog.toml.
        if gh == FIRST_PARTY_PLUGIN_REPO {
            return install_first_party_plugin(id, triple, yes, allow_unverified).await;
        }

        // Third-party sources keep the pinned-hash model unchanged: no
        // dynamic version/checksum lookup for a repo this binary doesn't
        // share a release with. Pinned, not `latest`: the bytes behind a
        // floating asset change, so no recorded hash could ever match it.
        let version = entry.version.as_deref().with_context(|| {
            format!(
                "plugin '{id}' has a github: source but no pinned `version` in \
                 catalog.toml; an unpinned release asset cannot be checksummed"
            )
        })?;
        let url = format!(
            "https://github.com/{gh}/releases/download/v{version}/{id}-v{version}-{triple}.zip"
        );
        install_from_url(&url, yes, entry.sha256.as_deref(), allow_unverified).await
    } else {
```

- [ ] **Step 8: Run the test to verify it passes**

Run: `cargo test -p vox-cli --lib plugin::install::tests -- --nocapture`

Expected: PASS, all tests in the module (existing plugin-install tests plus the three new ones)

- [ ] **Step 9: Compile-check the whole crate**

Run: `cargo check -p vox-cli`

Expected: no errors

- [ ] **Step 10: Commit**

```bash
git add crates/vox-cli/src/commands/plugin/install.rs
git commit -m "feat(cli): derive first-party plugin version+checksum, don't pin them

Pinning a GPU plugin's release sha256 in catalog.toml is unknowable
until after the release is built, which forces a two-release bootstrap:
ship the asset, THEN commit the hash in a follow-up, with install
correctly refusing in between.

For plugins shipped as assets of vox's own release, this reads the
expected hash from that release's own checksums.txt instead -- the
same mechanism voxup already uses to verify the vox binary itself, kept
within the same trust boundary (same repo, same release, same root the
user already trusts to run vox). The invariant is unchanged: an archive
is refused unless its computed hash matches one obtained from the
release. Only WHERE that hash comes from changes, and only for the one
FIRST_PARTY_PLUGIN_REPO constant -- third-party github: sources keep
the pinned-hash model, completely untouched.

Security-sensitive change on a fail-closed path; see the spec's Risks
section (docs/superpowers/specs/2026-08-24-distribution-completeness-design.md)."
```

---

### Task 6: Repoint the GPU plugin catalog entries at this repo

**Files:**
- Modify: `crates/vox-plugin-catalog/catalog.toml:24-40`

**Interfaces:**
- Consumes: Task 5's `FIRST_PARTY_PLUGIN_REPO` constant (`"vox-foundation/vox"`) — this task's `default-source` value must match it exactly, or `install_from_catalog` falls through to the third-party pinned-hash branch instead
- Produces: catalog entries that Task 8's verification step (`vox plugin install`) resolves against

- [ ] **Step 1: Read the current entries**

Run: `sed -n '23,41p' crates/vox-plugin-catalog/catalog.toml`

Expected: `mens-candle-cuda`'s `default-source = "github:vox-foundation/vox-plugin-mens-candle-cuda"` (a 404 repo) and `mens-candle-metal`'s `default-source = "local:crates/vox-plugin-mens-candle-metal"` (correctly gated behind `VOX_LOCAL_PLUGIN_FALLBACK`, refused for real users).

- [ ] **Step 2: Repoint both entries**

Change:

```toml
[[plugin]]
id = "mens-candle-cuda"
payload-kind = "code"
description = "ML training backend using Candle with CUDA acceleration."
status = "beta"
extension-points = ["MlBackend"]
requires-tag = "nvidia-gpu"
default-source = "github:vox-foundation/vox-plugin-mens-candle-cuda"
bundled-in = ["vox-ml", "vox-dev"]

[[plugin]]
id = "mens-candle-metal"
payload-kind = "code"
description = "ML training backend using Candle with Metal acceleration."
status = "beta"
extension-points = ["MlBackend"]
requires-tag = "apple-silicon"
default-source = "local:crates/vox-plugin-mens-candle-metal"
bundled-in = ["vox-ml-metal", "vox-dev"]
```

to:

```toml
[[plugin]]
id = "mens-candle-cuda"
payload-kind = "code"
description = "ML training backend using Candle with CUDA acceleration."
status = "beta"
extension-points = ["MlBackend"]
requires-tag = "nvidia-gpu"
# Ships as a release asset of vox's own repo, not a separate plugin repo --
# see install_from_catalog's FIRST_PARTY_PLUGIN_REPO check
# (crates/vox-cli/src/commands/plugin/install.rs). No version/sha256 pinned
# here: both are derived at install time from the running binary's own
# version and that release's checksums.txt.
default-source = "github:vox-foundation/vox"
bundled-in = ["vox-ml", "vox-dev"]

[[plugin]]
id = "mens-candle-metal"
payload-kind = "code"
description = "ML training backend using Candle with Metal acceleration."
status = "beta"
extension-points = ["MlBackend"]
requires-tag = "apple-silicon"
# Same first-party release-asset model as mens-candle-cuda above. The
# workspace-local fallback (VOX_LOCAL_PLUGIN_FALLBACK=1) still works
# transparently for contributors -- install_from_catalog checks for a
# matching on-disk crate before ever resolving this source, for BOTH
# local: and github: defaults.
default-source = "github:vox-foundation/vox"
bundled-in = ["vox-ml-metal", "vox-dev"]
```

- [ ] **Step 3: Validate the catalog still parses**

Run: `cargo build -p vox-plugin-catalog`

Expected: builds clean (this crate's build validates `catalog.toml` per its own header comment)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-plugin-catalog/catalog.toml
git commit -m "fix(catalog): ship GPU plugins as vox's own release assets

mens-candle-cuda's source repo (vox-foundation/vox-plugin-mens-candle-cuda)
returns HTTP 404 -- it never existed or was never published, and the
install could never resolve. mens-candle-metal's local: source is
correctly refused for real users by the distribution-security-floor
gate (CWD-relative, would let any directory supply native code).

Repointing both at github:vox-foundation/vox routes them through
install_from_catalog's new first-party path (previous commit), which
needs no version/sha256 pinned here -- both derive dynamically from the
release the plugin ships in. Also collapses the trust root to the one
users already trust to run vox itself."
```

---

### Task 7: Build and publish the GPU plugin release assets in CI

**Files:**
- Modify: `.github/workflows/release-binaries.yml`

**Interfaces:**
- Consumes: Task 4's `Plugin.toml` version (`0.6.0`, matching the workspace); Task 6's catalog `default-source`; the naming contract Task 5's `first_party_plugin_urls` constructs (`{id}-v{version}-{triple}.zip`); `vox_plugin_types::current_target_triple()` / `plugin_artifact_filename()`'s naming rule (`lib{crate_underscored}.{ext}`) for the cdylib inside the zip
- Produces: two new jobs (`gpu-plugin-metal`, `gpu-plugin-cuda`) whose `release-gpu-*`-prefixed artifacts feed both the existing `checksums.txt` generation and the existing `publish` step's `files:` glob without either needing to change, because both already match on the `release-*` artifact-directory prefix

- [ ] **Step 1: Confirm the existing artifact-naming convention this must match**

Run: `grep -n "name: release-" .github/workflows/release-binaries.yml`

Expected: `name: release-${{ matrix.target }}` — every matrix build artifact is named `release-<something>`. The `publish` job's checksum step (`for f in release-*/*`) and its `files:` glob (`release-artifacts/release-*/*`) both match on that `release-` prefix, so a new artifact named `release-gpu-metal-<triple>` is picked up automatically by both, with no change to either.

- [ ] **Step 2: Add the Metal plugin build job**

Metal needs no extra toolkit (ships with Xcode CLT) and builds cleanly on `macos-latest` — the proven path per the spec. Add this job to `.github/workflows/release-binaries.yml`, after the `build` job and before `dist-verify`:

```yaml
  gpu-plugin-metal:
    name: Build mens-candle-metal plugin
    runs-on: macos-latest
    timeout-minutes: 60
    # Non-blocking for the release: a plugin build failure must never take
    # down the core vox/vox-ml-cli/voxup release. continue-on-error at the
    # job level means `needs: [..., gpu-plugin-metal]` on `publish` below
    # is satisfied even if this job's steps fail -- publish still WAITS
    # for it (avoiding the "checksums.txt built before the plugin zip
    # exists" race), it just isn't blocked by it.
    continue-on-error: true
    steps:
      - uses: actions/checkout@v7

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"

      - name: Build the Metal cdylib
        run: cargo build -p vox-plugin-mens-candle-metal --profile dist --features metal

      - name: Package the plugin zip
        # Flat by construction: Plugin.toml and the renamed cdylib both go
        # into a fresh staging directory with nothing else in it, then that
        # directory's CONTENTS (not the directory itself) get zipped --
        # install_from_path only reads top-level files, so any nesting
        # here would extract fine and then be silently ignored.
        run: |
          set -euo pipefail
          version="$(cargo pkgid -p vox-plugin-mens-candle-metal | sed -E 's/.*[#@]//')"
          triple="macos-$(uname -m | sed 's/x86_64/x86_64/; s/arm64/aarch64/')"
          staging="$(mktemp -d)"
          cp crates/vox-plugin-mens-candle-metal/Plugin.toml "$staging/"
          cp target/dist/libvox_plugin_mens_candle_metal.dylib "$staging/"
          mkdir -p dist
          (cd "$staging" && zip -j "$OLDPWD/dist/mens-candle-metal-v${version}-${triple}.zip" ./*)
          echo "Packaged: mens-candle-metal-v${version}-${triple}.zip"

      - name: Upload plugin artifact
        uses: actions/upload-artifact@v7
        with:
          name: release-gpu-metal
          path: dist/mens-candle-metal-v*.zip
          if-no-files-found: error
```

- [ ] **Step 3: Add the CUDA plugin build job**

CUDA is the least certain item in this plan: no workflow in this repo installs a CUDA toolkit on a hosted runner today, and `nvcc` is otherwise only available on the self-hosted fleet this whole program routes around. `continue-on-error: true` here means a failed CUDA-toolkit install or build produces real information from a real attempt, without gambling the release on an unproven path. Add:

```yaml
  gpu-plugin-cuda:
    name: Build mens-candle-cuda plugin
    runs-on: ubuntu-latest
    timeout-minutes: 60
    # See gpu-plugin-metal's comment above -- same non-blocking contract.
    # CUDA is the least proven part of this whole plan: no workflow in this
    # repo has ever installed a CUDA toolkit on a hosted runner before this.
    continue-on-error: true
    steps:
      - uses: actions/checkout@v7

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"

      - name: Install CUDA toolkit
        uses: Jimver/cuda-toolkit@v0.2.24
        id: cuda-toolkit
        with:
          cuda: "12.4.0"

      - name: Build the CUDA cdylib
        run: cargo build -p vox-plugin-mens-candle-cuda --profile dist --features cuda

      - name: Package the plugin zip
        run: |
          set -euo pipefail
          version="$(cargo pkgid -p vox-plugin-mens-candle-cuda | sed -E 's/.*[#@]//')"
          triple="linux-x86_64"
          staging="$(mktemp -d)"
          cp crates/vox-plugin-mens-candle-cuda/Plugin.toml "$staging/"
          cp target/dist/libvox_plugin_mens_candle_cuda.so "$staging/"
          mkdir -p dist
          (cd "$staging" && zip -j "$OLDPWD/dist/mens-candle-cuda-v${version}-${triple}.zip" ./*)
          echo "Packaged: mens-candle-cuda-v${version}-${triple}.zip"

      - name: Upload plugin artifact
        uses: actions/upload-artifact@v7
        with:
          name: release-gpu-cuda
          path: dist/mens-candle-cuda-v*.zip
          if-no-files-found: error
```

- [ ] **Step 4: Wire both jobs into `publish`'s `needs:` without letting them block it**

Change:

```yaml
  publish:
    name: Publish GitHub release
    runs-on: ubuntu-latest
    timeout-minutes: 20
    needs: [build, dist-verify]
```

to:

```yaml
  publish:
    name: Publish GitHub release
    runs-on: ubuntu-latest
    timeout-minutes: 20
    # gpu-plugin-metal/-cuda are continue-on-error: true, so their presence
    # here only makes publish WAIT for them (avoiding a race where
    # checksums.txt is generated before their zips exist) -- it does not
    # make publish depend on their success. A GitHub Actions job with
    # continue-on-error: true reports as passed for downstream `needs:`
    # resolution even when its own steps failed.
    needs: [build, dist-verify, gpu-plugin-metal, gpu-plugin-cuda]
```

- [ ] **Step 5: Confirm the file is still valid YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/release-binaries.yml', encoding='utf-8')); print('OK')"`

Expected: `OK`

- [ ] **Step 6: Confirm the local gate accepts the new jobs**

Both new jobs run on already-registered hosted runners (`macos-latest`, `ubuntu-latest` — both already have exception rows for `release-binaries.yml` in `docs/src/ci/github-hosted-exceptions.md`), so no exceptions-table edit is needed. Confirm this holds:

Run: `grep -n "release-binaries.yml" docs/src/ci/github-hosted-exceptions.md`

Expected: the existing row already lists `windows-latest`, `macos-latest`, `ubuntu-latest` — no new runner OS is introduced by this task.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/release-binaries.yml
git commit -m "feat(ci): build and publish the GPU plugins as release assets

Adds gpu-plugin-metal (macos-latest, proven toolchain -- Metal ships
with Xcode CLT) and gpu-plugin-cuda (ubuntu-latest, via
Jimver/cuda-toolkit -- no workflow in this repo has installed CUDA on a
hosted runner before, this is the first attempt). Both continue-on-error
at the job level: publish's needs: list includes both (so checksums.txt
generation waits for them, avoiding a race where it runs before their
zips exist) but is never blocked by their failure.

Packaging is flat by construction -- Plugin.toml plus the renamed
cdylib in a staging dir, zipped with `zip -j` -- because
install_from_path only reads top-level files and silently ignores
anything nested.

Both artifacts are named release-gpu-<name>, matching the existing
release-* prefix publish's checksums generation and files: glob already
match on -- neither needed to change."
```

---

### Task 8: Push a verification tag and prove GUI + `.deb` + GPU plugin assets against the real release

**Files:**
- None modified — this task is verification of Tasks 1–7 against real CI, not new code

**Interfaces:**
- Consumes: every prior task's commit, all of which must be pushed to the branch this tag is cut from
- Produces: a pass/fail verdict on the actual goal, independent of workflow "green"

- [ ] **Step 1: Confirm all prior commits are pushed**

Run: `git log --oneline -8` and `git status --short`

Expected: Tasks 1–7's commits present in history, working tree clean.

Run: `git push origin <branch-name>`

- [ ] **Step 2: Get the current build number for a collision-free tag**

Run: `git rev-list --count HEAD`

Note the number — call it `<buildnum>` in the following steps.

- [ ] **Step 3: Cut and push the verification tag**

```bash
git tag v0.6.0-rc.<buildnum>
git push origin v0.6.0-rc.<buildnum>
```

- [ ] **Step 4: Confirm `version-tag-guard.yml` passes on this tag**

Run: `gh run list --workflow version-tag-guard.yml --limit 1 --json databaseId,status,headSha`

Wait for `status: completed`, then: `gh api repos/vox-foundation/vox/actions/runs/<databaseId> -q '.conclusion'`

Expected: `success` — this is Task 1's first real proof; a `failure` here means Task 1's extraction logic has a bug Step 4 of Task 1 didn't catch, and every remaining verification step in this task is meaningless until it's fixed.

- [ ] **Step 5: Wait for `release-gui.yml`, `release-binaries.yml`, and `release-installers.yml` to complete**

Bounded checks, not a poll loop — these jobs take 15 minutes to over an hour. Check status no more than once every 15–20 minutes:

```bash
gh api repos/vox-foundation/vox/actions/runs/<run-id> -q '.status, .conclusion'
```

Do other useful work between checks or end the turn and resume later. Continue only once all three report `completed`.

- [ ] **Step 6: Verify GUI installer assets actually exist**

Run: `gh release view v0.6.0-rc.<buildnum> --json assets -q '.assets[].name'`

Expected: at least one of `*.msi`, `*.dmg`, `*.AppImage`, `*.deb` (Tauri-produced, distinct from the CLI's own `vox-cli-*.msi` and `build-linux-deb`'s `.deb`) present. If none appear, read the specific `Build Tauri GUI` job's log for that platform before concluding anything — Task 2/3's fixes are unverified until this line has real data.

- [ ] **Step 7: Verify the CLI `.deb` (the previously-unverified fix) is present**

Run: `gh release view v0.6.0-rc.<buildnum> --json assets -q '.assets[].name' | grep -i "\.deb$"`

Expected: a `.deb` from `build-linux-deb`, proving the recursive-glob fix from the prior session's `f52f3acef` actually works — this is its first real test.

- [ ] **Step 8: Verify the GPU plugin assets**

Run: `gh release view v0.6.0-rc.<buildnum> --json assets -q '.assets[].name' | grep -i "mens-candle"`

Expected: `mens-candle-metal-v0.6.0-macos-aarch64.zip` (or `-x86_64-`, matching whichever arch `macos-latest` resolves to). `mens-candle-cuda-v0.6.0-linux-x86_64.zip` may or may not be present — Task 7's CUDA job is `continue-on-error`, so its absence is an expected possible outcome, not a plan failure. If Metal's zip is also absent, read that job's log; unlike CUDA, its absence is not expected.

- [ ] **Step 9: If the Metal zip is present, prove it actually installs**

This is the real end-to-end proof for Task 5's dynamic install path — not just that the asset exists, but that `vox plugin install` resolves its version, fetches `checksums.txt`, verifies the hash, and installs without `--allow-unverified`. Requires a machine that can run the plugin's target OS; do this from a macOS host if one is available in this environment, otherwise defer this specific step and note it as unverified in the final report rather than skipping it silently:

```bash
vox plugin install mens-candle-metal
```

Expected: succeeds with no `--allow-unverified` flag, and prints the plugin's own installed version as `0.6.0`.

- [ ] **Step 10: Report the verdict per asset category**

State plainly which of GUI/`.deb`/Metal/CUDA produced a genuinely working, downloadable, verifiable artifact and which did not, citing the specific evidence for each — this is the actual deliverable of this task, not a summary of workflow colors.

---

### Task 9: Prove the `.deb` installs on real Ubuntu (WSL2)

**Files:**
- None modified

**Interfaces:**
- Consumes: Task 8's verified `.deb` asset on the `v0.6.0-rc.<buildnum>` release
- Produces: proof that `dpkg -i` succeeds and the installed binary runs, not just that the file exists

- [ ] **Step 1: Confirm WSL2 Ubuntu is available**

Run: `wsl.exe -d Ubuntu -e bash -lc "lsb_release -ds"`

Expected: `Ubuntu 24.04.1 LTS` (already confirmed present during this plan's research)

- [ ] **Step 2: Download the `.deb` from the release inside WSL2**

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  cd /tmp
  gh release download v0.6.0-rc.<buildnum> --repo vox-foundation/vox --pattern "*.deb" --clobber
  ls -la *.deb
'
```

(If `gh` is not installed in this WSL2 distro, install it first: `wsl.exe -d Ubuntu -e bash -lc "sudo apt-get update && sudo apt-get install -y gh"`, or download via `curl -fL -o vox.deb <the asset's browser_download_url from Task 8 Step 7's output>`.)

- [ ] **Step 3: Install it with dpkg**

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  cd /tmp
  sudo dpkg -i *.deb
'
```

Expected: dpkg reports successful installation, no dependency errors. If dpkg reports missing dependencies, run `sudo apt-get install -f` once and re-verify — record whether this was needed, since it's a real packaging gap if so, not a WSL2-specific quirk.

- [ ] **Step 4: Prove the installed binary actually runs**

```bash
wsl.exe -d Ubuntu -e bash -lc "vox --version"
```

Expected: a semver-formatted version string. This — not the presence of the `.deb`, not `dpkg`'s exit code alone — is the actual proof this task exists to produce.

- [ ] **Step 5: Report the verdict**

State the exact `vox --version` output and confirm it matches `0.6.0` (the underlying `CARGO_PKG_VERSION`, independent of the `-rc.<buildnum>` tag suffix, per `release-binaries.yml`'s existing `--version ${{ github.ref_name }}` stamping — confirm whether the installed binary reports the full tag or the bare version, and note whichever it is, since that's user-visible behavior worth knowing precisely, not assuming).

---

### Task 10: Homebrew formula audit harness in WSL2

**Files:**
- Create: a formula-generation script — since this repo's VoxScript-first policy (AGENTS.md) requires new automation as `.vox`, not `.sh`/`.ps1`/`.py`: `scripts/homebrew/generate-formula.vox`

**Interfaces:**
- Consumes: the darwin tarball URL and its sha256 from the `v0.6.0-rc.<buildnum>` release (Task 8)
- Produces: a `vox.rb` formula file, validated with `brew audit --strict` / `brew style` — proves the formula is well-formed and its checksum matches; explicitly does **not** prove a macOS install (no macOS in this environment)

- [ ] **Step 1: Install Homebrew-on-Linux inside WSL2**

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  if ! command -v brew >/dev/null 2>&1; then
    NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  fi
  echo "eval \"\$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)\"" >> ~/.bashrc
  eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
  brew --version
'
```

Expected: a Homebrew version string. This step can take several minutes on first install.

- [ ] **Step 2: Get the darwin tarball's URL and sha256 from the release**

```bash
gh release view v0.6.0-rc.<buildnum> --json assets -q '.assets[] | select(.name == "vox-darwin.tar.gz") | .url'
gh release view v0.6.0-rc.<buildnum> --json assets -q '.assets[] | select(.name == "checksums.txt")'
```

Download `checksums.txt` and extract the `vox-darwin.tar.gz` line's hash:

```bash
gh release download v0.6.0-rc.<buildnum> --pattern checksums.txt --clobber -O /tmp/checksums.txt
grep "vox-darwin.tar.gz" /tmp/checksums.txt
```

Note the URL and the hash (call it `<darwin_sha256>`) for the next step. If `vox-darwin.tar.gz` is not in `checksums.txt` (it's uploaded by `publish-macos-brew` as a workflow artifact attached separately, not staged into a `release-*`-prefixed directory the way the checksums generator scans — confirm this by checking whether the hash appears), compute it directly instead: `gh release download v0.6.0-rc.<buildnum> --pattern vox-darwin.tar.gz -O /tmp/vox-darwin.tar.gz && sha256sum /tmp/vox-darwin.tar.gz`.

- [ ] **Step 2: Write the formula-generation script**

Create `scripts/homebrew/generate-formula.vox`:

```vox
// vox:skip -- this is a genuine out-of-file excerpt sketch for the plan;
// the actual script is written against this repo's real std.fs/std.process
// API surface at implementation time, per AGENTS.md's VoxScript-First
// Glue Code policy and vox-shell-stdlib-ssot-2026.md.
//
// Generates a Homebrew formula for `vox` from a release tag, a tarball URL,
// and its sha256. Run: vox run scripts/homebrew/generate-formula.vox
// --tag v0.6.0-rc.4735 --url <tarball-url> --sha256 <hash> --out /tmp/vox.rb

fn main() {
    let args = std.process.args();
    let tag = args.get_flag("--tag");
    let url = args.get_flag("--url");
    let sha256 = args.get_flag("--sha256");
    let out = args.get_flag("--out");

    let formula = "class Vox < Formula\n" +
        "  desc \"Vox programming language and toolchain\"\n" +
        "  homepage \"https://voxlang.org\"\n" +
        "  url \"" + url + "\"\n" +
        "  sha256 \"" + sha256 + "\"\n" +
        "  version \"" + tag.strip_prefix("v") + "\"\n" +
        "\n" +
        "  def install\n" +
        "    bin.install \"vox\"\n" +
        "  end\n" +
        "\n" +
        "  test do\n" +
        "    system \"#{bin}/vox\", \"--version\"\n" +
        "  end\n" +
        "end\n";

    std.fs.write(out, formula);
    println("Wrote formula to " + out);
}
```

- [ ] **Step 3: Generate the formula**

```bash
vox run scripts/homebrew/generate-formula.vox --tag v0.6.0-rc.<buildnum> --url <darwin-tarball-url-from-step-1> --sha256 <darwin_sha256> --out /tmp/vox.rb
cat /tmp/vox.rb
```

Expected: a well-formed Ruby formula file with the real URL and hash substituted in.

- [ ] **Step 4: Audit the formula in WSL2**

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
  cp /mnt/c/path/to/vox.rb /tmp/vox.rb
  brew audit --strict --formula /tmp/vox.rb
  brew style /tmp/vox.rb
'
```

(Adjust the `cp` source path to wherever Step 3's output actually landed relative to WSL2's mount of the Windows filesystem.)

Expected: both commands exit 0. Read any reported warnings/errors and fix the template in Step 2 if genuine formula problems are found — this step is the actual test of Step 2's output, not a formality.

- [ ] **Step 5: State the explicit limit of this proof, in the report**

This audit proves the formula is syntactically and stylistically well-formed, that its URL resolves (verify separately with `curl -fsI <url>` returning `200`), and that the declared sha256 matches the real tarball. It does **not** prove `brew install vox` succeeds on a Mac — there is no macOS anywhere in this environment. State this limit explicitly in the final report; do not let a green `brew audit` be read as "Homebrew install works."

- [ ] **Step 6: Commit the generator script**

```bash
git add scripts/homebrew/generate-formula.vox
git commit -m "feat(homebrew): add a formula-generation script for the audit harness

Generates a vox.rb Homebrew formula from a release tag + tarball URL +
sha256, so it can be validated with brew audit --strict / brew style in
WSL2 (Linuxbrew). Proves the formula is well-formed and its checksum
matches -- does NOT prove a macOS install, since there is no macOS in
this environment. The real tap (vox-foundation/homebrew-vox) does not
exist and this script does not publish to one; see the spec's Phase 4
for why that's out of scope."
```

---

## Self-Review Notes

**Spec coverage:**
- Phase 0 (versioning/tag) → Task 1.
- Phase 1 (GUI bundling) → Tasks 2, 3; verified in Task 8.
- Phase 2 (`.deb` proof) → Task 9 (the code fix itself, `f52f3acef`, already landed in the prior session; this plan only adds verification, per the spec).
- Phase 3 (GPU distribution): the "no hardcoded versions or hashes" sub-section → Tasks 4, 5, 6; structural problem A (version/URL mismatch) → dissolved by Task 4 + Task 1's tag alignment, no separate task needed; structural problem B (CUDA) → Task 7's `continue-on-error` design; packaging contract (flat zip) → Task 7 Steps 2–3; checksums → Task 5. All verified in Task 8.
- Phase 4 (Homebrew) → Task 10.
- Verification table's four rows → Tasks 8 (GUI, GPU), 9 (`.deb`), 10 (Homebrew).

**Placeholder scan:** no TBD/TODO; every step has literal file content, exact commands, and expected output. The two "if not present, do X" branches (Task 9 Step 3's `apt-get install -f` fallback, Task 10's `cp` path adjustment) are conditional real instructions, not deferred work.

**Type/name consistency:** `parse_checksums`, `first_party_plugin_urls`, `fetch_first_party_checksum`, `install_first_party_plugin`, and `FIRST_PARTY_PLUGIN_REPO` are defined once in Task 5 and referenced by name (not redefined) in Task 6's comment and Task 8's verification. Artifact names (`release-gpu-metal`, `release-gpu-cuda`) match between Task 7's `upload-artifact` step and its own `needs:` wiring — no other task references these names directly, so there's no drift to check there.

**Known gap, deliberately not a task:** CUDA on Windows is listed in `mens-candle-cuda`'s own `Plugin.toml` (`windows-x86_64` artifact target) but this plan only builds the Linux CUDA leg. Not fixed here — the spec's Risk section already flags CUDA as the least certain item, and adding a second unproven platform in the same pass compounds that risk rather than managing it. A Windows CUDA leg is a natural, separately-scoped follow-up once the Linux leg's first real run reports back.
