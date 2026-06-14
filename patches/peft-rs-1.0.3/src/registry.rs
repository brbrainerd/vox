//! Multi-adapter registry for managing multiple PEFT adapters.
//!
//! This module provides functionality for:
//! - Registering multiple named adapters
//! - Switching between active adapters
//! - Managing adapter lifecycle
//!
//! Note: Adapter composition (combining multiple adapters) is planned for a future release.

use std::collections::HashMap;

use candle_core::Tensor;

use crate::error::{PeftError, Result};
use crate::traits::Adapter;

/// Registry for managing multiple named adapters.
///
/// Allows switching between different adapters at runtime.
///
/// Note: Adapter composition (combining multiple adapters) is planned for a future release.
pub struct AdapterRegistry<A: Adapter> {
    /// Map of adapter names to adapters
    adapters: HashMap<String, A>,
    /// Currently active adapter name
    active_adapter: Option<String>,
}

impl<A: Adapter> AdapterRegistry<A> {
    /// Create a new empty adapter registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
            active_adapter: None,
        }
    }

    /// Register a new adapter with the given name.
    ///
    /// # Arguments
    /// * `name` - Unique name for the adapter
    /// * `adapter` - The adapter to register
    ///
    /// # Errors
    /// Returns an error if an adapter with this name already exists
    pub fn register_adapter(&mut self, name: impl Into<String>, adapter: A) -> Result<()> {
        let name = name.into();

        if self.adapters.contains_key(&name) {
            return Err(PeftError::AdapterExists { name });
        }

        self.adapters.insert(name.clone(), adapter);

        // Set as active if it's the first adapter
        if self.active_adapter.is_none() {
            self.active_adapter = Some(name);
        }

        Ok(())
    }

    /// Set the active adapter by name.
    ///
    /// # Arguments
    /// * `name` - Name of the adapter to activate
    ///
    /// # Errors
    /// Returns an error if no adapter with this name exists
    pub fn set_active_adapter(&mut self, name: impl Into<String>) -> Result<()> {
        let name = name.into();

        if !self.adapters.contains_key(&name) {
            return Err(PeftError::AdapterNotFound { name });
        }

        self.active_adapter = Some(name);
        Ok(())
    }

    /// Get a reference to the active adapter.
    ///
    /// # Errors
    /// Returns an error if no adapter is currently active
    pub fn get_active_adapter(&self) -> Result<&A> {
        let name = self
            .active_adapter
            .as_ref()
            .ok_or_else(|| PeftError::AdapterNotFound {
                name: "no active adapter".to_string(),
            })?;

        self.adapters
            .get(name)
            .ok_or_else(|| PeftError::AdapterNotFound { name: name.clone() })
    }

    /// Get a mutable reference to the active adapter.
    ///
    /// # Errors
    /// Returns an error if no adapter is currently active
    pub fn get_active_adapter_mut(&mut self) -> Result<&mut A> {
        let name = self
            .active_adapter
            .as_ref()
            .ok_or_else(|| PeftError::AdapterNotFound {
                name: "no active adapter".to_string(),
            })?
            .clone();

        self.adapters
            .get_mut(&name)
            .ok_or_else(|| PeftError::AdapterNotFound { name })
    }

    /// Get a reference to an adapter by name.
    ///
    /// # Arguments
    /// * `name` - Name of the adapter
    ///
    /// # Errors
    /// Returns an error if no adapter with this name exists
    pub fn get_adapter(&self, name: &str) -> Result<&A> {
        self.adapters
            .get(name)
            .ok_or_else(|| PeftError::AdapterNotFound {
                name: name.to_string(),
            })
    }

    /// Get a mutable reference to an adapter by name.
    ///
    /// # Arguments
    /// * `name` - Name of the adapter
    ///
    /// # Errors
    /// Returns an error if no adapter with this name exists
    pub fn get_adapter_mut(&mut self, name: &str) -> Result<&mut A> {
        self.adapters
            .get_mut(name)
            .ok_or_else(|| PeftError::AdapterNotFound {
                name: name.to_string(),
            })
    }

    /// Check if an adapter with the given name exists.
    #[must_use]
    pub fn contains_adapter(&self, name: &str) -> bool {
        self.adapters.contains_key(name)
    }

    /// Get the name of the currently active adapter.
    #[must_use]
    pub fn active_adapter_name(&self) -> Option<&str> {
        self.active_adapter.as_deref()
    }

    /// Get a list of all registered adapter names.
    #[must_use]
    pub fn adapter_names(&self) -> Vec<&str> {
        self.adapters.keys().map(String::as_str).collect()
    }

    /// Get the number of registered adapters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Remove an adapter by name.
    ///
    /// # Arguments
    /// * `name` - Name of the adapter to remove
    ///
    /// # Returns
    /// The removed adapter, if it existed
    ///
    /// # Errors
    /// Returns an error if trying to remove the active adapter
    pub fn remove_adapter(&mut self, name: &str) -> Result<Option<A>> {
        // Don't allow removing the active adapter
        if self.active_adapter.as_deref() == Some(name) {
            return Err(PeftError::InvalidConfig(
                "Cannot remove the currently active adapter".to_string(),
            ));
        }

        Ok(self.adapters.remove(name))
    }

    /// Clear all adapters from the registry.
    pub fn clear(&mut self) {
        self.adapters.clear();
        self.active_adapter = None;
    }

    /// Apply the active adapter to an input tensor.
    ///
    /// # Arguments
    /// * `input` - Input tensor
    /// * `base_output` - Optional base layer output
    ///
    /// # Errors
    /// Returns an error if no active adapter or forward pass fails
    pub fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        let adapter = self.get_active_adapter()?;
        adapter.forward(input, base_output)
    }
}

