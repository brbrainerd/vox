//! Commit-watcher producer: deterministic signal extraction from one commit.
//!
//! [`signals_from_commit`] is PURE — no git, no I/O. The CLI is responsible for
//! materializing [`CommitView`]s from `git log --numstat` and for the
//! intake-gate / draft-insert wiring; this module only decides which
//! research-worthy [`DiscoverySignal`]s a single commit warrants.
//!
//! A commit with no signal (chore/docs/style/test-only with nothing else)
//! yields an empty vector and the CLI creates NO candidate for it.

use crate::scientia_evidence::{
    DiscoverySignal, DiscoverySignalFamily, DiscoverySignalProvenance, DiscoverySignalStrength,
};
use std::sync::OnceLock;

/// A single commit's reviewable surface (filled by the CLI from `git log`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitView {
    pub sha: String,
    pub message: String,
    pub files_changed: Vec<String>,
    pub insertions: u64,
    pub deletions: u64,
}

/// Origin tag stamped into every signal this producer emits.
const ORIGIN: &str = "commit_watcher";

/// Compiled regex for a quantified performance claim: a number directly
/// followed (optionally after spaces) by a perf-relevant unit.
fn perf_claim_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // e.g. "12%", "1.5x", "300 ms", "40µs", "12 us", "9ns"
        // Number + perf unit. `%`/`x` need no boundary; the alpha units use a
        // trailing boundary so "must" / "nostalgia" cannot match "ms"/"ns".
        regex::Regex::new(r"(?i)\d+(\.\d+)?\s*(%|x\b|ms\b|µs\b|us\b|ns\b)")
            .expect("valid perf regex")
    })
}

/// The conventional-commit type prefix, lowercased (`"feat"`, `"perf"`, ...).
fn commit_type(message: &str) -> &str {
    let first = message.lines().next().unwrap_or("").trim_start();
    // Strip an optional scope: "feat(x): ..." -> "feat".
    first.split([':', '(', ' ']).next().unwrap_or("")
}

