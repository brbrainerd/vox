//! Producer composition. Holds an ordered list of producers, runs them
//! sequentially (producers are I/O-bound and small in number; parallel
//! execution is not yet worth the complexity), then deduplicates the
//! combined output by `finding_id`.

use super::dedup;
use super::producer::{Producer, ProducerContext};
use vox_research_events::ResearchEvent;

/// Container for one or more `Producer` implementations.
pub struct ProducerRegistry {
    producers: Vec<Box<dyn Producer>>,
}

impl ProducerRegistry {
    /// Empty registry. Tests register producers explicitly; production code
    /// uses [`ProducerRegistry::default_with_codex`].
    pub fn new() -> Self {
        Self {
            producers: Vec::new(),
        }
    }

    /// Add a producer to the end of the run order.
    pub fn register(&mut self, p: Box<dyn Producer>) {
        self.producers.push(p);
    }

    /// Names of registered producers, in run order.
    pub fn producer_names(&self) -> Vec<&'static str> {
        self.producers.iter().map(|p| p.name()).collect()
    }

    /// Number of registered producers.
    pub fn len(&self) -> usize {
        self.producers.len()
    }

    /// Whether the registry has any producers registered.
    pub fn is_empty(&self) -> bool {
        self.producers.is_empty()
    }

    /// Run every producer against `ctx` in order, then dedup the combined
    /// `FindingCandidateProposed` output by `finding_id`.
    pub async fn run_all(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        let mut out = Vec::new();
        for p in &self.producers {
            let mut events = p.observe(ctx).await;
            out.append(&mut events);
        }
        dedup::dedup_finding_candidates(out)
    }

    /// Production registry wiring all Phase A producers — the three
    /// DB-backed detectors (bench-history, Socrates-telemetry) plus the
    /// six file/working-tree-scan detectors (commit-graph, doc-corpus,
    /// test-corpus, ADR-emergence, API-surface, dep-adoption).
    pub fn default_with_codex(codex: vox_db::VoxDb) -> Self {
        let mut reg = Self::new();
        // File/working-tree scans (deterministic, no DB).
        reg.register(Box::new(super::commit_graph::CommitGraphProducer::new()));
        reg.register(Box::new(super::doc_corpus::DocCorpusProducer::new()));
        reg.register(Box::new(super::test_corpus::TestCorpusProducer::new()));
        reg.register(Box::new(super::adr_emergence::AdrEmergenceProducer::new()));
        reg.register(Box::new(super::api_surface::ApiSurfaceProducer::new()));
        reg.register(Box::new(super::dep_adoption::DepAdoptionProducer::new()));
        // DB-backed detectors (need a Codex handle).
        reg.register(Box::new(super::bench_history::BenchHistoryProducer::new(
            codex.clone(),
        )));
        reg.register(Box::new(
            super::socrates_telemetry::SocratesTelemetryProducer::new(codex),
        ));
        reg
    }
}

impl Default for ProducerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
