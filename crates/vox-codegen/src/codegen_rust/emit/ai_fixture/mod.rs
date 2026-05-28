//! AI-first fixture lowering for `@ai` / decorator-driven LLM bodies (`emit_fn` branch).

mod llm;

pub(super) fn emit_llm_function_body(out: &mut String, func: &vox_compiler::hir::HirFn) {
    llm::emit_llm_function_body(out, func);
}
