//! Configuration types for PEFT adapters.

use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::AdapterConfig;

/// Common configuration shared across adapter types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseAdapterConfig {
    /// Target modules to apply adapters to (e.g., [`q_proj`, `v_proj`]).
    pub target_modules: Vec<String>,

    /// Whether to apply to all linear layers matching the pattern.
    #[serde(default)]
    pub fan_in_fan_out: bool,

    /// Modules to exclude from adaptation.
    #[serde(default)]
    pub modules_to_exclude: Vec<String>,
}

impl Default for BaseAdapterConfig {
    fn default() -> Self {
        Self {
            target_modules: vec![
                "q_proj".into(),
                "k_proj".into(),
                "v_proj".into(),
                "o_proj".into(),
            ],
            fan_in_fan_out: false,
            modules_to_exclude: Vec::new(),
        }
    }
}

impl AdapterConfig for BaseAdapterConfig {
    fn validate(&self) -> Result<()> {
        if self.target_modules.is_empty() {
            return Err(PeftError::InvalidConfig(
                "target_modules cannot be empty".into(),
            ));
        }
        Ok(())
    }
}
