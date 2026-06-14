//! Model integration for PEFT adapters.
//!
//! This module provides functionality for:
//! - Wrapping models with PEFT adapter management
//! - Pattern matching for module names (e.g., `*.attention`, `layer.*`)
//! - Per-module adapter injection and switching

use std::collections::HashMap;

use candle_core::Tensor;

use crate::error::{PeftError, Result};
use crate::traits::Adapter;

/// Pattern for matching module names.
#[derive(Debug, Clone)]
pub enum ModulePattern {
    /// Match exact module name
    Exact(String),
    /// Match modules ending with suffix (e.g., `*.attention`)
    Suffix(String),
    /// Match modules starting with prefix (e.g., `layer.*`)
    Prefix(String),
    /// Match all modules
    All,
}

impl ModulePattern {
    /// Parse a pattern string into a `ModulePattern`.
    ///
    /// # Examples
    /// - `"encoder.layer.0"` -> `Exact`
    /// - `"*.attention"` -> `Suffix`
    /// - `"layer.*"` -> `Prefix`
    /// - `"*"` -> `All`
    #[must_use]
    pub fn parse(pattern: &str) -> Self {
        match pattern {
            "*" => Self::All,
            s if s.starts_with("*.") => Self::Suffix(s[2..].to_string()),
            s if s.ends_with(".*") => Self::Prefix(s[..s.len() - 2].to_string()),
            s => Self::Exact(s.to_string()),
        }
    }

    /// Check if a module name matches this pattern.
    #[must_use]
    pub fn matches(&self, module_name: &str) -> bool {
        match self {
            Self::Exact(name) => module_name == name,
            Self::Suffix(suffix) => module_name.ends_with(suffix),
            Self::Prefix(prefix) => module_name.starts_with(prefix),
            Self::All => true,
        }
    }
}

/// Adapter entry for a specific module.
struct ModuleAdapter<A: Adapter> {
    /// The adapter instance
    adapter: A,
    /// Whether this adapter is currently active
    active: bool,
}

/// PEFT model wrapper for managing adapters across modules.
///
/// Provides module-level adapter management with pattern-based targeting.
pub struct PeftModel<A: Adapter> {
    /// Map of module names to their adapters
    module_adapters: HashMap<String, HashMap<String, ModuleAdapter<A>>>,
    /// Currently active adapter name (global default)
    active_adapter: Option<String>,
    /// List of all registered adapter names
    adapter_names: Vec<String>,
}

impl<A: Adapter> PeftModel<A> {
    /// Create a new PEFT model wrapper.
    #[must_use]
    pub fn new() -> Self {
        Self {
            module_adapters: HashMap::new(),
            active_adapter: None,
            adapter_names: Vec::new(),
        }
    }

    /// Add an adapter to modules matching the given pattern.
    ///
    /// # Arguments
    /// * `adapter_name` - Unique name for the adapter
    /// * `pattern` - Pattern to match module names
    /// * `module_names` - List of all module names in the model
    /// * `adapter_factory` - Function to create adapter instances
    ///
    /// # Errors
    /// Returns an error if adapter creation fails
    pub fn add_adapter<F>(
        &mut self,
        adapter_name: impl Into<String>,
        pattern: &str,
        module_names: &[&str],
        adapter_factory: F,
    ) -> Result<usize>
    where
        F: Fn(&str) -> Result<A>,
    {
        let adapter_name = adapter_name.into();
        let pattern = ModulePattern::parse(pattern);
        let mut count = 0;

        for &module_name in module_names {
            if pattern.matches(module_name) {
                let adapter = adapter_factory(module_name)?;
                let module_name_owned = module_name.to_string();

                let module_entry = self.module_adapters.entry(module_name_owned).or_default();

                module_entry.insert(
                    adapter_name.clone(),
                    ModuleAdapter {
                        adapter,
                        active: self.active_adapter.is_none(),
                    },
                );
                count += 1;
            }
        }

        // Track adapter name
        if !self.adapter_names.contains(&adapter_name) {
            self.adapter_names.push(adapter_name.clone());
        }

        // Set as active if first adapter
        if self.active_adapter.is_none() && count > 0 {
            self.active_adapter = Some(adapter_name);
        }

        Ok(count)
    }

    /// Set the active adapter for a specific module.
    ///
    /// # Errors
    /// Returns an error if the module or adapter doesn't exist
    pub fn set_adapter(&mut self, module_name: &str, adapter_name: &str) -> Result<()> {
        let adapters = self.module_adapters.get_mut(module_name).ok_or_else(|| {
            PeftError::AdapterNotFound {
                name: format!("module '{module_name}' not found"),
            }
        })?;

        if !adapters.contains_key(adapter_name) {
            return Err(PeftError::AdapterNotFound {
                name: format!("adapter '{adapter_name}' not found in module '{module_name}'"),
            });
        }

        // Deactivate all adapters for this module
        for adapter_entry in adapters.values_mut() {
            adapter_entry.active = false;
        }

        // Activate the requested adapter
        if let Some(entry) = adapters.get_mut(adapter_name) {
            entry.active = true;
        }

        Ok(())
    }

