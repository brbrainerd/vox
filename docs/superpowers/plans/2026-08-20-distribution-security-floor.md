# Distribution Security Floor Implementation Plan (Phase 1b)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four fail-open holes in the path from a downloaded artifact to executing code — an installer that skips its own integrity check, an unguarded archive extractor, over-scoped release credentials, and native plugins that are `dlopen`'d with no verification whatsoever.

**Architecture:** Four independent hardenings, ordered cheapest-first. Three are under ten lines each and land immediately. The fourth — plugin integrity — is the one that matters most and is deliberately **fail-closed rather than pre-populated**: no plugin release assets exist yet, so requiring a `sha256` on all thirteen `github:`-sourced catalog entries today would simply break `vox plugin install`. Instead, an install with no expected hash is **refused** unless the operator passes an explicit override flag, and a recorded-hash sidecar lets the loader detect post-install tampering.

**Tech Stack:** Rust 1.96.0, `sha2`, `tar`, `flate2`, POSIX `sh`, GitHub Actions.

**Spec:** [`docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`](../specs/2026-08-20-vox-distribution-system-design.md) — findings F9, F11, F12, F14; architecture A6 items 2–5.

## Global Constraints

- Rust toolchain is pinned to **1.96.0**. Do not bump it.
- **`vox` is not on PATH in this worktree.** Every invocation is `cargo run -q -p vox-cli -- <args>`.
- **Never run `cargo fmt --all`** (Windows `CreateProcess` overflow, `os error 206`). Use `cargo run -q -p vox-cli -- run scripts/fmt.vox`.
- **Verify with `vox ci pre-push --full`, not `--complete`** — `--complete` runs no tests (`crates/vox-cli/src/commands/ci/pre_push.rs:8-10`).
- **Do not add a workspace crate-to-crate dependency edge** (`vox ci crate-edges` gates the exact set). Adding an *external* crate such as `sha2` to a crate's `Cargo.toml` is **not** an edge and is permitted. Duplicating a helper under ~50 lines requires a `// vox:defactored-from <crate> <date>` comment.
- Test-first is binding. Batch commits; open one review-ready PR; re-review via `@coderabbitai review`.
- **This plan does not sign anything.** Signing `checksums.txt` (spec F10) is blocked on a release key held outside GitHub and is not attempted here. Everything below raises the floor beneath that, and none of it substitutes for it.

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `scripts/install.sh` | Bootstrap installer; must never install unverified bytes | 1 |
| `crates/voxup/src/download.rs` | Archive extraction; must reject escaping entries on **all** platforms | 2 |
| `.github/workflows/*.yml` | Release credentials scoped per job | 3 |
| `crates/vox-cli-ci/src/workflow_permissions_guard.rs` | New gate: every workflow declares `permissions:` | 3 |
| `crates/vox-plugin-catalog/src/schema.rs`, `catalog.toml` | Optional `sha256` per plugin entry | 4 |
| `crates/vox-cli/src/commands/plugin/install.rs` | Fail-closed download verification; integrity sidecar | 4, 5 |
| `crates/vox-plugin-host/src/loader.rs`, `errors.rs` | Refuse a dylib whose hash differs from the recorded one | 5 |

---

### Task 1: Stop `install.sh` from installing unverified bytes

`verify_checksum` returns success when no hash tool is present, so on a minimal container (Alpine, busybox, stripped CI images) the installer prints a warning and installs anyway. In a `curl | sh` pipeline that warning scrolls past. `need_cmd` already guards `curl` and `tar` but never a hash tool.

