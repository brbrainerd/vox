# Distribution Security Floor Implementation Plan (Phase 1b)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the fail-open holes on the path from a downloaded artifact to executing native code — a default install path that verifies nothing, an installer that skips its own integrity check, two unguarded archive extractors, and over-scoped release credentials.

**Architecture:** Seven tasks, ordered by what makes the others meaningful. Task 1 is first because `vox plugin install <id>` currently reaches `install_from_path` through a workspace-local fallback that bypasses every check — until that is closed, verification added elsewhere is not consulted on the default path. Tasks 2–5 are small independent hardenings. Task 6 adds catalog-anchored hashes and is deliberately **fail-closed rather than pre-populated**: no plugin release assets exist yet, so requiring hashes on all thirteen `github:` entries today would break `vox plugin install` outright.

**Tech Stack:** Rust 1.96.0, `sha2`, `tar`, `flate2`, `zip`, `serde_yaml`, POSIX `sh`.

**Spec:** [`docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`](../specs/2026-08-20-vox-distribution-system-design.md) (revision 4) — findings F9, F11, F12, F14; architecture A6.

## Branch

**Do not push to `claude/vox-distribution-system-f7e4c0`.** Phase 0+1a owns the PR on that branch, and `.coderabbit.yaml` sets `auto_incremental_review: false`, so commits pushed there land in an already-reviewed PR that will never be re-reviewed. Start from that head:

```bash
git checkout -b claude/vox-distribution-security-floor claude/vox-distribution-system-f7e4c0
```

Task 7 opens a PR with `--base claude/vox-distribution-system-f7e4c0`.

## Global Constraints

- Rust toolchain is pinned to **1.96.0**.
- **`vox` is not on PATH in this worktree.** Every invocation is `cargo run -q -p vox-cli -- <args>`.
- **Never run `cargo fmt --all`.** Use `cargo run -q -p vox-cli -- run scripts/fmt.vox`.
- **Verify with `vox ci pre-push --full`, not `--complete`** (`--complete` runs no tests).
- **Do not add a workspace crate-to-crate dependency edge.** Adding an *external* crate is not an edge. Task 6 Step 8 needs one and **stops for user authorization** — per AGENTS.md, `exceptions` entries are user-authorized only.
- **Assert on parsed structures, not source text.** `serde_yaml`, `toml`, and `serde_json` are all already available.
- **`sha2` and `hex` are already dependencies of `vox-cli`** — `crates/vox-cli/Cargo.toml:158` (`sha2.workspace = true`) and `:298` (`hex.workspace = true`), in dotted form. Do **not** add them; a duplicate key is a manifest parse failure for the whole workspace.
- **This plan does not sign anything.** Signing `checksums.txt` (spec F10) is blocked on a release key held outside GitHub. Everything below raises the floor beneath that; none of it substitutes for it.

## What each control does and does not achieve

State this honestly in review — two are narrower than they look:

| Task | Real property |
|---|---|
| 1 fallback opt-in | **Closes the only bypass of the default path.** Without it every claim below is false. |
| 2 install.sh | Mechanically fail-closed. Value still bounded by an unsigned, same-origin `checksums.txt`. |
| 3 tar hardening | `download::extract` has one caller, six lines after `verify_sha256`. Defends against a checksum-consistency bug, **not** an adversary — that adversary controls the release and would ship a malicious `vox` as the expected entry. Ten lines; land it, don't headline it. |
| 4 zip hardening | The one extraction path that ends in `dlopen`, and no checksum gates it. Genuinely exposed. |
| 5 permissions | Fixes `release-binaries.yml`. **Does not fix `release-gui.yml`** — its sole job both compiles the graph and uploads, so it needs write until that job is split. |
| 6 catalog hashes | Collapses thirteen independent `github:` trust roots into the one root you already trust to run `vox`. The `Cargo.lock` argument, and it holds even unsigned. |

---

### Task 1: Close the workspace-local fallback bypass

`install_from_catalog` (`crates/vox-cli/src/commands/plugin/install.rs:158-171`) calls `install_from_path` **directly**, skipping every check, and it is **on by default** — `VOX_NO_LOCAL_PLUGIN_FALLBACK` is opt-*out*. `vox_plugin_host::workspace_local_plugin_source` (`crates/vox-plugin-host/src/lib.rs:89-116`) resolves from `$VOX_WORKSPACE_ROOT`, else **walks up eight levels from the current working directory** looking for `crates/vox-plugin-<id>/Plugin.toml`.

```text
/tmp/x/crates/vox-plugin-oratio/{Plugin.toml, liboratio.so}
cd /tmp/x/anything && vox plugin install oratio     # installs the attacker's cdylib
```

No privilege beyond writing a directory the user later enters. This is the `.`-in-`PATH` bug class, and it is why this task is first.

