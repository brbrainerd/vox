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
            if hash.len() != 64 { return None; }
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
    fs::create_dir_all(dest_dir)
        .with_context(|| format!("create {}", dest_dir.display()))?;
    if filename.ends_with(".tar.gz") {
        extract_targz(data, dest_dir)
    } else if filename.ends_with(".zip") {
        extract_zip(data, dest_dir)
    } else {
        bail!("unknown archive format for '{filename}' (expected .tar.gz or .zip)")
    }
}

fn extract_targz(data: &[u8], dest_dir: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    let gz = GzDecoder::new(Cursor::new(data));
    let mut archive = Archive::new(gz);
    archive.unpack(dest_dir)
        .with_context(|| format!("unpack tar.gz to {}", dest_dir.display()))?;
    info!("Extracted tar.gz to {}", dest_dir.display());
    Ok(())
}

fn extract_zip(data: &[u8], dest_dir: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let mut archive = zip::ZipArchive::new(Cursor::new(data))
            .context("open zip archive")?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).context("read zip entry")?;
            let outpath = dest_dir.join(entry.name());
            if entry.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() { fs::create_dir_all(p)?; }
                let mut outfile = fs::File::create(&outpath)
                    .with_context(|| format!("create {}", outpath.display()))?;
                std::io::copy(&mut entry, &mut outfile)
                    .with_context(|| format!("write {}", outpath.display()))?;
            }
        }
        info!("Extracted zip to {}", dest_dir.display());
        Ok(())
    }
    #[cfg(not(windows))]
    { bail!(".zip extraction is only supported on Windows") }
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
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use tar::Builder;
        let mut buf = Vec::new();
        let enc = GzEncoder::new(&mut buf, Compression::default());
        let mut ar = Builder::new(enc);
        let content = b"vox binary placeholder";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        ar.append_data(&mut header, "vox", content.as_slice()).unwrap();
        ar.finish().unwrap();
        drop(ar);
        let tmp = tempfile::tempdir().unwrap();
        extract(&buf, tmp.path(), "vox-0.7.0.tar.gz").unwrap();
        assert!(tmp.path().join("vox").exists());
    }
}
