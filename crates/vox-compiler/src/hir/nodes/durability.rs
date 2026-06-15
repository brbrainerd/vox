//! Durability annotations for functions in the HIR (Path A, TASK-2.6).
//!
//! A `DurabilityKind` on `HirFn` marks the function as a durable-execution
//! primitive — compiled with replay-safe semantics, persisted checkpoints, or
//! actor isolation depending on the variant.
//!
//! These are lowered from `workflow`/`activity`/`actor` source declarations.
//! See [`crate::ast::decl::logic::WorkflowDecl`] etc. for the AST shapes.

/// Execution durability classification for a [`super::HirFn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurabilityKind {
    /// Durable orchestration function — survives crashes; calls to `activity`
    /// functions are replayed via journal.  Corresponds to `workflow fn_name()`.
    Workflow,
    /// Leaf unit of work within a workflow — retried on failure, never replayed
    /// mid-execution.  Corresponds to `activity fn_name()`.
    Activity,
    /// Stateful entity with message handlers — isolated memory, serialised
    /// handler dispatch.  Corresponds to `actor Name { … }`.
    Actor,
}

impl DurabilityKind {
    /// Human-readable label for diagnostics and codegen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Workflow => "workflow",
            Self::Activity => "activity",
            Self::Actor => "actor",
        }
    }
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;

    // Catches: a swapped/mislabeled arm (e.g. Workflow returning "activity"),
    // which would corrupt durable-execution diagnostics and codegen routing.
    #[test]
    fn label_distinguishes_all_three_durability_kinds() {
        assert_eq!(DurabilityKind::Workflow.label(), "workflow");
        assert_eq!(DurabilityKind::Activity.label(), "activity");
        assert_eq!(DurabilityKind::Actor.label(), "actor");
    }
}
