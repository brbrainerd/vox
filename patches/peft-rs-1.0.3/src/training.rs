//! Training utilities for PEFT adapters.
//!
//! This module provides functionality for:
//! - Learning rate schedules for adapter training
//! - Training state management
//! - Parameter counting helpers

// Allow usize to f64 casts for learning rate calculations - this is standard in ML code
#![allow(clippy::cast_precision_loss)]

/// Learning rate schedule strategies.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LrSchedule {
    /// Constant learning rate
    #[default]
    Constant,
    /// Linear warmup from 0 to max LR
    LinearWarmup {
        /// Number of warmup steps
        warmup_steps: usize,
    },
    /// Cosine annealing from max LR to min LR
    CosineAnnealing {
        /// Total number of steps
        total_steps: usize,
        /// Minimum learning rate
        min_lr: f64,
    },
    /// Linear decay from max LR to min LR
    LinearDecay {
        /// Total number of steps
        total_steps: usize,
        /// Minimum learning rate
        min_lr: f64,
    },
}

impl LrSchedule {
    /// Compute the learning rate multiplier for the given step.
    ///
    /// # Arguments
    /// * `step` - Current training step (0-indexed)
    /// * `base_lr` - Base learning rate
    ///
    /// # Returns
    /// The learning rate for this step
    #[must_use]
    pub fn get_lr(&self, step: usize, base_lr: f64) -> f64 {
        match self {
            Self::Constant => base_lr,
            Self::LinearWarmup { warmup_steps } => {
                if *warmup_steps == 0 || step >= *warmup_steps {
                    base_lr
                } else {
                    base_lr * (step as f64 / *warmup_steps as f64)
                }
            }
            Self::CosineAnnealing {
                total_steps,
                min_lr,
            } => {
                if *total_steps == 0 || step >= *total_steps {
                    *min_lr
                } else {
                    let progress = step as f64 / *total_steps as f64;
                    let cosine_decay = f64::midpoint(1.0, (std::f64::consts::PI * progress).cos());
                    min_lr + (base_lr - min_lr) * cosine_decay
                }
            }
            Self::LinearDecay {
                total_steps,
                min_lr,
            } => {
                if *total_steps == 0 || step >= *total_steps {
                    *min_lr
                } else {
                    let progress = step as f64 / *total_steps as f64;
                    base_lr - (base_lr - min_lr) * progress
                }
            }
        }
    }
}

/// Configuration for adapter training.
#[derive(Debug, Clone)]
pub struct AdapterTrainingConfig {
    /// Base learning rate
    pub learning_rate: f64,
    /// Learning rate schedule
    pub lr_schedule: LrSchedule,
    /// Weight decay (L2 regularization)
    pub weight_decay: f64,
    /// Gradient accumulation steps
    pub gradient_accumulation_steps: usize,
    /// Maximum gradient norm for clipping (None = no clipping)
    pub max_grad_norm: Option<f64>,
}

impl Default for AdapterTrainingConfig {
    fn default() -> Self {
        Self {
            learning_rate: 1e-4,
            lr_schedule: LrSchedule::Constant,
            weight_decay: 0.0,
            gradient_accumulation_steps: 1,
            max_grad_norm: Some(1.0),
        }
    }
}

/// Training state for adapter fine-tuning.
#[derive(Debug, Clone)]
pub struct AdapterTrainingState {
    /// Current global step
    pub global_step: usize,
    /// Current epoch
    pub epoch: usize,
    /// Steps within current epoch
    pub steps_in_epoch: usize,
    /// Accumulated gradient steps (for gradient accumulation)
    pub accumulated_steps: usize,
    /// Best validation loss seen
    pub best_val_loss: Option<f64>,
    /// Training configuration
    config: AdapterTrainingConfig,
}

impl AdapterTrainingState {
    /// Create new training state with the given configuration.
    #[must_use]
    pub fn new(config: AdapterTrainingConfig) -> Self {
        Self {
            global_step: 0,
            epoch: 0,
            steps_in_epoch: 0,
            accumulated_steps: 0,
            best_val_loss: None,
            config,
        }
    }