impl<A: Adapter> Default for AdapterRegistry<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LoraConfig, LoraLayer};
    use candle_core::Device;

    #[test]
    fn test_registry_creation() {
        let registry: AdapterRegistry<LoraLayer> = AdapterRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.active_adapter_name().is_none());
    }

    #[test]
    fn test_register_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config.clone(), &device)?;
        let adapter2 = LoraLayer::new_with_zeros(768, 768, config, &device)?;

        registry.register_adapter("adapter1", adapter1)?;
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.active_adapter_name(), Some("adapter1"));

        registry.register_adapter("adapter2", adapter2)?;
        assert_eq!(registry.len(), 2);
        // Active adapter should still be adapter1
        assert_eq!(registry.active_adapter_name(), Some("adapter1"));

        Ok(())
    }

    #[test]
    fn test_register_duplicate_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config.clone(), &device)?;
        let adapter2 = LoraLayer::new_with_zeros(768, 768, config, &device)?;

        registry.register_adapter("adapter1", adapter1)?;
        let result = registry.register_adapter("adapter1", adapter2);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PeftError::AdapterExists { .. }
        ));

        Ok(())
    }

    #[test]
    fn test_set_active_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config.clone(), &device)?;
        let adapter2 = LoraLayer::new_with_zeros(768, 768, config, &device)?;

        registry.register_adapter("adapter1", adapter1)?;
        registry.register_adapter("adapter2", adapter2)?;

        assert_eq!(registry.active_adapter_name(), Some("adapter1"));

        registry.set_active_adapter("adapter2")?;
        assert_eq!(registry.active_adapter_name(), Some("adapter2"));

        Ok(())
    }

    #[test]
    fn test_set_nonexistent_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config, &device)?;
        registry.register_adapter("adapter1", adapter1)?;

        let result = registry.set_active_adapter("nonexistent");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PeftError::AdapterNotFound { .. }
        ));

        Ok(())
    }

    #[test]
    fn test_get_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config, &device)?;
        registry.register_adapter("adapter1", adapter1)?;

        let retrieved = registry.get_adapter("adapter1")?;
        assert_eq!(retrieved.num_parameters(), 768 * 8 + 8 * 768);

        Ok(())
    }

    #[test]
    fn test_get_active_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config, &device)?;
        registry.register_adapter("adapter1", adapter1)?;

        let active = registry.get_active_adapter()?;
        assert_eq!(active.num_parameters(), 768 * 8 + 8 * 768);

        Ok(())
    }

    #[test]
    fn test_contains_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config, &device)?;
        registry.register_adapter("adapter1", adapter1)?;

        assert!(registry.contains_adapter("adapter1"));
        assert!(!registry.contains_adapter("adapter2"));

        Ok(())
    }

    #[test]
    fn test_adapter_names() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config.clone(), &device)?;
        let adapter2 = LoraLayer::new_with_zeros(768, 768, config, &device)?;

        registry.register_adapter("adapter1", adapter1)?;
        registry.register_adapter("adapter2", adapter2)?;

        let mut names = registry.adapter_names();
        names.sort_unstable();
        assert_eq!(names, vec!["adapter1", "adapter2"]);

        Ok(())
    }

    #[test]
    fn test_remove_adapter() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config.clone(), &device)?;
        let adapter2 = LoraLayer::new_with_zeros(768, 768, config, &device)?;

        registry.register_adapter("adapter1", adapter1)?;
        registry.register_adapter("adapter2", adapter2)?;

        // Can remove non-active adapter
        let removed = registry.remove_adapter("adapter2")?;
        assert!(removed.is_some());
        assert_eq!(registry.len(), 1);

        // Cannot remove active adapter
        let result = registry.remove_adapter("adapter1");
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_clear() -> Result<()> {
        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter1 = LoraLayer::new_with_zeros(768, 768, config, &device)?;
        registry.register_adapter("adapter1", adapter1)?;

        assert_eq!(registry.len(), 1);
        registry.clear();
        assert_eq!(registry.len(), 0);
        assert!(registry.active_adapter_name().is_none());

        Ok(())
    }

    #[test]
    fn test_forward_with_active_adapter() -> Result<()> {
        use candle_core::{DType, Tensor};

        let mut registry = AdapterRegistry::new();
        let device = Device::Cpu;
        let config = LoraConfig::default();

        let adapter = LoraLayer::new_with_zeros(768, 768, config, &device)?;
        registry.register_adapter("test_adapter", adapter)?;

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device)?;
        let output = registry.forward(&input, None)?;

        assert_eq!(output.shape().dims(), &[1, 10, 768]);

        Ok(())
    }
}
