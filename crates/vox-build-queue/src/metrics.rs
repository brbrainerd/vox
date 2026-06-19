//! Build metrics record + summary.
//!
//! Each cargo invocation through the broker appends one JSON line. `summarize`
//! powers `vox build-broker stats`, which yields the go/no-go verdict for the
//! deferred coalescing daemon (Layer 1b).

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricRecord {
    pub ts_ms: u128,
    pub worktree: String,
    pub subcmd: String,
    pub queue_wait_ms: u64,
    pub ran_ms: u64,
    pub argv_hash: u64,
    pub env_hash: u64,
    pub would_coalesce: bool,
}

/// Wall-clock milliseconds since the Unix epoch (0 if the clock is before it).
pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Append one JSON line to the metrics file (created with parents if absent).
pub fn append(path: &Path, rec: &MetricRecord) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{}", serde_json::to_string(rec)?)?;
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub p50_wait_ms: u64,
    pub p95_wait_ms: u64,
    pub coalesce_rate: f64,
}

impl Summary {
    /// Go/no-go for the deferred coalescing daemon (Layer 1b): worth building
    /// only if a material share of invocations had a coalescing opportunity AND
    /// queue waits actually occurred.
    pub fn daemon_recommended(&self) -> bool {
        self.coalesce_rate >= 0.10 && self.p50_wait_ms > 0
    }

    /// One-line human summary for `vox build-broker stats`.
    pub fn render(&self) -> String {
        let verdict = if self.daemon_recommended() {
            "BUILD-DAEMON: recommended (coalesce>=10% and queue waits present)"
        } else {
            "BUILD-DAEMON: not needed (daemonless shim sufficient)"
        };
        format!(
            "builds={} p50_wait={}ms p95_wait={}ms coalesce_rate={:.1}% -> {}",
            self.count,
            self.p50_wait_ms,
            self.p95_wait_ms,
            self.coalesce_rate * 100.0,
            verdict
        )
    }
}

/// Read a metrics.jsonl file and summarize. Malformed lines are skipped so a
/// partially-written tail never breaks `stats`.
pub fn summarize(path: &Path) -> anyhow::Result<Summary> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    summarize_str(&text)
}

/// Aggregate every `<hash>/metrics.jsonl` under a worktree's `.vox/build-queue`
/// into one summary (there is usually one hash dir, but be robust to several).
pub fn summarize_worktree(worktree: &Path) -> anyhow::Result<Summary> {
    let root = worktree.join(".vox/build-queue");
    let mut all = String::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            let m = e.path().join("metrics.jsonl");
            if m.is_file() {
                all.push_str(&std::fs::read_to_string(&m).unwrap_or_default());
            }
        }
    }
    summarize_str(&all)
}

/// Summarize already-loaded JSONL content (used when aggregating across dirs).
pub fn summarize_str(text: &str) -> anyhow::Result<Summary> {
    let recs: Vec<MetricRecord> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let count = recs.len();
    if count == 0 {
        return Ok(Summary {
            count: 0,
            p50_wait_ms: 0,
            p95_wait_ms: 0,
            coalesce_rate: 0.0,
        });
    }
    let mut waits: Vec<u64> = recs.iter().map(|r| r.queue_wait_ms).collect();
    waits.sort_unstable();
    let pct = |p: f64| waits[((waits.len() as f64 - 1.0) * p).round() as usize];
    let coalesce = recs.iter().filter(|r| r.would_coalesce).count() as f64 / count as f64;
    Ok(Summary {
        count,
        p50_wait_ms: pct(0.50),
        p95_wait_ms: pct(0.95),
        coalesce_rate: coalesce,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(wait: u64, coalesce: bool) -> MetricRecord {
        MetricRecord {
            ts_ms: 0,
            worktree: "wt".into(),
            subcmd: "test".into(),
            queue_wait_ms: wait,
            ran_ms: 100,
            argv_hash: 1,
            env_hash: 2,
            would_coalesce: coalesce,
        }
    }

    #[test]
    fn append_then_summarize() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("metrics.jsonl");
        for (w, c) in [(0, false), (100, true), (200, false), (300, false)] {
            append(&p, &rec(w, c)).unwrap();
        }
        let s = summarize(&p).unwrap();
        assert_eq!(s.count, 4);
        assert_eq!(s.p50_wait_ms, 200); // round((4-1)*0.5)=2 -> sorted[2]=200
        assert!((s.coalesce_rate - 0.25).abs() < 1e-9);
    }

    #[test]
    fn summarize_missing_file_is_empty() {
        let s = summarize(Path::new("does-not-exist.jsonl")).unwrap();
        assert_eq!(s.count, 0);
    }

    #[test]
    fn verdict_not_needed_without_coalesce_or_waits() {
        let s = Summary {
            count: 10,
            p50_wait_ms: 0,
            p95_wait_ms: 0,
            coalesce_rate: 0.0,
        };
        assert!(!s.daemon_recommended());
        assert!(s.render().contains("not needed"));
    }

    #[test]
    fn verdict_recommended_with_coalesce_and_waits() {
        let s = Summary {
            count: 10,
            p50_wait_ms: 50,
            p95_wait_ms: 200,
            coalesce_rate: 0.2,
        };
        assert!(s.daemon_recommended());
        assert!(s.render().contains("recommended"));
    }

    #[test]
    fn summarize_worktree_aggregates_hash_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let wt = tmp.path();
        for h in ["aaa", "bbb"] {
            let d = wt.join(".vox/build-queue").join(h);
            std::fs::create_dir_all(&d).unwrap();
            append(&d.join("metrics.jsonl"), &rec(10, true)).unwrap();
        }
        let s = summarize_worktree(wt).unwrap();
        assert_eq!(s.count, 2);
    }

    #[test]
    fn malformed_lines_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("m.jsonl");
        std::fs::write(&p, "not json\n").unwrap();
        append(&p, &rec(5, false)).unwrap();
        assert_eq!(summarize(&p).unwrap().count, 1);
    }
}
