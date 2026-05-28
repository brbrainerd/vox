//! Commit-graph signal producer.
//!
//! Walks the last `ctx.commit_window` commits reachable from `HEAD` via `gix`
//! and inspects each commit's subject line (first line of the message) for
//! conventional-commit-style patterns indicating publishable work:
//!
//! | Subject pattern (case-insensitive prefix or keyword) | Candidate class            |
//! |------------------------------------------------------|----------------------------|
//! | `perf:` / `perf(`                                    | `algorithmic_improvement`  |
//! | `refactor:` / `refactor(`                            | `reproducibility_infra`    |
//! | `test:` / `tests:` / `test(`                         | `reproducibility_infra`    |
//! | `feat:` ... containing "compress" or "optimize"      | `algorithmic_improvement`  |
//! | `policy:` / `gov(` / `governance`                    | `policy_governance`        |
//!
//! Determinism: `finding_id` is `algimp-<date>-<sha7>` (or class-prefixed
//! analog), where `<date>` is derived from `ctx.now_ms` (not commit timestamp,
//! so re-running on a fixed `now_ms` produces stable ids). `<sha7>` is the
//! first 7 hex chars of the commit OID.
//!
//! Missing or corrupt repo → empty output (we never panic).

use async_trait::async_trait;
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "commit_graph";

/// Match outcome for a single commit subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitClass {
    AlgorithmicImprovement,
    ReproducibilityInfra,
    PolicyGovernance,
}

impl CommitClass {
    fn id_prefix(&self) -> &'static str {
        match self {
            Self::AlgorithmicImprovement => "algimp",
            Self::ReproducibilityInfra => "repinf",
            Self::PolicyGovernance => "polgov",
        }
    }
}

pub struct CommitGraphProducer;

impl CommitGraphProducer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommitGraphProducer {
    fn default() -> Self {
        Self::new()
    }
}

/// Heuristic subject-line classifier.
///
/// Public so unit tests can exercise the rules without needing a real repo.
pub fn classify_subject(subject: &str) -> Option<CommitClass> {
    let s = subject.trim().to_ascii_lowercase();
    if s.starts_with("perf:") || s.starts_with("perf(") {
        return Some(CommitClass::AlgorithmicImprovement);
    }
    if s.starts_with("refactor:") || s.starts_with("refactor(") {
        return Some(CommitClass::ReproducibilityInfra);
    }
    if s.starts_with("test:") || s.starts_with("tests:") || s.starts_with("test(") {
        return Some(CommitClass::ReproducibilityInfra);
    }
    if (s.starts_with("feat:") || s.starts_with("feat("))
        && (s.contains("compress") || s.contains("optimize") || s.contains("speedup"))
    {
        return Some(CommitClass::AlgorithmicImprovement);
    }
    if s.starts_with("policy:")
        || s.starts_with("gov(")
        || s.starts_with("gov:")
        || s.contains("governance")
    {
        return Some(CommitClass::PolicyGovernance);
    }
    None
}

#[async_trait]
impl Producer for CommitGraphProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        scan_commits(&ctx.repo_root, ctx.commit_window, ctx.now_ms, &ctx.session_id)
    }
}

/// Synchronous core; testable without a Tokio runtime.
fn scan_commits(
    repo_root: &std::path::Path,
    commit_window: usize,
    now_ms: i64,
    session_id: &str,
) -> Vec<ResearchEvent> {
    let repo = match gix::open(repo_root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let head = match repo.head_id() {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(head.detach());

    let slug = date_slug(now_ms);

    while let Some(oid) = queue.pop_front() {
        if out.len() >= commit_window {
            break;
        }
        if !visited.insert(oid) {
            continue;
        }
        let Ok(commit) = repo.find_commit(oid) else {
            continue;
        };
        let Ok(decoded) = commit.decode() else {
            continue;
        };
        // `decoded.message` is the full commit message as a `&BStr`. Take the
        // first line as the subject.
        let message_bytes: &[u8] = decoded.message.as_ref();
        let subject_end = message_bytes
            .iter()
            .position(|&b| b == b'\n')
            .unwrap_or(message_bytes.len());
        let subject = std::str::from_utf8(&message_bytes[..subject_end]).unwrap_or("");
        if let Some(class) = classify_subject(subject) {
            let oid_hex = oid.to_string();
            let sha7 = &oid_hex[..7.min(oid_hex.len())];
            let finding_id = format!("{}-{}-{}", class.id_prefix(), slug, sha7);
            // Confidence is a fixed heuristic baseline; downstream worthiness
            // gates produce the real signal_strength based on evidence.
            let worthiness_score = 0.4;
            out.push(ResearchEvent::FindingCandidateProposed {
                finding_id,
                claim_ids: vec![],
                worthiness_score,
                session_id: session_id.to_string(),
            });
        }
        for p in commit.parent_ids() {
            queue.push_back(p.detach());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_subject_classifies_as_algimp() {
        assert_eq!(
            classify_subject("perf: reduce allocations in foo"),
            Some(CommitClass::AlgorithmicImprovement)
        );
        assert_eq!(
            classify_subject("perf(parser): cut p95 by 23%"),
            Some(CommitClass::AlgorithmicImprovement)
        );
    }

    #[test]
    fn refactor_and_test_classify_as_repinf() {
        assert_eq!(
            classify_subject("refactor: extract producer trait"),
            Some(CommitClass::ReproducibilityInfra)
        );
        assert_eq!(
            classify_subject("test: add round-trip coverage"),
            Some(CommitClass::ReproducibilityInfra)
        );
        assert_eq!(
            classify_subject("tests: add round-trip coverage"),
            Some(CommitClass::ReproducibilityInfra)
        );
    }

    #[test]
    fn feat_with_compress_keyword_classifies_as_algimp() {
        assert_eq!(
            classify_subject("feat: compress legacy log format"),
            Some(CommitClass::AlgorithmicImprovement)
        );
        assert_eq!(classify_subject("feat: new theme picker"), None);
    }

    #[test]
    fn policy_subjects_classify_as_polgov() {
        assert_eq!(
            classify_subject("policy: tighten approval gate"),
            Some(CommitClass::PolicyGovernance)
        );
        assert_eq!(
            classify_subject("docs: add governance section"),
            Some(CommitClass::PolicyGovernance)
        );
    }

    #[test]
    fn unrelated_subjects_return_none() {
        assert_eq!(classify_subject("fix: typo"), None);
        assert_eq!(classify_subject("chore: bump deps"), None);
        assert_eq!(classify_subject(""), None);
    }

    #[test]
    fn missing_repo_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let out = scan_commits(tmp.path(), 100, 1_747_000_000_000, "s");
        assert!(out.is_empty());
    }
}
