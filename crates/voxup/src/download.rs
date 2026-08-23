//! HTTP download, SHA-256 verification, and archive extraction.

use anyhow::{Context, Result, bail};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tracing::info;

pub async fn fetch_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    let bytes = client
        .get(url)
        .header("User-Agent", concat!("voxup/", env!("CARGO_PKG_VERSION")))
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error for {url}"))?
        .bytes()
        .await
        .with_context(|| format!("reading body of {url}"))?;
    Ok(bytes.to_vec())
}

/// Parse `checksums.txt` format: `<sha256hex>  <filename>` per line.
pub fn parse_checksums(text: &str) -> HashMap<String, String> {
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

pub fn verify_sha256(data: &[u8], expected: &str) -> Result<()> {
    let mut h = Sha256::new();
    h.update(data);
    let actual = hex::encode(h.finalize());
    if actual.to_lowercase() != expected.to_lowercase() {
        bail!("checksum mismatch\n  expected: {expected}\n  actual:   {actual}");
    }
    Ok(())
}

pub fn extract(data: &[u8], dest_dir: &Path, filename: &str) -> Result<()> {
    fs::create_dir_all(dest_dir).with_context(|| format!("create {}", dest_dir.display()))?;
    if filename.ends_with(".tar.gz") {
        extract_targz(data, dest_dir)
    } else if filename.ends_with(".zip") {
        extract_zip(data, dest_dir)
    } else {
        bail!("unknown archive format for '{filename}' (expected .tar.gz or .zip)")
    }
}

/// Maximum total uncompressed bytes from one archive (512 MiB). tar-rs bounds
/// each entry's reader with `io::Take` at `Entry::size()` — which is the raw
/// header field, EXCEPT when a PAX `size` extension record is present, in which
/// case the PAX value wins. Summing `entry.size()` (not `entry.header().size()`)
/// therefore tracks the same bound the reader is actually limited by, so it is
/// a real upper bound on bytes written, not merely advisory.
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

        // `entry.size()` (not `entry.header().size()`) — the raw header field is
        // overridden by a PAX `size` extension record when one is present, and
        // tar-rs bounds its reader with `entry.size()`, PAX override included
        // (vendored tar-0.4.46/src/archive.rs:337-360). Checking the header field
        // alone lets a small ustar size + a large PAX size sail past this cap.
        let declared = entry.size();
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

#[cfg_attr(not(windows), allow(unused_variables))]
fn extract_zip(data: &[u8], dest_dir: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        return extract_zip_capped(data, dest_dir, MAX_UNCOMPRESSED_BYTES);
    }
    #[cfg(not(windows))]
    {
        let _ = (data, dest_dir);
        bail!(".zip extraction is only supported on Windows")
    }
}