**Files:**
- Modify: `crates/vox-cli/src/commands/plugin/install.rs:158-171`
- Test: `crates/vox-cli/src/commands/plugin/install.rs` (new `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub(crate) const LOCAL_FALLBACK_ENV: &str = "VOX_LOCAL_PLUGIN_FALLBACK";` and `fn local_fallback_enabled() -> bool`.

- [ ] **Step 1: Confirm the current behaviour**

Run: `sed -n '150,175p' crates/vox-cli/src/commands/plugin/install.rs`

Expected: the fallback block, with `VOX_NO_LOCAL_PLUGIN_FALLBACK` read as an opt-*out*, and the "installing from there" message printed *before* that check.

- [ ] **Step 2: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises every test that mutates `LOCAL_FALLBACK_ENV`.
    ///
    /// `std::env::set_var`/`remove_var` change process-global state, and cargo
    /// runs the tests in this binary in parallel — without this, Task 6's
    /// `catalog_install_refuses_an_unpinned_entry_before_downloading` (which
    /// removes the var) races the two tests below (which set and remove it),
    /// and all three are intermittently wrong. Recover from poisoning rather
    /// than cascading a panic into unrelated tests.
    pub(super) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The workspace-local fallback must be OPT-IN. As an opt-out it let any
    /// directory the user happened to be inside supply a cdylib for a catalog
    /// plugin id, bypassing every integrity check — the `.`-in-PATH bug class.
    #[test]
    fn local_fallback_is_opt_in_not_opt_out() {
        let src = include_str!("install.rs");
        // Split so this needle cannot match the assertion message below.
        let opt_out = concat!("VOX_NO_", "LOCAL_PLUGIN_FALLBACK");
        assert!(
            !src.contains(opt_out),
            "the workspace-local plugin fallback is still opt-out; it must require \
             {LOCAL_FALLBACK_ENV} to be set before it can bypass verification"
        );
        assert_eq!(LOCAL_FALLBACK_ENV, "VOX_LOCAL_PLUGIN_FALLBACK");
    }

    #[test]
    fn local_fallback_disabled_when_env_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_LOCK serialises every mutator of this variable.
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
        assert!(!local_fallback_enabled(), "fallback must be off unless explicitly enabled");
    }

    #[test]
    fn local_fallback_enabled_when_env_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_LOCK serialises every mutator of this variable.
        unsafe { std::env::set_var(LOCAL_FALLBACK_ENV, "1") };
        assert!(local_fallback_enabled());
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: FAIL to compile — `cannot find value 'LOCAL_FALLBACK_ENV'`.

- [ ] **Step 4: Write the implementation**

Above `install_from_catalog`:

```rust
/// Opt-in switch for the workspace-local plugin source.
///
/// This was an opt-out (`VOX_NO_LOCAL_PLUGIN_FALLBACK`). Because
/// `workspace_local_plugin_source` walks up eight levels from the CURRENT
/// WORKING DIRECTORY, an opt-out default meant any directory the user happened
/// to be inside could supply a cdylib for a catalog plugin id, bypassing every
/// integrity check. Contributors who want the local source set this explicitly;
/// `--path <dir>` remains the documented alternative.
pub(crate) const LOCAL_FALLBACK_ENV: &str = "VOX_LOCAL_PLUGIN_FALLBACK";

fn local_fallback_enabled() -> bool {
    matches!(
        std::env::var(LOCAL_FALLBACK_ENV).as_deref(),
        Ok("1") | Ok("true")
    )
}
```

Replace the fallback block so the check precedes the message:

```rust
    if !source.starts_with("local:") && local_fallback_enabled() {
        if let Some(local) = vox_plugin_host::workspace_local_plugin_source(id) {
            println!(
                "ℹ {}=1 — installing plugin '{}' from the local workspace source at {} \
                 instead of the catalog default ('{}'). This path performs NO \
                 integrity verification.",
                LOCAL_FALLBACK_ENV,
                id,
                local.display(),
                source
            );
            return install_from_path(&local, yes);
        }
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: PASS, three tests.

- [ ] **Step 6: Update anything documenting the old name**

Run: `grep -rn "VOX_NO_LOCAL_PLUGIN_FALLBACK\|workspace_local_plugin_source" --include=*.rs --include=*.md --include=*.vox --include=*.yml . | grep -v "^./docs/superpowers"`

Expected: hits in `crates/vox-plugin-host/src/lib.rs` (the resolver) and any docs. Update every doc mention to the opt-in name and semantics.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/plugin/install.rs
git commit -m "fix(plugin): make the workspace-local install fallback opt-in

install_from_catalog called install_from_path directly through a fallback that
was on by default, and workspace_local_plugin_source walks up eight levels from
the CURRENT WORKING DIRECTORY. Any directory the user happened to be inside
could supply a cdylib for a catalog plugin id, bypassing every check — the
`.`-in-PATH bug class. It now requires VOX_LOCAL_PLUGIN_FALLBACK=1.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Stop `install.sh` from installing unverified bytes

`verify_checksum` returns success when no hash tool is present, so on a minimal container the installer warns and installs anyway. In a `curl | sh` pipeline that warning scrolls past.

**Files:**
- Modify: `scripts/install.sh:50-68`
- Test: `crates/vox-cli/src/commands/ci/release_build.rs` (existing `#[cfg(test)] mod tests`)

**Interfaces:** none.

- [ ] **Step 1: Write the failing test**

Execute the real function under `sh` with every hash tool shadowed — that is exactly the container scenario. A source-text assertion is defeated by rewording and breaks if anyone documents the removed branch in a comment.

```rust
/// `install.sh` must abort, not continue, when no SHA-256 tool exists. Executes
/// the real `verify_checksum` with every hash tool reported missing.
#[cfg(unix)]
#[test]
fn install_sh_aborts_when_no_hash_tool_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sh = std::fs::read_to_string(root.join("scripts/install.sh")).expect("read install.sh");

    // Source the helpers without running main(), then shadow `command -v` so
    // every hash tool reports missing.
    let harness = format!(
        "{}\n\
         command() {{ if [ \"$1\" = \"-v\" ]; then case \"$2\" in \
           sha256sum|shasum|openssl) return 1;; esac; fi; builtin command \"$@\"; }}\n\
         verify_checksum /dev/null deadbeef\n\
         echo REACHED_INSTALL\n",
        sh.replace("main \"$@\"", "")
    );

    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&harness)
        .output()
        .expect("spawn sh");

    assert!(
        !out.status.success(),
        "verify_checksum returned success with no hash tool available"
    );
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("REACHED_INSTALL"),
        "install.sh continued past an unverifiable checksum"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli --lib install_sh_aborts_when_no_hash_tool_exists -- --nocapture`

