/// State machine declaration AST types for TASK-4.1.
///
/// Syntax:
/// ```vox
/// state_machine AgentLifecycle {
///   state Idle
///   state Working(task: Task)
///   terminal state Retired
///
///   on Assign(t) from Idle    -> Working(t)
///   on Retire    from any     -> Retired
/// }
/// ```
use crate::span::Span;
use crate::types::TypeExpr;

/// Top-level `state_machine Name { … }` declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StateMachineDecl {
    /// Machine name (PascalCase).
    pub name: String,
    /// All state declarations inside the block.
    pub states: Vec<SmState>,
    /// All transition declarations inside the block.
    pub transitions: Vec<SmTransition>,
    /// `partial state_machine` — skips (state, event) exhaustiveness check.
    pub is_partial: bool,
    /// Exported.
    pub is_pub: bool,
    /// Source span.
    pub span: Span,
}

/// `state Name` or `state Name(field: Type, …)` or `terminal state Name(…)`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SmState {
    /// State variant name.
    pub name: String,
    /// Payload fields (`Working(task: Task)`).
    pub fields: Vec<SmField>,
    /// `terminal state` — no outgoing transitions allowed.
    pub is_terminal: bool,
    /// Source span.
    pub span: Span,
}

/// A named, optionally typed field in a state variant or event payload.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SmField {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub span: Span,
}

/// `on Event(params) from State -> TargetState`
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SmTransition {
    /// Event name (`Assign`, `Pause`, …).
    pub event_name: String,
    /// Event parameter names (untyped at parse time).
    pub event_params: Vec<String>,
    /// Origin state pattern.
    pub from: SmFromPattern,
    /// Target state name.
    pub to_state: String,
    /// Source span.
    pub span: Span,
}

/// The `from …` clause of a transition.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SmFromPattern {
    /// `from StateName` — transition applies to one specific origin state.
    Named(String),
    /// `from any` — transition applies to every non-terminal origin state.
    Any,
}
