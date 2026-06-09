//! Logic bits for training loop.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use crate::config::LoraTrainingConfig;
use crate::model::CheckpointSegment;
use candle_core::Tensor;
use qlora_rs::training::QLoraTrainer;
use vox_tensor::data::TrainingPair;

/// Gradient-checkpointed backward + optimizer step.
///
/// The checkpointed forward severed the autograd tape at each segment boundary,
/// so `loss`'s graph spans only the **final** segment + head. To get correct
/// grads for every layer's LoRA params without ever materializing the full-model
/// backward graph (the single-backward peak that OOMs 3B on 16GB), we:
///
/// 1. `loss.backward()` — grads for the last segment's LoRA Vars **and**
///    `dL/d(input)` at the last segment's boundary activation.
/// 2. Walk segments in reverse (excluding the last): recompute the segment's
///    forward from its stored detached boundary (tape live), then inject the
///    upstream cotangent via [`qlora_rs::backward_from_cotangent`] (candle 0.9's
///    `backward()` only seeds `ones_like`, so the surrogate-scalar VJP is how we
///    start backprop from `dL/d(seg_out)`). Fold the segment's Var grads into the
///    accumulator and take `dL/d(seg_input)` as the next upstream cotangent.
/// 3. One [`QLoraTrainer::optimizer_step_with_grads`] (clip + AdamW), honoring
///    grad-accumulation cadence identically to the eager path.
///
/// Correctness (grads identical to a full-graph backward) is proven by
/// `qlora_rs::training::tests::checkpointed_backward_matches_full_graph_backward`.
pub fn checkpointed_backward_step(
    trainer: &mut QLoraTrainer,
    model: &crate::candle_qlora_train::TrainGraphModel,
    loss: &Tensor,
    segments: Vec<CheckpointSegment>,
) -> anyhow::Result<()> {
    if segments.is_empty() {
        anyhow::bail!("checkpointed_backward_step: no segments");
    }

    // Mirror backward_step's grad-accumulation loss scaling so numerics match.
    let accum_steps = trainer
        .config
        .adapter_config
        .gradient_accumulation_steps
        .max(1);
    let scaled_loss = if accum_steps > 1 {
        let scale = Tensor::new(1.0f32 / accum_steps as f32, loss.device())?;
        loss.broadcast_mul(&scale)?
    } else {
        loss.clone()
    };

    let all_vars = trainer.trainable_vars();

    // Step 1: backward over the final segment + head. This store becomes our
    // accumulator (we own it); it already holds the last segment's Var grads.
    let mut grads = scaled_loss.backward()?;

    // Upstream cotangent for the previous segment = dL/d(last segment input).
    let last = segments.last().expect("non-empty");
    let mut upstream = grads
        .get(&last.input)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!(
            "checkpointed backward: missing boundary grad for final segment input"
        ))?;

    // Step 2: recompute-and-backward each earlier segment in reverse.
    for seg in segments.iter().rev().skip(1) {
        let (input_var, seg_out) = model.recompute_segment(seg)?;
        let seg_grads = qlora_rs::backward_from_cotangent(&seg_out, &upstream)?;
        // Fold this segment's LoRA Var grads into the accumulator. Each segment's
        // recompute only touches its own layers' Vars, so this is auto-scoped.
        qlora_rs::accumulate_grads_for_vars(&mut grads, &seg_grads, &all_vars)?;
        // Next upstream cotangent = dL/d(this segment's input boundary).
        upstream = seg_grads.get(input_var.as_tensor()).cloned().ok_or_else(|| {
            anyhow::anyhow!("checkpointed backward: missing boundary grad for segment input")
        })?;
    }
    // `upstream` now holds dL/d(embedding); embeddings are frozen so it is unused.

    // Step 3: clip + optimizer step from the combined grads.
    trainer
        .optimizer_step_with_grads(grads)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

pub fn trajectory_weight_for_pair(pair: &TrainingPair, config: &LoraTrainingConfig) -> (f64, bool) {
    if !config.trajectory_weighting_enabled {
        return (1.0, false);
    }
    let mut weight = 1.0_f64;
    if let Some(category) = pair.category.as_deref() {
        let c = category.to_ascii_lowercase();
        if c.contains("tool_trace") || c.contains("trajectory") {
            weight *= config.trajectory_tool_trace_boost.max(0.0) as f64;
        }
        if c.contains("fail") || c.contains("error") {
            weight *= config.trajectory_failure_category_boost.max(0.0) as f64;
        }
    }
    if let (Some(floor), Some(rating)) = (config.trajectory_quality_floor, pair.rating)
        && rating >= floor
    {
        weight *= config.trajectory_quality_boost.max(0.0) as f64;
    }
    if !weight.is_finite() {
        return (1.0, true);
    }
    const MAX_TRAJECTORY_WEIGHT: f64 = 8.0;
    let clamped = weight.clamp(0.0, MAX_TRAJECTORY_WEIGHT);
    let was_clamped = (clamped - weight).abs() > f64::EPSILON;
    (clamped, was_clamped)
}
