//! SCIENTIA Phase A — self-observation signal producers.
//!
//! Producers turn the developer's own activity in this workspace into
//! `FindingCandidate` rows via deterministic detectors (no LLM in the
//! producer path). LLM-grounded T2/T3 promotion happens downstream in the
//! existing claim extractor and worthiness gates.
//!
//! ## Composition
//!
//! ```text
//!     ProducerRegistry::default_with_codex(codex)
//!         │
//!         ├─ CommitGraphProducer       (algorithmic_improvement / reproducibility_infra)
//!         ├─ BenchHistoryProducer      (algorithmic_improvement)
//!         └─ SocratesTelemetryProducer (telemetry_trust)
//!         │
//!         ▼
//!     ResearchEvent::FindingCandidateProposed { … }
//! ```
//!
//! Persistence to `scientia_finding_candidates` is the caller's responsibility
//! (see `vox-cli scientia scout` in Phase F).

pub mod adr_emergence;
pub mod api_surface;
pub mod bench_history;
pub mod commit_graph;
pub mod dedup;
pub mod dep_adoption;
pub mod doc_corpus;
pub mod heuristics;
pub mod producer;
pub mod registry;
pub mod socrates_telemetry;
pub mod test_corpus;

pub use producer::{Producer, ProducerContext};
pub use registry::ProducerRegistry;

pub use adr_emergence::AdrEmergenceProducer;
pub use api_surface::ApiSurfaceProducer;
pub use bench_history::BenchHistoryProducer;
pub use commit_graph::CommitGraphProducer;
pub use dep_adoption::DepAdoptionProducer;
pub use doc_corpus::DocCorpusProducer;
pub use socrates_telemetry::SocratesTelemetryProducer;
pub use test_corpus::TestCorpusProducer;
