//! Logic bits for training loop.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use crate::config::LoraTrainingConfig;
use vox_tensor::data::TrainingPair;

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
