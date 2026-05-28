//! CUDA tier gate for `@training_step` and `@distributed_train` workflows (Mn-T5).

use crate::hir::HirModule;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// Minimum `VOX_CUDA_TIER` value required to compile training surfaces locally.
pub const MIN_TRAINING_CUDA_TIER: u32 = 70;

#[must_use]
pub fn cuda_tier_for_check() -> u32 {
    std::env::var("VOX_CUDA_TIER")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
}

/// Emit diagnostics when this host's CUDA tier is below [`MIN_TRAINING_CUDA_TIER`].
pub fn check_training_cuda_tier(module: &HirModule, source: &str) -> Vec<Diagnostic> {
    let tier = cuda_tier_for_check();
    if tier >= MIN_TRAINING_CUDA_TIER {
        return vec![];
    }
    let mut diags = Vec::new();
    for f in &module.functions {
        if !(f.training_step || f.distributed_train.is_some()) {
            continue;
        }
        let mut d = Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "`@training_step` / `@distributed_train` requires CUDA tier >= {MIN_TRAINING_CUDA_TIER} on this host (VOX_CUDA_TIER={tier})."
            ),
            span: f.span,
            expected_type: None,
            found_type: None,
            context: Some(Diagnostic::capture_context(source, f.span)),
            suggestions: vec![
                "Set VOX_CUDA_TIER to 70 or higher when CUDA-capable, or remove training annotations for CPU-only checks.".into(),
            ],
            category: DiagnosticCategory::RuntimeContract,
            code: Some("vox/train/cuda-required".into()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: Some("MensTrainingCudaGate".into()),
        };
        d = d.with_line_col(source);
        diags.push(d);
    }
    diags
}

#[cfg(test)]
// Rust 2024 made `std::env::remove_var` `unsafe`. The single test below runs
// in this module's exclusive scope; SAFETY rationale lives at each block.
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use crate::ast::Span;
    use crate::hir::{DefId, HirFn};

    #[test]
    fn cuda_gate_skips_when_tier_high() {
        unsafe {
            std::env::remove_var("VOX_CUDA_TIER");
        }
        let f = HirFn {
            id: DefId(0),
            name: "step".into(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
            is_async: false,
            is_pub: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: false,
            schedule_interval: None,
            durability: None,
            actor_state_fields: vec![],
            capabilities: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: Span::new(0, 1),
            inference_model: None,
            training_step: true,
            distributed_train: None,
        };
        let module = HirModule {
            functions: vec![f],
            ..Default::default()
        };
        assert!(check_training_cuda_tier(&module, "").is_empty());
    }
}