**Files:**
- Modify: `scripts/install.sh:50-68` (`verify_checksum`)
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (existing `#[cfg(test)] mod tests` at `:194`)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing later tasks consume.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-cli/src/commands/ci/release_build.rs`:

```rust
/// `install.sh` must never install a binary it could not verify. The original
/// `verify_checksum` returned success when neither sha256sum nor shasum was
/// present, so a minimal container silently got an unverified binary.
#[test]
fn install_sh_never_skips_the_integrity_check() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sh = std::fs::read_to_string(root.join("scripts/install.sh")).expect("read install.sh");

    assert!(
        !sh.contains("skipping integrity check"),
        "install.sh still has a fail-open branch in verify_checksum; a missing \
         sha256 tool must abort the install, not warn and continue"
    );
    assert!(
        sh.contains("openssl"),
        "install.sh should try `openssl dgst -sha256` before giving up, so the \
         hard failure only fires when no hashing tool exists at all"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib install_sh_never_skips_the_integrity_check -- --nocapture`

Expected: FAIL with `install.sh still has a fail-open branch in verify_checksum`.

- [ ] **Step 3: Write the implementation**

Replace the `verify_checksum` function in `scripts/install.sh`:

```sh
verify_checksum() {
    _file="$1"
    _expected="$2"

    # Fail CLOSED. A missing hashing tool must abort the install, never
    # downgrade to installing unverified bytes — this runs inside `curl | sh`,
    # where a printed warning scrolls past unread.
    if command -v sha256sum >/dev/null 2>&1; then
        _actual="$(sha256sum "$_file" | cut -d ' ' -f1)"
    elif command -v shasum >/dev/null 2>&1; then
        _actual="$(shasum -a 256 "$_file" | cut -d ' ' -f1)"
    elif command -v openssl >/dev/null 2>&1; then
        _actual="$(openssl dgst -sha256 "$_file" | awk '{print $NF}')"
    else
        err "no SHA-256 tool found (need one of: sha256sum, shasum, openssl).\n  Refusing to install an unverified binary."
    fi

    if [ "$_actual" != "$_expected" ]; then
        err "SHA-256 mismatch for $_file\n  expected: $_expected\n  actual:   $_actual"
    fi
    say "Checksum OK"
}
```

`err` already exits non-zero — confirm by reading its definition near the top of `scripts/install.sh` before relying on it.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib install_sh_never_skips_the_integrity_check -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Verify the script still parses**

Run: `sh -n scripts/install.sh && echo "POSIX syntax OK"`

Expected: `POSIX syntax OK`. This is the same check `release-installers.yml` runs.

- [ ] **Step 6: Commit**

```bash
git add scripts/install.sh crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(install): fail closed when no SHA-256 tool is available

verify_checksum returned success when neither sha256sum nor shasum existed,
so minimal containers installed an unverified binary behind a warning that
scrolls past in a curl|sh pipeline. Adds an openssl fallback, then aborts.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Guard tar extraction on the platforms that actually use it

`extract_zip` validates `enclosed_name()` and `starts_with(dest_dir)` and has a regression test — but both are `#[cfg(windows)]`, and the archive extension is `.zip` only on Windows. **The tar path is what every Linux and macOS user takes, and `extract_targz` is a bare `archive.unpack(dest_dir)`** with no path validation, no symlink rejection, and no size cap.

The target is concrete: extraction lands in `~/.vox/toolchains/vox-<ver>/`, one `..` from `~/.vox/toolchains/bin`, which `crates/voxup/src/proxy.rs:209-216` **prepends to `PATH`** for every proxied `vox` invocation. Today the only thing preventing an escape is the `tar` crate skipping such entries — silently, so a tampered archive surfaces as the unrelated message "Extraction succeeded but 'vox' not found".

**Files:**
- Modify: `crates/voxup/src/download.rs:64-74` (`extract_targz`)
- Test: `crates/voxup/src/download.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `extract_targz` keeps its signature `fn extract_targz(data: &[u8], dest_dir: &Path) -> Result<()>`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/voxup/src/download.rs`:

```rust
/// Build a gzipped tar in memory containing one entry at `entry_path`.
#[cfg(unix)]
fn targz_with_entry(entry_path: &str, contents: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, entry_path, contents)
            .expect("append entry");
        builder.finish().expect("finish tar");
    }
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    use std::io::Write;
    gz.write_all(&tar_bytes).expect("gzip write");
    gz.finish().expect("gzip finish")
}

/// Tar-slip: an entry whose path escapes the destination must be REJECTED with
/// an error, not silently skipped. The extraction root sits one `..` from
/// ~/.vox/toolchains/bin, which proxy.rs prepends to PATH.
#[cfg(unix)]
#[test]
fn test_extract_targz_rejects_path_traversal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data = targz_with_entry("../escaped.txt", b"pwned");
    let err = extract_targz(&data, dir.path()).expect_err("must reject escaping entry");
    let msg = err.to_string();
    assert!(
        msg.contains("escapes destination") || msg.contains("Tar Slip"),
        "expected a traversal rejection, got: {msg}"
    );
    assert!(
        !dir.path().parent().unwrap().join("escaped.txt").exists(),
        "escaping entry was written outside the destination"
    );
}

/// A symlink entry could redirect a later write outside the destination, and is
/// never needed in a Vox release archive.
#[cfg(unix)]
#[test]
fn test_extract_targz_rejects_symlink_entries() {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header
            .set_link_name("/etc/passwd")
            .expect("set link name");
        header.set_cksum();
        builder
            .append_data(&mut header, "link", &[][..])
            .expect("append symlink");
        builder.finish().expect("finish tar");
    }
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&tar_bytes).expect("gzip write");
    let data = gz.finish().expect("gzip finish");

    let dir = tempfile::tempdir().expect("tempdir");
    let err = extract_targz(&data, dir.path()).expect_err("must reject symlink entry");
    assert!(
        err.to_string().contains("unsupported entry type"),
        "expected a symlink rejection, got: {err}"
    );
}

/// The happy path must keep working.
#[cfg(unix)]
#[test]
fn test_extract_targz_accepts_normal_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data = targz_with_entry("vox", b"#!/bin/sh\n");
    extract_targz(&data, dir.path()).expect("normal entry must extract");
    assert!(dir.path().join("vox").is_file(), "entry was not written");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p voxup --lib download::tests -- --nocapture`

Expected: FAIL — `test_extract_targz_rejects_path_traversal` and `test_extract_targz_rejects_symlink_entries` both fail, because `unpack` skips escaping entries silently (returning `Ok`) and happily writes symlinks. On Windows these are `#[cfg(unix)]` and will not run; that is intended — run this task's verification on Linux or macOS, or under WSL.

- [ ] **Step 3: Confirm `tempfile` is available to voxup's tests**

Run: `grep -n "tempfile" crates/voxup/Cargo.toml`

Expected: `tempfile = { workspace = true }` under `[dev-dependencies]` (verified present at `crates/voxup/Cargo.toml:42`). This step is a guard against drift, not an expected edit.

- [ ] **Step 4: Write the implementation**

Replace `extract_targz` in `crates/voxup/src/download.rs`:

```rust
/// Maximum total uncompressed bytes we will write from one archive (512 MiB).
/// Guards against a decompression bomb; the largest real Vox artifact is far
/// under this.
const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum number of entries in one archive.
const MAX_ENTRIES: usize = 10_000;

fn extract_targz(data: &[u8], dest_dir: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::{Archive, EntryType};

    // Explicit entry loop rather than `archive.unpack()`. `unpack` SILENTLY
    // SKIPS entries that escape the destination, so a tampered archive
    // surfaces as "Extraction succeeded but 'vox' not found" rather than as a
    // security error. It also happily writes symlinks. This matters because
    // dest_dir is ~/.vox/toolchains/vox-<ver>/, one `..` from
    // ~/.vox/toolchains/bin, which proxy.rs prepends to PATH.
    let gz = GzDecoder::new(Cursor::new(data));
    let mut archive = Archive::new(gz);

    let mut total_bytes: u64 = 0;
    let mut count: usize = 0;

    for entry in archive.entries().context("read tar entries")? {
        let mut entry = entry.context("read tar entry")?;

        count += 1;
        if count > MAX_ENTRIES {
            bail!("archive has more than {MAX_ENTRIES} entries; refusing to extract");
        }

        match entry.header().entry_type() {
            EntryType::Regular | EntryType::Directory => {}
            other => bail!(
                "unsupported entry type {:?} in archive entry {:?}; only regular \
                 files and directories are allowed",
                other,
                entry.path().map(|p| p.display().to_string())
            ),
        }

        let path = entry.path().context("decode tar entry path")?.into_owned();
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            bail!("Tar Slip detected: path {:?} escapes destination", path);
        }
        let outpath = dest_dir.join(&path);
        if !outpath.starts_with(dest_dir) {
            bail!("Tar Slip detected: path {:?} escapes destination", path);
        }

        total_bytes = total_bytes.saturating_add(entry.header().size().unwrap_or(0));
        if total_bytes > MAX_UNCOMPRESSED_BYTES {
            bail!("archive expands beyond {MAX_UNCOMPRESSED_BYTES} bytes; refusing to extract");
        }

        if entry.header().entry_type() == EntryType::Directory {
            fs::create_dir_all(&outpath)
                .with_context(|| format!("create dir {}", outpath.display()))?;
            continue;
        }
        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        entry
            .unpack(&outpath)
            .with_context(|| format!("unpack entry to {}", outpath.display()))?;
    }

    info!("Extracted tar.gz to {}", dest_dir.display());
    Ok(())
}
```

Confirm `bail!` and `fs` are already imported at the top of `download.rs` (they are — `bail` is used by `verify_sha256` and the zip path uses `fs`). Add `use std::path::Component;` only if the fully-qualified form above is changed.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p voxup --lib download::tests -- --nocapture`

Expected: PASS — including the pre-existing `extract_targz_round_trip` happy-path test, which must not regress.

- [ ] **Step 6: Commit**

```bash
git add crates/voxup/src/download.rs crates/voxup/Cargo.toml
git commit -m "fix(voxup): validate tar entries instead of trusting unpack()

extract_zip guarded against zip-slip but was #[cfg(windows)], so the tar path
every Linux and macOS user takes had no guard at all. archive.unpack() skips
escaping entries silently and writes symlinks; the extraction root is one `..`
from a directory proxy.rs prepends to PATH. Adds explicit entry validation,
symlink rejection, and size/count caps, with unix regression tests.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Scope release credentials per job, and gate it

`release-binaries.yml:8-9` and `release-gui.yml:10-11` declare `contents: write` **top-level**, so build-matrix jobs that compile 1600+ third-party crates hold a write token they never use. `release-installers.yml` declares **no `permissions:` block at all**, inheriting the repository default.

**Files:**
- Modify: `.github/workflows/release-binaries.yml`, `release-gui.yml`, `release-installers.yml`
- Create: `crates/vox-cli-ci/src/workflow_permissions_guard.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs` (module declaration)
- Test: `crates/vox-cli-ci/src/workflow_permissions_guard.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn workflow_declares_permissions(yml: &str) -> bool` and `pub fn run(root: &Path, strict: bool) -> anyhow::Result<()>` in `vox_cli_ci::workflow_permissions_guard`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli-ci/src/workflow_permissions_guard.rs`:

```rust
//! Gate: every workflow must declare an explicit `permissions:` block.
//!
//! Without one, a workflow inherits the repository default token scope. If that
//! default is the legacy "read and write all scopes", every job — including ones
//! that compile third-party crates or download and execute release binaries —
//! carries a fully privileged GITHUB_TOKEN. Shaped after
//! `workflow_concurrency_guard`.

use anyhow::{Result, bail};
use std::path::Path;

/// True when the workflow declares a top-level `permissions:` key.
///
/// Only a column-zero `permissions:` counts as top-level; a job-level one is
/// indented and does not satisfy the repository default problem on its own.
pub fn workflow_declares_permissions(yml: &str) -> bool {
    yml.lines().any(|l| l.starts_with("permissions:"))
}

/// Check every `.github/workflows/*.yml`. In `strict` mode a missing block is
/// an error; otherwise it is reported and tolerated.
pub fn run(root: &Path, strict: bool) -> Result<()> {
    let dir = root.join(".github/workflows");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        if !workflow_declares_permissions(&text) {
            offenders.push(
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("<unknown>")
                    .to_string(),
            );
        }
    }
    offenders.sort();
    if !offenders.is_empty() {
        let list = offenders.join(", ");
        if strict {
            bail!(
                "workflows without an explicit top-level `permissions:` block: {list}\n\
                 Add `permissions: contents: read` and grant more per job only where needed."
            );
        }
        eprintln!("warning: workflows without `permissions:`: {list}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_top_level_permissions_block() {
        let yml = "name: x\non:\n  push:\npermissions:\n  contents: read\njobs:\n";
        assert!(workflow_declares_permissions(yml));
    }

    #[test]
    fn a_job_level_block_does_not_count_as_top_level() {
        let yml = "name: x\njobs:\n  build:\n    permissions:\n      contents: read\n";
        assert!(!workflow_declares_permissions(yml));
    }

    #[test]
    fn missing_block_is_detected() {
        let yml = "name: x\non:\n  push:\njobs:\n  build:\n    runs-on: ubuntu-latest\n";
        assert!(!workflow_declares_permissions(yml));
    }

    /// The three release workflows must each declare permissions explicitly.
    /// release-installers.yml historically had none at all.
    #[test]
    fn release_workflows_declare_permissions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for wf in [
            "release-binaries.yml",
            "release-gui.yml",
            "release-installers.yml",
        ] {
            let text = std::fs::read_to_string(root.join(".github/workflows").join(wf))
                .unwrap_or_else(|e| panic!("read {wf}: {e}"));
            assert!(
                workflow_declares_permissions(&text),
                "{wf} has no top-level `permissions:` block and inherits the repo default"
            );
            assert!(
                text.contains("contents: read"),
                "{wf} must default to `contents: read` and grant write only on the \
                 job that uploads release assets"
            );
        }
    }
}
```

- [ ] **Step 2: Register the module**

Add to `crates/vox-cli-ci/src/lib.rs`, alongside the other `pub mod` declarations (find `pub mod workflow_concurrency_guard;` and place it adjacent):

```rust
pub mod workflow_permissions_guard;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-cli-ci workflow_permissions_guard -- --nocapture`

Expected: FAIL on `release_workflows_declare_permissions` with `release-installers.yml has no top-level 'permissions:' block`.

- [ ] **Step 4: Write the implementation**

In `.github/workflows/release-installers.yml`, add after the `on:` block:

```yaml
permissions:
  contents: read
