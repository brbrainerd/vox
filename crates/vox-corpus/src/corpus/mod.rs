//! Portable corpus utilities (mix YAML, structured eval helpers, decoding modes).

pub mod agentic_synth;
pub mod augment;
pub mod benchmark;
pub mod constrained_decoding;
pub mod corpus_readiness;
pub mod coverage;
pub mod decl_coverage;
pub mod dogfood;
pub mod dpo;
pub mod eval_agentic_metrics;
pub mod eval_rust_metrics;
pub mod extract_docs;
pub mod extract_rs;
pub mod extract_vox;
pub mod log_ingest;
pub mod mix;
pub mod preflight;
pub mod prompt_gen;
pub mod rust_authoring;
pub mod structured_eval;
pub mod argument_generation_synth;
pub mod eval_split;
pub mod harness_union;
pub mod tool_selection_synth;
pub mod trace_ingest;

pub use benchmark::produce_benchmark;
pub use log_ingest::ingest_training_logs;
pub use mix::{
    ASR_REFINE_INSTRUCTION, MixConfigSchema, MixRunOptions, MixRunReport, MixSourceReportRow,
    normalize_training_jsonl_line, run_mix, run_mix_with_options,
};
