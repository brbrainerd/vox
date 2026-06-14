//! Prompt Tuning implementation.
//!
//! Prompt tuning prepends learnable "soft prompt" embeddings to the input,
//! allowing the model to be steered without modifying weights.
//!
//! Reference: <https://arxiv.org/abs/2104.08691>

use candle_core::{Device, Tensor};
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig};

/// Configuration for prompt tuning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTuningConfig {
    /// Number of virtual tokens (soft prompt length).
    pub num_virtual_tokens: usize,

    /// Hidden size of the model embeddings.
    pub hidden_size: usize,

    /// Initialization strategy.
    #[serde(default)]
    pub init_strategy: PromptInit,
}

/// Initialization strategy for soft prompts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum PromptInit {
    /// Random initialization from normal distribution.
    #[default]
    Random,
    /// Initialize from text tokens (requires tokenizer).
    Text(String),
}

impl Default for PromptTuningConfig {
    fn default() -> Self {
        Self {
            num_virtual_tokens: 20,
            hidden_size: 768,
            init_strategy: PromptInit::Random,
        }
    }
}

impl AdapterConfig for PromptTuningConfig {
    fn validate(&self) -> Result<()> {
        if self.num_virtual_tokens == 0 {
            return Err(PeftError::InvalidConfig(
                "num_virtual_tokens must be > 0".into(),
            ));
        }
        if self.hidden_size == 0 {
            return Err(PeftError::InvalidConfig("hidden_size must be > 0".into()));
        }
        Ok(())
    }
}

/// Prompt tuning layer.
///
/// Maintains soft prompt embeddings that are prepended to input embeddings.
pub struct PromptTuningLayer {
    /// Soft prompt embeddings: [`num_virtual_tokens`, `hidden_size`]
    soft_prompt: Tensor,
    /// Configuration
    config: PromptTuningConfig,
}

impl PromptTuningLayer {
    /// Create a new prompt tuning layer with random initialization.
    ///
    /// # Arguments
    /// * `config` - Prompt tuning configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails or layer construction fails.
    pub fn new(config: PromptTuningConfig, device: &Device) -> Result<Self> {
        config.validate()?;

        let soft_prompt = Tensor::randn(
            0.0f32,
            0.02,
            (config.num_virtual_tokens, config.hidden_size),
            device,
        )?;

        Ok(Self {
            soft_prompt,
            config,
        })
    }

    /// Get the soft prompt embeddings.
    #[must_use]
    pub fn soft_prompt(&self) -> &Tensor {
        &self.soft_prompt
    }

    /// Prepend soft prompts to input embeddings.
    ///
    /// # Arguments
    /// * `input_embeds` - Input embeddings [batch, `seq_len`, hidden]
    ///
    /// # Returns
    /// Concatenated embeddings [batch, `num_virtual_tokens` + `seq_len`, hidden]
    ///
    /// # Errors
    ///
    /// Returns an error if tensor operations fail.
    pub fn prepend_to_input(&self, input_embeds: &Tensor) -> Result<Tensor> {
        let batch_size = input_embeds.dim(0)?;

        // Expand soft prompt for batch: [1, num_virtual_tokens, hidden] -> [batch, ...]
        let expanded_prompt = self.soft_prompt.unsqueeze(0)?.expand((
            batch_size,
            self.config.num_virtual_tokens,
            self.config.hidden_size,
        ))?;

        // Concatenate along sequence dimension
        Ok(Tensor::cat(&[&expanded_prompt, input_embeds], 1)?)
    }
}

impl Adapter for PromptTuningLayer {
    type Config = PromptTuningConfig;

    fn forward(&self, input: &Tensor, _base_output: Option<&Tensor>) -> Result<Tensor> {
        self.prepend_to_input(input)
    }

    fn num_parameters(&self) -> usize {
        self.config.num_virtual_tokens * self.config.hidden_size
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::DType;

    #[test]
    fn test_prompt_tuning_creation() {
        let config = PromptTuningConfig::default();
        let device = Device::Cpu;
        let layer = PromptTuningLayer::new(config, &device);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_prepend_to_input() {
        let config = PromptTuningConfig {
            num_virtual_tokens: 10,
            hidden_size: 768,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = PromptTuningLayer::new(config, &device).unwrap();

        let input = Tensor::zeros(&[2, 20, 768], DType::F32, &device).unwrap();
        let output = layer.prepend_to_input(&input).unwrap();

        // Output should be [2, 10+20, 768] = [2, 30, 768]
        assert_eq!(output.shape().dims(), &[2, 30, 768]);
    }

    #[test]
    fn test_num_parameters() {
        let config = PromptTuningConfig {
            num_virtual_tokens: 20,
            hidden_size: 768,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = PromptTuningLayer::new(config, &device).unwrap();

        assert_eq!(layer.num_parameters(), 20 * 768);
    }
}