```

In `.github/workflows/release-binaries.yml`, change the top-level block to read-only and grant write on the publishing job only:

```yaml
permissions:
  contents: read
```

Then on the `publish` job add:

```yaml
    permissions:
      contents: write
```

Do the same in `.github/workflows/release-gui.yml`: top-level `contents: read`, and `contents: write` on the `build-tauri` job (it uploads via `tauri-action`).

Any job that later gains attestation needs `id-token: write` and `attestations: write` added alongside its `contents: write` — that arrives in Phase 3, not here.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-cli-ci workflow_permissions_guard -- --nocapture`

Expected: PASS, four tests.

- [ ] **Step 6: Check what else the new gate would flag**

Survey the rest of the tree so the gate's future strict mode is not a surprise:

Run: `for f in .github/workflows/*.yml; do grep -q '^permissions:' "$f" || echo "MISSING: $f"; done`

Record the output in the commit message. **Do not** fix unrelated workflows in this task — this task's contract is the three release workflows plus the gate. Wiring `run(root, true)` into `ssot-drift` happens once the rest of the tree is clean.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/ crates/vox-cli-ci/src/workflow_permissions_guard.rs crates/vox-cli-ci/src/lib.rs
git commit -m "fix(ci): scope release token permissions per job

release-binaries.yml and release-gui.yml declared contents:write top-level, so
build jobs compiling 1600+ third-party crates held a write token they never
used. release-installers.yml declared no permissions block at all and inherited
the repo default. Adds a guard module so a workflow without an explicit block
is detectable.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Refuse to install a plugin that cannot be verified

