//! I/O utilities for saving and loading adapter weights and configurations.
//!
//! This module provides functionality for:
//! - Saving adapter weights to safetensors format
//! - Loading adapter weights from safetensors format
//! - Saving adapter configurations to JSON
//! - Loading adapter configurations from JSON

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use candle_core::{Device, Tensor};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::{PeftError, Result};

/// Trait for adapters that can be saved and loaded.
pub trait SaveLoad {
    /// Get all adapter tensors as a map of name -> tensor.
    ///
    /// # Errors
    ///
    /// Returns an error if tensor retrieval fails.
    fn state_dict(&self) -> Result<HashMap<String, Tensor>>;

    /// Load adapter tensors from a state dict.
    ///
    /// # Errors
    ///
    /// Returns an error if tensor loading fails.
    fn load_state_dict(&mut self, state_dict: HashMap<String, Tensor>) -> Result<()>;
}

/// Save adapter weights to a safetensors file.
///
/// # Arguments
/// * `adapter` - The adapter implementing `SaveLoad` trait
/// * `path` - Path to save the safetensors file
///
/// # Errors
/// Returns an error if:
/// - Failed to get state dict from adapter
/// - Failed to serialize tensors to safetensors format
/// - Failed to write file to disk
pub fn save_adapter_weights<P: AsRef<Path>>(adapter: &dyn SaveLoad, path: P) -> Result<()> {
    let state_dict = adapter.state_dict()?;

    // Convert HashMap to Vec for safetensors
    let tensors: Vec<(&str, Tensor)> = state_dict
        .iter()
        .map(|(name, tensor)| (name.as_str(), tensor.clone()))
        .collect();

    // Use candle's built-in safetensors serialization
    safetensors::tensor::serialize_to_file(tensors, None, path.as_ref())
        .map_err(|e| PeftError::Io(format!("Failed to save safetensors: {e}")))?;

    Ok(())
}

/// Load adapter weights from a safetensors file.
///
/// # Arguments
/// * `adapter` - The adapter to load weights into
/// * `path` - Path to the safetensors file
/// * `device` - Device to load tensors on
///
/// # Errors
/// Returns an error if:
/// - Failed to read file from disk
/// - Failed to parse safetensors format
/// - Failed to load tensors into adapter
pub fn load_adapter_weights<P: AsRef<Path>>(
    adapter: &mut dyn SaveLoad,
    path: P,
    device: &Device,
) -> Result<()> {
    // Use candle's built-in safetensors loading
    let tensors = candle_core::safetensors::load(path.as_ref(), device)?;

    // Load into adapter
    adapter.load_state_dict(tensors)?;

    Ok(())
}

/// Save adapter configuration to a JSON file.
///
/// # Arguments
/// * `config` - The configuration to save
/// * `path` - Path to save the JSON file
///
/// # Errors
/// Returns an error if serialization or file writing fails
pub fn save_adapter_config<T: Serialize, P: AsRef<Path>>(config: &T, path: P) -> Result<()> {
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| PeftError::Io(format!("Failed to serialize config: {e}")))?;

    fs::write(path, json)
        .map_err(|e| PeftError::Io(format!("Failed to write config file: {e}")))?;

    Ok(())
}

/// Load adapter configuration from a JSON file.
///
/// # Arguments
/// * `path` - Path to the JSON file
///
/// # Errors
/// Returns an error if file reading or deserialization fails
pub fn load_adapter_config<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<T> {
    let json = fs::read_to_string(path)
        .map_err(|e| PeftError::Io(format!("Failed to read config file: {e}")))?;

    let config = serde_json::from_str(&json)
        .map_err(|e| PeftError::Io(format!("Failed to parse config: {e}")))?;

    Ok(config)
}

/// Default filename for adapter weights in `HuggingFace` PEFT format.
pub const ADAPTER_WEIGHTS_FILENAME: &str = "adapter_model.safetensors";

/// Default filename for adapter config in `HuggingFace` PEFT format.
pub const ADAPTER_CONFIG_FILENAME: &str = "adapter_config.json";

