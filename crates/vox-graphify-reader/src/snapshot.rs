//! Bounded graph history: copy `graph.json` + manifest into `snapshots/<stamp>/`, list, prune-to-N.
//! Timestamps are caller-supplied (filesystem-safe, lexically sortable) so this stays pure + testable.
use std::fs;
use std::path::{Path, PathBuf};

const SNAPSHOT_FILES: [&str; 2] = ["graph.json", ".graphify_manifest.v1.json"];

/// Copy the corpus's current graph + manifest into `<corpus_dir>/snapshots/<stamp>/`.
/// Missing source files are skipped (a first-ever snapshot may be empty).
pub fn snapshot_corpus(corpus_dir: &Path, stamp: &str) -> std::io::Result<PathBuf> {
    let dst = corpus_dir.join("snapshots").join(stamp);
    fs::create_dir_all(&dst)?;
    for name in SNAPSHOT_FILES {
        let src = corpus_dir.join(name);
        if src.is_file() {
            fs::copy(&src, dst.join(name))?;
        }
    }
    Ok(dst)
}

/// Snapshot stamps, lexically sorted (oldest first). Empty if none.
pub fn list_snapshots(corpus_dir: &Path) -> Vec<String> {
    let base = corpus_dir.join("snapshots");
    let Ok(rd) = fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut v: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    v.sort();
    v
}

/// Remove the oldest snapshots, keeping the newest `keep`. Returns how many were removed.
pub fn prune_snapshots(corpus_dir: &Path, keep: usize) -> std::io::Result<usize> {
    let snaps = list_snapshots(corpus_dir);
    if snaps.len() <= keep {
        return Ok(0);
    }
    let base = corpus_dir.join("snapshots");
    let mut removed = 0usize;
    for s in &snaps[..snaps.len() - keep] {
        fs::remove_dir_all(base.join(s))?;
        removed += 1;
    }
    Ok(removed)
}
