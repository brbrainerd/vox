//! UTF-8 file reads capped by [`vox_scaling_policy::ScalingPolicy::embedded`] `max_file_bytes_hint`.
//!
//! This crate is the workspace SSOT for scaling-policy-aware capped reads used by CI, MCP,
//! publisher, Populi, and other crates. Prefer it over per-crate copies of `bounded_fs`.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use vox_scaling_policy::ScalingPolicy;

/// Current cap from embedded scaling policy.
#[must_use]
pub fn max_file_bytes_hint() -> u64 {
    ScalingPolicy::embedded().thresholds.max_file_bytes_hint
}

/// Read a file as UTF-8; errors if size exceeds [`max_file_bytes_hint`] or bytes are not valid UTF-8.
pub fn read_utf8_path_capped(path: &Path) -> Result<String> {
    let cap = max_file_bytes_hint();
    let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.len() > cap {
        anyhow::bail!(
            "{} is {} bytes; exceeds scaling policy max_file_bytes_hint ({})",
            path.display(),
            meta.len(),
            cap
        );
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    String::from_utf8(bytes)
        .map_err(|e| anyhow::anyhow!("{}: invalid UTF-8: {}", path.display(), e))
}

/// Same as [`read_utf8_path_capped`] but returns an empty string on any failure.
#[must_use]
pub fn read_utf8_path_capped_or_empty(path: &Path) -> String {
    read_utf8_path_capped(path).unwrap_or_default()
}

/// Same as [`read_utf8_path_capped`] but returns `None` on any failure.
#[must_use]
pub fn read_utf8_path_capped_opt(path: &Path) -> Option<String> {
    read_utf8_path_capped(path).ok()
}

/// Capped read on the blocking pool (for async call sites; avoids unbounded `tokio::fs::read_to_string`).
#[cfg(feature = "async")]
pub async fn read_utf8_path_capped_async(path: &Path) -> Result<String> {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || read_utf8_path_capped(&p))
        .await
        .map_err(|e| anyhow::anyhow!("read join error: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_oversized_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("big.bin");
        let cap = max_file_bytes_hint();
        let oversize = cap.saturating_add(1).max(1);
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(&vec![0u8; oversize as usize]).unwrap();
        drop(f);
        let err = read_utf8_path_capped(&p).unwrap_err().to_string();
        assert!(
            err.contains("exceeds scaling policy max_file_bytes_hint"),
            "{err}"
        );
    }

    #[test]
    fn reads_small_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.txt");
        fs::write(&p, "hello").unwrap();
        assert_eq!(read_utf8_path_capped(&p).unwrap(), "hello");
    }
}
