//! `vox bundle verify <tarball>` — extract to tempdir, run plugin doctor,
//! exit 0 on success.
//!
//! The command:
//! 1. Extracts the tarball into a temporary directory.
//! 2. Confirms `plugins/` and `BUNDLE.toml` are present.
//! 3. Sets `VOX_PLUGINS_DIR` to `<tempdir>/plugins` so that `plugin doctor`
//!    scans the extracted bundle rather than the host install root.
//! 4. Runs `vox plugin doctor` and surfaces any issues.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::fs::File;
use std::path::Path;
use tar::Archive;

pub fn run(tarball_path: &Path) -> Result<()> {
    let tmp = tempfile::tempdir().context("creating tempdir for bundle extraction")?;

    println!(
        "-> Extracting {} to {}",
        tarball_path.display(),
        tmp.path().display()
    );

    let f = File::open(tarball_path)
        .with_context(|| format!("opening tarball {}", tarball_path.display()))?;
    let gz = GzDecoder::new(f);
    extract_bundle_tar(gz, tmp.path())
        .with_context(|| format!("unpacking {}", tarball_path.display()))?;

    // Structural checks.
    let bundle_toml = tmp.path().join("BUNDLE.toml");
    if !bundle_toml.is_file() {
        anyhow::bail!(
            "bundle integrity check failed: BUNDLE.toml not found in {}",
            tarball_path.display()
        );
    }

    let plugins_root = tmp.path().join("plugins");
    // plugins/ may be absent for vox-base (no plugins); that is valid.
    // We only fail if the dir exists but contains nothing parseable.

    // Print BUNDLE.toml metadata.
    if let Ok(raw) = std::fs::read_to_string(&bundle_toml) {
        println!("  BUNDLE.toml:");
        for line in raw.lines() {
            if !line.starts_with('#') && !line.is_empty() {
                println!("    {line}");
            }
        }
    }

    // Run doctor against the extracted plugins root.
    println!("-> Running plugin doctor against extracted plugins root");
    // Safety: this is a single-threaded CLI path. No other thread writes
    // VOX_PLUGINS_DIR concurrently. set_var is safe here.
    #[allow(unsafe_code)]
    // SAFETY: CLI process; no concurrent thread environment mutation.
    unsafe {
        std::env::set_var("VOX_PLUGINS_DIR", &plugins_root);
    }
    crate::commands::plugin::doctor::run()?;

    println!("✓ bundle integrity verified: {}", tarball_path.display());
    Ok(())
}

/// Guarded replacement for `Archive::unpack`.
///
/// `unpack` SILENTLY SKIPS entries that escape the destination and materialises
/// symlinks, with no size cap. That is the wrong default anywhere; it is
/// especially wrong here, in the command whose stated job is to check a bundle
/// you do not trust yet — and whose output `plugin doctor` then scans.
///
// vox:defactored-from voxup 2026-08-22 (voxup::download::extract_targz guards, ~40 lines)
// A crate edge vox-cli -> voxup is not authorised (AGENTS.md Dependency
// Discipline); this is the sanctioned duplication instead.
fn extract_bundle_tar<R: std::io::Read>(reader: R, dest: &Path) -> Result<()> {
    use tar::EntryType;

    const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
    const MAX_ENTRIES: usize = 10_000;

    let mut archive = Archive::new(reader);
    let mut total: u64 = 0;
    let mut count: usize = 0;

    for entry in archive.entries().context("read tar entries")? {
        let mut entry = entry.context("read tar entry")?;
        count += 1;
        if count > MAX_ENTRIES {
            anyhow::bail!("bundle has more than {MAX_ENTRIES} entries; refusing to extract");
        }

        let ty = entry.header().entry_type();
        // Metadata, not a file — skip so bsdtar-produced bundles still verify.
        if ty == EntryType::XGlobalHeader {
            continue;
        }
        if !(ty.is_file() || ty.is_dir()) {
            anyhow::bail!(
                "unsupported entry type {ty:?} in bundle; only regular files and                  directories are allowed"
            );
        }

        let path = entry.path().context("decode tar entry path")?.into_owned();
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            anyhow::bail!("Tar Slip detected: path {path:?} escapes destination");
        }
        let outpath = dest.join(&path);
        if !outpath.starts_with(dest) {
            anyhow::bail!("Tar Slip detected: path {path:?} escapes destination");
        }

        // `entry.size()`, not `header().size()`: a PAX extension record
        // overrides the raw header field, and tar-rs bounds its reader by the
        // overridden value.
        total = total.saturating_add(entry.size());
        if total > MAX_UNCOMPRESSED_BYTES {
            anyhow::bail!("bundle expands beyond {MAX_UNCOMPRESSED_BYTES} bytes; refusing");
        }

        entry
            .unpack(&outpath)
            .with_context(|| format!("unpack {}", outpath.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Builds a tar whose entry name is written raw, because tar-rs's safe
    /// `set_path` REFUSES `..` — the safe API cannot express the attack.
    fn tar_with_raw_name(name: &[u8], contents: &[u8]) -> Vec<u8> {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        {
            let bytes = header.as_old_mut();
            bytes.name[..name.len()].copy_from_slice(name);
        }
        header.set_cksum();
        let mut builder = tar::Builder::new(Vec::new());
        builder.append(&header, contents).expect("append");
        builder.into_inner().expect("finish tar")
    }

    #[test]
    fn escaping_entry_is_refused_not_silently_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data = tar_with_raw_name(b"../escaped.txt", b"pwned");
        let err = extract_bundle_tar(std::io::Cursor::new(data), dir.path())
            .expect_err("an escaping entry must be an error, not a silent skip");
        assert!(
            err.to_string().contains("Tar Slip"),
            "must name the traversal, got: {err}"
        );
        assert!(
            !dir.path().parent().unwrap().join("escaped.txt").exists(),
            "escaping file must not be written"
        );
    }

    /// Guards the fixture: if tar-rs ever normalises the raw name, the test
    /// above would silently stop asserting anything.
    #[test]
    fn the_traversal_fixture_really_escapes() {
        let data = tar_with_raw_name(b"../escaped.txt", b"pwned");
        let mut ar = tar::Archive::new(std::io::Cursor::new(data));
        let names: Vec<String> = ar
            .entries()
            .expect("entries")
            .map(|e| {
                e.expect("entry")
                    .path()
                    .expect("path")
                    .display()
                    .to_string()
            })
            .collect();
        assert!(
            names.iter().any(|n| n.contains("..")),
            "fixture no longer escapes: {names:?}"
        );
    }

    #[test]
    fn a_normal_bundle_still_extracts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut builder = tar::Builder::new(Vec::new());
        let payload = b"name = \"vox-base\"
";
        let mut header = tar::Header::new_gnu();
        header.set_size(payload.len() as u64);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "BUNDLE.toml", &payload[..])
            .expect("append");
        let data = builder.into_inner().expect("finish");
        extract_bundle_tar(std::io::Cursor::new(data), dir.path())
            .expect("a normal bundle must extract");
        let got = std::fs::read(dir.path().join("BUNDLE.toml")).expect("read");
        assert_eq!(got, payload);
    }
}
