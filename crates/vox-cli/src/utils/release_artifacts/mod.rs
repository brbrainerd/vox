//! Deterministic single-binary archives and SHA-256 checksum lines for release + `vox compile`.
//!
//! Layout matches `vox ci release-build`: one executable member at the archive root.

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tar::Builder;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

/// Windows MSVC-style triple detection for archive format (.zip vs .tar.gz).
pub fn is_windows_target(target: &str) -> bool {
    target.contains("windows")
}

pub fn artifact_extension(target: &str) -> &'static str {
    if is_windows_target(target) {
        "zip"
    } else {
        "tar.gz"
    }
}

pub fn artifact_filename(name: &str, version: &str, target: &str) -> String {
    format!("{name}-{version}-{target}.{}", artifact_extension(target))
}

/// Deterministic archive layout: a single member at the archive root named `archive_name`.
pub fn package_tar_gz(binary_path: &Path, artifact_path: &Path, archive_name: &str) -> Result<()> {
    let artifact = File::create(artifact_path)
        .with_context(|| format!("create archive {}", artifact_path.display()))?;
    let encoder = GzEncoder::new(artifact, Compression::default());
    let mut tar = Builder::new(encoder);
    tar.append_path_with_name(binary_path, archive_name)
        .with_context(|| format!("add {} to tar", binary_path.display()))?;
    tar.finish().context("finish tar archive")?;
    Ok(())
}

pub fn package_zip(binary_path: &Path, artifact_path: &Path, archive_name: &str) -> Result<()> {
    let artifact = File::create(artifact_path)
        .with_context(|| format!("create archive {}", artifact_path.display()))?;
    let mut zip = zip::ZipWriter::new(artifact);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(archive_name, options)
        .context("start zip file entry")?;

    let mut src = File::open(binary_path)
        .with_context(|| format!("open binary for zipping {}", binary_path.display()))?;
    let mut buf = Vec::new();
    src.read_to_end(&mut buf)
        .with_context(|| format!("read binary {}", binary_path.display()))?;
    zip.write_all(&buf).context("write binary bytes to zip")?;
    zip.finish().context("finish zip archive")?;
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn checksum_line(sha256_hex: &str, filename: &str) -> String {
    format!("{sha256_hex}  {filename}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_filename_contract_is_stable() {
        assert_eq!(
            artifact_filename("myapp", "v1.2.3", "x86_64-pc-windows-msvc"),
            "myapp-v1.2.3-x86_64-pc-windows-msvc.zip"
        );
        assert_eq!(
            artifact_filename("myapp", "v1.2.3", "x86_64-unknown-linux-gnu"),
            "myapp-v1.2.3-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn checksum_manifest_line_format() {
        let line = checksum_line("deadbeef", "app-v1.tar.gz");
        assert_eq!(line, "deadbeef  app-v1.tar.gz\n");
    }
}