/// Save adapter weights and configuration to a directory in `HuggingFace` PEFT format.
///
/// Creates the directory if it doesn't exist. Saves:
/// - `adapter_model.safetensors` - Adapter weights
/// - `adapter_config.json` - Adapter configuration
///
/// # Arguments
/// * `adapter` - The adapter implementing `SaveLoad` trait
/// * `config` - The adapter configuration
/// * `dir` - Directory path to save to
///
/// # Errors
/// Returns an error if:
/// - Failed to create directory
/// - Failed to save weights or config
///
/// # Example
/// ```rust,ignore
/// use peft_rs::{save_pretrained, LoraConfig, LoraLayer};
///
/// let adapter = LoraLayer::new_with_zeros(768, 768, config, &device)?;
/// save_pretrained(&adapter, &config, "path/to/adapter")?;
/// ```
pub fn save_pretrained<T: Serialize, P: AsRef<Path>>(
    adapter: &dyn SaveLoad,
    config: &T,
    dir: P,
) -> Result<()> {
    let dir = dir.as_ref();

    // Create directory if it doesn't exist
    if !dir.exists() {
        fs::create_dir_all(dir)
            .map_err(|e| PeftError::Io(format!("Failed to create directory: {e}")))?;
    }

    // Save weights
    let weights_path = dir.join(ADAPTER_WEIGHTS_FILENAME);
    save_adapter_weights(adapter, &weights_path)?;

    // Save config
    let config_path = dir.join(ADAPTER_CONFIG_FILENAME);
    save_adapter_config(config, &config_path)?;

    Ok(())
}

