use crate::features::ExtractedFeatures;
use crate::rules::WorkspaceContext;
use vox_code_audit::rules::{Finding, Severity};

pub mod body_hash;
pub mod call_shape;
pub mod literal_dedup;
pub mod numeric_dedup;

pub trait SweepRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn severity(&self) -> Severity;
    fn sweep(&self, files: &[ExtractedFeatures], ctx: &WorkspaceContext) -> Vec<Finding>;
}

pub fn all_sweep_rules() -> Vec<Box<dyn SweepRule>> {
    vec![
        Box::new(literal_dedup::LiteralDedupRule::default()),
        Box::new(numeric_dedup::NumericDedupRule::default()),
        Box::new(body_hash::BodyHashRule::default()),
        Box::new(call_shape::CallShapeRule::default()),
    ]
}
