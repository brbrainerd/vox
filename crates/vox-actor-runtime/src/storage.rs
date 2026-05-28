use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

pub use vox_config::{STORAGE_URL_PREFIX, WAL_FLUSH_BATCH_SIZE};

// ---------------------------------------------------------------------------
// WAL entry
// ---------------------------------------------------------------------------

/// A single pending write in the write-ahead log.
struct WalEntry {
    path: PathBuf,
    data: bytes::Bytes,
}

// ---------------------------------------------------------------------------
// LocalStorage
// ---------------------------------------------------------------------------

/// Async, WAL-buffered local storage backend.
///
/// Writes are batched and flushed via `tokio::fs` so the async executor
/// is never blocked on blocking disk I/O. The WAL accumulates entries and
/// flushes them in a single batch when either:
///
/// - [`WAL_FLUSH_BATCH_SIZE`] entries are pending, or
/// - the caller explicitly calls [`Self::flush`].
#[derive(Clone)]
pub struct LocalStorage {
    base_dir: PathBuf,
    wal: Arc<Mutex<Vec<WalEntry>>>,
}

impl LocalStorage {
    /// Create a new local storage backend at the specified directory.
    ///
    /// The directory is created synchronously during construction (one-time
    /// setup cost; acceptable at startup).
    pub fn new(base_dir: &str) -> io::Result<Self> {
        let path = PathBuf::from(base_dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            base_dir: path,
            wal: Arc::new(Mutex::new(Vec::with_capacity(WAL_FLUSH_BATCH_SIZE))),
        })
    }

    /// Queue a file write and return a content-based file ID.
    ///
    /// The data is hashed immediately (synchronous, CPU-bound) and the write
    /// is batched in the WAL. Call [`Self::flush`] to ensure durability, or
    /// let the background batch threshold trigger automatically.
    pub async fn store(&self, data: bytes::Bytes) -> io::Result<String> {
        let id = content_hash(&data);
        let path = self.base_dir.join(&id);

        let should_flush = {
            let mut wal = self.wal.lock().await;
            wal.push(WalEntry {
                path,
                data,
            });
            wal.len() >= WAL_FLUSH_BATCH_SIZE
        };

        if should_flush {
            self.flush().await?;
        }

        Ok(id)
    }

    /// Force all pending WAL entries to be flushed to disk.
    ///
    /// Uses `tokio::fs` throughout — no blocking I/O on the async executor.
    pub async fn flush(&self) -> io::Result<()> {
        let entries: Vec<WalEntry> = {
            let mut wal = self.wal.lock().await;
            std::mem::take(&mut *wal)
        };

        for entry in entries {
            // Ensure parent directory exists (non-blocking after first call).
            if let Some(parent) = entry.path.parent() {
                fs::create_dir_all(parent).await?;
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&entry.path)
                .await?;
            file.write_all(&entry.data).await?;
            file.flush().await?;
        }

        Ok(())
    }

    /// Get the URL for a stored file ID.
    pub fn get_url(&self, id: &str) -> String {
        format!("{}/{}", STORAGE_URL_PREFIX, id)
    }

    /// Synchronous store for use in non-async contexts (e.g. test setup).
    ///
    /// Writes directly to disk without the WAL. Prefer [`Self::store`] in
    /// production async code.
    pub fn store_sync(&self, data: &[u8]) -> io::Result<String> {
        let id = content_hash(data);
        let path = self.base_dir.join(&id);
        std::fs::write(path, data)?;
        Ok(id)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute a hex-encoded 64-bit FNV-1a hash of the given data.
///
/// Deterministic and fast; sufficient for local file deduplication without
/// the overhead of a cryptographic hash function.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    #[test]
    fn get_url_format() {
        let storage = LocalStorage::new(std::env::temp_dir().to_str().unwrap()).unwrap();
        let url = storage.get_url("abc123");
        assert_eq!(url, "/storage/abc123");
    }

    #[tokio::test]
    async fn store_flush_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(dir.path().to_str().unwrap()).unwrap();
        let data = bytes::Bytes::from_static(b"vox-wal-test");
        let id = storage.store(data.clone()).await.unwrap();
        storage.flush().await.unwrap();
        let written = std::fs::read(dir.path().join(&id)).unwrap();
        assert_eq!(written, data.as_ref());
    }
}