`plugin/install.rs:106-138` fetches a zip over HTTPS and calls `archive.extract()` with no checksum, no signature, and no pinned version, then the host `dlopen`s the cdylib. Anyone able to publish a release asset in the named repo owns the user's process.

**This task is fail-closed, not pre-populated.** Thirteen catalog entries are `github:`-sourced and no plugin release assets exist yet, so requiring a `sha256` on all of them today would break `vox plugin install` outright. Instead: an unverifiable install is **refused** with a message naming the catalog entry, and an explicit `--allow-unverified` flag exists for the operator who knowingly accepts the risk.

**Files:**
- Modify: `crates/vox-plugin-catalog/src/schema.rs` (add `sha256` to `PluginCatalogEntry`)
- Modify: `crates/vox-cli/src/commands/plugin/install.rs` (`run`, `install_from_url`, `install_from_catalog`)
- Modify: `crates/vox-cli/src/cli_args.rs` (the `plugin install` argument definition)
- Test: `crates/vox-cli/src/commands/plugin/install.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `PluginCatalogEntry.sha256: Option<String>`
  - `fn verify_plugin_archive(data: &[u8], expected: Option<&str>, allow_unverified: bool, source: &str) -> anyhow::Result<String>` — returns the archive's hex sha256 on success. Task 5 calls this and stores the returned hash.
  - `install_from_url(url: &str, yes: bool, expected_sha256: Option<&str>, allow_unverified: bool) -> Result<()>`

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)] mod tests` block at the bottom of `crates/vox-cli/src/commands/plugin/install.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &[u8] = b"pretend this is a plugin zip";
    // sha256 of PAYLOAD, computed by the same helper under test.
    fn payload_hash() -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(PAYLOAD))
    }

    #[test]
    fn matching_hash_is_accepted_and_returned() {
        let want = payload_hash();
        let got = verify_plugin_archive(PAYLOAD, Some(&want), false, "test://x")
            .expect("matching hash must verify");
        assert_eq!(got, want);
    }

    #[test]
    fn mismatched_hash_is_rejected() {
        let err = verify_plugin_archive(PAYLOAD, Some(&"a".repeat(64)), false, "test://x")
            .expect_err("mismatched hash must fail");
        assert!(
            err.to_string().contains("checksum mismatch"),
            "expected a checksum mismatch, got: {err}"
        );
    }

    /// The core of this task: with no expected hash, installation is REFUSED.
    /// Previously this path silently downloaded and dlopen'd arbitrary code.
    #[test]
    fn missing_hash_is_refused_by_default() {
        let err = verify_plugin_archive(PAYLOAD, None, false, "https://example/p.zip")
            .expect_err("an unverifiable plugin must not install");
        let msg = err.to_string();
        assert!(
            msg.contains("no sha256"),
            "error must say why it refused, got: {msg}"
        );
        assert!(
            msg.contains("--allow-unverified"),
            "error must name the explicit override, got: {msg}"
        );
    }

    #[test]
    fn missing_hash_is_allowed_with_the_explicit_override() {
        let got = verify_plugin_archive(PAYLOAD, None, true, "https://example/p.zip")
            .expect("--allow-unverified must permit the install");
        assert_eq!(got, payload_hash(), "the actual hash is still computed and returned");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: FAIL to compile — `cannot find function 'verify_plugin_archive' in this scope`.

- [ ] **Step 3: Confirm the hashing dependencies are present**

Run: `grep -nE "^(sha2|hex) " crates/vox-cli/Cargo.toml`

Expected: both listed. If either is missing, add `sha2 = { workspace = true }` / `hex = { workspace = true }` — external crates, not workspace edges.

- [ ] **Step 4: Add the catalog field**

In `crates/vox-plugin-catalog/src/schema.rs`, inside `PluginCatalogEntry`, immediately after the `requires_tag` field:

```rust
    /// SHA-256 (lowercase hex) of the published plugin archive.
    ///
    /// REQUIRED in practice for `github:` sources: without it
    /// `vox plugin install <id>` refuses, because the archive is fetched over
    /// the network and then `dlopen`'d as native code. Absent for `local:`
    /// sources, which are built from workspace source already trusted.
    #[serde(default)]
    pub sha256: Option<String>,

    /// Release version to fetch for `github:` sources, without a leading `v`
    /// (e.g. `"0.7.0"`).
    ///
    /// Required alongside `sha256` and for the same reason: the previous code
    /// built a `releases/latest/download/...` URL, and the bytes behind a
    /// floating `latest` change over time, so no recorded hash can ever match
    /// it. Absent for `local:` sources.
    #[serde(default)]
    pub version: Option<String>,