Expected: FAIL — the current fail-open branch returns 0 and `REACHED_INSTALL` prints. This test is `#[cfg(unix)]`; **verify this task on Linux, macOS, or WSL**, not on a Windows host.

- [ ] **Step 3: Write the implementation**

```sh
verify_checksum() {
    _file="$1"
    _expected="$2"

    # Fail CLOSED. A missing hashing tool must abort, never downgrade to
    # installing unverified bytes — this runs inside `curl | sh`, where a
    # printed warning scrolls past unread.
    if command -v sha256sum >/dev/null 2>&1; then
        _actual="$(sha256sum "$_file" | cut -d ' ' -f1)"
    elif command -v shasum >/dev/null 2>&1; then
        _actual="$(shasum -a 256 "$_file" | cut -d ' ' -f1)"
    elif command -v openssl >/dev/null 2>&1; then
        # OpenSSL 3.x prints "SHA2-256(f)= <hex>", 1.x "SHA256(f)= <hex>",
        # LibreSSL "SHA256 (f) = <hex>". The hex is the last field in all three.
        _actual="$(openssl dgst -sha256 "$_file" | awk '{print $NF}')"
    else
        err "no SHA-256 tool found (need one of: sha256sum, shasum, openssl)."
    fi

    if [ "$_actual" != "$_expected" ]; then
        err "SHA-256 mismatch for $_file (expected $_expected, got $_actual)"
    fi
    say "Checksum OK"
}
```

`err` is `say "error: $*"; exit 1`, and `say` uses `printf "voxup: %s\n"`, which does **not** interpret `\n` — so these messages are deliberately single-line, unlike the version they replace, whose embedded `\n` printed literally.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli --lib install_sh_aborts_when_no_hash_tool_exists -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Confirm the pre-existing script gates still pass**

```bash
sh -n scripts/install.sh && echo "POSIX syntax OK"
```

```bash
cargo test -p vox-cli --lib ci::release_build
```

The second matters: `install_scripts_cover_release_targets` (`release_build.rs:287-307`) requires every `SUPPORTED_RELEASE_TARGETS` triple to remain a literal string somewhere in `scripts/install.sh`. The rewrite must not drop any.

- [ ] **Step 6: Sync the published copy**

Phase 0 Task 1 added `docs-astro/public/voxup` as a byte-identical copy, gated by `documented_install_urls_are_served`:

```bash
cp scripts/install.sh docs-astro/public/voxup
cargo test -p vox-cli --lib documented_install_urls_are_served
```

- [ ] **Step 7: Commit**

```bash
git add scripts/install.sh docs-astro/public/voxup crates/vox-cli/src/commands/ci/release_build.rs
git commit -m "fix(install): fail closed when no SHA-256 tool is available

verify_checksum returned success when neither sha256sum nor shasum existed, so
minimal containers installed an unverified binary behind a warning that scrolls
past in a curl|sh pipeline. Adds an openssl fallback, then aborts. The test
executes the real function under sh with every hash tool shadowed.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Guard tar extraction, and make extraction atomic

`extract_zip` validates `enclosed_name()` and `starts_with(dest_dir)` and has a regression test — but both are `#[cfg(windows)]`, and the archive extension is `.zip` only on Windows. `extract_targz` is a bare `archive.unpack(dest_dir)`, and it is the path every Linux and macOS user takes.

**Files:**
- Modify: `crates/voxup/src/download.rs:64-74`
- Modify: `crates/voxup/src/install.rs:97-98`
- Test: `crates/voxup/src/download.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `extract_targz` keeps its signature; `pub(crate) const MAX_UNCOMPRESSED_BYTES: u64`.

- [ ] **Step 1: Write the failing test**

The safe `tar` API **cannot build a traversal fixture**: `Builder::append_data` → `Header::set_path` rejects `..` outright (*"paths in archives must not have `..`"*), and the GNU-longname fallback only engages at ≥100 bytes. The fixture must stamp the raw GNU name field, exactly as a real attacker would.

```rust
/// Build a gzipped tar whose single entry carries `name` verbatim.
///
/// Uses the raw GNU header rather than `append_data`, because tar-rs's
/// `Header::set_path` REFUSES `..` — the safe API cannot express the attack
/// this test exists to catch.
#[cfg(unix)]
fn targz_with_raw_name(name: &[u8], contents: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        {
            let gnu = header.as_gnu_mut().expect("gnu header");
            assert!(name.len() < gnu.name.len(), "fixture name too long");
            gnu.name[..name.len()].copy_from_slice(name);
        }
        header.set_cksum();
        builder.append(&header, contents).expect("append raw entry");
        builder.finish().expect("finish tar");
    }
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&tar_bytes).expect("gzip write");
    gz.finish().expect("gzip finish")
}

/// Guards the fixture itself: if tar-rs ever normalises the raw name, the
/// traversal test would silently start asserting nothing.
#[cfg(unix)]
#[test]
fn traversal_fixture_really_contains_an_escaping_entry() {
    let data = targz_with_raw_name(b"../escaped.txt", b"pwned");
    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(Cursor::new(&data)));
    let paths: Vec<String> = ar
        .entries()
        .expect("entries")
        .map(|e| e.expect("entry").path().expect("path").display().to_string())
        .collect();
    assert_eq!(paths, vec!["../escaped.txt".to_string()], "fixture no longer escapes");
}

#[cfg(unix)]
#[test]
fn extract_targz_rejects_path_traversal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data = targz_with_raw_name(b"../escaped.txt", b"pwned");
    let err = extract_targz(&data, dir.path()).expect_err("must reject escaping entry");
    assert!(
        err.to_string().contains("escapes destination"),
        "expected a traversal rejection, got: {err}"
    );
    assert!(
        !dir.path().parent().unwrap().join("escaped.txt").exists(),
        "escaping entry was written outside the destination"
    );
}

