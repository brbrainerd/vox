//! Vox Console discovery engine: per-user exposure ledger, FSRS-style scheduler,
//! and suggestion ranking. Local and deterministic — no LLM.

pub mod fsrs;
pub mod ledger;
pub mod rank;

pub use fsrs::{MemoryState, Recall, update as fsrs_update};