```

Also correct the now-load-bearing comment on `requires_tag` in the same struct — it currently reads "informational only", which stops being true once installers act on it (spec A1):

```rust
    /// Capability tag (e.g. "nvidia-gpu") gating this plugin to matching
    /// hardware. Load-bearing: installers preselect tagged plugins only when
    /// the tag matches detected hardware.
    #[serde(default)]
    pub requires_tag: Option<String>,
```

- [ ] **Step 5: Write the verification helper**

Add to `crates/vox-cli/src/commands/plugin/install.rs`, above `install_from_url`:

```rust
/// Verify a downloaded plugin archive and return its lowercase hex SHA-256.
///
/// Fail-closed: with no `expected` hash this REFUSES unless `allow_unverified`
/// is set. The archive is `dlopen`'d as native code after installation, so an
/// unverified download is arbitrary code execution — see spec finding F9.
fn verify_plugin_archive(
    data: &[u8],
    expected: Option<&str>,
    allow_unverified: bool,
    source: &str,
) -> Result<String> {
    use sha2::{Digest, Sha256};
    let actual = hex::encode(Sha256::digest(data));

    match expected {
        Some(want) => {
            let want = want.trim().to_lowercase();
            if want != actual {
                bail!(
                    "plugin checksum mismatch for {source}\n  expected: {want}\n  actual:   {actual}"
                );
            }
            Ok(actual)
        }
        None if allow_unverified => {
            eprintln!(
                "⚠ Installing {source} with no sha256 to check against. \
                 Its contents will be loaded as native code. Actual sha256: {actual}"
            );
            Ok(actual)
        }
        None => bail!(
            "refusing to install {source}: no sha256 recorded for this plugin.\n  \
             Add a `sha256` to its entry in crates/vox-plugin-catalog/catalog.toml, \
             or pass --allow-unverified to accept the risk explicitly.\n  \
             Actual sha256 of the fetched archive: {actual}"
        ),
    }
}
```

- [ ] **Step 6: Thread it through the install paths**

Change `install_from_url`'s signature and insert the check between the download and the extraction:

```rust
async fn install_from_url(
    url: &str,
    yes: bool,
    expected_sha256: Option<&str>,
    allow_unverified: bool,
) -> Result<()> {
```

After `let bytes = … .context("reading response bytes")?;` and **before** the archive is written or extracted:

```rust
    // Verify BEFORE unpacking: the payload becomes a dlopen'd cdylib.
    verify_plugin_archive(&bytes, expected_sha256, allow_unverified, url)?;
```

In `install_from_catalog`, pass the catalog's hash through, and pin the version instead of the literal `latest`:

```rust
    } else if let Some(gh) = source.strip_prefix("github:") {
        // github:owner/repo → release asset URL, pinned to the catalog version.
        // A floating `latest` cannot be checksummed: the bytes behind it change.
        let triple = vox_plugin_host::current_target_triple_key();
        let version = entry
            .version
            .as_deref()
            .with_context(|| format!(
                "plugin '{id}' has a github: source but no pinned `version` in \
                 catalog.toml; an unpinned release asset cannot be checksummed"
            ))?;
        let url = format!(
            "https://github.com/{}/releases/download/v{}/{}-v{}-{}.zip",
            gh, version, id, version, triple
        );
        install_from_url(&url, yes, entry.sha256.as_deref(), allow_unverified).await
    } else {
```


Update `run`'s signature and its two call sites to thread `allow_unverified` through, and `install_from_catalog(plugin_id, yes)` becomes `install_from_catalog(plugin_id, yes, allow_unverified)`.

- [ ] **Step 7: Add the CLI flag**

In `crates/vox-cli/src/cli_args.rs`, find the `plugin install` subcommand definition and add:

```rust
        /// Install even when no sha256 is recorded for the plugin. The archive
        /// is loaded as native code — only use this for a source you trust.
        #[arg(long)]
        allow_unverified: bool,
```

Thread it to the `run(...)` call in the plugin dispatch.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: PASS, four tests.

- [ ] **Step 9: Confirm the catalog still builds and the refusal is real**

Run: `cargo build -p vox-plugin-catalog && cargo run -q -p vox-cli -- plugin install oratio --yes`

Expected: **non-zero exit** with `refusing to install …: no sha256 recorded for this plugin`. That refusal is this task's deliverable — before it, this command silently downloaded and installed native code.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-plugin-catalog/src/schema.rs crates/vox-cli/src/commands/plugin/install.rs crates/vox-cli/src/cli_args.rs crates/vox-cli/Cargo.toml
git commit -m "fix(plugin): refuse to install a plugin that cannot be verified

plugin install fetched a zip over HTTPS and extracted it with no checksum, no
signature, and an unpinned `latest` URL, then the host dlopen'd the cdylib —
anyone able to publish a release asset owned the user's process. Verification
is now fail-closed, with an explicit --allow-unverified override, and github:
sources must pin a version because a floating asset cannot be checksummed.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Detect post-install tampering at load time

Task 4 closes the network path. This closes the local one: nothing stops a process from replacing an installed `.so`/`.dll` after a verified install, and `Loader::load` checks only an ABI integer before executing it.

**Files:**
- Modify: `crates/vox-cli/src/commands/plugin/install.rs` (`install_from_path` writes the sidecar)
- Modify: `crates/vox-plugin-host/src/loader.rs` (verify before `load_from_file`)
- Modify: `crates/vox-plugin-host/src/errors.rs` (new `LoadError` variant)
- Modify: `crates/vox-plugin-host/Cargo.toml` (add `sha2`, `hex`)
- Test: `crates/vox-plugin-host/src/loader.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `verify_plugin_archive` from Task 4 (its returned hash is what gets recorded).
- Produces: `pub fn verify_recorded_hash(dylib_path: &Path) -> Result<(), LoadError>` in `vox_plugin_host::loader`, and the sidecar filename constant `INTEGRITY_FILE: &str = ".vox-integrity"`.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-plugin-host/src/loader.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(dir: &std::path::Path, body: &[u8], recorded: Option<&str>) -> std::path::PathBuf {
        let dylib = dir.join("plugin.so");
        std::fs::write(&dylib, body).expect("write dylib");
        if let Some(h) = recorded {
            std::fs::write(dir.join(INTEGRITY_FILE), format!("plugin.so {h}\n"))
                .expect("write integrity file");
        }
        dylib
    }

    fn hash_of(body: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(body))
    }

    #[test]
    fn matching_recorded_hash_verifies() {
        let dir = tempfile::tempdir().expect("tempdir");
        let body = b"plugin bytes";
        let p = write_plugin(dir.path(), body, Some(&hash_of(body)));
        verify_recorded_hash(&p).expect("matching hash must verify");
    }

    /// The point of this task: a dylib swapped after a verified install must be
    /// refused before dlopen, not executed.
    #[test]
    fn swapped_dylib_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = write_plugin(dir.path(), b"original bytes", Some(&hash_of(b"original bytes")));
        std::fs::write(&p, b"malicious replacement").expect("swap dylib");
        let err = verify_recorded_hash(&p).expect_err("swapped dylib must be refused");
        assert!(
            matches!(err, LoadError::IntegrityMismatch { .. }),
            "expected IntegrityMismatch, got: {err:?}"
        );
    }

    /// Plugins installed before this feature existed have no sidecar. Refusing
    /// them would break every existing install, so absence is permitted and the
    /// install path is what guarantees new installs get one.
    #[test]
    fn absent_integrity_file_is_permitted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = write_plugin(dir.path(), b"legacy plugin", None);
        verify_recorded_hash(&p).expect("a plugin with no sidecar must still load");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-plugin-host loader::tests -- --nocapture`

Expected: FAIL to compile — `cannot find value 'INTEGRITY_FILE'` and `cannot find function 'verify_recorded_hash'`.

- [ ] **Step 3: Add dependencies**

In `crates/vox-plugin-host/Cargo.toml` under `[dependencies]`:

```toml
sha2 = { workspace = true }
hex  = { workspace = true }
```

`tempfile` is already a dev-dependency of this crate (`crates/vox-plugin-host/Cargo.toml:31`), so no change is needed there. `sha2` and `hex` are external crates, not workspace edges.

- [ ] **Step 4: Add the error variant**

In `crates/vox-plugin-host/src/errors.rs`, add to `LoadError`:

```rust
    #[error("plugin dylib at {path:?} does not match its recorded hash (expected {expected}, found {actual}); refusing to load")]
    IntegrityMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
