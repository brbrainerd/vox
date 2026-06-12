//! Vox Console discovery engine: per-user exposure ledger, FSRS-style scheduler,
//! and suggestion ranking. Local and deterministic — no LLM.

pub mod fsrs;
pub mod rank;

pub use fsrs::{update as fsrs_update, MemoryState, Recall};
