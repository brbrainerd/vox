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
    /// Version-control / repository operations (`repo.*` / `vcs.*` builtins).
    Vcs,
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
            HirEffectKind::Vcs => "vcs".into(),
            HirEffectKind::Mcp(tool) => format!("mcp({tool})"),
        }
    }
}

/// Sorted, deduplicated set of declared effects on a function.
pub type HirEffectSet = Vec<HirEffectKind>;

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;

    // Catches: a plain-variant label arm returning the wrong/PascalCase string
    // (e.g. "GpuCompute" instead of "gpu_compute") — these strings are the
    // diagnostic/codegen surface for `uses` clauses.
    #[test]
    fn label_maps_plain_variants_to_snake_case() {
        assert_eq!(HirEffectKind::Net.label(), "net");
        assert_eq!(HirEffectKind::GpuCompute.label(), "gpu_compute");
        assert_eq!(HirEffectKind::Vcs.label(), "vcs");
    }

    // Catches: the Mcp(tool) arm dropping the tool name or using wrong
    // delimiters — it must interpolate as `mcp(<tool>)`, not bare "mcp".
    #[test]
    fn label_formats_mcp_with_parameterized_tool() {
        assert_eq!(HirEffectKind::Mcp("search".into()).label(), "mcp(search)");
    }
}
