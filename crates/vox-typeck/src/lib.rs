pub mod ty;
pub mod env;
pub mod infer;
pub mod unify;
pub mod check;
pub mod builtins;
pub mod diagnostics;

pub use check::typecheck_module;
pub use diagnostics::Diagnostic;
