# Distribution Completeness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four remaining distribution gaps — the Tauri desktop GUI never bundles anything, the two GPU plugins are undeliverable, the `.deb` release fix is unverified, and there is no Homebrew formula audit — while eliminating every hand-pinned version/hash so a future version bump touches nothing.

**Architecture:** Config and CI fixes for the GUI bundler and its signing step; a dynamic-but-fail-closed install path in `vox-cli` that lets first-party GPU plugins derive version and checksum from the release they ship in; new CI jobs that build, package, and upload those plugins as release assets; a widened `version-tag-guard` that admits prerelease tags; and verification tasks that prove each fix against the real GitHub Release API and a real `dpkg` install.

**Tech Stack:** Rust (`vox-cli`, `vox-plugin-*`), GitHub Actions YAML, Tauri v2, WSL2/Ubuntu 24.04, Linuxbrew.

**Spec:** `docs/superpowers/specs/2026-08-24-distribution-completeness-design.md`

> **This plan was revised after a 14-agent adversarial critique (7 tracks + 7 independent verification passes) against the real codebase.** Seven first-try blockers were found and corrected, and several claims in the first draft were confirmed FALSE POSITIVES and removed. Corrections are marked **[CRITIQUE]** inline so the reasoning travels with the fix.

## Global Constraints

- **`gh` is NOT on the default PATH in this environment.** It exists at `C:\Program Files\GitHub CLI\gh.exe`. Every task using `gh` must first run: `export PATH="/c/Program Files/GitHub CLI:$PATH"`. **[CRITIQUE]** — the first draft's bare `gh` commands would have failed immediately.
- No hardcoded version or hash anywhere in the chain — every value is inherited (`version.workspace = true`), derived at build/install time, or guarded by a test that fails on drift.
- Task 5 is on a fail-closed security path: an archive must still be refused unless its computed SHA-256 matches one obtained from the release. Third-party `github:OWNER/REPO` sources keep the pinned-hash model, untouched.
- CUDA build attempts must never block Metal, the core release, or `publish`.
- Verification runs on `v0.6.0-rc.<VOX_BUILD_NUMBER>` tags (prerelease, never `latest`).
- **`v0.6.0` is ALREADY TAGGED on origin** (points at `b81ef6991ec8`, 2026-05-26, months before this work; no GitHub Release was ever created from it). **[CRITIQUE]** — the first draft's endgame "cut the real `v0.6.0` after verification" was **unreachable**. Cutting a real release now requires a version-bump decision (`0.6.1` vs `0.7.0`) plus a `CHANGELOG.md` entry per AGENTS.md. **That decision is explicitly the user's and is NOT part of this plan.** This plan's deliverable is a *proven pipeline*, verified on rc tags.

---

### Task 1: Widen `version-tag-guard.yml` to admit prereleases

**Files:**
- Modify: `.github/workflows/version-tag-guard.yml` (the `tag_version=` extraction and the echo line)

**Interfaces:**
- Consumes: nothing (first task)
- Produces: any `v<core>-<suffix>` tag now passes when `<core>` matches `Cargo.toml`'s workspace version — every later verification task's tag push depends on this

> **[CRITIQUE] verified-correct:** the critique confirmed this task's shell extraction is right and that it "will work exactly as written." Only the plan's own `sed` line range was wrong and is corrected below.

- [ ] **Step 1: Read the current comparison logic**

Run: `grep -n "tag_version=\|tag_core=\|!= \"\${cargo_version}\"\|git tag        :" .github/workflows/version-tag-guard.yml`

Expected: shows `tag_version="${tag#v}"`, the echo line, and the `!=` comparison. **[CRITIQUE]** — the first draft said `sed -n '44,71p'`, a range that does not frame these lines; grep by content instead of by line number.

- [ ] **Step 2: Change the tag-version extraction to strip prerelease/build metadata**

Change:

```bash
          tag="${GITHUB_REF_NAME}"          # e.g. v0.7.0
          tag_version="${tag#v}"            # strip leading v -> 0.7.0
```

to:

```bash
          tag="${GITHUB_REF_NAME}"          # e.g. v0.7.0, v0.6.0-rc.4735
          tag_core="${tag#v}"               # strip leading v -> 0.7.0, 0.6.0-rc.4735
          # Strip semver prerelease/build metadata so v0.6.0-rc.4735 and
          # v0.6.0-nightly.4735 compare equal to Cargo.toml's plain "0.6.0".
          # This only WIDENS what passes; a real mismatch (v0.7.0 tagged
          # against a 0.6.0 Cargo.toml) still fails, suffix or not.
          # Strip '-' first then '+': in semver, build metadata follows the
          # prerelease, so cutting at the first '-' already removes both.
          tag_version="${tag_core%%-*}"
          tag_version="${tag_version%%+*}"   # bare 0.6.0+build.9 (no prerelease)
```

- [ ] **Step 3: Update the echo line**

Change `echo "git tag        : ${tag} (version ${tag_version})"` to `echo "git tag        : ${tag} (core version ${tag_version})"`.

- [ ] **Step 4: Verify the extraction against every tag shape**

```bash
for tag in v0.6.0 v0.6.0-rc.4735 v0.6.0-nightly.4735 v0.7.0 v0.6.0+build.9; do
  tag_core="${tag#v}"; tag_version="${tag_core%%-*}"; tag_version="${tag_version%%+*}"
  echo "$tag -> $tag_version"
done
```

Expected:
```
v0.6.0 -> 0.6.0
v0.6.0-rc.4735 -> 0.6.0
v0.6.0-nightly.4735 -> 0.6.0
v0.7.0 -> 0.7.0
v0.6.0+build.9 -> 0.6.0
```

- [ ] **Step 5: Validate YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/version-tag-guard.yml', encoding='utf-8')); print('OK')"`
Expected: `OK`

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/version-tag-guard.yml
git commit -m "fix(ci): let version-tag-guard admit prerelease tags

Only v<workspace-version> could ever pass -- v0.6.0-rc.1 failed exactly
as hard as a real drift. This made every verification tag push produce a
permanently-red guard, and made nightly tags structurally impossible.