/// Load adapter weights and configuration from a directory in `HuggingFace` PEFT format.
///
/// Expects:
/// - `adapter_model.safetensors` - Adapter weights
/// - `adapter_config.json` - Adapter configuration
///
/// # Arguments
/// * `adapter` - The adapter to load weights into
/// * `dir` - Directory path to load from
/// * `device` - Device to load tensors on
///
/// # Returns
/// The loaded adapter configuration
///
/// # Errors
/// Returns an error if:
/// - Directory doesn't exist
/// - Failed to load weights or config
///
/// # Example
/// ```rust,ignore
/// use peft_rs::{load_pretrained, LoraConfig, LoraLayer};
///
/// let mut adapter = LoraLayer::new_with_zeros(768, 768, LoraConfig::default(), &device)?;
/// let config: LoraConfig = load_pretrained(&mut adapter, "path/to/adapter", &device)?;
/// ```
pub fn load_pretrained<T: DeserializeOwned, P: AsRef<Path>>(
    adapter: &mut dyn SaveLoad,
    dir: P,
    device: &Device,
) -> Result<T> {
    let dir = dir.as_ref();

    if !dir.exists() {
        return Err(PeftError::Io(format!(
            "Directory does not exist: {}",
            dir.display()
        )));
    }

    // Load weights
    let weights_path = dir.join(ADAPTER_WEIGHTS_FILENAME);
    load_adapter_weights(adapter, &weights_path, device)?;

    // Load config
    let config_path = dir.join(ADAPTER_CONFIG_FILENAME);
    let config: T = load_adapter_config(&config_path)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use std::collections::HashMap;
    use tempfile::TempDir;

    struct MockAdapter {
        weights: HashMap<String, Tensor>,
    }

    impl SaveLoad for MockAdapter {
        fn state_dict(&self) -> Result<HashMap<String, Tensor>> {
            Ok(self.weights.clone())
        }

        fn load_state_dict(&mut self, state_dict: HashMap<String, Tensor>) -> Result<()> {
            self.weights = state_dict;
            Ok(())
        }
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_save_load_adapter_weights() -> anyhow::Result<()> {
        let device = Device::Cpu;
        let temp_dir = TempDir::new()?;
        let weights_path = temp_dir.path().join("adapter.safetensors");

        // Create mock adapter with some weights
        let mut weights = HashMap::new();
        weights.insert(
            "lora_a".to_string(),
            Tensor::randn(0f32, 1f32, (64, 8), &device)?,
        );
        weights.insert(
            "lora_b".to_string(),
            Tensor::randn(0f32, 1f32, (8, 64), &device)?,
        );

        let adapter = MockAdapter {
            weights: weights.clone(),
        };

        // Save weights
        save_adapter_weights(&adapter, &weights_path)?;
        assert!(weights_path.exists());

        // Load weights into new adapter
        let mut loaded_adapter = MockAdapter {
            weights: HashMap::new(),
        };
        load_adapter_weights(&mut loaded_adapter, &weights_path, &device)?;

        // Verify loaded weights exist and have correct properties
        assert_eq!(loaded_adapter.weights.len(), 2);
        assert!(loaded_adapter.weights.contains_key("lora_a"));
        assert!(loaded_adapter.weights.contains_key("lora_b"));

        // Verify shapes match
        assert_eq!(
            loaded_adapter.weights["lora_a"].dims(),
            weights["lora_a"].dims()
        );
        assert_eq!(
            loaded_adapter.weights["lora_b"].dims(),
            weights["lora_b"].dims()
        );

        // Verify tensor values are preserved (compare sum as a simple check)
        let original_a_sum = weights["lora_a"].sum_all()?.to_scalar::<f32>()?;
        let loaded_a_sum = loaded_adapter.weights["lora_a"]
            .sum_all()?
            .to_scalar::<f32>()?;
        assert!(
            (original_a_sum - loaded_a_sum).abs() < 1e-5,
            "lora_a sum mismatch: {original_a_sum} vs {loaded_a_sum}"
        );

        let original_b_sum = weights["lora_b"].sum_all()?.to_scalar::<f32>()?;
        let loaded_b_sum = loaded_adapter.weights["lora_b"]
            .sum_all()?
            .to_scalar::<f32>()?;
        assert!(
            (original_b_sum - loaded_b_sum).abs() < 1e-5,
            "lora_b sum mismatch: {original_b_sum} vs {loaded_b_sum}"
        );

        Ok(())
    }

    #[test]
    fn test_save_load_config() -> anyhow::Result<()> {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct TestConfig {
            r: usize,
            alpha: usize,
            dropout: f64,
        }

        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.json");

        let config = TestConfig {
            r: 8,
            alpha: 16,
            dropout: 0.1,
        };

        // Save config
        save_adapter_config(&config, &config_path)?;
        assert!(config_path.exists());

        // Load config
        let loaded_config: TestConfig = load_adapter_config(&config_path)?;
        assert_eq!(config, loaded_config);

        Ok(())
    }

    #[test]
    fn test_save_load_pretrained() -> anyhow::Result<()> {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct TestConfig {
            r: usize,
            alpha: usize,
        }

        let device = Device::Cpu;
        let temp_dir = TempDir::new()?;

        // Create mock adapter with weights
        let mut weights = HashMap::new();
        weights.insert(
            "lora_a".to_string(),
            Tensor::randn(0f32, 1f32, (64, 8), &device)?,
        );
        weights.insert(
            "lora_b".to_string(),
            Tensor::randn(0f32, 1f32, (8, 64), &device)?,
        );

        let adapter = MockAdapter {
            weights: weights.clone(),
        };

        let config = TestConfig { r: 8, alpha: 16 };

        // Save pretrained
        save_pretrained(&adapter, &config, temp_dir.path())?;

        // Verify files exist
        assert!(temp_dir.path().join(ADAPTER_WEIGHTS_FILENAME).exists());
        assert!(temp_dir.path().join(ADAPTER_CONFIG_FILENAME).exists());

        // Load pretrained
        let mut loaded_adapter = MockAdapter {
            weights: HashMap::new(),
        };
        let loaded_config: TestConfig =
            load_pretrained(&mut loaded_adapter, temp_dir.path(), &device)?;

        // Verify config
        assert_eq!(config, loaded_config);

        // Verify weights
        assert_eq!(loaded_adapter.weights.len(), 2);
        assert!(loaded_adapter.weights.contains_key("lora_a"));
        assert!(loaded_adapter.weights.contains_key("lora_b"));

        Ok(())
    }
}