    /// Get the current learning rate based on schedule.
    #[must_use]
    pub fn current_lr(&self) -> f64 {
        self.config
            .lr_schedule
            .get_lr(self.global_step, self.config.learning_rate)
    }

    /// Check if gradient accumulation is complete.
    #[must_use]
    pub fn should_update(&self) -> bool {
        self.accumulated_steps >= self.config.gradient_accumulation_steps
    }

    /// Step after processing a batch.
    ///
    /// Returns `true` if an optimizer step should be taken.
    pub fn step(&mut self) -> bool {
        self.accumulated_steps += 1;
        self.steps_in_epoch += 1;

        if self.should_update() {
            self.global_step += 1;
            self.accumulated_steps = 0;
            true
        } else {
            false
        }
    }

    /// Start a new epoch.
    pub fn new_epoch(&mut self) {
        self.epoch += 1;
        self.steps_in_epoch = 0;
    }

    /// Update best validation loss.
    ///
    /// Returns `true` if this is the new best loss.
    pub fn update_best_val_loss(&mut self, val_loss: f64) -> bool {
        match self.best_val_loss {
            Some(best) if val_loss >= best => false,
            _ => {
                self.best_val_loss = Some(val_loss);
                true
            }
        }
    }

    /// Get gradient accumulation steps.
    #[must_use]
    pub fn gradient_accumulation_steps(&self) -> usize {
        self.config.gradient_accumulation_steps
    }

    /// Get maximum gradient norm for clipping.
    #[must_use]
    pub fn max_grad_norm(&self) -> Option<f64> {
        self.config.max_grad_norm
    }

    /// Get weight decay.
    #[must_use]
    pub fn weight_decay(&self) -> f64 {
        self.config.weight_decay
    }
}

/// Count trainable parameters in an adapter.
///
/// # Arguments
/// * `adapter` - The adapter to count parameters for
///
/// # Returns
/// Number of trainable parameters
#[must_use]
pub fn count_trainable_parameters<A: crate::traits::Adapter>(adapter: &A) -> usize {
    adapter.num_parameters()
}