    /// Set the active adapter for all modules.
    ///
    /// # Errors
    /// Returns an error if the adapter doesn't exist in any module
    pub fn set_adapter_all(&mut self, adapter_name: impl Into<String>) -> Result<()> {
        let adapter_name = adapter_name.into();

        if !self.adapter_names.contains(&adapter_name) {
            return Err(PeftError::AdapterNotFound { name: adapter_name });
        }

        for adapters in self.module_adapters.values_mut() {
            // Deactivate all
            for entry in adapters.values_mut() {
                entry.active = false;
            }
            // Activate the requested one if it exists
            if let Some(entry) = adapters.get_mut(&adapter_name) {
                entry.active = true;
            }
        }

        self.active_adapter = Some(adapter_name);
        Ok(())
    }

    /// Get the active adapter name.
    #[must_use]
    pub fn active_adapter_name(&self) -> Option<&str> {
        self.active_adapter.as_deref()
    }

    /// Get all registered adapter names.
    #[must_use]
    pub fn adapter_names(&self) -> &[String] {
        &self.adapter_names
    }

    /// Get module names that have adapters.
    #[must_use]
    pub fn module_names(&self) -> Vec<&str> {
        self.module_adapters.keys().map(String::as_str).collect()
    }

    /// Check if a module has any adapters.
    #[must_use]
    pub fn has_adapter(&self, module_name: &str) -> bool {
        self.module_adapters.contains_key(module_name)
    }

    /// Forward pass for a specific module.
    ///
    /// # Arguments
    /// * `module_name` - Name of the module
    /// * `input` - Input tensor
    /// * `base_output` - Optional base layer output
    ///
    /// # Errors
    /// Returns an error if module not found or no active adapter
    pub fn forward_module(
        &self,
        module_name: &str,
        input: &Tensor,
        base_output: Option<&Tensor>,
    ) -> Result<Tensor> {
        let adapters =
            self.module_adapters
                .get(module_name)
                .ok_or_else(|| PeftError::AdapterNotFound {
                    name: format!("module '{module_name}' not found"),
                })?;

        // Find active adapter
        for entry in adapters.values() {
            if entry.active {
                return entry.adapter.forward(input, base_output);
            }
        }

        Err(PeftError::AdapterNotFound {
            name: format!("no active adapter for module '{module_name}'"),
        })
    }

    /// Get a reference to an adapter for a module.
    ///
    /// # Errors
    /// Returns an error if module or adapter not found
    pub fn get_adapter(&self, module_name: &str, adapter_name: &str) -> Result<&A> {
        let adapters =
            self.module_adapters
                .get(module_name)
                .ok_or_else(|| PeftError::AdapterNotFound {
                    name: format!("module '{module_name}' not found"),
                })?;

        adapters
            .get(adapter_name)
            .map(|entry| &entry.adapter)
            .ok_or_else(|| PeftError::AdapterNotFound {
                name: format!("adapter '{adapter_name}' not found in module '{module_name}'"),
            })
    }

    /// Get the total number of trainable parameters across all active adapters.
    #[must_use]
    pub fn num_parameters(&self) -> usize {
        self.module_adapters
            .values()
            .flat_map(|adapters| adapters.values())
            .filter(|entry| entry.active)
            .map(|entry| entry.adapter.num_parameters())
            .sum()
    }

    /// Get the number of modules with adapters.
    #[must_use]
    pub fn num_modules(&self) -> usize {
        self.module_adapters.len()
    }
}

impl<A: Adapter> Default for PeftModel<A> {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a PEFT model with adapters injected into matching modules.
///
/// This is a convenience function for common use cases.
///
/// # Arguments
/// * `module_names` - List of all module names in the model
/// * `pattern` - Pattern to match module names for adapter injection
/// * `adapter_name` - Name for the adapter
/// * `adapter_factory` - Function to create adapter instances
///
/// # Returns
/// A `PeftModel` with adapters injected into matching modules
///
/// # Errors
/// Returns an error if adapter creation fails
pub fn get_peft_model<A: Adapter, F>(
    module_names: &[&str],
    pattern: &str,
    adapter_name: impl Into<String>,
    adapter_factory: F,
) -> Result<PeftModel<A>>
where
    F: Fn(&str) -> Result<A>,
{
    let mut model = PeftModel::new();
    model.add_adapter(adapter_name, pattern, module_names, adapter_factory)?;
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LoraConfig, LoraLayer};
    use candle_core::{DType, Device, Tensor};