/// Inner form with an injectable cap, so tests can trip the bound without
/// building a 512 MiB archive.
#[cfg(windows)]
fn extract_zip_capped(data: &[u8], dest_dir: &Path, max_uncompressed: u64) -> Result<()> {
    {
        // Same bounds as `extract_targz`. This is the WINDOWS release path and
        // carries identical trust, so leaving it unbounded just meant an
        // attacker picked the weaker platform.
        let mut archive = zip::ZipArchive::new(Cursor::new(data)).context("open zip archive")?;
        if archive.len() > MAX_ENTRIES {
            bail!("archive has more than {MAX_ENTRIES} entries; refusing to extract");
        }
        let mut total_bytes: u64 = 0;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).context("read zip entry")?;
            let enclosed = entry
                .enclosed_name()
                .with_context(|| format!("Zip Slip detected: invalid path {:?}", entry.name()))?;
            let outpath = dest_dir.join(enclosed);
            if !outpath.starts_with(dest_dir) {
                bail!(
                    "Zip Slip detected: path {:?} escapes destination",
                    entry.name()
                );
            }
            if entry.is_dir() {
                fs::create_dir_all(&outpath)?;
                continue;
            }
            // Allowlist regular files, mirroring the tar path. A real Vox
            // archive holds exactly one regular file.
            if entry.is_symlink() {
                bail!(
                    "archive contains a symlink entry {:?}; refusing to extract",
                    entry.name()
                );
            }
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = fs::File::create(&outpath)
                .with_context(|| format!("create {}", outpath.display()))?;
            // Bound the COPY, not the declared size. Unlike tar — where tar-rs
            // bounds its reader by `entry.size()` — a zip entry's uncompressed
            // size is central-directory metadata that DEFLATE need not honour,
            // so a small declared size can inflate without limit. `take` caps
            // what `io::copy` can pull; +1 distinguishes "exactly at the cap"
            // from "over it".
            use std::io::Read;
            let remaining = max_uncompressed.saturating_sub(total_bytes);
            let mut limited = (&mut entry).take(remaining.saturating_add(1));
            let written = std::io::copy(&mut limited, &mut outfile)
                .with_context(|| format!("write {}", outpath.display()))?;
            if written > remaining {
                bail!("archive expands beyond {max_uncompressed} bytes; refusing to extract");
            }
            total_bytes = total_bytes.saturating_add(written);
        }
        info!("Extracted zip to {}", dest_dir.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha256_hex(data: &[u8]) -> String {
        hex::encode(Sha256::digest(data))
    }

    #[test]
    fn parse_checksums_handles_standard_format() {
        let hash = "a".repeat(64);
        let text = format!("{hash}  vox-0.7.0-linux.tar.gz\n");
        let map = parse_checksums(&text);
        assert_eq!(map.get("vox-0.7.0-linux.tar.gz").unwrap(), &hash);
    }

    #[test]
    fn parse_checksums_skips_blank_lines() {
        let hash = "b".repeat(64);
        let text = format!("{hash}  file.txt\n\n  \n");
        assert_eq!(parse_checksums(&text).len(), 1);
    }

    #[test]
    fn parse_checksums_skips_short_hashes() {
        let text = "da39a3ee5e6b4b0d3255bfef95601890afd80709  bad.txt\n";
        assert!(parse_checksums(text).is_empty());
    }

    #[test]
    fn verify_sha256_passes_on_correct_hash() {
        let data = b"hello voxup";
        assert!(verify_sha256(data, &sha256_hex(data)).is_ok());
    }

    #[test]
    fn verify_sha256_fails_on_wrong_hash() {
        let err = verify_sha256(b"hello", &"0".repeat(64)).unwrap_err();
        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn verify_sha256_is_case_insensitive() {
        let data = b"test";
        let upper = sha256_hex(data).to_uppercase();
        assert!(verify_sha256(data, &upper).is_ok());
    }

    #[test]
    fn extract_rejects_unknown_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let err = extract(b"", tmp.path(), "vox.rar").unwrap_err();
        assert!(err.to_string().contains("unknown archive format"));
    }

    #[cfg(unix)]
    #[test]
    fn extract_targz_round_trip() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::Builder;
        let mut buf = Vec::new();
        let enc = GzEncoder::new(&mut buf, Compression::default());
        let mut ar = Builder::new(enc);
        let content = b"vox binary placeholder";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        ar.append_data(&mut header, "vox", content.as_slice())
            .unwrap();
        ar.finish().unwrap();
        drop(ar);
        let tmp = tempfile::tempdir().unwrap();
        extract(&buf, tmp.path(), "vox-0.7.0.tar.gz").unwrap();
        assert!(tmp.path().join("vox").exists());
    }

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
            .map(|e| {
                e.expect("entry")
                    .path()
                    .expect("path")
                    .display()
                    .to_string()
            })
            .collect();
        assert_eq!(
            paths,
            vec!["../escaped.txt".to_string()],
            "fixture no longer escapes"
        );
    }

    /// Parity tests for the WINDOWS path. `extract_zip` is `#[cfg(windows)]`,
    /// so these are too — on other hosts the function is a `bail!` stub.
    #[cfg(windows)]
    #[test]
    fn extract_zip_rejects_a_zip_bomb_bounded_on_actual_bytes() {
        use std::io::Write;
        // Declare a small uncompressed size, then inflate far past it. The
        // declared value is central-directory metadata; only bounding the copy
        // catches this.
        let payload = vec![0u8; 4096];
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            w.start_file("bomb.bin", opts).expect("start file");
            w.write_all(&payload).expect("write");
            w.finish().expect("finish");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        // Cap below the real payload so the bounded copy must trip.
        let err = extract_zip_capped(&buf, dir.path(), 1024)
            .expect_err("an entry inflating past the cap must be refused");
        assert!(err.to_string().contains("expands beyond"), "got: {err}");
    }

    #[cfg(windows)]
    #[test]
    fn extract_zip_accepts_a_normal_entry() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            w.start_file("vox.exe", opts).expect("start file");
            w.write_all(b"binary").expect("write");
            w.finish().expect("finish");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        extract_zip(&buf, dir.path()).expect("a normal archive must extract");
        assert_eq!(
            std::fs::read(dir.path().join("vox.exe")).expect("read extracted"),
            b"binary"
        );
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

    #[cfg(windows)]
    #[test]
    fn test_extract_zip_prevents_zip_slip() {
        use zip::ZipWriter;
        use zip::write::SimpleFileOptions;
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut buf));
            writer
                .start_file("../escaping-file.txt", SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut writer, b"dangerous content").unwrap();
            writer.finish().unwrap();
        }
        let tmp = tempfile::tempdir().unwrap();
        let err = extract_zip(&buf, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("Zip Slip detected"));
    }
}
