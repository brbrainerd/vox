//! HIR representation of function effect annotations (TASK-4.2).
//!
//! Distinct from [`crate::hir::nodes::decl::HirEffect`], which represents
//! reactive component `effect { ... }` blocks. This module covers the
//! *capability* side: `fn f() uses net, db, mcp(tool) -> T { ... }`.

/// A single capability effect in HIR.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HirEffectKind {
    Net,
    Db,
    Fs,
    Env,
    Clock,
    Random,
    Spawn,
    GpuCompute,
    Mutate,
    /// Parameterized MCP tool call.
    Mcp(String),
}

impl HirEffectKind {
    pub fn label(&self) -> String {
        match self {
            HirEffectKind::Net => "net".into(),
            HirEffectKind::Db => "db".into(),
            HirEffectKind::Fs => "fs".into(),
            HirEffectKind::Env => "env".into(),
            HirEffectKind::Clock => "clock".into(),
            HirEffectKind::Random => "random".into(),
            HirEffectKind::Spawn => "spawn".into(),
            HirEffectKind::GpuCompute => "gpu_compute".into(),
            HirEffectKind::Mutate => "mutate".into(),
            HirEffectKind::Mcp(tool) => format!("mcp({tool})"),
        }
    }
}

/// Sorted, deduplicated set of declared effects on a function.
pub type HirEffectSet = Vec<HirEffectKind>;