#[cfg(unix)]
#[test]
fn extract_targz_rejects_symlink_entries() {
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
        header.set_link_name("/etc/passwd").expect("set link name");
        header.set_cksum();
        builder.append_data(&mut header, "link", &[][..]).expect("append symlink");
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

/// tar-rs bounds each entry's reader with `io::Take` at the header-declared
/// size, so a lying header can only UNDERSTATE — which is why checking the
/// declared size before unpacking is a real upper bound, not advisory.
#[cfg(unix)]
#[test]
fn extract_targz_rejects_an_oversized_archive() {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    let mut tar_bytes = Vec::new();
    {
        let mut b = tar::Builder::new(&mut tar_bytes);
        let mut h = tar::Header::new_gnu();
        h.set_size(MAX_UNCOMPRESSED_BYTES + 1);
        h.set_mode(0o644);
        h.set_entry_type(tar::EntryType::Regular);
        h.as_gnu_mut().unwrap().name[..3].copy_from_slice(b"big");
        h.set_cksum();
        b.append(&h, &[][..]).expect("append");
        b.finish().expect("finish");
    }
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&tar_bytes).expect("gzip write");
    let data = gz.finish().expect("gzip finish");

    let dir = tempfile::tempdir().expect("tempdir");
    let err = extract_targz(&data, dir.path()).expect_err("must reject oversized archive");
    assert!(err.to_string().contains("expands beyond"), "got: {err}");
}
```

Do **not** add a "normal entry extracts" test — the pre-existing `extract_targz_round_trip` (`download.rs:169`) already covers the happy path and must keep passing unchanged.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p voxup --lib download::tests -- --nocapture`

Expected: FAIL — traversal and symlink entries are silently skipped or written today, and `MAX_UNCOMPRESSED_BYTES` does not exist.

- [ ] **Step 3: Write the implementation**

```rust
/// Maximum total uncompressed bytes from one archive (512 MiB). tar-rs bounds
/// each entry's reader with `io::Take` at the header-declared size, so a lying
/// header can only understate — checking the declared size before unpacking is
/// therefore an upper bound on bytes written, not merely advisory.
pub(crate) const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum entries in one archive.
const MAX_ENTRIES: usize = 10_000;

fn extract_targz(data: &[u8], dest_dir: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::{Archive, EntryType};

    // Explicit entry loop rather than `archive.unpack()`. `unpack` SILENTLY
    // SKIPS escaping entries, so a tampered archive surfaces as "Extraction
    // succeeded but 'vox' not found" rather than a security error. It also
    // writes symlinks. Real Vox archives contain exactly one regular file
    // (release_artifacts::package_tar_gz calls append_path_with_name once), so
    // this allowlist is non-breaking.
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

        let ty = entry.header().entry_type();
        // A pax global-extension record is metadata, not a file; skip rather
        // than fail, so a bsdtar-produced archive still extracts.
        if ty == EntryType::XGlobalHeader {
            continue;
        }
        if !(ty.is_file() || ty.is_dir()) {
            bail!(
                "unsupported entry type {:?} in archive entry {:?}; only regular \
                 files and directories are allowed",
                ty,
                entry.path().map(|p| p.display().to_string())
            );
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

        let declared = entry.header().size().context("read tar entry size")?;
        total_bytes = total_bytes.saturating_add(declared);
        if total_bytes > MAX_UNCOMPRESSED_BYTES {
            bail!("archive expands beyond {MAX_UNCOMPRESSED_BYTES} bytes; refusing to extract");
        }

        if ty.is_dir() {
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

`bail!`, `fs`, `Cursor`, and `info!` are already imported at `download.rs:3-10`; no `Cargo.toml` edit is needed (`tempfile` is already a dev-dependency at `crates/voxup/Cargo.toml:42`).

- [ ] **Step 4: Make extraction atomic**

A mid-loop `bail!` currently leaves a half-populated `~/.vox/toolchains/vox-<ver>/` that survives to the next run. Read the surrounding lines first and match the existing variable names, then extract into a sibling staging dir and rename on success:

```rust
    // Extract to a sibling staging dir, then rename. A failed extraction must
    // not leave a partially-populated version dir behind for the next run.
    let staging = tc_dir.with_extension("incoming");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .with_context(|| format!("create staging dir {}", staging.display()))?;
    crate::download::extract(&archive_bytes, &asset_name, &staging)?;
    let _ = std::fs::remove_dir_all(&tc_dir);
    std::fs::rename(&staging, &tc_dir)
        .with_context(|| format!("promote {} -> {}", staging.display(), tc_dir.display()))?;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p voxup -- --nocapture`

Expected: PASS, including the pre-existing `extract_targz_round_trip`, which must not regress.

- [ ] **Step 6: Commit**

```bash
git add crates/voxup/src/download.rs crates/voxup/src/install.rs
git commit -m "fix(voxup): validate tar entries and make extraction atomic

extract_zip guarded against zip-slip but was #[cfg(windows)], so the tar path
every Linux and macOS user takes had no guard. archive.unpack() skips escaping
entries silently and writes symlinks; the extraction root is one `..` from a
directory proxy.rs prepends to PATH. Adds entry validation, size and count
caps, and extract-to-staging-then-rename. The traversal fixture stamps the raw
GNU header because tar-rs's set_path refuses `..`.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Harden the zip extraction that ends in `dlopen`

Task 3 hardened `voxup`'s tar extractor, which sits six lines after a `verify_sha256`. The **plugin** zip extractor has no checksum in front of it and its output is `dlopen`'d. `zip::ZipArchive::extract` creates symlinks and applies no size cap.

**Files:**
- Modify: `crates/vox-cli/src/commands/plugin/install.rs:129-135`
- Test: same file's `#[cfg(test)] mod tests` (created in Task 1)

**Interfaces:**
- Consumes: the test module from Task 1.
- Produces: `fn extract_plugin_zip(data: &[u8], dest: &Path) -> Result<()>`.

