//! LLVM backend - deferred per user decision.
//! The crate defines the CodegenBackend trait that all backends must implement.

/// Trait for code generation backends.
pub trait CodegenBackend {
    /// The output type produced by this backend.
    type Output;
    /// The error type for code generation failures.
    type Error: std::error::Error;

    /// Generate output from a HIR module.
    fn generate(&self, module: &vox_hir::hir::HirModule) -> Result<Self::Output, Self::Error>;
}
