//! Inference configuration shared by registry resolution (`registry_model_resolve`).

use serde::{Deserialize, Serialize};

use crate::config::CostPreference;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Modalities {
    pub vision: bool,
    pub web_search: bool,
    pub structured_output: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityLevel {
    Flash,
    #[default]
    Balanced,
    Premium,
}

impl QualityLevel {
    /// Map quality level to a cost preference for model selection.
    ///
    /// Free-by-default policy: `Flash` and `Balanced` both resolve to `Economy`
    /// so that the two most common quality tiers prefer free/cheap models.
    /// Only `Premium` opts in to `Performance` (paid-model-preferred) routing.
    #[must_use]
    pub fn to_cost_preference(self) -> CostPreference {
        match self {
            Self::Flash | Self::Balanced => CostPreference::Economy,
            Self::Premium => CostPreference::Performance,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TierProfile {
    #[default]
    Automatic,
    Manual(String),
    BringYourOwnKey {
        provider: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionModeProfile {
    Efficient,
    LegacyDefault,
    Fast,
    Verbose,
    Precision,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InferenceConfig {
    pub modalities: Modalities,
    pub quality: QualityLevel,
    pub tier: TierProfile,
    #[serde(default)]
    pub free_only: bool,
}

impl InferenceConfig {
    #[must_use]
    #[inline]
    pub fn is_free_only(&self) -> bool {
        self.free_only
    }
}
