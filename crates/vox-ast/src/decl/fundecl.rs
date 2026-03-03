use crate::expr::{Expr, Param};
use crate::span::Span;
use crate::stmt::Stmt;
use crate::types::TypeExpr;

/// Function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub is_async: bool,
    pub is_deprecated: bool,
    pub is_pure: bool,
    pub is_traced: bool,
    pub is_llm: bool,
    pub llm_model: Option<String>,
    pub is_layout: bool,
    pub is_pub: bool,
    pub is_metric: bool,
    pub metric_name: Option<String>,
    pub is_health: bool,
    pub auth_provider: Option<String>,
    pub roles: Vec<String>,
    pub cors: Option<String>,
    /// Precondition expressions from `@require(expr)` decorators.
    pub preconditions: Vec<Expr>,
    pub span: Span,
}

/// Component declaration (wraps a function with @component semantics).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDecl {
    pub func: FnDecl,
    pub styles: Vec<StyleBlock>,
}

/// A scoped style block within a component.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleBlock {
    pub selector: String,
    pub properties: Vec<(String, String)>,
    pub span: Span,
}

/// Test declaration (wraps a function with @test semantics).
#[derive(Debug, Clone, PartialEq)]
pub struct TestDecl {
    pub func: FnDecl,
}

/// Server function declaration (wraps a function with @server semantics).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerFnDecl {
    pub func: FnDecl,
}

/// Query declaration: a read-only database function.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryDecl {
    pub func: FnDecl,
}

/// Mutation declaration: a write database function with transaction semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationDecl {
    pub func: FnDecl,
}

/// Action declaration: server-side logic that can call queries and mutations.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionDecl {
    pub func: FnDecl,
}

/// Skill declaration: a modular AI capability.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillDecl {
    pub func: FnDecl,
}

/// Agent definition declaration: defines the core logic and interface for an AI agent.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDefDecl {
    pub func: FnDecl,
}

/// Scheduled function declaration — runs at a fixed interval or cron schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledDecl {
    pub interval: String,
    pub func: FnDecl,
}

/// MCP tool declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolDecl {
    pub description: String,
    pub func: FnDecl,
}

/// MCP resource declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct McpResourceDecl {
    pub uri: String,
    pub description: String,
    pub func: FnDecl,
}

/// Mock declaration for testing.
#[derive(Debug, Clone, PartialEq)]
pub struct MockDecl {
    pub target: String,
    pub func: FnDecl,
}

/// A frontend hook function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct HookDecl {
    pub func: FnDecl,
}

/// Fixture declaration: setup code for tests.
#[derive(Debug, Clone, PartialEq)]
pub struct FixtureDecl {
    pub func: FnDecl,
    pub span: Span,
}