/// Format parameter count with appropriate units.
///
/// # Arguments
/// * `count` - Number of parameters
///
/// # Returns
/// Human-readable string (e.g., "12.3K", "1.5M", "2.1B")
#[must_use]
pub fn format_parameter_count(count: usize) -> String {
    if count >= 1_000_000_000 {
        format!("{:.2}B", count as f64 / 1_000_000_000.0)
    } else if count >= 1_000_000 {
        format!("{:.2}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.2}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_lr() {
        let schedule = LrSchedule::Constant;
        assert!((schedule.get_lr(0, 0.001) - 0.001).abs() < 1e-10);
        assert!((schedule.get_lr(100, 0.001) - 0.001).abs() < 1e-10);
        assert!((schedule.get_lr(1000, 0.001) - 0.001).abs() < 1e-10);
    }

    #[test]
    fn test_linear_warmup() {
        let schedule = LrSchedule::LinearWarmup { warmup_steps: 100 };
        assert!((schedule.get_lr(0, 0.001) - 0.0).abs() < 1e-10);
        assert!((schedule.get_lr(50, 0.001) - 0.0005).abs() < 1e-10);
        assert!((schedule.get_lr(100, 0.001) - 0.001).abs() < 1e-10);
        assert!((schedule.get_lr(200, 0.001) - 0.001).abs() < 1e-10);
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_cosine_annealing() {
        let schedule = LrSchedule::CosineAnnealing {
            total_steps: 100,
            min_lr: 0.0001,
        };

        // At step 0, should be at max LR
        let lr_0 = schedule.get_lr(0, 0.001);
        assert!((lr_0 - 0.001).abs() < 1e-10);

        // At halfway, should be at average of max and min
        let lr_50 = schedule.get_lr(50, 0.001);
        let expected_50 = 0.0001 + (0.001 - 0.0001) * 0.5;
        assert!((lr_50 - expected_50).abs() < 1e-6);

        // At end, should be at min LR
        let lr_100 = schedule.get_lr(100, 0.001);
        assert!((lr_100 - 0.0001).abs() < 1e-10);
    }

    #[test]
    fn test_linear_decay() {
        let schedule = LrSchedule::LinearDecay {
            total_steps: 100,
            min_lr: 0.0001,
        };

        assert!((schedule.get_lr(0, 0.001) - 0.001).abs() < 1e-10);
        assert!((schedule.get_lr(50, 0.001) - 0.00055).abs() < 1e-10);
        assert!((schedule.get_lr(100, 0.001) - 0.0001).abs() < 1e-10);
    }

    #[test]
    fn test_training_state_step() {
        let config = AdapterTrainingConfig {
            gradient_accumulation_steps: 4,
            ..Default::default()
        };
        let mut state = AdapterTrainingState::new(config);

        assert!(!state.step()); // 1/4
        assert!(!state.step()); // 2/4
        assert!(!state.step()); // 3/4
        assert!(state.step()); // 4/4 - should update
        assert_eq!(state.global_step, 1);
        assert_eq!(state.accumulated_steps, 0);

        assert!(!state.step()); // 1/4
        assert!(!state.step()); // 2/4
        assert!(!state.step()); // 3/4
        assert!(state.step()); // 4/4 - should update
        assert_eq!(state.global_step, 2);
    }

    #[test]
    fn test_training_state_epoch() {
        let config = AdapterTrainingConfig::default();
        let mut state = AdapterTrainingState::new(config);

        state.step();
        state.step();
        assert_eq!(state.steps_in_epoch, 2);

        state.new_epoch();
        assert_eq!(state.epoch, 1);
        assert_eq!(state.steps_in_epoch, 0);
    }

    #[test]
    fn test_best_val_loss() {
        let config = AdapterTrainingConfig::default();
        let mut state = AdapterTrainingState::new(config);

        assert!(state.update_best_val_loss(1.0));
        assert_eq!(state.best_val_loss, Some(1.0));

        assert!(state.update_best_val_loss(0.5));
        assert_eq!(state.best_val_loss, Some(0.5));

        assert!(!state.update_best_val_loss(0.8));
        assert_eq!(state.best_val_loss, Some(0.5));
    }

    #[test]
    fn test_format_parameter_count() {
        assert_eq!(format_parameter_count(100), "100");
        assert_eq!(format_parameter_count(1_234), "1.23K");
        assert_eq!(format_parameter_count(12_345_678), "12.35M");
        assert_eq!(format_parameter_count(1_234_567_890), "1.23B");
    }

    #[test]
    fn test_current_lr_with_schedule() {
        let config = AdapterTrainingConfig {
            learning_rate: 0.001,
            lr_schedule: LrSchedule::LinearWarmup { warmup_steps: 10 },
            ..Default::default()
        };
        let mut state = AdapterTrainingState::new(config);

        // At step 0, LR should be 0
        assert!((state.current_lr() - 0.0).abs() < 1e-10);

        // Advance 5 steps
        for _ in 0..5 {
            state.step();
        }
        assert!((state.current_lr() - 0.0005).abs() < 1e-10);

        // Advance 5 more steps
        for _ in 0..5 {
            state.step();
        }
        assert!((state.current_lr() - 0.001).abs() < 1e-10);
    }

    #[test]
    fn test_zero_warmup_steps() {
        // Edge case: zero warmup steps should return base_lr immediately
        let schedule = LrSchedule::LinearWarmup { warmup_steps: 0 };
        assert!((schedule.get_lr(0, 0.001) - 0.001).abs() < 1e-10);
        assert!((schedule.get_lr(100, 0.001) - 0.001).abs() < 1e-10);
    }

    #[test]
    fn test_zero_total_steps_cosine() {
        // Edge case: zero total steps should return min_lr immediately
        let schedule = LrSchedule::CosineAnnealing {
            total_steps: 0,
            min_lr: 0.0001,
        };
        assert!((schedule.get_lr(0, 0.001) - 0.0001).abs() < 1e-10);
    }

    #[test]
    fn test_zero_total_steps_linear_decay() {
        // Edge case: zero total steps should return min_lr immediately
        let schedule = LrSchedule::LinearDecay {
            total_steps: 0,
            min_lr: 0.0001,
        };
        assert!((schedule.get_lr(0, 0.001) - 0.0001).abs() < 1e-10);
    }
}
