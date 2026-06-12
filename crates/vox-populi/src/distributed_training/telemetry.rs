//! Distributed training telemetry naming (Mn-T10).
//!
//! Runtimes should attach these as tracing span names / OpenTelemetry attributes under `vox.train.*`.

/// Root span for a logical training session.
pub const SPAN_SESSION_ROOT: &str = "vox.train.session";

/// One optimizer step on a rank.
pub const SPAN_TRAIN_STEP: &str = "vox.train.step";

/// Gradient shard journal / all-reduce envelope.
pub const SPAN_GRADIENT_SHARD: &str = "vox.train.gradient_shard";

/// Checkpoint emission + CAS persist.
pub const SPAN_CHECKPOINT: &str = "vox.train.checkpoint";