- [ ] **Step 1: Confirm `zip` is available**

Run: `grep -nE '^zip[. =]' crates/vox-cli/Cargo.toml`

Expected: `zip = { features = ["deflate"], version = "2" }` at `:280`, a normal dependency. **This step is a drift guard, not an edit** — adding a second `zip` key would be a duplicate-key manifest failure for the whole workspace.

- [ ] **Step 2: Write the failing test**

```rust
    /// Plugin archives are extracted and then dlopen'd. An escaping entry must
    /// be refused, not materialised.
    #[test]
    fn extract_plugin_zip_rejects_escaping_entries() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            w.start_file("../escaped.txt", opts).expect("start file");
            w.write_all(b"pwned").expect("write");
            w.finish().expect("finish");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let err = extract_plugin_zip(&buf, dir.path()).expect_err("must reject escaping entry");
        assert!(err.to_string().contains("escapes destination"), "got: {err}");
        assert!(!dir.path().parent().unwrap().join("escaped.txt").exists());
    }

    #[test]
    fn extract_plugin_zip_accepts_a_normal_entry() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            w.start_file("Plugin.toml", opts).expect("start file");
            w.write_all(b"[plugin]\n").expect("write");
            w.finish().expect("finish");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        extract_plugin_zip(&buf, dir.path()).expect("normal entry must extract");
        assert!(dir.path().join("Plugin.toml").is_file());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: FAIL to compile — `extract_plugin_zip` undefined.

- [ ] **Step 4: Write the implementation**

```rust
/// Extract a plugin archive, refusing anything that escapes `dest` or is not a
/// plain file or directory.
///
/// `ZipArchive::extract` materialises symlinks and applies no size cap. This is
/// the one extraction path in the codebase whose output is `dlopen`'d, and
/// unlike voxup's tar path it has no checksum gate in front of it.
fn extract_plugin_zip(data: &[u8], dest: &Path) -> Result<()> {
    const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
    const MAX_ENTRIES: usize = 10_000;

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).context("open plugin zip")?;
    if archive.len() > MAX_ENTRIES {
        bail!("plugin archive has more than {MAX_ENTRIES} entries; refusing to extract");
    }

    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("read zip entry")?;
        let enclosed = entry
            .enclosed_name()
            .with_context(|| format!("entry {:?} escapes destination", entry.name()))?;
        let outpath = dest.join(&enclosed);
        if !outpath.starts_with(dest) {
            bail!("entry {:?} escapes destination", entry.name());
        }

        total = total.saturating_add(entry.size());
        if total > MAX_UNCOMPRESSED_BYTES {
            bail!("plugin archive expands beyond {MAX_UNCOMPRESSED_BYTES} bytes");
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)
                .with_context(|| format!("create dir {}", outpath.display()))?;
            continue;
        }
        // Anything that is neither a plain file nor a directory is refused
        // rather than materialised.
        if entry
            .unix_mode()
            .is_some_and(|m| m & 0o170000 == 0o120000)
        {
            bail!("plugin archive contains a symlink entry {:?}; refusing", entry.name());
        }
        if let Some(parent) = outpath.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let mut out = std::fs::File::create(&outpath)
            .with_context(|| format!("create {}", outpath.display()))?;
        std::io::copy(&mut entry, &mut out)
            .with_context(|| format!("write {}", outpath.display()))?;
    }
    Ok(())
}
```

Replace the `zip::ZipArchive::new(...)` / `archive.extract(&tmp_base)` pair in `install_from_url` with `extract_plugin_zip(&bytes, &tmp_base)?`, and drop the now-unneeded intermediate `zip_path` write.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/plugin/install.rs crates/vox-cli/Cargo.toml
git commit -m "fix(plugin): guard the zip extraction whose output is dlopen'd

ZipArchive::extract materialises symlinks and has no size cap. This is the one
extraction path in the codebase that ends in dlopen, and unlike voxup's tar
path no checksum gates it.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Scope release credentials per job

**Files:**
- Modify: `.github/workflows/release-{binaries,gui,installers}.yml`
- Create: `crates/vox-cli-ci/src/workflow_permissions_guard.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn top_level_permissions(yml: &str) -> Option<serde_yaml::Value>` and `pub fn run(root: &Path, strict: bool) -> Result<()>`.

- [ ] **Step 1: Write the failing test**

Parse the document. A substring check passes `permissions: write-all`, and a `contains("contents: read")` assertion is satisfied by a *job-level* read block in an otherwise write-all workflow — precisely the state being fixed.

```rust
//! Gate: every workflow declares an explicit top-level `permissions:` block.
//!
//! Without one a workflow inherits the repository default token scope. If that
//! default is the legacy "read and write all scopes", every job — including ones
//! compiling 1600+ third-party crates — carries a fully privileged token.

use anyhow::{Result, bail};
use std::path::Path;

/// The top-level `permissions:` value, or `None` when absent.
pub fn top_level_permissions(yml: &str) -> Option<serde_yaml::Value> {
    let v: serde_yaml::Value = serde_yaml::from_str(yml).ok()?;
    let p = v.get("permissions")?;
    (!p.is_null()).then(|| p.clone())
}

