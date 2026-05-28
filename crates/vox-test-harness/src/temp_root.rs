//! Temporary directory wrapper for tests (`tempfile`).
//!
//! Prefer [`TempRoot`] over `std::env::temp_dir()` joins so paths stay unique per test.

use std::path::Path;

use tempfile::TempDir;

pub struct TempRoot {
    inner: TempDir,
}

impl TempRoot {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            inner: TempDir::new().map_err(|e| anyhow::anyhow!("TempDir::new: {e}"))?,
        })
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Persists the directory after this helper is dropped (no automatic cleanup).
    pub fn persist(self) -> std::path::PathBuf {
        self.inner.keep()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_writable_dir() {
        let t = TempRoot::new().unwrap();
        let p = t.path().join("x.txt");
        std::fs::write(&p, b"ok").unwrap();
        assert!(p.is_file());
    }
}
