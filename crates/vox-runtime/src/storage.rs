use std::fs;
use std::io;
use std::path::PathBuf;

/// URL path prefix for serving stored files.
const STORAGE_URL_PREFIX: &str = "/storage";

/// Basic local storage backend for storing and retrieving files.
#[derive(Clone)]
pub struct LocalStorage {
    base_dir: PathBuf,
}

impl LocalStorage {
    /// Create a new local storage backend at the specified directory.
    pub fn new(base_dir: &str) -> io::Result<Self> {
        let path = PathBuf::from(base_dir);
        fs::create_dir_all(&path)?;
        Ok(Self { base_dir: path })
    }

    /// Store a file and return a content-based file ID.
    ///
    /// Uses a simple hash of the file content to generate a deterministic,
    /// collision-resistant identifier.
    pub fn store(&self, data: &[u8]) -> io::Result<String> {
        let id = content_hash(data);
        let path = self.base_dir.join(&id);
        fs::write(path, data)?;
        Ok(id)
    }

    /// Get the URL for a stored file ID.
    pub fn get_url(&self, id: &str) -> String {
        format!("{}/{}", STORAGE_URL_PREFIX, id)
    }
}

/// Compute a hex-encoded 64-bit FNV-1a hash of the given data.
/// Deterministic and fast; sufficient for local file deduplication.
fn content_hash(data: &[u8]) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{:016x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_deterministic() {
        let h1 = content_hash(b"hello world");
        let h2 = content_hash(b"hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn content_hash_differs_for_different_data() {
        let h1 = content_hash(b"hello");
        let h2 = content_hash(b"world");
        assert_ne!(h1, h2);
    }
}