fn first_repo_path(c: &CommitView) -> Option<String> {
    c.files_changed
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn provenance(c: &CommitView) -> DiscoverySignalProvenance {
    DiscoverySignalProvenance {
        origin: Some(ORIGIN.to_string()),
        repo_path: first_repo_path(c),
        digest: Some(c.sha.clone()),
        ..Default::default()
    }
}

/// PURE: deterministic signal extraction from one commit. No git, no I/O.
///
/// Emits at most one signal per rule; chore/docs/style/test-only commits with
/// no other signal return an empty vector (no candidate).
#[must_use]
pub fn signals_from_commit(c: &CommitView) -> Vec<DiscoverySignal> {
    let mut out: Vec<DiscoverySignal> = Vec::new();
    let msg_lower = c.message.to_ascii_lowercase();
    let ctype = commit_type(&c.message).to_ascii_lowercase();
    let norm_files: Vec<String> = c.files_changed.iter().map(|f| normalize(f)).collect();

    // 1. Quantified perf claim on a perf/feat commit.
    if (ctype == "perf" || ctype == "feat") && perf_claim_regex().is_match(&c.message) {
        out.push(DiscoverySignal {
            code: "perf_delta_quantified".to_string(),
            summary: "Commit message states a quantified performance delta (number + unit) on a perf/feat change — corroborate with a benchmark pair before claiming.".to_string(),
            strength: DiscoverySignalStrength::Strong,
            source_ref: Some(format!("git:{}", c.sha)),
            family: DiscoverySignalFamily::BenchmarkPair,
            provenance: provenance(c),
        });
    }

    // 2. Golden-corpus growth.
    let golden_files = norm_files.iter().any(|f| {
        f.contains("goldens/")
            || (f.contains("golden") && f.ends_with(".vox"))
            || f.contains("/golden")
    });
    if golden_files || msg_lower.contains("golden") {
        out.push(DiscoverySignal {
            code: "golden_corpus_growth".to_string(),
            summary: "Change touches the golden corpus — a reproducible behavioral artifact worth tracking.".to_string(),
            strength: DiscoverySignalStrength::Supporting,
            source_ref: Some(format!("git:{}", c.sha)),
            family: DiscoverySignalFamily::ReproducibilityArtifact,
            provenance: provenance(c),
        });
    }

    // 3. New capability surface.
    let capability_surface = norm_files.iter().any(|f| {
        f.starts_with("contracts/")
            || f.contains("/contracts/")
            || f.ends_with("catalog.v1.yaml")
            || (f.starts_with("crates/") && f.ends_with("/src/lib.rs"))
    });
    if capability_surface {
        out.push(DiscoverySignal {
            code: "capability_surface_change".to_string(),
            summary: "Change touches a capability surface (contract, catalog, or a crate lib root) — may widen the linked corpus.".to_string(),
            strength: DiscoverySignalStrength::Supporting,
            source_ref: Some(format!("git:{}", c.sha)),
            family: DiscoverySignalFamily::LinkedCorpus,
            provenance: provenance(c),
        });
    }

    // 4. Benchmark file change.
    if norm_files.iter().any(|f| f.contains("bench")) {
        out.push(DiscoverySignal {
            code: "benchmark_touch".to_string(),
            summary: "Change touches a benchmark path (informational context only).".to_string(),
            strength: DiscoverySignalStrength::Informational,
            source_ref: Some(format!("git:{}", c.sha)),
            family: DiscoverySignalFamily::BenchmarkPair,
            provenance: provenance(c),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cv(message: &str, files: &[&str]) -> CommitView {
        CommitView {
            sha: "0123456789abcdef0123456789abcdef01234567".to_string(),
            message: message.to_string(),
            files_changed: files.iter().map(|s| (*s).to_string()).collect(),
            insertions: 10,
            deletions: 2,
        }
    }

    #[test]
    fn perf_delta_commit_yields_strong_signal() {
        let c = cv(
            "perf(interp): cut eval loop by 35% on db programs",
            &["crates/vox-interp/src/eval.rs"],
        );
        let sigs = signals_from_commit(&c);
        let perf = sigs
            .iter()
            .find(|s| s.code == "perf_delta_quantified")
            .expect("perf signal present");
        assert_eq!(perf.strength, DiscoverySignalStrength::Strong);
        assert_eq!(perf.family, DiscoverySignalFamily::BenchmarkPair);
        assert_eq!(
            perf.source_ref.as_deref(),
            Some("git:0123456789abcdef0123456789abcdef01234567")
        );
        assert_eq!(perf.provenance.origin.as_deref(), Some("commit_watcher"));
    }

    #[test]
    fn perf_claim_without_perf_or_feat_type_is_not_strong() {
        // A chore that mentions a percentage must NOT produce the perf signal.
        let c = cv("chore: bump coverage floor to 70%", &["Cargo.toml"]);
        let sigs = signals_from_commit(&c);
        assert!(sigs.iter().all(|s| s.code != "perf_delta_quantified"));
    }

    #[test]
    fn chore_commit_yields_no_candidate() {
        let c = cv("chore: tidy imports", &["crates/vox-cli/src/main.rs"]);
        assert!(
            signals_from_commit(&c).is_empty(),
            "chore-only commit must yield no signal"
        );
    }

    #[test]
    fn docs_only_commit_yields_no_candidate() {
        let c = cv("docs: clarify README", &["README.md"]);
        assert!(signals_from_commit(&c).is_empty());
    }

    #[test]
    fn golden_corpus_entry_is_supporting_signal() {
        let c = cv(
            "test: add db ops golden",
            &["crates/vox-cli/goldens/db_operations.vox"],
        );
        let sig = signals_from_commit(&c)
            .into_iter()
            .find(|s| s.code == "golden_corpus_growth")
            .expect("golden signal present");
        assert_eq!(sig.strength, DiscoverySignalStrength::Supporting);
        assert_eq!(sig.family, DiscoverySignalFamily::ReproducibilityArtifact);
    }

    #[test]
    fn capability_surface_change_detected() {
        let contract = cv(
            "feat: add discovery-signal schema",
            &["contracts/scientia/discovery-signal.schema.json"],
        );
        assert!(
            signals_from_commit(&contract)
                .iter()
                .any(|s| s.code == "capability_surface_change"),
            "contracts/ touch must surface a capability change"
        );

        let lib = cv("feat: new crate", &["crates/vox-thing/src/lib.rs"]);
        assert!(
            signals_from_commit(&lib)
                .iter()
                .any(|s| s.code == "capability_surface_change"),
            "new crate lib root must surface a capability change"
        );
    }

    #[test]
    fn benchmark_touch_is_informational() {
        let c = cv(
            "chore: tune bench harness",
            &["crates/vox-bench/src/throughput.rs"],
        );
        let sig = signals_from_commit(&c)
            .into_iter()
            .find(|s| s.code == "benchmark_touch")
            .expect("benchmark signal present");
        assert_eq!(sig.strength, DiscoverySignalStrength::Informational);
    }
}
