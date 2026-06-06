//! DomainMode — copied verbatim from vox-speech `src/refine/mod.rs` for use inside the
//! vox-plugin-oratio cdylib without pulling in the full refine module.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DomainMode {
    /// General speech domain (default).
    #[default]
    General,
    /// Rust/code-oriented domain.
    Code,
}
