pub mod builtins;
pub mod check;
pub mod diagnostics;
pub mod env;
pub mod infer;
pub mod ty;
pub mod unify;

pub use check::typecheck_module;
pub use diagnostics::Diagnostic;
