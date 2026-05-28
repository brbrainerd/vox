//! Telemetry helpers for the training loop.
//!
//! Ported from vox-populi (SP3 sub-batch C).
//! GPU metrics (HardwareRegistry::monitor) are STUBBED — returns None.
//! Sub-batch D can reconnect via a host capability.

use crate::telemetry_schema;

#[allow(clippy::too_many_arguments)]
pub fn build_train_step_payload(
    epoch: usize,
    global_step: u32,
    optimizer_step_count: u32,
    loss_val: f32,
    lr_applied_this_step: f64,
    eta_s: Option<u64>,
    total_optimizer_steps_planned: u32,
    skip_no_supervised_positions: u64,
    skip_short_seq: u64,
    skip_curriculum: u64,
    skip_token_id_oob: u64,
    trajectory_weighted_pairs: u64,
    trajectory_clamped_pairs: u64,
    ema_steps_per_sec: Option<f64>,
    total_valid_tokens: u64,
    total_theoretical_tokens: u64,
    batch_size: u64,
    seq_len: u64,
    syntax_weight_sum: f64,
) -> serde_json::Value {
    let supervised_ratio_pct = if total_theoretical_tokens == 0 {
        0.0
    } else {
        (total_valid_tokens as f64 / total_theoretical_tokens as f64) * 100.0
    };
    let token_throughput_proxy = ema_steps_per_sec.map(|s| s * batch_size as f64 * seq_len as f64);
    let mut payload = serde_json::json!({
        telemetry_schema::keys::EPOCH: epoch,
        telemetry_schema::keys::STEP: global_step,
        "optimizer_step": optimizer_step_count,
        telemetry_schema::keys::LOSS: loss_val,
        telemetry_schema::keys::LR: lr_applied_this_step,
        telemetry_schema::keys::LEARNING_RATE: lr_applied_this_step,
        telemetry_schema::keys::ETA_SECONDS_REMAINING: eta_s,
        telemetry_schema::keys::PROGRESS_FRACTION: optimizer_step_count as f64 / total_optimizer_steps_planned.max(1) as f64,
        telemetry_schema::keys::STEPS_PER_SEC_EMA: ema_steps_per_sec,
        telemetry_schema::keys::TOKENS_PER_SEC: token_throughput_proxy,
        telemetry_schema::keys::TOKENS_PER_SEC_IS_PROXY: true,
        telemetry_schema::keys::VALID_TOKENS: total_valid_tokens,
        telemetry_schema::keys::THEORETICAL_TOKENS: total_theoretical_tokens,
        telemetry_schema::keys::SUPERVISED_RATIO_PCT: supervised_ratio_pct,
        "skip_no_supervised_positions": skip_no_supervised_positions,
        "skip_short_seq": skip_short_seq,
        "skip_curriculum": skip_curriculum,
        "skip_token_id_oob": skip_token_id_oob,
        "trajectory_weighted_pairs": trajectory_weighted_pairs,
        "trajectory_clamped_pairs": trajectory_clamped_pairs,
        "syntax_weight_sum": syntax_weight_sum,
    });

    // SP3-C stub: HardwareRegistry::monitor() is vox-populi-internal.
    // GPU metrics (gpu_temp_c, gpu_power_w, etc.) are omitted here.
    // Sub-batch D can reconnect via a plugin host capability.
    let _ = &mut payload; // suppress unused_mut if gpu block is removed

    payload
}
