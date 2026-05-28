//! Documentation-corpus signal producer.
//!
//! Scans the repo's `docs/src/**.md` tree and emits a `reproducibility_infra`
//! candidate for each long-form markdown file (≥ [`MIN_LINES`] lines). The
//! finding_id is deterministic from `path + sha3(content)`, so re-runs over
//! the same content yield identical ids and the dedup layer collapses
//! duplicates.
//!
//! Heuristic: long-form internal documentation often becomes the kernel of
//! a "we built and documented X" reproducibility-infra paper.

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "doc_corpus";
/// Minimum line count for a markdown file to be considered "long-form".
pub const MIN_LINES: usize = 200;

pub struct DocCorpusProducer;

impl DocCorpusProducer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocCorpusProducer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Producer for DocCorpusProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        scan(&ctx.repo_root.join("docs").join("src"), ctx.now_ms, &ctx.session_id)
    }
}

fn scan(
    docs_root: &std::path::Path,
    now_ms: i64,
    session_id: &str,
) -> Vec<ResearchEvent> {
    if !docs_root.is_dir() {
        return Vec::new();
    }
    let slug = date_slug(now_ms);
    let mut out = Vec::new();
    walk_md(docs_root, &mut |path, contents| {
        let line_count = contents.lines().count();
        if line_count < MIN_LINES {
            return;
        }
        let mut h = Sha3_256::new();
        h.update(PRODUCER_NAME.as_bytes());
        h.update(b"::");
        h.update(path.to_string_lossy().as_bytes());
        h.update(b"::");
        h.update(contents.as_bytes());
        let digest = h.finalize();
        let sha8: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        let finding_id = format!("repinf-{slug}-doc-{sha8}");
        // Worthiness score scales with size up to a cap.
        let score = ((line_count as f64) / 1000.0).min(1.0);
        out.push(ResearchEvent::FindingCandidateProposed {
            finding_id,
            claim_ids: vec![],
            worthiness_score: score,
            session_id: session_id.to_string(),
        });
    });
    out
}

fn walk_md(dir: &std::path::Path, on_md: &mut dyn FnMut(&std::path::Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_md(&path, on_md);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                on_md(&path, &contents);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_md(dir: &std::path::Path, name: &str, line_count: usize) {
        std::fs::write(
            dir.join(name),
            "# Header\n".to_string() + &"This is a line of content.\n".repeat(line_count),
        )
        .unwrap();
    }

    #[test]
    fn empty_docs_dir_yields_no_events() {
        let tmp = tempfile::tempdir().unwrap();
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }

    #[test]
    fn missing_docs_dir_returns_empty_not_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let out = scan(&tmp.path().join("nope"), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }

    #[test]
    fn short_md_files_below_threshold_do_not_emit() {
        let tmp = tempfile::tempdir().unwrap();
        write_md(tmp.path(), "short.md", 10);
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }

    #[test]
    fn long_md_file_emits_repinf_candidate() {
        let tmp = tempfile::tempdir().unwrap();
        write_md(tmp.path(), "long.md", 300);
        let out = scan(tmp.path(), 1_747_000_000_000, "s");
        assert_eq!(out.len(), 1);
        match &out[0] {
            ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
                assert!(finding_id.starts_with("repinf-"));
                assert!(finding_id.contains("-doc-"));
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn finding_id_stable_across_runs_on_same_content() {
        let tmp = tempfile::tempdir().unwrap();
        write_md(tmp.path(), "stable.md", 300);
        let a = scan(tmp.path(), 1_747_000_000_000, "s");
        let b = scan(tmp.path(), 1_747_000_000_000, "s");
        assert_eq!(a, b);
    }
}
