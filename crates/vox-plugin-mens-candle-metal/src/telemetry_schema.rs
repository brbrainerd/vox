//! Stable telemetry event names and payload keys (kernel-agnostic where possible).
//!
//! Ported verbatim from `vox-populi/src/mens/tensor/telemetry_schema.rs` (SP3 sub-batch C).

/// Top-level JSONL line: `event` field.
pub mod events {
    pub const TRAIN_START: &str = "train_start";
    pub const TRAIN_STEP: &str = "step";
    pub const TRAIN_COMPLETE: &str = "train_complete";
    pub const GPU_FALLBACK: &str = "gpu_fallback";
}

/// Common payload keys across Burn and Candle.
pub mod keys {
    pub const TRAIN_FILE: &str = "train_file";
    pub const OUTPUT_DIR: &str = "output";
    pub const SEED: &str = "seed";
    pub const GRAD_ACCUM: &str = "grad_accum";
    pub const PAIRS_LOADED: &str = "pairs_loaded";
    pub const EXECUTION_KERNEL: &str = "execution_kernel";
    pub const CONTRACT_DIGEST: &str = "contract_digest";
    pub const TELEMETRY_SCHEMA: &str = "telemetry_schema";
    pub const CANDLE_COMPAT_MODE: &str = "candle_compat_mode";
    pub const EPOCH: &str = "epoch";
    pub const STEP: &str = "step";
    pub const LOSS: &str = "loss";
    pub const LR: &str = "lr";
    pub const LEARNING_RATE: &str = "learning_rate";
    pub const TOKENS_PER_SEC: &str = "tokens_per_sec";
    pub const TOKENS_PER_SEC_IS_PROXY: &str = "tokens_per_sec_is_proxy";
    pub const VALID_TOKENS: &str = "valid_tokens";
    pub const THEORETICAL_TOKENS: &str = "theoretical_tokens";
    pub const SUPERVISED_RATIO_PCT: &str = "supervised_ratio_pct";
    pub const PLANNED_STEPS_PER_EPOCH: &str = "planned_steps_per_epoch";
    pub const PLANNED_STEPS_TOTAL: &str = "planned_steps_total";
    pub const EPOCHS: &str = "epochs";
    pub const ETA_SECONDS_REMAINING: &str = "eta_seconds_remaining";
    pub const PROGRESS_FRACTION: &str = "progress_fraction";
    pub const STEPS_PER_SEC_EMA: &str = "steps_per_sec_ema";
    pub const ROUTING_EFFICIENCY: &str = "routing_efficiency";
}

pub const TELEMETRY_SCHEMA_VERSION: u32 = 1;