    #[test]
    fn test_module_pattern_exact() {
        let pattern = ModulePattern::parse("encoder.layer.0");
        assert!(pattern.matches("encoder.layer.0"));
        assert!(!pattern.matches("encoder.layer.1"));
        assert!(!pattern.matches("decoder.layer.0"));
    }

    #[test]
    fn test_module_pattern_suffix() {
        let pattern = ModulePattern::parse("*.attention");
        assert!(pattern.matches("layer.0.attention"));
        assert!(pattern.matches("encoder.layer.0.attention"));
        assert!(!pattern.matches("attention.output"));
    }

    #[test]
    fn test_module_pattern_prefix() {
        let pattern = ModulePattern::parse("encoder.*");
        assert!(pattern.matches("encoder.layer.0"));
        assert!(pattern.matches("encoder.attention"));
        assert!(!pattern.matches("decoder.layer.0"));
    }

    #[test]
    fn test_module_pattern_all() {
        let pattern = ModulePattern::parse("*");
        assert!(pattern.matches("anything"));
        assert!(pattern.matches("encoder.layer.0"));
        assert!(pattern.matches(""));
    }

    #[test]
    fn test_peft_model_creation() {
        let model: PeftModel<LoraLayer> = PeftModel::new();
        assert!(model.module_names().is_empty());
        assert!(model.active_adapter_name().is_none());
    }

    #[test]
    fn test_add_adapter_with_pattern() -> Result<()> {
        let mut model: PeftModel<LoraLayer> = PeftModel::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let module_names = vec![
            "encoder.layer.0.attention",
            "encoder.layer.0.mlp",
            "encoder.layer.1.attention",
            "encoder.layer.1.mlp",
            "decoder.layer.0.attention",
        ];

        let count = model.add_adapter("lora", "*.attention", &module_names, |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        assert_eq!(count, 3); // 3 attention modules
        assert_eq!(model.active_adapter_name(), Some("lora"));
        assert!(model.has_adapter("encoder.layer.0.attention"));
        assert!(model.has_adapter("encoder.layer.1.attention"));
        assert!(model.has_adapter("decoder.layer.0.attention"));
        assert!(!model.has_adapter("encoder.layer.0.mlp"));

        Ok(())
    }

    #[test]
    fn test_set_adapter() -> Result<()> {
        let mut model: PeftModel<LoraLayer> = PeftModel::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let module_names = vec!["layer.0"];

        model.add_adapter("adapter1", "*", &module_names, |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        model.add_adapter("adapter2", "*", &module_names, |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        // Switch adapter for specific module
        model.set_adapter("layer.0", "adapter2")?;

        Ok(())
    }

    #[test]
    fn test_set_adapter_all() -> Result<()> {
        let mut model: PeftModel<LoraLayer> = PeftModel::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let module_names = vec!["layer.0", "layer.1"];

        model.add_adapter("adapter1", "*", &module_names, |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        model.add_adapter("adapter2", "*", &module_names, |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        assert_eq!(model.active_adapter_name(), Some("adapter1"));

        model.set_adapter_all("adapter2")?;
        assert_eq!(model.active_adapter_name(), Some("adapter2"));

        Ok(())
    }

    #[test]
    fn test_forward_module() -> Result<()> {
        let mut model: PeftModel<LoraLayer> = PeftModel::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let module_names = vec!["layer.0"];

        model.add_adapter("lora", "*", &module_names, |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device)?;
        let output = model.forward_module("layer.0", &input, None)?;

        assert_eq!(output.dims(), &[1, 10, 768]);

        Ok(())
    }

    #[test]
    fn test_num_parameters() -> Result<()> {
        let mut model: PeftModel<LoraLayer> = PeftModel::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let module_names = vec!["layer.0", "layer.1"];

        model.add_adapter("lora", "*", &module_names, |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        // 2 modules, each with 768*8 + 8*768 = 12,288 parameters
        assert_eq!(model.num_parameters(), 2 * (768 * 8 + 8 * 768));

        Ok(())
    }

    #[test]
    fn test_get_peft_model() -> Result<()> {
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let module_names = vec!["layer.0.attention", "layer.0.mlp", "layer.1.attention"];

        let model = get_peft_model(&module_names, "*.attention", "lora", |_| {
            LoraLayer::new_with_zeros(768, 768, config.clone(), &device)
        })?;

        assert_eq!(model.num_modules(), 2);
        assert!(model.has_adapter("layer.0.attention"));
        assert!(model.has_adapter("layer.1.attention"));
        assert!(!model.has_adapter("layer.0.mlp"));

        Ok(())
    }
}
