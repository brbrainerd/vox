//! Parse `checksums.txt` lines (`<hex> <filename>`) and verify release archive bytes.
//!
//! Matches the binary release contract used by `vox upgrade`, `vox-bootstrap`, and CI
//! (`checksums.txt` lists basenames; paths in the file are accepted).

use sha2::{Digest, Sha256};
use std::path::Path;

/// First N distinct basenames listed in the manifest (for error diagnostics).
pub fn sample_checksum_basenames(checksums_txt: &str, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in checksums_txt.lines() {
        let mut parts = line.split_whitespace();
        if parts.next().is_none() {
            continue;
        }
        if let Some(file) = parts.next() {
            let base = Path::new(file)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(file);
            out.push(base.to_string());
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// Lowercase SHA-256 hex for `asset_name`, matching either basename or full path entry.
pub fn checksum_for_asset(checksums_txt: &str, asset_name: &str) -> Option<String> {
    let needle_file_name = Path::new(asset_name)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(asset_name);
    for line in checksums_txt.lines() {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?;
        let file_name = Path::new(file)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(file);
        if file_name == needle_file_name {
            return Some(hash.to_lowercase());
        }
    }
    None
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Verify `asset_bytes` against the manifest entry for `asset_name` (basename match).
pub fn verify_checksum(
    asset_bytes: &[u8],
    checksums_txt: &str,
    asset_name: &str,
) -> Result<(), String> {
    let expected = checksum_for_asset(checksums_txt, asset_name).ok_or_else(|| {
        let sample = sample_checksum_basenames(checksums_txt, 5);
        format!(
            "checksum entry not found for `{asset_name}` in checksums.txt (first basenames in file: {sample:?})"
        )
    })?;
    let actual = sha256_hex(asset_bytes);
    if actual != expected {
        return Err(format!(
            "checksum mismatch for {asset_name} (expected {expected}, got {actual})"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_lookup_accepts_path_prefix() {
        let txt = "abcd0123eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee release/vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz\n";
        let found = checksum_for_asset(txt, "vox-v1.2.3-x86_64-unknown-linux-gnu.tar.gz");
        assert_eq!(
            found.as_deref(),
            Some("abcd0123eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
        );
    }
}