```

- [ ] **Step 5: Write the loader check**

In `crates/vox-plugin-host/src/loader.rs`, above `impl Loader`:

```rust
/// Sidecar recording the SHA-256 of each file installed alongside a plugin,
/// written by `vox plugin install`. Format: one `<filename> <hex-sha256>` per line.
pub const INTEGRITY_FILE: &str = ".vox-integrity";

/// Refuse a dylib whose on-disk hash differs from the one recorded at install.
///
/// Task 4 verifies the archive at download time; this closes the other half —
/// nothing otherwise stops a process from replacing the `.so`/`.dll` after a
/// verified install, and the only check before executing it is an ABI integer.
///
/// A missing sidecar is PERMITTED: plugins installed before this existed have
/// none, and refusing them would break every current install.
pub fn verify_recorded_hash(dylib_path: &Path) -> Result<(), LoadError> {
    let Some(dir) = dylib_path.parent() else {
        return Ok(());
    };
    let record_path = dir.join(INTEGRITY_FILE);
    let Ok(records) = std::fs::read_to_string(&record_path) else {
        return Ok(());
    };
    let Some(file_name) = dylib_path.file_name().and_then(|n| n.to_str()) else {
        return Ok(());
    };

    let expected = records.lines().find_map(|line| {
        let (name, hash) = line.trim().split_once(' ')?;
        (name == file_name).then(|| hash.trim().to_lowercase())
    });
    let Some(expected) = expected else {
        return Ok(());
    };

    let bytes = std::fs::read(dylib_path).map_err(|source| LoadError::Io {
        path: dylib_path.to_path_buf(),
        source,
    })?;
    use sha2::{Digest, Sha256};
    let actual = hex::encode(Sha256::digest(&bytes));

    if actual != expected {
        return Err(LoadError::IntegrityMismatch {
            path: dylib_path.to_path_buf(),
            expected,
            actual,
        });
    }
    Ok(())
}
```

Then call it as the **first** statement in `Loader::load`, before `VoxPluginRootRef::load_from_file`:

```rust
        let started = Instant::now();

        verify_recorded_hash(dylib_path).inspect_err(|_| {
            telemetry::load_failed(plugin_id, version, "integrity");
        })?;