Strip -prerelease and +build metadata before comparing. A real mismatch
(v0.7.0 against a 0.6.0 Cargo.toml) still fails."
```

---

### Task 2: Turn on the Tauri bundler

**Files:**
- Modify: `crates/vox-gui/tauri.conf.json` (add two keys to the `bundle` object)

**Interfaces:**
- Consumes: nothing
- Produces: a `bundle.active: true` config that `tauri-action` will actually produce installers from

> **[CRITIQUE] BLOCKER CORRECTED — the first draft removed `"version"` from this file. That fails on the first try.** `crates/vox-cli-ci/src/gui_version_sync.rs` treats the key as **required** (`.ok_or_else(|| anyhow!("{} missing string `version` field"))?`), and that check runs in *three* blocking places: the `lefthook.yml` pre-commit hook (whose glob matches this exact file, so the commit itself is rejected), `run_ssot_drift` (fast pre-push tier **and** CI), and `.github/workflows/ci.yml`. The `.ok_or_else(...)?` fires before the `--write` branch, so even the auto-regen bot cannot self-heal it.
>
> The rationale in the first draft was also simply wrong: `"version": "0.6.0"` here is **not** a hand-maintained duplicate — `gui_version_sync` auto-rewrites it from `Cargo.toml [workspace.package] version`. It is already exactly the "SSOT-plus-drift idiom" the spec asks for. **Leave it in place.**

- [ ] **Step 1: Read the current bundle block**

Run: `python -c "import json; d=json.load(open('crates/vox-gui/tauri.conf.json',encoding='utf-8')); print(json.dumps(d['bundle'],indent=2)); print('version key present:', 'version' in d)"`

Expected: `bundle` contains only `icon` and `externalBin`; `version key present: True`.

- [ ] **Step 2: Add `active` and `targets` — changing nothing else**

Edit `crates/vox-gui/tauri.conf.json` so the `bundle` object becomes exactly:

```json
  "bundle": {
    "active": true,
    "targets": ["msi", "dmg", "app", "appimage", "deb"],
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
```

Do **not** touch `"version"`, `"productName"`, `"identifier"`, `"build"`, or `"app"`.

Target strings verified against the vendored `tauri-utils-2.9.3/src/config.rs:191-206` `BundleType` deserializer — valid values are `deb`, `rpm`, `appimage`, `msi`, `nsis`, `app`, `dmg`. `"app"` is included because macOS `dmg` wraps a `.app`; listing it explicitly costs nothing and removes a class of "failed to find .app bundle" first-run failure. `bundle.active` defaulting to `false` was confirmed at `config.rs:1563-1565`.

- [ ] **Step 3: Validate JSON and confirm `version` survived**

Run: `python -c "import json; d=json.load(open('crates/vox-gui/tauri.conf.json',encoding='utf-8')); assert d['bundle']['active'] is True; assert 'version' in d, 'version key MUST remain -- gui_version_sync requires it'; print('OK', d['bundle']['targets'])"`

Expected: `OK ['msi', 'dmg', 'app', 'appimage', 'deb']`

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/tauri.conf.json
git commit -m "fix(gui): turn on the Tauri bundler

bundle.active defaults to false in Tauri v2 (tauri-utils config.rs:1563).
Every recorded release-gui.yml run (6/6) built the bare executable,
bundled nothing, and tauri-action correctly reported 'No artifacts were
found' -- this alone explains every historical failure on every platform.

Target strings verified against tauri-utils' BundleType deserializer.
'app' included so the macOS .app is produced explicitly alongside dmg.

Deliberately does NOT touch the version key: gui_version_sync.rs
requires it and auto-syncs it from Cargo.toml, so it is already a
derived SSOT value, not a hand-maintained duplicate."
```

---

### Task 3: Fix the Windows GUI signing step

**Files:**
- Modify: `.github/workflows/release-gui.yml` (job-level `env:`, the signing step's `if:`, `files-folder:`, and `signing-account-name:`)

**Interfaces:**
- Consumes: Task 2's `bundle.active: true` (nothing to sign until then)
- Produces: a signing step that points at the real bundle path and that skips cleanly — but *visibly* — when the cert is absent

> **[CRITIQUE] BLOCKER CORRECTED — the first draft put `AZURE_CODE_SIGNING_ACCOUNT` in the step's own `env:` and referenced it from that same step's `if:`.** A step's `if:` is evaluated before its own `env:` block is materialized, so the expression reads empty and the step is **always skipped — including on a repo that has the cert**, silently shipping unsigned MSIs while the workflow reports green. That is strictly worse than the status quo. The fix is to hoist the variable to **job-level `env:`**, which a step `if:` can read.
>
> **[CRITIQUE] verified-correct:** `./target/release/bundle/msi` is the right path (`.cargo/config.toml` pins `CARGO_TARGET_DIR` to the repo root; `projectPath: ./crates/vox-gui` does not introduce a `src-tauri/` level).

- [ ] **Step 1: Read the signing step and the job's current env**

Run: `grep -n "build-tauri:\|^    env:\|Sign Windows Installer" -A 6 .github/workflows/release-gui.yml | head -40`

- [ ] **Step 2: Add a job-level `env:` to `build-tauri`**

In the `build-tauri` job, immediately after the `timeout-minutes: 240` line and before `permissions:`, add:

```yaml
    env:
      # Job-level, NOT step-level: a step's `if:` is evaluated before that
      # step's own env: block exists, so a step-scoped var reads empty in
      # its own condition and the step is silently skipped forever. The
      # `secrets` context is not available in `if:` at all, which is why
      # this env hop exists -- it just has to happen one scope up.
      AZURE_CODE_SIGNING_ACCOUNT: ${{ secrets.AZURE_CODE_SIGNING_ACCOUNT }}
```

- [ ] **Step 3: Gate the signing step and fix its path**

Change the `Sign Windows Installer` step's `if:` to:

```yaml
        if: runner.os == 'Windows' && env.AZURE_CODE_SIGNING_ACCOUNT != ''
```

Change `signing-account-name: ${{ secrets.AZURE_CODE_SIGNING_ACCOUNT }}` to `signing-account-name: ${{ env.AZURE_CODE_SIGNING_ACCOUNT }}`.

Change:
```yaml
          files-folder: ./crates/vox-gui/src-tauri/target/release/bundle/msi
```
to:
```yaml
          # target/ is the workspace-root CARGO_TARGET_DIR (.cargo/config.toml);
          # there is no src-tauri/ directory in this repo.
          files-folder: ./target/release/bundle/msi
```

- [ ] **Step 4: Add a visible skip notice so an unsigned release is never silent**

Immediately **before** the `Sign Windows Installer` step, add:

```yaml
      - name: Note unsigned MSI (no signing cert configured)
        if: runner.os == 'Windows' && env.AZURE_CODE_SIGNING_ACCOUNT == ''
        run: echo "::warning::AZURE_CODE_SIGNING_ACCOUNT is not set -- the MSI in this release is UNSIGNED."
```

A skipped signing step and a correctly-gated one look identical in the log; this makes the difference legible without failing the release.

- [ ] **Step 5: Validate YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/release-gui.yml', encoding='utf-8')); print('OK')"`
Expected: `OK`

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/release-gui.yml
git commit -m "fix(ci): correct the Windows GUI signing path and gate it correctly

files-folder pointed at crates/vox-gui/src-tauri/target/..., a directory
that does not exist in this repo, while CARGO_TARGET_DIR is pinned to the
workspace root. Latent until now (the build died before signing); would
have failed immediately once the bundler started producing an MSI.

Gates signing on the cert existing, via JOB-level env -- a step's if: is
evaluated before its own env: block, so the step-scoped form would have
skipped signing forever, including where the cert IS configured, while
reporting green. Adds an explicit ::warning:: on the unsigned path so it
is never silent."
```

---

### Task 4: Sync both `Plugin.toml` versions to the workspace, with a drift gate

**Files:**
- Modify: `crates/vox-plugin-mens-candle-cuda/Plugin.toml`, `crates/vox-plugin-mens-candle-cuda/Cargo.toml`
- Create: `crates/vox-plugin-mens-candle-cuda/tests/plugin_toml_version_matches_crate.rs`
- Modify: `crates/vox-plugin-mens-candle-metal/Plugin.toml`, `crates/vox-plugin-mens-candle-metal/Cargo.toml`
- Create: `crates/vox-plugin-mens-candle-metal/tests/plugin_toml_version_matches_crate.rs`
- Modify: `contracts/reports/test-inventory.v1.json` (regenerated — **[CRITIQUE]**, see Step 8)

**Interfaces:**
- Consumes: nothing
- Produces: `Plugin.toml` `version` = `0.6.0`, matching the release the assets ship in

> **[CRITIQUE] BLOCKER ADDED — creating a `tests/` directory in these crates breaks a blocking CI gate the first draft never mentioned.** Both crates are currently inline-test-only and appear in `contracts/reports/test-inventory.v1.json`'s `inline_only_crates`. Adding `tests/*.rs` flips `has_integration_dir`, `integration_rs_files`, `integration_tests`, and two `summary` totals. The gate is byte-exact and blocking (`.github/workflows/ci.yml`: `vox ci test-inventory --check contracts/reports/test-inventory.v1.json`), and — critically — `test_inventory` appears in **neither** `run_ssot_drift` nor `pre_push.rs`, so this passes every local tier and then fails CI, and the ssot-autoregen bot will not fix it. Step 8 regenerates it.

- [ ] **Step 1: Write the failing test (CUDA)**

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

(`toml = "0.8"` is already a workspace dependency at root `Cargo.toml:210` — no new external crate.)

- [ ] **Step 3: Run the test to verify it fails (CUDA)**

Run: `cargo test -p vox-plugin-mens-candle-cuda --test plugin_toml_version_matches_crate -- --nocapture`
Expected: FAIL — `Plugin.toml's version (0.1.0) does not match this crate's Cargo.toml version (0.6.0)`

- [ ] **Step 4: Fix `Plugin.toml` (CUDA)**

In `crates/vox-plugin-mens-candle-cuda/Plugin.toml`, change `version = "0.1.0"` to `version = "0.6.0"`.

- [ ] **Step 5: Run the test to verify it passes (CUDA)**

Run: `cargo test -p vox-plugin-mens-candle-cuda --test plugin_toml_version_matches_crate -- --nocapture`
Expected: PASS

- [ ] **Step 6: Repeat Steps 1–5 for Metal**

Create `crates/vox-plugin-mens-candle-metal/tests/plugin_toml_version_matches_crate.rs` with byte-identical content to Step 1 (`include_str!("../Plugin.toml")` and `env!("CARGO_PKG_VERSION")` resolve per-crate).

Add `toml = { workspace = true }` to `crates/vox-plugin-mens-candle-metal/Cargo.toml`'s `[dev-dependencies]`.

Run: `cargo test -p vox-plugin-mens-candle-metal --test plugin_toml_version_matches_crate -- --nocapture`
Expected: FAIL — version mismatch

Change `crates/vox-plugin-mens-candle-metal/Plugin.toml`'s `version = "0.1.0"` to `version = "0.6.0"`, then re-run.
Expected: PASS

- [ ] **Step 7: Build the `vox` binary needed for the inventory regen**

The `test-inventory` regen needs a built `vox`. If `target/debug/vox.exe` does not exist, build it now (background it — 15–40 min cold):

Run: `ls target/debug/vox.exe 2>/dev/null || cargo build -p vox-cli --bin vox`

- [ ] **Step 8: Regenerate the test inventory**

Run: `target/debug/vox.exe ci test-inventory --output contracts/reports/test-inventory.v1.json`

Then confirm the two crates moved out of `inline_only_crates`:

Run: `python -c "import json; d=json.load(open('contracts/reports/test-inventory.v1.json',encoding='utf-8')); s=json.dumps(d); print('cuda in inline_only:', 'vox-plugin-mens-candle-cuda' in json.dumps(d.get('inline_only_crates',[]))); print('metal in inline_only:', 'vox-plugin-mens-candle-metal' in json.dumps(d.get('inline_only_crates',[])))"`

Expected: both `False`. If the file did not change at all, the regen did not run — do not proceed, or CI will fail on a byte-exact mismatch.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-plugin-mens-candle-cuda/Plugin.toml crates/vox-plugin-mens-candle-cuda/Cargo.toml crates/vox-plugin-mens-candle-cuda/tests/plugin_toml_version_matches_crate.rs crates/vox-plugin-mens-candle-metal/Plugin.toml crates/vox-plugin-mens-candle-metal/Cargo.toml crates/vox-plugin-mens-candle-metal/tests/plugin_toml_version_matches_crate.rs contracts/reports/test-inventory.v1.json
git commit -m "fix(plugins): sync Plugin.toml version to the workspace, add a drift gate

Both plugin crates already inherit version.workspace = true; only the
hand-written Plugin.toml files hardcoded 0.1.0, a stale duplicate of a
number Cargo already knows. Bumped to 0.6.0 and added a per-crate test
that fails on any future drift.

Also regenerates contracts/reports/test-inventory.v1.json: adding a
tests/ dir moves both crates out of inline_only_crates, and that gate is
byte-exact and blocking in CI while being covered by neither ssot-drift
nor pre-push -- so it would pass locally and fail on the PR."
```

---

### Task 5: Dynamic, checksums.txt-verified install path for first-party plugins

**Files:**
- Modify: `crates/vox-cli/src/commands/plugin/install.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks. The asset name it builds (`{id}-v{version}-{triple}.zip`) must equal Task 7's zip filename; the `checksums.txt` format it parses is `release-binaries.yml`'s `sha256sum`-style `<hash>  <basename>` (two spaces — verified).
- Produces: `FIRST_PARTY_PLUGIN_REPO`, `PLUGIN_RELEASE_TAG_ENV`, `parse_checksums`, `first_party_plugin_urls`, `fetch_first_party_checksum`, `install_first_party_plugin`. Task 6 depends on `FIRST_PARTY_PLUGIN_REPO`'s exact value.

> **[CRITIQUE] BLOCKER CORRECTED — the first draft's design made its own security-critical path unverifiable.** `install_first_party_plugin` derived the release tag from `env!("CARGO_PKG_VERSION")` (= `0.6.0`), but verification happens on a `v0.6.0-rc.<buildnum>` tag. The install would fetch `.../download/v0.6.0/checksums.txt` — a release that does not exist — so Task 8's "prove it installs" step was **unreachable by construction**, and the plan never noticed. Fixed with a testing-only `VOX_PLUGIN_RELEASE_TAG` override (below), so the rc release can actually be exercised.
>
> **[CRITIQUE] also corrected:** with `--allow-unverified` the first draft skipped the checksum fetch entirely, discarding a mismatch it could have *reported*. Now it always attempts the fetch and downgrades a failure to a warning, so the flag means "proceed despite", not "don't look".
>
> **[CRITIQUE] verified-correct (do not re-litigate):** `sha2`/`hex` are already deps; `reqwest`'s `.text()` is available without the `charset` feature; `mod tests` exists at `install.rs:407` and the filter `plugin::install::tests` works; the tests need no `HashMap` import; the two-space `checksums.txt` split matches exactly.

- [ ] **Step 1: Write the failing tests**

In `crates/vox-cli/src/commands/plugin/install.rs`, inside the existing `#[cfg(test)] mod tests` block, add:

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

    #[test]
    fn first_party_plugin_urls_are_built_from_the_running_binary_version() {
        let (asset_url, checksums_url, asset_name) =
            first_party_plugin_urls("mens-candle-metal", "0.6.0", "macos-aarch64");
        assert_eq!(asset_name, "mens-candle-metal-v0.6.0-macos-aarch64.zip");
        assert_eq!(
            asset_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0/mens-candle-metal-v0.6.0-macos-aarch64.zip"
        );
        assert_eq!(
            checksums_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0/checksums.txt"
        );
    }

    /// The release tag and the plugin's own version are separate inputs: a
    /// prerelease (v0.6.0-rc.4735) ships a plugin whose version is still
    /// 0.6.0, so the asset NAME and the release TAG must be allowed to
    /// differ. Without this the security-critical install path cannot be
    /// exercised on any rc tag.
    #[test]
    fn a_release_tag_override_changes_only_the_tag_not_the_asset_name() {
        let (asset_url, checksums_url, asset_name) = first_party_plugin_urls_tagged(
            "mens-candle-metal",
            "0.6.0",
            "macos-aarch64",
            "v0.6.0-rc.4735",
        );
        assert_eq!(asset_name, "mens-candle-metal-v0.6.0-macos-aarch64.zip");
        assert!(asset_url.contains("/download/v0.6.0-rc.4735/"), "got {asset_url}");
        assert!(asset_url.ends_with("mens-candle-metal-v0.6.0-macos-aarch64.zip"), "got {asset_url}");
        assert_eq!(
            checksums_url,
            "https://github.com/vox-foundation/vox/releases/download/v0.6.0-rc.4735/checksums.txt"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-cli --lib plugin::install::tests -- --nocapture`
Expected: FAIL — `cannot find function 'parse_checksums'` / `'first_party_plugin_urls'` / `'first_party_plugin_urls_tagged'`

- [ ] **Step 3: Implement the helpers**

Add to `crates/vox-cli/src/commands/plugin/install.rs`, module-level, near `verify_plugin_archive`:

```rust
/// The repo whose own GitHub Release first-party plugins ship as assets of.
/// `install_from_catalog` compares a `github:` source against this to decide
/// whether the dynamic, checksums.txt-verified path below applies instead of
/// requiring a hand-pinned version+sha256.
const FIRST_PARTY_PLUGIN_REPO: &str = "vox-foundation/vox";

/// Override for the release TAG a first-party plugin is fetched from.
///
/// The plugin's version (and therefore its asset filename) always comes from
/// this binary's own `CARGO_PKG_VERSION`. The release it lives in is normally
/// `v<that version>` -- but a prerelease verification run publishes to e.g.
/// `v0.6.0-rc.4735` while the binary still reports `0.6.0`. Without this
/// override the install path cannot be exercised before the final release
/// exists, which would leave a fail-closed security path shipping unproven.
/// Verification-only; never needed for a normal install.
pub(crate) const PLUGIN_RELEASE_TAG_ENV: &str = "VOX_PLUGIN_RELEASE_TAG";

/// Parse a `checksums.txt` body (`sha256sum` output: `<hash>  <filename>`)
/// into filename -> lowercase hex sha256.
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

/// Asset URL, checksums URL, and asset filename for a first-party plugin
/// published under release tag `tag`. Pure and deterministic.
fn first_party_plugin_urls_tagged(
    id: &str,
    version: &str,
    triple: &str,
    tag: &str,
) -> (String, String, String) {
    let asset_name = format!("{id}-v{version}-{triple}.zip");
    let base = format!("https://github.com/{FIRST_PARTY_PLUGIN_REPO}/releases/download/{tag}");
    (
        format!("{base}/{asset_name}"),
        format!("{base}/checksums.txt"),
        asset_name,
    )
}

/// Same, for the normal case where the release tag is `v<version>`.
fn first_party_plugin_urls(id: &str, version: &str, triple: &str) -> (String, String, String) {
    first_party_plugin_urls_tagged(id, version, triple, &format!("v{version}"))
}

/// Fetch `checksums.txt` and return the hash recorded for `asset_name`.
///
/// Fail-closed: a network error, malformed body, or missing entry is always
/// an `Err`. Whether that is fatal is the caller's decision, made solely
/// from the explicit `allow_unverified` flag -- never inferred here.
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
        format!(
            "no checksums.txt entry found for {asset_name} at {checksums_url}. \
             If this plugin's build was skipped for this release (e.g. the CUDA \
             job is allowed to fail), no asset was published for your platform."
        )
    })
}

/// Install a first-party plugin shipped as a release asset of vox's own repo.
///
/// Neither version nor sha256 is pinned in catalog.toml: the plugin ships in
/// the SAME release as this running binary, so its version is this binary's
/// own `CARGO_PKG_VERSION` and its expected hash comes from that release's
/// `checksums.txt` -- the mechanism `voxup` already uses to verify the `vox`
/// binary itself. This stays inside the trust boundary the running binary is
/// already in; it does not widen it to arbitrary `github:` sources.
async fn install_first_party_plugin(
    id: &str,
    triple: &str,
    yes: bool,
    allow_unverified: bool,
) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let (asset_url, checksums_url, asset_name) = match std::env::var(PLUGIN_RELEASE_TAG_ENV) {
        Ok(tag) if !tag.trim().is_empty() => {
            eprintln!(
                "ℹ {PLUGIN_RELEASE_TAG_ENV}={tag} -- fetching {id} from that release \
                 instead of v{version}."
            );
            first_party_plugin_urls_tagged(id, version, triple, tag.trim())
        }
        _ => first_party_plugin_urls(id, version, triple),
    };

    // Always ATTEMPT the fetch, even with --allow-unverified: the flag means
    // "install despite a failed check", not "skip looking". A user who opts
    // out still gets told what the recorded hash was, or why there wasn't one.
    let expected_sha256 = match fetch_first_party_checksum(&checksums_url, &asset_name).await {
        Ok(hash) => Some(hash),
        Err(e) if allow_unverified => {
            eprintln!("⚠ could not obtain a recorded checksum ({e:#}); continuing because --allow-unverified was passed.");
            None
        }
        Err(e) => return Err(e),
    };

    install_from_url(&asset_url, yes, expected_sha256.as_deref(), allow_unverified).await
}
```

- [ ] **Step 4: Rewire `install_from_catalog`'s `github:` branch**

Change:

```rust
    } else if let Some(gh) = source.strip_prefix("github:") {
        // Pinned, not `latest`: the bytes behind a floating asset change, so no
        // recorded hash could ever match it.
        let triple = vox_plugin_host::current_target_triple_key();
        let version = entry.version.as_deref().with_context(|| {
```

to:

```rust
    } else if let Some(gh) = source.strip_prefix("github:") {
        let triple = vox_plugin_host::current_target_triple_key();

        // First-party plugins shipped inside vox's own release derive both
        // version and checksum dynamically -- see install_first_party_plugin.
        if gh == FIRST_PARTY_PLUGIN_REPO {
            return install_first_party_plugin(id, triple, yes, allow_unverified).await;
        }

        // Third-party sources keep the pinned-hash model, unchanged: no
        // dynamic lookup for a repo this binary shares no release with.
        let version = entry.version.as_deref().with_context(|| {
```

(The rest of that branch — the `format!` URL and the `install_from_url` call — stays exactly as it is.)

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p vox-cli --lib plugin::install::tests -- --nocapture`
Expected: PASS — all tests in the module

- [ ] **Step 6: Compile-check**

Run: `cargo check -p vox-cli`
Expected: no errors

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/plugin/install.rs
git commit -m "feat(cli): derive first-party plugin version+checksum, don't pin them

Pinning a GPU plugin's release sha256 in catalog.toml is unknowable until
after the release is built, forcing a two-release bootstrap with install
correctly refusing in between.

For plugins shipped as assets of vox's own release, read the expected hash
from that release's checksums.txt instead -- the mechanism voxup already
uses for the vox binary, inside the same trust boundary. The invariant is
unchanged: an archive is refused unless its computed hash matches one
obtained from the release. Only WHERE the hash comes from changes, and
only for FIRST_PARTY_PLUGIN_REPO; third-party github: sources keep the
pinned model untouched.

Adds VOX_PLUGIN_RELEASE_TAG so a prerelease (v0.6.0-rc.N) release can be
exercised while the binary still reports 0.6.0 -- without it this
fail-closed path could not be verified until the final release existed.

--allow-unverified now still ATTEMPTS the checksum fetch and downgrades
failure to a warning, so it means 'proceed despite' rather than 'skip
looking'."
```

---

### Task 6: Repoint the GPU catalog entries, fix the test they break, regenerate the docs

**Files:**
- Modify: `crates/vox-plugin-catalog/catalog.toml`
- Modify: `crates/vox-cli/src/commands/plugin/install.rs` (**[CRITIQUE]** — unlisted in the first draft)
- Modify: `docs/src/reference/plugin-catalog.generated.md` (**[CRITIQUE]** — unlisted in the first draft)

**Interfaces:**
- Consumes: Task 5's `FIRST_PARTY_PLUGIN_REPO` = `"vox-foundation/vox"` — this task's `default-source` must match that string exactly or the first-party branch is never taken
- Produces: catalog entries Task 8 verifies against

> **[CRITIQUE] BLOCKER 1 — this task breaks an existing test in a file the first draft never listed.** `install.rs`'s `catalog_local_source_is_refused_without_the_opt_in` hardcodes `install_from_catalog("mens-candle-metal", ...)` and asserts the error names `VOX_LOCAL_PLUGIN_FALLBACK`. Repointing metal to `github:` routes it into Task 5's new branch, makes a **live network call from a unit test**, and fails the assertion. Step 2 repoints the test at `nvml-probe`, a real `local:` entry (`catalog.toml`).
>
> **[CRITIQUE] BLOCKER 2 — `plugin-catalog.generated.md` embeds `default-source` verbatim** and is in AGENTS.md's ssot-drift-verified set. Not regenerating it fails `ssot-drift` at push time. Step 4 regenerates it.
>
> **[CRITIQUE] verified NON-issues — do not "fix" these:** `distribution-bundles.generated.md` is unaffected (it renders only id/description); no test asserts the current sources; no catalog validation requires `version`+`sha256` for `github:` sources; `contracts/distribution/profiles.v1.yaml` has no plugin coupling at all and `distribution-parity.yml` isn't even triggered by a `catalog.toml` edit; `min-vox-version` is parsed but never enforced.

- [ ] **Step 1: Repoint both catalog entries**

In `crates/vox-plugin-catalog/catalog.toml`, change `mens-candle-cuda`'s `default-source` line to:

```toml
# Ships as a release asset of vox's own repo, not a separate plugin repo --
# see install_from_catalog's FIRST_PARTY_PLUGIN_REPO check. No version or
# sha256 pinned here: both derive at install time from the running binary's
# version and that release's checksums.txt.
default-source = "github:vox-foundation/vox"
```

And `mens-candle-metal`'s `default-source` line to:

```toml
# Same first-party release-asset model as mens-candle-cuda above.
default-source = "github:vox-foundation/vox"
```

- [ ] **Step 2: Repoint the test that this breaks**

In `crates/vox-cli/src/commands/plugin/install.rs`, in `catalog_local_source_is_refused_without_the_opt_in`, change:

```rust
        let err = install_from_catalog("mens-candle-metal", true, false)
```

to:

```rust
        // nvml-probe, not mens-candle-metal: the GPU plugins now ship as
        // first-party release assets (github:vox-foundation/vox), so they no
        // longer exercise the local: refusal this test exists to guard --
        // and would make a live network call from a unit test. nvml-probe is
        // still a local: entry in catalog.toml.
        let err = install_from_catalog("nvml-probe", true, false)
```

- [ ] **Step 3: Verify the catalog parses and the test still passes**

Run: `cargo build -p vox-plugin-catalog`
Expected: builds clean.

Run: `cargo test -p vox-cli --lib plugin::install -- --nocapture`
Expected: PASS — including `catalog_local_source_is_refused_without_the_opt_in`. If it fails with a network/HTTP error, Step 2 was not applied.

- [ ] **Step 4: Regenerate the catalog docs**

Run: `target/debug/vox.exe ci generate-plugin-catalog-docs`

Then confirm the generated doc picked up the change:

Run: `grep -n "mens-candle" docs/src/reference/plugin-catalog.generated.md`
Expected: both rows now show `github:vox-foundation/vox`. If they still show the old sources, the regen did not run — `ssot-drift` will fail at push.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-plugin-catalog/catalog.toml crates/vox-cli/src/commands/plugin/install.rs docs/src/reference/plugin-catalog.generated.md
git commit -m "fix(catalog): ship GPU plugins as vox's own release assets

mens-candle-cuda's source repo (vox-foundation/vox-plugin-mens-candle-cuda)
returns HTTP 404 -- the install could never resolve. mens-candle-metal's
local: source is correctly refused for real users by the security-floor
gate. Repointing both at github:vox-foundation/vox routes them through the
first-party path, which needs no version/sha256 pinned here.

Also repoints catalog_local_source_is_refused_without_the_opt_in at
nvml-probe: it hardcoded mens-candle-metal, so this change would have sent
a unit test through the network path and broken its assertion.

Also regenerates plugin-catalog.generated.md, which embeds default-source
verbatim and is ssot-drift-gated."
```

---

### Task 7: Build and publish the GPU plugin release assets

**Files:**
- Modify: `.github/workflows/release-binaries.yml`

**Interfaces:**
- Consumes: Task 4's `Plugin.toml` version; Task 6's catalog source; the `{id}-v{version}-{triple}.zip` name Task 5 constructs
- Produces: `release-gpu-metal-*` / `release-gpu-cuda` artifacts, picked up unchanged by the existing `release-*` checksum and `files:` globs

> **[CRITIQUE] verified-correct:** job-level `continue-on-error: true` **does** satisfy a downstream `needs:` (so `publish` waits without being blocked); the `release-*` 2-level globs do match these artifacts; `cargo pkgid | sed` does yield `0.6.0`; `--profile dist` outputs to `target/dist/`; the cdylib filenames match `plugin_artifact_filename`; the `.deb` filename-collision worry was a **false alarm**.
>
> **[CRITIQUE] corrections applied below:** missing `CARGO_BUILD_JOBS` (24 concurrent rustc on a 4-core runner), a 60-minute timeout against neighbours budgeting 240 for the same fat-LTO work, a missing `CUDA_COMPUTE_CAP` (candle's CUDA build derives it from that env or `nvidia-smi`; a hosted runner has neither), and Metal building only one arch while `Plugin.toml` advertises two — a guaranteed 404 for Intel Macs.

- [ ] **Step 1: Confirm the artifact-naming convention**

Run: `grep -n "name: release-\|for f in release-\|files: release-artifacts" .github/workflows/release-binaries.yml`

Expected: `name: release-${{ matrix.target }}`, `for f in release-*/*`, `files: release-artifacts/release-*/*` — confirming any `release-`-prefixed artifact is picked up by both the checksum generation and the upload with no change to either.

- [ ] **Step 2: Add the Metal plugin job (both macOS arches)**

Add to `.github/workflows/release-binaries.yml`, after the `build` job and before `dist-verify`:

```yaml
  gpu-plugin-metal:
    name: Build mens-candle-metal plugin (${{ matrix.triple }})
    runs-on: macos-latest
    timeout-minutes: 240
    # Non-blocking: a plugin build failure must never take down the core
    # vox/vox-ml-cli/voxup release. Job-level continue-on-error sets the job
    # conclusion to success for `needs:` resolution, so publish still WAITS
    # for it (no race where checksums.txt is built before the zip exists)
    # without being blocked by it.
    continue-on-error: true
    # Both arches: Plugin.toml declares macos-aarch64 AND macos-x86_64, and
    # current_target_triple() will resolve to macos-x86_64 on an Intel Mac.
    # Building only the runner's native arch guarantees a 404 for those users.
    strategy:
      fail-fast: false
      matrix:
        include:
          - triple: macos-aarch64
            rust_target: aarch64-apple-darwin
          - triple: macos-x86_64
            rust_target: x86_64-apple-darwin
    env:
      # Same RSS reason as the build/dist-verify jobs: .cargo/config.toml sets
      # [build] jobs = 24 repo-wide, which would run 24 rustc processes on a
      # 4-core hosted runner against a fat-LTO, codegen-units=1 profile.
      CARGO_BUILD_JOBS: "2"
    steps:
      - uses: actions/checkout@v7

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"
          targets: ${{ matrix.rust_target }}

      - name: Build the Metal cdylib
        run: cargo build -p vox-plugin-mens-candle-metal --profile dist --features metal --target ${{ matrix.rust_target }}

      - name: Package the plugin zip
        # Flat by construction: Plugin.toml and the cdylib go into an empty
        # staging dir and are zipped with -j (junk paths). install_from_path
        # reads ONLY top-level files -- anything nested extracts fine and is
        # then silently ignored, producing an install that appears to succeed.
        run: |
          set -euo pipefail
          version="$(cargo pkgid -p vox-plugin-mens-candle-metal | sed -E 's/.*[#@]//')"
          staging="$(mktemp -d)"
          cp crates/vox-plugin-mens-candle-metal/Plugin.toml "$staging/"
          cp "target/${{ matrix.rust_target }}/dist/libvox_plugin_mens_candle_metal.dylib" "$staging/"
          mkdir -p dist
          zip -j "dist/mens-candle-metal-v${version}-${{ matrix.triple }}.zip" "$staging"/*
          echo "Packaged: mens-candle-metal-v${version}-${{ matrix.triple }}.zip"

      - name: Upload plugin artifact
        uses: actions/upload-artifact@v7
        with:
          name: release-gpu-metal-${{ matrix.triple }}
          path: dist/mens-candle-metal-v*.zip
          if-no-files-found: error
```

- [ ] **Step 3: Add the CUDA plugin job**

```yaml
  gpu-plugin-cuda:
    name: Build mens-candle-cuda plugin
    runs-on: ubuntu-latest
    timeout-minutes: 240
    # See gpu-plugin-metal -- same non-blocking contract. CUDA is the least
    # proven part of this plan: no workflow in this repo has ever installed a
    # CUDA toolkit on a hosted runner, and the only CUDA-compiling job in CI
    # runs on the self-hosted fleet this whole effort routes around.
    continue-on-error: true
    env:
      CARGO_BUILD_JOBS: "2"
      # candle-kernels/bindgen_cuda derives the target compute capability from
      # this env var or from nvidia-smi. A hosted runner has no GPU and no
      # nvidia-smi, so without this the build fails even when the toolkit
      # installs cleanly. 80 = Ampere (A100/A10), a reasonable default target.
      CUDA_COMPUTE_CAP: "80"
    steps:
      - uses: actions/checkout@v7

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.96.0"

      - name: Install CUDA toolkit
        uses: Jimver/cuda-toolkit@v0.2.24
        with:
          cuda: "12.4.0"
          method: "network"

      - name: Build the CUDA cdylib
        run: cargo build -p vox-plugin-mens-candle-cuda --profile dist --features cuda

      - name: Package the plugin zip
        run: |
          set -euo pipefail
          version="$(cargo pkgid -p vox-plugin-mens-candle-cuda | sed -E 's/.*[#@]//')"
          staging="$(mktemp -d)"
          cp crates/vox-plugin-mens-candle-cuda/Plugin.toml "$staging/"
          cp target/dist/libvox_plugin_mens_candle_cuda.so "$staging/"
          mkdir -p dist
          zip -j "dist/mens-candle-cuda-v${version}-linux-x86_64.zip" "$staging"/*
          echo "Packaged: mens-candle-cuda-v${version}-linux-x86_64.zip"

      - name: Upload plugin artifact
        uses: actions/upload-artifact@v7
        with:
          name: release-gpu-cuda
          path: dist/mens-candle-cuda-v*.zip
          if-no-files-found: error
```

- [ ] **Step 4: Wire both into `publish`'s `needs:`**

Change `needs: [build, dist-verify]` to:

```yaml
    # gpu-plugin-* are continue-on-error, so listing them here only makes
    # publish WAIT (so checksums.txt is generated after their zips exist) --
    # it does not make publish depend on their success.
    needs: [build, dist-verify, gpu-plugin-metal, gpu-plugin-cuda]
```

- [ ] **Step 5: Validate YAML**

Run: `python -c "import yaml; d=yaml.safe_load(open('.github/workflows/release-binaries.yml', encoding='utf-8')); print('jobs:', list(d['jobs'].keys()))"`
Expected: `jobs: ['build', 'gpu-plugin-metal', 'gpu-plugin-cuda', 'dist-verify', 'publish']`

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/release-binaries.yml
git commit -m "feat(ci): build and publish the GPU plugins as release assets

Adds gpu-plugin-metal (macos-latest, BOTH arches -- Plugin.toml declares
macos-aarch64 and macos-x86_64, so building only the runner's native arch
would 404 for every Intel Mac) and gpu-plugin-cuda (ubuntu-latest via
Jimver/cuda-toolkit; first time this repo has installed CUDA on a hosted
runner).

Both set CARGO_BUILD_JOBS=2 (the repo-wide [build] jobs = 24 would run 24
rustc processes on a 4-core runner at fat LTO) and timeout-minutes: 240 to
match the neighbouring jobs doing the same class of build. CUDA also sets
CUDA_COMPUTE_CAP, which candle's kernel build needs and cannot infer on a
GPU-less runner.

Both continue-on-error at the job level: publish's needs: includes them so
checksums.txt waits for their zips, but their failure never blocks the
release. Packaging is flat (zip -j) because install_from_path reads only
top-level files and silently ignores nesting."
```

---

### Task 8: Make `release-gui.yml` agree with the release-flag convention

**Files:**
- Modify: `.github/workflows/release-gui.yml`

**Interfaces:**
- Consumes: Task 2 (this only matters once the GUI actually produces artifacts)
- Produces: consistent `prerelease`/draft handling across all three workflows that write the same release

> **[CRITIQUE] NEW TASK — a real conflict the first draft never noticed.** `release-gui.yml` sets `releaseDraft: true` and `prerelease: false`, while `release-binaries.yml` and `release-installers.yml` both use the hyphen rule (`prerelease: ${{ contains(github.ref_name, '-') }}`). All three trigger on `tags: "v*"` with no ordering. This was **inert only because the GUI produced zero artifacts in 6/6 runs — Task 2 activates it.** Whichever workflow reaches the release API first sets the flags, so an rc tag could publish a non-prerelease, or `tauri-action` could leave the release a **draft** that no user can download while `gh release view` still shows assets to the token owner (making verification read green on an invisible release).

- [ ] **Step 1: Read the current flags**

Run: `grep -n "releaseDraft\|prerelease\|tagName\|releaseName" .github/workflows/release-gui.yml`

- [ ] **Step 2: Align them with the other two workflows**

Change:

```yaml
          releaseDraft: true
          prerelease: false
```

to:

```yaml
          # Match release-binaries.yml / release-installers.yml: all three
          # workflows write the SAME release object for a given tag with no
          # ordering between them, so their flags must agree or whichever
          # runs first wins. Hyphen => prerelease, per the repo convention.
          releaseDraft: false
          prerelease: ${{ contains(github.ref_name, '-') }}
```

- [ ] **Step 3: Validate YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/release-gui.yml', encoding='utf-8')); print('OK')"`
Expected: `OK`

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release-gui.yml
git commit -m "fix(ci): make release-gui agree with the release-flag convention

All three release workflows write the same release object for a tag, with
no ordering. release-gui.yml pinned releaseDraft: true / prerelease: false
while the other two use the hyphen rule, so whichever ran first won.

Inert until now only because the GUI produced zero artifacts in 6/6 runs;
turning the bundler on activates the conflict. A draft release would also
make verification read green on something no user can download."
```

---

### Task 9: Push a verification tag and audit the real release

**Files:** none (verification)

**Interfaces:**
- Consumes: Tasks 1–8, all pushed
- Produces: a per-asset verdict on the actual goal

- [ ] **Step 1: Push all work and confirm a clean tree**

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
git status --short
git push origin <branch-name>
```

- [ ] **Step 2: Cut a collision-free rc tag**

```bash
buildnum="$(git rev-list --count HEAD)"
git tag "v0.6.0-rc.${buildnum}"
git push origin "v0.6.0-rc.${buildnum}"
echo "tagged v0.6.0-rc.${buildnum}"
```

- [ ] **Step 3: Confirm the guard passes (Task 1's first real proof)**

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
gh run list --workflow version-tag-guard.yml --limit 3 --json databaseId,headBranch,status,conclusion
```

Match the run whose `headBranch` is your rc tag — **[CRITIQUE]** `--limit 1` alone is ref-blind and may show an unrelated run. Wait for `completed`, then expect `conclusion: success`. A failure here means Task 1 is wrong and everything below is meaningless.

- [ ] **Step 4: Wait for the three release workflows**

Bounded checks only — no sleep-loops. These take 15 minutes to several hours (`timeout-minutes: 240`). Every 15–20 minutes:

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
gh api repos/vox-foundation/vox/actions/runs/<run-id> -q '.status, .conclusion'
```

Do other work between checks. Continue when `release-gui.yml`, `release-binaries.yml`, and `release-installers.yml` all report `completed`.

- [ ] **Step 5: List every asset actually on the release**

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
gh release view "v0.6.0-rc.${buildnum}" --json isDraft,isPrerelease,assets -q '{draft:.isDraft, prerelease:.isPrerelease, names:[.assets[].name]}'
```

Expected: `draft: false`, `prerelease: true` (Task 8's fix), and a name list to check against the next three steps.

- [ ] **Step 6: Verify the GUI installers (Tasks 2/3)**

From Step 5's list, confirm at least one Tauri-produced installer: a `.msi`, `.dmg`, `.AppImage`, or GUI `.deb`. These are distinct from the CLI's own `vox-cli-*.msi` and `cargo-deb`'s `.deb`. If none appear, read that platform's `Build Tauri GUI` job log before concluding anything.

- [ ] **Step 7: Verify the CLI `.deb` (the previously unverified glob fix)**

From Step 5's list, confirm a `cargo-deb`-produced `.deb` is present — the first real test of `f52f3acef`'s recursive glob.

- [ ] **Step 8: Verify the GPU plugin zips**

From Step 5's list, expect `mens-candle-metal-v0.6.0-macos-aarch64.zip` **and** `mens-candle-metal-v0.6.0-macos-x86_64.zip`. `mens-candle-cuda-v0.6.0-linux-x86_64.zip` may be absent — its job is `continue-on-error`, so that is an expected outcome, not a plan failure. Metal's absence is *not* expected; read that job's log if so.

- [ ] **Step 9: Prove a plugin actually installs (Task 5's only end-to-end proof)**

**[CRITIQUE]** This is why `VOX_PLUGIN_RELEASE_TAG` exists — without it the binary would look for a `v0.6.0` release that does not exist yet, and this step could never pass on an rc tag.

`mens-candle-metal` is macOS-only, so on this Windows host the honest substitute is to verify the **CUDA** plugin from WSL2 if its zip was published; otherwise defer this step and report it as unverified rather than skipping it silently.

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  export VOX_PLUGIN_RELEASE_TAG="v0.6.0-rc.<buildnum>"
  vox plugin install mens-candle-cuda --yes
'
```

Expected: resolves the rc release, fetches `checksums.txt`, verifies the hash, and installs **without** `--allow-unverified`. Note `--yes` — `install_from_url` otherwise blocks on an interactive stdin prompt.

- [ ] **Step 10: Report a per-asset verdict**

State plainly, for each of GUI / `.deb` / Metal / CUDA: produced a real downloadable artifact, or not, with the specific evidence. This is the deliverable — not a summary of workflow colors.

---

### Task 10: Prove the `.deb` installs on real Ubuntu (WSL2)

**Files:** none

**Interfaces:**
- Consumes: Task 9's verified `.deb`
- Produces: proof that `dpkg -i` succeeds and the installed binary runs

> **[CRITIQUE] two blockers corrected.** (1) `gh` is **not installed inside WSL2** and would be unauthenticated there even if it were — download on the Windows side and hand the file across `/mnt/c`. (2) After Task 2, the release carries **two** `.deb` files; `--pattern '*.deb'` + `dpkg -i *.deb` would also install the Tauri GUI `.deb`, which depends on WebKitGTK that bare WSL2 lacks — dpkg would fail and it would read as a CLI `.deb` packaging failure. Install exactly one file, by name.

- [ ] **Step 1: Confirm WSL2 is available**

Run: `wsl.exe -d Ubuntu -e bash -lc "lsb_release -ds; dpkg --version | head -1"`
Expected: `Ubuntu 24.04.1 LTS` and a dpkg version.

- [ ] **Step 2: Identify the CLI `.deb` precisely, and download it on the Windows side**

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
gh release view "v0.6.0-rc.${buildnum}" --json assets -q '.assets[].name' | grep -i '\.deb$'
```

Pick the `cargo-deb` one (a `vox`/`vox-cli` package name, typically `..._amd64.deb`), **not** the Tauri GUI one, then:

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
mkdir -p /tmp/debcheck
gh release download "v0.6.0-rc.${buildnum}" --pattern '<the-exact-cli-deb-name>' --dir /tmp/debcheck --clobber
ls -la /tmp/debcheck
```

- [ ] **Step 3: Install it inside WSL2 from the Windows-side copy**

```bash
wsl.exe -d Ubuntu -u root -e bash -lc '
  set -euo pipefail
  cp /mnt/c/Users/iacch/AppData/Local/Temp/debcheck/*.deb /tmp/ 2>/dev/null || cp "$(wslpath -u "C:/tmp/debcheck")"/*.deb /tmp/
  cd /tmp
  dpkg -i ./*.deb || apt-get install -f -y
  echo "dpkg exit: $?"
'
```

`-u root` avoids `sudo`, which requires a password in this distro. If `apt-get install -f` was needed, record it — a missing declared dependency is a real `cargo-deb` packaging gap, not a WSL quirk.

- [ ] **Step 4: Prove the installed binary runs**

```bash
wsl.exe -d Ubuntu -e bash -lc "vox --version"
```

Expected: a semver string. **[CRITIQUE]** — it will report `0.6.0` (plus build metadata), **not** the rc tag: `release_build.rs` uses `--version ${{ github.ref_name }}` for *filenames only*, while the embedded version comes from `CARGO_PKG_VERSION`. That is correct behavior, not a bug.

- [ ] **Step 5: Report the verdict**

State the exact `vox --version` output, whether `apt-get install -f` was needed, and which `.deb` was installed.

---

### Task 11: Homebrew formula audit harness in WSL2

**Files:**
- Create: `scripts/homebrew/vox.rb.template` and the generated `vox.rb` (not committed)

**Interfaces:**
- Consumes: Task 9's release assets
- Produces: a `brew audit --strict`-validated formula

> **[CRITIQUE] three corrections.** (1) The first draft's `.vox` script used a **fabricated API** (`std.process.args().get_flag()`, `std.fs.write`) and required a `vox` binary that is not on PATH here — it would fail on the first run. Since AGENTS.md's VoxScript-first policy governs *new automation*, and this is a one-shot local audit harness, generate the formula with the tooling that actually exists (`python`, already used throughout this plan) rather than inventing an unverified Vox API. (2) `sudo` requires a password in this distro — Homebrew also refuses to run as root, so install it as the normal user into `$HOME`. (3) **The formula must be arch-aware**: `vox-darwin.tar.gz` is built with no `--target` on `macos-latest` (arm64), so a single-URL formula silently ships an arm64 binary to Intel Macs — and `brew audit` cannot see that. `release-binaries.yml` already builds *both* properly-named darwin tarballs, which are also in `checksums.txt`; point the formula at those two.

- [ ] **Step 1: Install Homebrew-on-Linux in WSL2 as a normal user**

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  if ! command -v brew >/dev/null 2>&1 && [ ! -x /home/linuxbrew/.linuxbrew/bin/brew ]; then
    NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  fi
  eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv 2>/dev/null || true)"
  brew --version
'
```

If Homebrew declines to install (it refuses to run as root and needs `sudo` for `/home/linuxbrew`), fall back to a user-local clone:

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  [ -d "$HOME/.homebrew" ] || git clone --depth=1 https://github.com/Homebrew/brew "$HOME/.homebrew"
  "$HOME/.homebrew/bin/brew" --version
'
```

- [ ] **Step 2: Collect both darwin tarballs and their real hashes**

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
gh release download "v0.6.0-rc.${buildnum}" --pattern 'checksums.txt' --dir /tmp/brewcheck --clobber
grep -E "vox-v.*-(aarch64|x86_64)-apple-darwin\.tar\.gz" /tmp/brewcheck/checksums.txt
```

Expected: two lines — the arm64 and x86_64 darwin tarballs, both present in `checksums.txt` (unlike `vox-darwin.tar.gz`, which is produced by a different workflow and never appears there).

- [ ] **Step 3: Generate an arch-aware formula**

```bash
python - <<'PY'
import re, pathlib
tag = "v0.6.0-rc.<buildnum>"          # substitute the real tag
ver = tag.lstrip("v").split("-")[0]    # 0.6.0
sums = pathlib.Path("/tmp/brewcheck/checksums.txt").read_text(encoding="utf-8")
want = {}
for line in sums.splitlines():
    if "  " not in line:
        continue
    h, name = line.split("  ", 1)
    name = name.strip()
    if re.search(r"^vox-v.*-aarch64-apple-darwin\.tar\.gz$", name):
        want["arm"] = (name, h.strip())
    elif re.search(r"^vox-v.*-x86_64-apple-darwin\.tar\.gz$", name):
        want["intel"] = (name, h.strip())
assert "arm" in want and "intel" in want, f"need both darwin tarballs, got {want}"
base = f"https://github.com/vox-foundation/vox/releases/download/{tag}"
f = f'''class Vox < Formula
  desc "Vox programming language and toolchain"
  homepage "https://voxlang.org"
  version "{ver}"

  on_macos do
    on_arm do
      url "{base}/{want['arm'][0]}"
      sha256 "{want['arm'][1]}"
    end
    on_intel do
      url "{base}/{want['intel'][0]}"
      sha256 "{want['intel'][1]}"
    end
  end

  def install
    bin.install "vox"
  end

  test do
    system "#{{bin}}/vox", "--version"
  end
end
'''
pathlib.Path("/tmp/brewcheck/vox.rb").write_text(f, encoding="utf-8")
print(f)
PY
```

- [ ] **Step 4: Audit it in WSL2**

```bash
wsl.exe -d Ubuntu -e bash -lc '
  set -euo pipefail
  BREW="$(command -v brew || echo /home/linuxbrew/.linuxbrew/bin/brew)"
  [ -x "$BREW" ] || BREW="$HOME/.homebrew/bin/brew"
  cp "$(wslpath -u "C:/Users/iacch/AppData/Local/Temp/brewcheck/vox.rb")" /tmp/vox.rb 2>/dev/null || cp /tmp/brewcheck/vox.rb /tmp/vox.rb
  "$BREW" style /tmp/vox.rb || true
  "$BREW" audit --strict --formula /tmp/vox.rb || true
'
```

Read the output. Fix genuine formula errors in Step 3's template and re-run. `|| true` is deliberate: the first run is diagnostic, and a `brew audit` failure here is information, not a reason to abort the task.

- [ ] **Step 5: Verify the tarball layout matches `bin.install "vox"`**

`release_build.rs` packages a bare `vox` at the archive root, so `bin.install "vox"` is correct. Confirm directly:

```bash
export PATH="/c/Program Files/GitHub CLI:$PATH"
gh release download "v0.6.0-rc.${buildnum}" --pattern 'vox-v*-aarch64-apple-darwin.tar.gz' --dir /tmp/brewcheck --clobber
tar -tzf /tmp/brewcheck/vox-v*-aarch64-apple-darwin.tar.gz | head
```

Expected: a single top-level `vox` entry, no leading directory.

- [ ] **Step 6: Report — with the limit stated**

This proves the formula is well-formed, its URLs resolve, its checksums match the real assets, and the tarball layout suits `bin.install`. It does **not** prove `brew install vox` works on a Mac — there is no macOS in this environment. Say so explicitly; a green audit must not be read as a working macOS install. Publishing to a real tap (`vox-foundation/homebrew-vox`) remains out of scope: the repo does not exist and needs a token this agent cannot create.

---

## Self-Review Notes

**Spec coverage:** Phase 0 → Tasks 1, 9. Phase 1 → Tasks 2, 3, 8; verified in Task 9. Phase 2 → Task 10. Phase 3 → Tasks 4, 5, 6, 7; verified in Task 9. Phase 4 → Task 11.

**[CRITIQUE] Confirmed false positives — removed from scope, do not re-introduce:**
- GPU plugins absent from `contracts/distribution/profiles.v1.yaml` — true but **inert**: that file has no plugin list, `distribution_parity.rs` has no plugin/catalog reference, and `distribution-parity.yml` isn't even triggered by a `catalog.toml` edit.
- `distribution-bundles.generated.md` — unaffected (renders only id/description).
- `skeleton/untested-pub-api` — satisfied; the new `install.rs` functions are not `pub`.
- `distribution-ssot.md` needs no GPU-related update.
- The two-`.deb` *filename collision* — false alarm; they don't collide. (The two-`.deb` *download* problem in Task 10 is real and handled.)

**Known gaps, deliberately not tasks:**
- **Windows CUDA.** `mens-candle-cuda`'s `Plugin.toml` declares a `windows-x86_64` artifact, but only the Linux leg is built — a Windows NVIDIA user gets a 404. Adding a second unproven CUDA platform in the same pass compounds risk; this is a scoped follow-up once the Linux leg reports back.
- **Cutting a real release.** `v0.6.0` is already tagged (see Global Constraints), so shipping a non-prerelease requires a version-bump decision plus a `CHANGELOG.md` entry — explicitly the user's call.
- **Stale docs.** `distribution-ssot.md`'s "release + nightly" claim stays wrong (nightly is now *possible* but still unwired), and `2026-08-23-release-pipeline-verified-design.md:166-171` records a decision Task 1 reverses. Prose-only; worth a follow-up commit.
