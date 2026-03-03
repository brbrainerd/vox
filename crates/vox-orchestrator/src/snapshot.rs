//! Auto-snapshot working state — inspired by Jujutsu's "working copy is a commit" model.
//!
//! Every agent action is bracketed by automatic snapshots so the orchestrator
//! always knows the before/after state of every file.  This eliminates the need
//! for agents to manually `git add`/`git commit`.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::AgentId;

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// Unique snapshot identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub u64);

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S-{:06}", self.0)
    }
}

/// Thread-safe generator for [`SnapshotId`]s.
#[derive(Debug)]
pub struct SnapshotIdGenerator(AtomicU64);

impl SnapshotIdGenerator {
    /// Create a new generator starting at 1.
    pub fn new() -> Self {
        Self(AtomicU64::new(1))
    }

    /// Produce the next unique [`SnapshotId`].
    pub fn next(&self) -> SnapshotId {
        SnapshotId(self.0.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for SnapshotIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// File-level snapshot entry
// ---------------------------------------------------------------------------

/// Hash of a file's contents at a point in time (hex-encoded SHA-3-256).
pub type ContentHash = String;

/// Record of a single file at a single point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative path within the workspace.
    pub path: PathBuf,
    /// SHA-3-256 hex digest of the file contents (empty string if file was absent).
    pub content_hash: ContentHash,
    /// File size in bytes (0 if absent).
    pub size_bytes: u64,
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

/// A full snapshot of tracked files at a single moment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique identifier.
    pub id: SnapshotId,
    /// Agent that triggered this snapshot.
    pub agent_id: AgentId,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Optional human-readable description (e.g. "pre vox_run_tests").
    pub description: String,
    /// Per-file entries keyed by relative path.
    pub files: HashMap<PathBuf, FileEntry>,
}

// ---------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------

/// Describes how a single file changed between two snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileDiffKind {
    /// File was added (not present in `before`).
    Added,
    /// File was removed (not present in `after`).
    Removed,
    /// File contents changed.
    Modified,
}

/// A diff entry for one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: PathBuf,
    pub kind: FileDiffKind,
    pub before_hash: Option<ContentHash>,
    pub after_hash: Option<ContentHash>,
}

// ---------------------------------------------------------------------------
// SnapshotStore
// ---------------------------------------------------------------------------

/// In-memory store of snapshots with a configurable retention limit.
///
/// Uses a content-addressable storage (CAS) blob store internally:
/// identical file content is stored once and referenced by hash,
/// eliminating redundant storage across snapshots.
#[derive(Debug)]
pub struct SnapshotStore {
    id_gen: SnapshotIdGenerator,
    snapshots: Vec<Snapshot>,
    /// Maximum number of snapshots to keep in memory (ring-buffer style).
    max_snapshots: usize,
    /// CAS blob store: content_hash → raw bytes.
    /// Blobs are reference-counted by how many FileEntry objects point to them.
    blobs: HashMap<ContentHash, Vec<u8>>,
    /// Total bytes stored (before dedup).
    total_bytes_logical: u64,
    /// Total unique bytes stored (after dedup).
    total_bytes_physical: u64,
}

