//! Curriculum logic.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

use crate::config::LoraTrainingConfig;

pub fn max_difficulty_for_epoch(epoch: usize, config: &LoraTrainingConfig) -> u8 {
    if !config.curriculum {
        return 10;
    }
    if let Some(ref sched) = config.curriculum_schedule {
        let val = match epoch {
            1 => sched.epoch_1_max_difficulty,
            2 => sched.epoch_2_max_difficulty,
            3 => sched.epoch_3_max_difficulty,
            _ => None,
        };
        if let Some(v) = val {
            return v;
        }
    }
    if config.epochs > 1 {
        let progress = (epoch - 1) as f32 / (config.epochs - 1) as f32;
        (3.0 + progress * 7.0).ceil() as u8
    } else {
        10
    }
}
