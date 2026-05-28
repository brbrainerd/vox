//! Routing-profile types: high-level intent profiles that the model-selection engine
//! uses to specialise scoring and filter eligible models.
//!
//! A `RoutingProfile` captures *what the task needs* (vision, structured output, etc.)
//! independently of the model chosen to satisfy it.  The selection layer maps profiles
//! to scoring bonuses and capability filters.

use serde::{Deserialize, Serialize};

/// High-level routing intent used by the model-selection engine.
///
/// Derived from a combination of task category, modality flags, and capability hints.
/// Used as a first-class telemetry key and as a scoring input in [`selection`].
///
/// # Mapping summary
///
/// | Profile         | Primary use case                                    |
/// |-----------------|-----------------------------------------------------|
/// | `General`       | Default; no strong specialisation signal             |
/// | `Research`      | Web-search, deep-research, and literature tasks      |
/// | `Vision`        | Image/screenshot input required                      |
/// | `StrictJson`    | Structured-output / schema-constrained generation    |
/// | `Planning`      | Long-horizon planning and reflection tasks           |
/// | `VoxComposer`   | Code-review, PR synthesis, composition tasks         |
/// | `RustLangdev`   | Rust type-checking, debugging, parsing               |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RoutingProfile {
    /// No strong specialisation signal; use generic scoring weights.
    #[default]
    General,
    /// Web-search grounding or deep-research required.
    Research,
    /// Image or screenshot input required.
    Vision,
    /// Strict JSON / structured-output required.
    StrictJson,
    /// Long-horizon planning or reflection task.
    Planning,
    /// Code-review, composition, or PR synthesis task.
    VoxComposer,
    /// Rust-specific debugging, type-checking, or parsing task.
    RustLangdev,
}

impl RoutingProfile {
    /// Returns the canonical string key used in telemetry and dashboard routes.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Research => "research",
            Self::Vision => "vision",
            Self::StrictJson => "strict_json",
            Self::Planning => "planning",
            Self::VoxComposer => "vox_composer",
            Self::RustLangdev => "rust_langdev",
        }
    }

    /// Whether this profile requires native web-search capability in the selected model.
    #[must_use]
    pub fn requires_web_search(self) -> bool {
        matches!(self, Self::Research)
    }

    /// Whether this profile requires vision (image-input) capability.
    #[must_use]
    pub fn requires_vision(self) -> bool {
        matches!(self, Self::Vision)
    }

    /// Whether this profile requires structured-output (JSON schema) capability.
    #[must_use]
    pub fn requires_structured_output(self) -> bool {
        matches!(self, Self::StrictJson)
    }
}

impl std::fmt::Display for RoutingProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