impl SnapshotStore {
    /// Create a new store with the given retention limit.
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            id_gen: SnapshotIdGenerator::new(),
            snapshots: Vec::new(),
            max_snapshots,
            blobs: HashMap::new(),
            total_bytes_logical: 0,
            total_bytes_physical: 0,
        }
    }

    /// Hash a file's contents using SHA-3-256.
    pub fn hash_file(path: &Path) -> Option<(ContentHash, u64)> {
        use sha3::{Digest, Sha3_256};

        let data = std::fs::read(path).ok()?;
        let size = data.len() as u64;
        let mut hasher = Sha3_256::new();
        hasher.update(&data);
        let hash = format!("{:x}", hasher.finalize());
        Some((hash, size))
    }

    /// Hash raw bytes using SHA-3-256. Useful for in-memory content.
    pub fn hash_bytes(data: &[u8]) -> ContentHash {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Store a blob in the CAS store. Returns the content hash.
    /// If the blob already exists (same hash), it is not duplicated.
    pub fn store_blob(&mut self, data: Vec<u8>) -> ContentHash {
        let hash = Self::hash_bytes(&data);
        let size = data.len() as u64;
        self.total_bytes_logical += size;
        if !self.blobs.contains_key(&hash) {
            self.total_bytes_physical += size;
            self.blobs.insert(hash.clone(), data);
        }
        hash
    }

    /// Retrieve a blob by content hash.
    pub fn get_blob(&self, hash: &ContentHash) -> Option<&[u8]> {
        self.blobs.get(hash).map(|v| v.as_slice())
    }

    /// Take a snapshot of the given file paths.
    /// Paths that don't exist on disk are recorded with an empty hash.
    pub fn take_snapshot(
        &mut self,
        agent_id: AgentId,
        paths: &[PathBuf],
        description: impl Into<String>,
    ) -> SnapshotId {
        let id = self.id_gen.next();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut files = HashMap::new();
        for p in paths {
            let (content_hash, size_bytes) = if let Ok(data) = std::fs::read(p) {
                let hash = self.store_blob(data);
                // size already tracked in store_blob
                let size = self.blobs.get(&hash).map(|b| b.len() as u64).unwrap_or(0);
                (hash, size)
            } else {
                (String::new(), 0)
            };
            files.insert(
                p.clone(),
                FileEntry {
                    path: p.clone(),
                    content_hash,
                    size_bytes,
                },
            );
        }

        let snap = Snapshot {
            id,
            agent_id,
            timestamp_ms,
            description: description.into(),
            files,
        };

        self.snapshots.push(snap);

        // Evict oldest if over limit.
        if self.snapshots.len() > self.max_snapshots {
            let excess = self.snapshots.len() - self.max_snapshots;
            self.snapshots.drain(..excess);
        }

        id
    }

    /// Take a snapshot of raw in-memory content (no filesystem I/O).
    /// Each entry is `(path, content_bytes)`.
    pub fn take_snapshot_in_memory(
        &mut self,
        agent_id: AgentId,
        entries: Vec<(PathBuf, Vec<u8>)>,
        description: impl Into<String>,
    ) -> SnapshotId {
        let id = self.id_gen.next();
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut files = HashMap::new();
        for (path, data) in entries {
            let size = data.len() as u64;
            let content_hash = self.store_blob(data);
            files.insert(
                path.clone(),
                FileEntry { path, content_hash, size_bytes: size },
            );
        }

        let snap = Snapshot { id, agent_id, timestamp_ms, description: description.into(), files };
        self.snapshots.push(snap);
        if self.snapshots.len() > self.max_snapshots {
            let excess = self.snapshots.len() - self.max_snapshots;
            self.snapshots.drain(..excess);
        }
        id
    }

    /// Get a snapshot by ID.
    pub fn get(&self, id: SnapshotId) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// List recent snapshots (newest first), optionally filtered by agent.
    pub fn list(&self, agent_id: Option<AgentId>, limit: usize) -> Vec<&Snapshot> {
        self.snapshots
            .iter()
            .rev()
            .filter(|s| agent_id.is_none_or(|a| s.agent_id == a))
            .take(limit)
            .collect()
    }

    /// Compute the diff between two snapshots.
    pub fn diff(before: &Snapshot, after: &Snapshot) -> Vec<FileDiff> {
        let mut diffs = Vec::new();

        for (path, after_entry) in &after.files {
            match before.files.get(path) {
                None => diffs.push(FileDiff {
                    path: path.clone(),
                    kind: FileDiffKind::Added,
                    before_hash: None,
                    after_hash: Some(after_entry.content_hash.clone()),
                }),
                Some(before_entry) if before_entry.content_hash != after_entry.content_hash => {
                    diffs.push(FileDiff {
                        path: path.clone(),
                        kind: FileDiffKind::Modified,
                        before_hash: Some(before_entry.content_hash.clone()),
                        after_hash: Some(after_entry.content_hash.clone()),
                    });
                }
                _ => {}
            }
        }

        for path in before.files.keys() {
            if !after.files.contains_key(path) {
                diffs.push(FileDiff {
                    path: path.clone(),
                    kind: FileDiffKind::Removed,
                    before_hash: before.files.get(path).map(|e| e.content_hash.clone()),
                    after_hash: None,
                });
            }
        }

        diffs
    }

    /// Total number of stored snapshots.
    pub fn count(&self) -> usize {
        self.snapshots.len()
    }

    /// Number of unique blobs in the CAS store.
    pub fn blob_count(&self) -> usize {
        self.blobs.len()
    }

    /// Deduplication ratio: logical_bytes / physical_bytes.
    /// Returns 1.0 if no duplicates exist. Higher is better.
    pub fn dedup_ratio(&self) -> f64 {
        if self.total_bytes_physical == 0 {
            return 1.0;
        }
        self.total_bytes_logical as f64 / self.total_bytes_physical as f64
    }

    /// Remove blobs not referenced by any snapshot. Returns number of blobs freed.
    pub fn compact(&mut self) -> usize {
        let referenced: std::collections::HashSet<&ContentHash> = self
            .snapshots
            .iter()
            .flat_map(|s| s.files.values())
            .map(|e| &e.content_hash)
            .filter(|h| !h.is_empty())
            .collect();

        let before = self.blobs.len();
        self.blobs.retain(|h, _| referenced.contains(h));
        // Recompute physical bytes after gc.
        self.total_bytes_physical = self.blobs.values().map(|b| b.len() as u64).sum();
        before - self.blobs.len()
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new(500)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn agent() -> AgentId {
        AgentId(1)
    }

    #[test]
    fn snapshot_id_display() {
        assert_eq!(SnapshotId(42).to_string(), "S-000042");
    }

    #[test]
    fn take_and_list_snapshots() {
        let mut store = SnapshotStore::new(10);
        let id = store.take_snapshot(agent(), &[], "test snap");
        assert_eq!(store.count(), 1);
        assert!(store.get(id).is_some());
    }

    #[test]
    fn diff_detects_added_and_removed() {
        let a = Snapshot {
            id: SnapshotId(1),
            agent_id: agent(),
            timestamp_ms: 0,
            description: String::new(),
            files: HashMap::from([(
                PathBuf::from("a.rs"),
                FileEntry {
                    path: PathBuf::from("a.rs"),
                    content_hash: "aaa".into(),
                    size_bytes: 10,
                },
            )]),
        };
        let b = Snapshot {
            id: SnapshotId(2),
            agent_id: agent(),
            timestamp_ms: 1,
            description: String::new(),
            files: HashMap::from([(
                PathBuf::from("b.rs"),
                FileEntry {
                    path: PathBuf::from("b.rs"),
                    content_hash: "bbb".into(),
                    size_bytes: 20,
                },
            )]),
        };

        let diffs = SnapshotStore::diff(&a, &b);
        assert_eq!(diffs.len(), 2);
        assert!(diffs
            .iter()
            .any(|d| d.path == PathBuf::from("b.rs") && matches!(d.kind, FileDiffKind::Added)));
        assert!(diffs
            .iter()
            .any(|d| d.path == PathBuf::from("a.rs") && matches!(d.kind, FileDiffKind::Removed)));
    }

    #[test]
    fn diff_detects_modified() {
        let make = |hash: &str| Snapshot {
            id: SnapshotId(1),
            agent_id: agent(),
            timestamp_ms: 0,
            description: String::new(),
            files: HashMap::from([(
                PathBuf::from("x.rs"),
                FileEntry {
                    path: PathBuf::from("x.rs"),
                    content_hash: hash.into(),
                    size_bytes: 5,
                },
            )]),
        };

        let diffs = SnapshotStore::diff(&make("old"), &make("new"));
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0].kind, FileDiffKind::Modified));
    }

    #[test]
    fn eviction_respects_max() {
        let mut store = SnapshotStore::new(3);
        for _ in 0..5 {
            store.take_snapshot(agent(), &[], "snap");
        }
        assert_eq!(store.count(), 3);
    }

    #[test]
    fn hash_file_works_on_real_file() {
        let dir = std::env::temp_dir().join("vox_snap_test");
        std::fs::create_dir_all(&dir).ok();
        let file = dir.join("test.txt");
        let mut f = std::fs::File::create(&file).expect("create temp");
        write!(f, "hello world").expect("write");
        drop(f);

        let result = SnapshotStore::hash_file(&file);
        assert!(result.is_some());
        let (hash, size) = result.expect("hash");
        assert_eq!(size, 11);
        assert!(!hash.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cas_deduplication() {
        let mut store = SnapshotStore::new(10);
        let data = b"shared content".to_vec();

        // Store the same blob twice — physical bytes should not double.
        let h1 = store.store_blob(data.clone());
        let h2 = store.store_blob(data.clone());
        assert_eq!(h1, h2);
        assert_eq!(store.blob_count(), 1);
        assert!(store.dedup_ratio() >= 2.0 - f64::EPSILON);
    }

    #[test]
    fn cas_get_blob() {
        let mut store = SnapshotStore::new(10);
        let data = b"hello cas".to_vec();
        let hash = store.store_blob(data.clone());
        assert_eq!(store.get_blob(&hash), Some(data.as_slice()));
        assert_eq!(store.get_blob(&"nonexistent".to_string()), None);
    }

    #[test]
    fn cas_compact_removes_unreferenced_blobs() {
        let mut store = SnapshotStore::new(10);
        // Store a blob not referenced by any snapshot.
        store.store_blob(b"orphaned blob".to_vec());
        assert_eq!(store.blob_count(), 1);

        let freed = store.compact();
        assert_eq!(freed, 1);
        assert_eq!(store.blob_count(), 0);
    }

    #[test]
    fn take_snapshot_in_memory_deduplicates() {
        let mut store = SnapshotStore::new(10);
        let shared = b"shared file content".to_vec();

        // Two snapshots with the same file — blob stored once.
        store.take_snapshot_in_memory(
            AgentId(1),
            vec![(PathBuf::from("a.rs"), shared.clone())],
            "snap 1",
        );
        store.take_snapshot_in_memory(
            AgentId(1),
            vec![(PathBuf::from("b.rs"), shared.clone())],
            "snap 2",
        );

        assert_eq!(store.blob_count(), 1);
        assert_eq!(store.count(), 2);
        assert!(store.dedup_ratio() >= 2.0 - f64::EPSILON);
    }
}
