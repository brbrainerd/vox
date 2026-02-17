use crate::span::Span;
use crate::expr::Param;
use crate::stmt::Stmt;
use crate::types::TypeExpr;

/// HTTP method for route declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

/// An import path segment: `react.use_state`
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPath {
    pub segments: Vec<String>,
    pub span: Span,
}

/// ADT variant in a type definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<VariantField>,
    pub span: Span,
}

/// A field within an ADT variant.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantField {
    pub name: String,
    pub type_ann: TypeExpr,
    pub span: Span,
}

/// Actor handler definition: `on receive(msg: str) to Unit:`
#[derive(Debug, Clone, PartialEq)]
pub struct ActorHandler {
    pub event_name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// All top-level declaration types in Vox.
#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    /// Function definition: `fn name(params) to RetType: body`
    Function(FnDecl),

    /// Component definition: `@component fn Chat() to Element: ...`
    Component(ComponentDecl),

    /// Type / ADT definition: `type Shape = | Circle(r: float) | Point`
    TypeDef(TypeDefDecl),

    /// Import declaration: `import react.use_state, network.HTTP`
    Import(ImportDecl),

    /// Actor definition: `actor Worker: on receive(msg: str) to Unit: ...`
    Actor(ActorDecl),

    /// Workflow definition: `workflow process_document(file: File) to Result[str]: ...`
    Workflow(WorkflowDecl),

    /// Activity definition: `activity send_email(to: str) to Result[bool]: ...`
    Activity(ActivityDecl),

    /// HTTP route definition: `http post "/api/chat" to Result: ...`
    HttpRoute(HttpRouteDecl),

    /// MCP tool definition: `@mcp.tool("description") fn name(...) to Type: ...`
    McpTool(McpToolDecl),

    /// Test definition: `@test fn name(): ...`
    Test(TestDecl),

    /// Server function: `@server fn name(params) to RetType: ...`
    ServerFn(ServerFnDecl),

    /// Table definition: `@table type Task: title: str ...`
    Table(TableDecl),

    /// Index definition: `@index Task.by_done on (done, priority)`
    Index(IndexDecl),

    /// v0.dev generated component: `@v0 "prompt" fn Name() to Element`
    V0Component(V0ComponentDecl),

    /// Client-side routes: `routes: "/" to Home, "/about" to About`
    Routes(RoutesDecl),
}

/// Function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub is_pub: bool,
    pub span: Span,
}

/// Component declaration (wraps a function with @component semantics).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentDecl {
    pub func: FnDecl,
    pub styles: Vec<StyleBlock>,
}

/// A scoped style block within a component.
/// Corresponds to `style:` blocks with selector + properties.
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
/// Auto-generates an API route + typed fetch wrapper.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerFnDecl {
    pub func: FnDecl,
}

/// Type / ADT declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDefDecl {
    pub name: String,
    pub variants: Vec<Variant>,
    pub is_pub: bool,
    pub span: Span,
}

/// Import declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub paths: Vec<ImportPath>,
    pub span: Span,
}

/// Actor declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ActorDecl {
    pub name: String,
    pub handlers: Vec<ActorHandler>,
    pub span: Span,
}

/// Workflow declaration (durable execution).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Activity declaration (durable execution side-effect).
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// HTTP route declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRouteDecl {
    pub method: HttpMethod,
    pub path: String,
    pub return_type: Option<TypeExpr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// MCP tool declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolDecl {
    pub description: String,
    pub func: FnDecl,
}

/// Table declaration: a persistent record type.
/// `@table type Task: title: str, done: bool`
#[derive(Debug, Clone, PartialEq)]
pub struct TableDecl {
    pub name: String,
    pub fields: Vec<TableField>,
    pub is_pub: bool,
    pub span: Span,
}

/// A field within a table declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TableField {
    pub name: String,
    pub type_ann: TypeExpr,
    pub span: Span,
}

/// Index declaration for a table.
/// `@index Task.by_done on (done, priority)`
#[derive(Debug, Clone, PartialEq)]
pub struct IndexDecl {
    pub table_name: String,
    pub index_name: String,
    pub columns: Vec<String>,
    pub span: Span,
}

/// v0.dev AI-generated component declaration.
/// `@v0 "A dashboard with charts" fn Dashboard() to Element`
/// `@v0 from "design.png" fn Dashboard() to Element`
#[derive(Debug, Clone, PartialEq)]
pub struct V0ComponentDecl {
    pub prompt: String,
    pub image_path: Option<String>,
    pub name: String,
    pub return_type: Option<TypeExpr>,
    pub span: Span,
}

/// Client-side routing declaration.
/// `routes: "/" to Home, "/about" to About`
#[derive(Debug, Clone, PartialEq)]
pub struct RoutesDecl {
    pub entries: Vec<RouteEntry>,
    pub span: Span,
}

/// A single route entry mapping a path to a component.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteEntry {
    pub path: String,
    pub component_name: String,
    pub span: Span,
}

/// A complete Vox source module (one file).
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub declarations: Vec<Decl>,
    pub span: Span,
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Decl::Function(f) => f.span,
            Decl::Component(c) => c.func.span,
            Decl::TypeDef(t) => t.span,
            Decl::Import(i) => i.span,
            Decl::Actor(a) => a.span,
            Decl::Workflow(w) => w.span,
            Decl::Activity(a) => a.span,
            Decl::HttpRoute(h) => h.span,
            Decl::McpTool(m) => m.func.span,
            Decl::Test(t) => t.func.span,
            Decl::ServerFn(s) => s.func.span,
            Decl::Table(t) => t.span,
            Decl::Index(i) => i.span,
            Decl::V0Component(v) => v.span,
            Decl::Routes(r) => r.span,
        }
    }
}