/// Check every workflow. In `strict` mode a missing block is an error.
pub fn run(root: &Path, strict: bool) -> Result<()> {
    let dir = root.join(".github/workflows");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("yml") && ext != Some("yaml") {
            continue;
        }
        if top_level_permissions(&std::fs::read_to_string(&path)?).is_none() {
            offenders.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    offenders.sort();
    if !offenders.is_empty() {
        let list = offenders.join(", ");
        if strict {
            bail!("workflows without an explicit top-level `permissions:` block: {list}");
        }
        eprintln!("warning: workflows without `permissions:`: {list}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_top_level_block() {
        assert!(top_level_permissions("on:\n  push:\npermissions:\n  contents: read\n").is_some());
    }

    #[test]
    fn a_job_level_block_does_not_count() {
        let yml = "jobs:\n  build:\n    permissions:\n      contents: read\n";
        assert!(top_level_permissions(yml).is_none());
    }

    /// The real assertion: top-level defaults to read, and `contents: write`
    /// appears on the publishing job and nowhere else.
    #[test]
    fn release_workflows_grant_write_only_where_needed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // release-gui's sole job both builds and uploads, so it needs write
        // until that job is split — see the plan's honesty table.
        for (wf, writer) in [
            ("release-binaries.yml", Some("publish")),
            ("release-gui.yml", Some("build-tauri")),
            ("release-installers.yml", None),
        ] {
            let text = std::fs::read_to_string(root.join(".github/workflows").join(wf))
                .unwrap_or_else(|e| panic!("read {wf}: {e}"));
            let v: serde_yaml::Value = serde_yaml::from_str(&text).expect("valid YAML");

            assert_eq!(
                v["permissions"]["contents"].as_str(),
                Some("read"),
                "{wf} top-level contents must be read"
            );
            for (name, job) in v["jobs"].as_mapping().expect("jobs mapping") {
                let writes = job["permissions"]["contents"].as_str() == Some("write");
                let should = writer == name.as_str();
                assert_eq!(
                    writes, should,
                    "{wf} job {name:?}: contents:write must appear on {writer:?} and nowhere else"
                );
            }
        }
    }
}
```

- [ ] **Step 2: Register the module**

Add `pub mod workflow_permissions_guard;` to `crates/vox-cli-ci/src/lib.rs`, beside `pub mod workflow_concurrency_guard;`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-cli-ci workflow_permissions_guard -- --nocapture`

Expected: FAIL on `release_workflows_grant_write_only_where_needed`.

- [ ] **Step 4: Write the implementation**

Set the top-level block in all three workflows to `permissions:\n  contents: read`. Add a job-level `permissions:\n      contents: write` to `release-binaries.yml`'s `publish` job and `release-gui.yml`'s `build-tauri` job. `release-installers.yml` gets no job-level write — none of its six jobs writes to the release.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-cli-ci workflow_permissions_guard -- --nocapture`

Expected: PASS, three tests.

- [ ] **Step 6: Record what strict mode would flag**

Run: `for f in .github/workflows/*.yml; do grep -q '^permissions:' "$f" || echo "MISSING: $f"; done`

Expected: **20 files**. Leaving `run(root, true)` unwired is therefore correct for now; note the count in the commit message so the follow-up is scoped. `vox-cli-ci` is already layer 3 and this adds a module rather than a crate, so no layers or `crate-edges` change is owed.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/ crates/vox-cli-ci/src/workflow_permissions_guard.rs crates/vox-cli-ci/src/lib.rs
git commit -m "fix(ci): scope release token permissions per job

release-binaries.yml and release-gui.yml declared contents:write top-level, so
build jobs compiling 1600+ third-party crates held a write token.
release-installers.yml declared no permissions block and inherited the repo
default. The guard parses the document rather than scanning it, because a
substring check passes `permissions: write-all`. Strict mode stays unwired: 20
of 45 workflows still lack a block.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Refuse to install a plugin that cannot be verified

**Files:**
- Modify: `crates/vox-plugin-catalog/src/schema.rs`
- Modify: `crates/vox-cli/src/commands/plugin/install.rs`, `crates/vox-cli/src/commands/plugin/mod.rs:25-36` and `:73-75`
- Modify: `crates/vox-cli/src/commands/plugin_bundle/apply.rs:29`
- Modify: `docs/src/reference/cli.md`

**Interfaces:**
- Consumes: `LOCAL_FALLBACK_ENV` (Task 1) — without it this task's guarantee is bypassable; `extract_plugin_zip` (Task 4).
- Produces: `PluginCatalogEntry.{sha256, version, artifacts_sha256}` and `fn verify_plugin_archive(data: &[u8], expected: Option<&str>, allow_unverified: bool, source: &str) -> Result<String>`.

- [ ] **Step 1: Write the failing test**

```rust
    const PAYLOAD: &[u8] = b"pretend this is a plugin zip";

    fn payload_hash() -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(PAYLOAD))
    }

    #[test]
    fn matching_hash_is_accepted_and_returned() {
        let want = payload_hash();
        let got = verify_plugin_archive(PAYLOAD, Some(&want), false, "test://x").unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn mismatched_hash_is_rejected() {
        let err = verify_plugin_archive(PAYLOAD, Some(&"a".repeat(64)), false, "test://x")
            .expect_err("mismatched hash must fail");
        assert!(err.to_string().contains("checksum mismatch"), "got: {err}");
    }

    /// The core property: with no expected hash, installation is REFUSED.
    #[test]
    fn missing_hash_is_refused_by_default() {
        let err = verify_plugin_archive(PAYLOAD, None, false, "https://example/p.zip")
            .expect_err("an unverifiable plugin must not install");
        let m = err.to_string();
        assert!(m.contains("no sha256"), "error must say why: {m}");
        assert!(m.contains("--allow-unverified"), "error must name the override: {m}");
    }

    #[test]
    fn missing_hash_is_allowed_with_the_explicit_override() {
        let got = verify_plugin_archive(PAYLOAD, None, true, "https://example/p.zip").unwrap();
        assert_eq!(got, payload_hash());
    }

    /// A catalog install must fail BEFORE any network call when the entry is
    /// unpinned — an unpinned `latest` asset cannot be checksummed at all.
    #[tokio::test]
    async fn catalog_install_refuses_an_unpinned_entry_before_downloading() {
        // ENV_LOCK (defined in Task 1's test module) serialises this against the
        // fallback tests, which set and remove the same process-global variable.
        // `#[tokio::test]` is single-threaded, so holding the guard across the
        // await is sound.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: ENV_LOCK serialises every mutator of this variable.
        unsafe { std::env::remove_var(LOCAL_FALLBACK_ENV) };
        let err = install_from_catalog("oratio", true, false)
            .await
            .expect_err("unpinned catalog entry must not install");
        let m = err.to_string();
        assert!(
            m.contains("no pinned `version`") || m.contains("no sha256"),
            "expected a pre-network refusal, got: {m}"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: FAIL to compile — `verify_plugin_archive` undefined.

- [ ] **Step 3: Add the catalog fields**

In `crates/vox-plugin-catalog/src/schema.rs`, inside `PluginCatalogEntry`, after `requires_tag`:

```rust
    /// SHA-256 (lowercase hex) of the published plugin ARCHIVE.
    ///
    /// Checked by `vox plugin install` before extraction. Absent for `local:`
    /// sources, which are built from already-trusted workspace source.
    #[serde(default)]
    pub sha256: Option<String>,

    /// Release version for `github:` sources, without a leading `v`.
    ///
    /// Required alongside `sha256`: the previous code built a
    /// `releases/latest/download/...` URL, and the bytes behind a floating
    /// `latest` change, so no recorded hash could ever match it.
    #[serde(default)]
    pub version: Option<String>,

    /// SHA-256 per target triple of the installed DYLIB, keyed like `artifacts`
    /// in `Plugin.toml`.
    ///
    /// Distinct from `sha256` and NOT derivable from it: one covers the archive,
    /// the other the file that gets `dlopen`'d. Consumed by the load-time check,
    /// which is gated on the crate-edge authorization in Step 8.
    #[serde(default)]
    pub artifacts_sha256: Option<std::collections::BTreeMap<String, String>>,
```

Also correct the now-load-bearing comment on `requires_tag` — it reads "informational only", which stops being true once installers act on it (spec A1):

```rust
    /// Capability tag (e.g. "nvidia-gpu") gating this plugin to matching
    /// hardware. Load-bearing: installers preselect tagged plugins only when the
    /// tag matches detected hardware.
```

- [ ] **Step 4: Write the verification helper**

```rust
/// Verify a downloaded plugin archive and return its lowercase hex SHA-256.
///
/// Fail-closed: with no `expected` hash this REFUSES unless `allow_unverified`.
/// The archive is `dlopen`'d as native code after installation, so an unverified
/// download is arbitrary code execution — see spec finding F9.
///
// vox:defactored-from voxup 2026-08-21 (voxup::download::verify_sha256, ~10 lines)
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
                "⚠ Installing {source} with no sha256 to check against. Its contents \
                 will be loaded as native code. Actual sha256: {actual}"
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

**No env-var bypass.** The workspace already has one integrity bypass of exactly the wrong shape (`VOX_PM_ALLOW_GIT_UNVERIFIED`, `crates/vox-cli/src/commands/pm/verify.rs:71-81`); do not add a second. The flag is explicit-only.

- [ ] **Step 5: Thread it through both install paths**

Change `install_from_url` to
`async fn install_from_url(url: &str, yes: bool, expected_sha256: Option<&str>, allow_unverified: bool) -> Result<()>`,
and call `verify_plugin_archive(&bytes, expected_sha256, allow_unverified, url)?` **after** `.bytes()` and **before** `extract_plugin_zip` (Task 4).

In `install_from_catalog`, pin the version and pass the hash:

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

Update `run`'s signature to take `allow_unverified: bool` and thread it to both branches.

- [ ] **Step 6: Update BOTH callers**

There are exactly two, and the second is easy to miss:

1. `crates/vox-cli/src/commands/plugin/mod.rs:73-75` — the dispatch. Add the field to the `Install` **struct variant** at `:25-36` (it lives here, **not** in `cli_args.rs`):

```rust
        /// Install even when no sha256 is recorded for the plugin. The archive is
        /// loaded as native code — only use this for a source you trust.
        #[arg(long)]
        allow_unverified: bool,
    },
```
```rust
        PluginCmd::Install { id, path, url, yes, allow_unverified } => {
            install::run(id.as_deref(), path.as_deref(), url.as_deref(), yes, allow_unverified).await
        }
```

2. `crates/vox-cli/src/commands/plugin_bundle/apply.rs:29` — `install::run(Some(&p.id), None, None, yes)` becomes `install::run(Some(&p.id), None, None, yes, false)`. **It must pass `false`.** This means `vox plugin-bundle apply` will start refusing every `github:` plugin until hashes exist — intended, and it belongs in the PR body.

Document `--allow-unverified` in the `vox plugin install` section of `docs/src/reference/cli.md`. No generated artifact is affected: `command-sync` is registry-derived and records command *paths*, not flags, and `render_catalog_md` does not project the new catalog fields.

- [ ] **Step 7: Run tests, then verify the refusal by hand**

Run: `cargo test -p vox-cli --lib commands::plugin::install::tests -- --nocapture`

Expected: PASS.

Run: `cargo run -q -p vox-cli -- ci generate-plugin-catalog-docs`

Expected: no diff — the generator renders a fixed column set and does not project the new fields.

Run: `cargo run -q -p vox-cli -- plugin install --url https://example.invalid/p.zip --yes`

Expected: non-zero exit with `refusing to install …: no sha256 recorded`. **Use the `--url` form** — a *catalog* install now fails earlier, on the missing pinned `version`, so it does not exercise this message.

- [ ] **Step 8: STOP — load-time verification needs your authorization**

Verifying the dylib at load time is the other half of F9, and it requires `vox-plugin-host` to read the compiled-in catalog. **`vox-plugin-host` does not currently depend on `vox-plugin-catalog`**, and AGENTS.md §Dependency Discipline is explicit: *"`exceptions` entries are USER-AUTHORIZED-ONLY. Never write one yourself."*

The proposal, for the PR description:

> **Proposed `crate-edges` exception:** `vox-plugin-host` → `vox-plugin-catalog`.
> Direction is downward (L3 → L0), and `vox-plugin-catalog` is a leaf SSOT crate that
> `include_str!`s `catalog.toml` (`src/lib.rs:13`) and parses it through a `OnceLock`, so a
> hash recorded there is exactly as trustworthy as the `vox` binary. The enforcement point
> must be `load_code_plugin` (`vox-plugin-host/src/lib.rs:144`), because that is what
> `vox-ml-cli`, `vox-actor-runtime`, and `vox-orchestrator-mcp` all call. Threading an
> expected hash through every caller instead would push the same edge onto three crates and
> let any caller opt out by passing `None`.

Once authorized, the check reads `artifacts_sha256[triple]` for the plugin id and **fails when the entry is catalog-known but has no recorded hash** — otherwise the check is bypassed by deleting a record. Plugins absent from the catalog (`--path` installs, workspace builds, `plugin_abi_parity.rs:211`) fall through permissively.

**Do not implement this step without an explicit yes.** Everything above stands on its own: Task 6 closes the network path and Task 1 closes the bypass.

**Do not build a disk sidecar as a substitute.** A `.vox-integrity` file beside the dylib is not a security control: it is writable by the same user, absence would have to be permitted for existing installs, and the expected value is looked up by filename — so `Plugin.toml` can simply be repointed at a second file. It detects corruption, nothing more.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-plugin-catalog/src/schema.rs crates/vox-cli/src/commands/plugin/ crates/vox-cli/src/commands/plugin_bundle/apply.rs docs/src/reference/cli.md
git commit -m "fix(plugin): refuse to install a plugin that cannot be verified

plugin install fetched a zip and extracted it with no checksum, no signature,
and an unpinned `latest` URL, then the host dlopen'd the cdylib. Verification is
now fail-closed with an explicit --allow-unverified override, and github:
sources must pin a version because a floating asset cannot be checksummed.
plugin-bundle apply passes false deliberately, so it refuses github: plugins
until hashes are recorded.

Load-time verification is deferred pending a crate-edge authorization.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Verify the phase and open a stacked PR

- [ ] **Step 1: Format**

Run: `cargo run -q -p vox-cli -- run scripts/fmt.vox`

- [ ] **Step 2: Full local gate tier**

Run: `cargo run -q -p vox-cli -- ci pre-push --full`

Expected: exit 0.

- [ ] **Step 3: Confirm no crate edge and no policy violation**

```bash
cargo run -q -p vox-cli -- ci crate-edges
```

```bash
cargo run -q -p vox-cli -- audit code
```

Both exit 0. The second matters because AGENTS.md §Cryptography Policy routes crypto through `vox-crypto`; `vox-cli` and `voxup` already call `sha2` directly, so precedent exists — confirm here rather than discovering it in CI.

- [ ] **Step 4: Confirm the plugin and catalog gates still pass**

```bash
cargo test -p vox-plugin-catalog && cargo test -p vox-cli-ci plugin_
```

Expected: exit 0. The schema gained three `#[serde(default)]` fields, so existing entries still parse, and `plugin_catalog_sync` is `toml_edit`-surgical over a fixed field set.

- [ ] **Step 5: Push the stacked branch and open the PR**

```bash
git push -u origin claude/vox-distribution-security-floor
```

```bash
gh pr create --base claude/vox-distribution-system-f7e4c0 --title "fix(security): close the fail-open holes in the plugin and install path" --body "$(cat <<'PRBODY'
Phase 1b of the Vox distribution system design. Stacked on the Phase 0+1a PR.

- **The default install path verified nothing.** `install_from_catalog` reached
  `install_from_path` through a workspace-local fallback that was on by default,
  and `workspace_local_plugin_source` walks up eight levels from the *current
  working directory*. Any directory the user happened to be inside could supply
  a cdylib for a catalog plugin id. Now opt-in via `VOX_LOCAL_PLUGIN_FALLBACK`.
- **`install.sh` failed open** when no hash tool was present.
- **Two extractors were unguarded.** voxup's tar path (the one Linux and macOS
  take) had no validation; the plugin zip path — the only extraction in the
  codebase whose output is `dlopen`'d, and the only one with no checksum in
  front of it — materialised symlinks with no size cap.
- **Release tokens were over-scoped.**
- **Plugin installs are now fail-closed** with an explicit `--allow-unverified`.
  `vox plugin-bundle apply` passes `false`, so it will refuse `github:` plugins
  until hashes are recorded — deliberate.

**Requires a decision:** load-time dylib verification needs a `crate-edges`
exception for `vox-plugin-host` -> `vox-plugin-catalog` (L3 -> L0, downward).
Per AGENTS.md those entries are user-authorized only, so the step is proposed,
not implemented. Details in the plan's Task 6 Step 8.

**Not in scope:** signing `checksums.txt` (spec F10), blocked on a release key
held outside GitHub. Nothing here substitutes for it — a same-origin unsigned
checksum detects corruption, not a compromised release.

Spec: `docs/superpowers/specs/2026-08-20-vox-distribution-system-design.md`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
PRBODY
)"
```

---

## Follow-on

- **Hash recording** — ship a `vox plugin publish` that writes `sha256` + `version` into `catalog.toml`, or `--allow-unverified` becomes muscle memory before the first hash lands. This belongs in the same release as Task 6, not later.
- **Phase 1c** — `vox ci gen-installer-manifests` and its five registration points; installer naming via the `release_artifacts` SSOT; a behavioural `detect_target` test to replace the comment-grep guard.
- **Phases 2–4** — see the Phase 0+1a plan's follow-on list.
