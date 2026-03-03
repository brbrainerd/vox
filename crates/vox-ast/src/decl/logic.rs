use crate::expr::Param;
use crate::span::Span;
use crate::stmt::Stmt;
use crate::types::TypeExpr;

/// Actor declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorDecl {
    pub name: String,
    pub state_fields: Vec<crate::decl::typedef::VariantField>,
    pub handlers: Vec<ActorHandler>,
    pub is_deprecated: bool,
    pub span: Span,
}

/// Actor handler definition: `on receive(msg: str) to Unit:`
#[derive(Debug, Clone, PartialEq)]
pub struct ActorHandler {
    pub event_name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub is_traced: bool,
    pub span: Span,
}

/// Native agent declaration
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDecl {
    pub name: String,
    pub version: Option<String>,
    pub state_fields: Vec<crate::decl::typedef::VariantField>,
    pub handlers: Vec<AgentHandler>,
    pub migrations: Vec<MigrationRule>,
    pub is_deprecated: bool,
    pub span: Span,
}

/// Agent handler definition: `on Event(msg) to Type:`
#[derive(Debug, Clone, PartialEq)]
pub struct AgentHandler {
    pub event_name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub is_traced: bool,
    pub span: Span,
}

/// Agent migration rule: `migrate from "1.0":`
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationRule {
    pub from_version: String,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Native message declaration
#[derive(Debug, Clone, PartialEq)]
pub struct MessageDecl {
    pub name: String,
    pub fields: Vec<crate::decl::typedef::VariantField>,
    pub is_deprecated: bool,
    pub span: Span,
}

/// Workflow declaration (durable execution).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub is_traced: bool,
    pub is_deprecated: bool,
    pub span: Span,
}

/// Activity declaration (durable execution side-effect).
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub options: Option<crate::expr::Expr>,
    pub prompt: Option<String>,
    pub is_traced: bool,
    pub is_deprecated: bool,
    pub span: Span,
}

/// HTTP route declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRouteDecl {
    pub method: crate::decl::HttpMethod,
    pub path: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub auth_provider: Option<String>,
    pub roles: Vec<String>,
    pub cors: Option<String>,
    pub is_traced: bool,
    pub is_deprecated: bool,
    pub span: Span,
}
