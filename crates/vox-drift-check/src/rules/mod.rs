use crate::features::ExtractedFeatures;
use crate::layers_manifest::LayersManifest;
use std::path::PathBuf;
use vox_code_audit::rules::{Finding, Language, Severity};

pub mod bearer_header;
pub mod reqwest_bypass;
pub mod serde_default_dup;
pub mod timeout_literal;
pub mod version_string;
pub mod vox_dir_unanchored;
pub mod vox_path_literal;

pub struct WorkspaceContext {
    pub workspace_version: String,
    pub workspace_root: PathBuf,
    /// Parsed `docs/src/architecture/layers.toml` — single source of truth for
    /// per-crate attributes like `sibling_of`. Empty when the file is missing
    /// (rules degrade gracefully to "no siblings declared").
    pub layers: LayersManifest,
}

pub trait DriftRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn severity(&self) -> Severity;
    fn languages(&self) -> &[Language];
    fn check(&self, features: &ExtractedFeatures, ctx: &WorkspaceContext) -> Vec<Finding>;
}

pub fn all_drift_rules() -> Vec<Box<dyn DriftRule>> {
    vec![
        Box::new(reqwest_bypass::ReqwestBypassRule),
        Box::new(vox_path_literal::VoxPathLiteralRule),
        Box::new(timeout_literal::TimeoutLiteralRule),
        Box::new(serde_default_dup::SerdeDefaultDupRule),
        Box::new(version_string::VersionStringRule),
        Box::new(bearer_header::BearerHeaderRule),
        Box::new(vox_dir_unanchored::VoxDirUnanchoredRule),
    ]
}
