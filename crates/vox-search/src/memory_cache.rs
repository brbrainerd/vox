//! Process-wide cache for in-memory BM25 indices keyed by memory paths + mtimes.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::context::SearchRuntimeContext;
use crate::memory_hybrid::MemorySearchEngine;
use crate::policy::SearchPolicy;

struct CacheEntry {
    repo_root: PathBuf,
    log_dir: PathBuf,
    md_path: PathBuf,
    log_stamp_nanos: u128,
    md_stamp_nanos: u128,
    bm25_k1_bits: u64,
    bm25_b_bits: u64,
}

fn mtime_nanos(path: &std::path::Path) -> u128 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn build_cached_memory_engine_sync(
    ctx: &SearchRuntimeContext,
    policy: &SearchPolicy,
) -> MemorySearchEngine {
    static CACHE: Mutex<Option<(CacheEntry, MemorySearchEngine)>> = Mutex::new(None);
    let log_stamp = mtime_nanos(&ctx.memory_log_dir);
    let md_stamp = mtime_nanos(&ctx.memory_md_path);
    let k1 = policy.clamped_memory_bm25_k1();
    let b = policy.clamped_memory_bm25_b();
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache_hit = guard.as_ref().is_some_and(|(e, _)| {
        e.repo_root == ctx.repo_root
            && e.log_dir == ctx.memory_log_dir
            && e.md_path == ctx.memory_md_path
            && e.log_stamp_nanos == log_stamp
            && e.md_stamp_nanos == md_stamp
            && e.bm25_k1_bits == k1.to_bits()
            && e.bm25_b_bits == b.to_bits()
    });
    if cache_hit {
        return guard.as_ref().expect("cache_hit implies Some").1.clone();
    }

    let git_map = crate::memory_hybrid::git_latest_mtime_map(&ctx.repo_root, &ctx.memory_log_dir);
    let mut engine = MemorySearchEngine::new().with_bm25_params(k1, b);
    engine.index_dir_with_repo(&ctx.memory_log_dir, &ctx.repo_root, git_map.as_ref());
    if !ctx.memory_md_path.starts_with(&ctx.memory_log_dir) {
        let gm = git_map.as_ref();
        engine.index_file_with_repo(&ctx.memory_md_path, &ctx.repo_root, gm);
    }
    *guard = Some((
        CacheEntry {
            repo_root: ctx.repo_root.clone(),
            log_dir: ctx.memory_log_dir.clone(),
            md_path: ctx.memory_md_path.clone(),
            log_stamp_nanos: log_stamp,
            md_stamp_nanos: md_stamp,
            bm25_k1_bits: k1.to_bits(),
            bm25_b_bits: b.to_bits(),
        },
        engine.clone(),
    ));
    engine
}

/// Returns a [`MemorySearchEngine`] indexed for `ctx`'s log dir + standalone md path.
///
/// Reuses the last build when paths, mtimes, repo root, and BM25 policy match (best-effort).
/// CPU-heavy indexing runs on a blocking thread pool.
pub(crate) async fn cached_memory_engine(
    ctx: &SearchRuntimeContext,
    policy: &SearchPolicy,
) -> MemorySearchEngine {
    let ctx = ctx.clone();
    let policy = policy.clone();
    tokio::task::spawn_blocking(move || build_cached_memory_engine_sync(&ctx, &policy))
        .await
        .unwrap_or_else(|e| {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            panic!("memory cache spawn_blocking cancelled: {e}");
        })
}