```

- [ ] **Step 6: Write the sidecar at install time**

In `crates/vox-cli/src/commands/plugin/install.rs`, inside `install_from_path`'s copy loop, accumulate hashes and write the sidecar after the loop. Replace the copy loop and the success message with:

```rust
    // Copy all files from src_dir into dest, recording each file's SHA-256 so
    // the loader can detect a post-install swap (spec F9).
    use sha2::{Digest, Sha256};
    let mut copied = 0usize;
    let mut records = String::new();
    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let from = entry.path();
        if from.is_file() {
            let name = entry.file_name();
            // Never record the sidecar itself.
            if name == std::ffi::OsStr::new(vox_plugin_host::loader::INTEGRITY_FILE) {
                continue;
            }
            let to = dest.join(&name);
            std::fs::copy(&from, &to)
                .with_context(|| format!("copying {} -> {}", from.display(), to.display()))?;
            let bytes = std::fs::read(&to)
                .with_context(|| format!("hashing {}", to.display()))?;
            if let Some(n) = name.to_str() {
                records.push_str(&format!("{n} {}\n", hex::encode(Sha256::digest(&bytes))));
            }
            copied += 1;
        }
    }
    std::fs::write(dest.join(vox_plugin_host::loader::INTEGRITY_FILE), &records)
        .with_context(|| format!("writing integrity record in {}", dest.display()))?;
```

`vox-cli` already depends on `vox-plugin-host` (it calls `vox_plugin_host::workspace_local_plugin_source` and `current_target_triple_key`), so this adds no new crate edge. Confirm `loader` is publicly re-exported from `vox-plugin-host`'s `lib.rs`; if it is private, add `pub mod loader;` or re-export the two items.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p vox-plugin-host loader::tests -- --nocapture`

Expected: PASS, three tests.

- [ ] **Step 8: Confirm nothing else regressed**

Run: `cargo test -p vox-plugin-host && cargo test -p vox-cli --lib commands::plugin`

Expected: PASS. `crates/vox-cli-ci/src/plugin_abi_parity.rs:211` also calls `Loader::load` against workspace-built dylibs that have no sidecar — the "absent sidecar is permitted" rule keeps that green, which is exactly why that rule exists.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-plugin-host/ crates/vox-cli/src/commands/plugin/install.rs
git commit -m "feat(plugin): refuse to load a dylib swapped after install

Task 4 verifies the download; this closes the local half. plugin install now
records a SHA-256 per installed file and Loader::load refuses a dylib whose
on-disk hash differs, before dlopen. A missing sidecar is permitted so
existing installs and workspace-built plugins keep loading.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Verify the phase and open one PR

- [ ] **Step 1: Format**

Run: `cargo run -q -p vox-cli -- run scripts/fmt.vox`

Expected: exit 0. Never `cargo fmt --all`.

- [ ] **Step 2: Run the full local gate tier**

Run: `cargo run -q -p vox-cli -- ci pre-push --full`

Expected: exit 0. `--full` is required — this phase is almost entirely tests.

- [ ] **Step 3: Confirm no workspace crate edge was added**

Run: `cargo run -q -p vox-cli -- ci crate-edges`

Expected: exit 0. Tasks 4 and 5 add only external crates (`sha2`, `hex`, `tempfile`); the `vox-cli` → `vox-plugin-host` edge already existed.

- [ ] **Step 4: Confirm the plugin surface gates still pass**

Run: `cargo test -p vox-cli-ci plugin_ && cargo test -p vox-plugin-catalog`

Expected: exit 0. The catalog schema gained two optional fields; both are `#[serde(default)]`, so existing entries still parse.

- [ ] **Step 5: Push and open one review-ready PR**

```bash
git add -A
git commit -m "style: rustfmt after distribution security floor

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" || echo "nothing to commit"
git push -u origin claude/vox-distribution-system-f7e4c0
```

```bash
gh pr create --title "fix(security): close four fail-open holes in the distribution path" --body "$(cat <<'PRBODY'
Phase 1b of the Vox distribution system design — the security floor beneath
release signing.

- **`install.sh` failed open.** `verify_checksum` returned success when no hash
  tool was present, so minimal containers installed unverified binaries behind
  a warning that scrolls past in `curl | sh`. Now tries openssl, then aborts.
- **Tar extraction was unguarded on the platforms that use it.** The zip-slip
  guard and its test were both `#[cfg(windows)]`, but the archive is `.zip` only
  on Windows — every Linux and macOS user took a bare `archive.unpack()`, which
  skips escaping entries *silently*. The extraction root is one `..` from a
  directory `proxy.rs` prepends to `PATH`.
- **Release tokens were over-scoped.** Two workflows declared `contents: write`
  top-level; `release-installers.yml` declared no `permissions:` at all.
- **Plugins were `dlopen`'d with no verification.** `plugin install` fetched a
  zip from an unpinned `latest` URL and extracted it with no checksum. Now
  fail-closed with an explicit `--allow-unverified` override, plus an integrity
  sidecar so a post-install swap is refused before `dlopen`.

**Not in scope:** signing `checksums.txt` (spec F10) is blocked on a release key
held outside GitHub. None of the above substitutes for it — a same-origin
unsigned checksum detects corruption, not a compromised release.

Spec: `docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
PRBODY
)"
```

Do not re-push to trigger re-review — comment `@coderabbitai review`.

---

## What this phase deliberately does not do

- **Sign anything.** Blocked on external blocker 3 (a release key outside GitHub). Until then the chain's root property is GitHub's TLS and account security.
- **Populate the thirteen `github:` catalog hashes.** No plugin release assets exist yet. Task 4 makes their absence a *refusal* rather than a silent execution; recording real hashes happens when those releases are first cut, and Task 4's pinned-`version` requirement is what makes them checksummable.
- **Turn the permissions guard on in strict mode.** Task 3 Step 6 surveys the rest of the tree; wiring `run(root, true)` into `ssot-drift` waits until it is clean.
