use vox_ast::span::Span;

/// Unique identifier for definitions within a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefId(pub u32);

/// A fully lowered Vox module ready for type checking and code generation.
#[derive(Debug, Clone)]
pub struct HirModule {
    pub imports: Vec<HirImport>,
    pub functions: Vec<HirFn>,
    pub types: Vec<HirTypeDef>,
    pub routes: Vec<HirRoute>,
    pub actors: Vec<HirActor>,
    pub workflows: Vec<HirWorkflow>,
    pub activities: Vec<HirActivity>,
    pub tests: Vec<HirFn>,
    pub server_fns: Vec<HirServerFn>,
    pub tables: Vec<HirTable>,
    pub indexes: Vec<HirIndex>,
    pub mcp_tools: Vec<HirMcpTool>,
}

/// A resolved import.
#[derive(Debug, Clone)]
pub struct HirImport {
    pub module_path: Vec<String>,
    pub item: String,
    pub span: Span,
}

/// A function or component in HIR.
#[derive(Debug, Clone)]
pub struct HirFn {
    pub id: DefId,
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: Vec<HirStmt>,
    pub is_component: bool,
    pub is_async: bool,
    pub is_pub: bool,
    pub span: Span,
}

/// A function parameter in HIR.
#[derive(Debug, Clone)]
pub struct HirParam {
    pub id: DefId,
    pub name: String,
    pub type_ann: Option<HirType>,
    pub default: Option<HirExpr>,
    pub span: Span,
}

/// Type representation in HIR (resolved from TypeExpr).
#[derive(Debug, Clone, PartialEq)]
pub enum HirType {
    Named(String),
    Generic(String, Vec<HirType>),
    Function(Vec<HirType>, Box<HirType>),
    Tuple(Vec<HirType>),
    Unit,
}

/// Expression in HIR (mirrors AST but with resolved names).
#[derive(Debug, Clone)]
pub enum HirExpr {
    IntLit(i64, Span),
    FloatLit(f64, Span),
    StringLit(String, Span),
    BoolLit(bool, Span),
    Ident(String, Span),
    ObjectLit(Vec<(String, HirExpr)>, Span),
    ListLit(Vec<HirExpr>, Span),
    Binary(HirBinOp, Box<HirExpr>, Box<HirExpr>, Span),
    Unary(HirUnOp, Box<HirExpr>, Span),
    Call(Box<HirExpr>, Vec<HirArg>, bool, Span),
    MethodCall(Box<HirExpr>, String, Vec<HirArg>, Span),
    FieldAccess(Box<HirExpr>, String, Span),
    Match(Box<HirExpr>, Vec<HirMatchArm>, Span),
    If(Box<HirExpr>, Vec<HirStmt>, Option<Vec<HirStmt>>, Span),
    For(String, Box<HirExpr>, Box<HirExpr>, Span),
    Lambda(Vec<HirParam>, Option<HirType>, Box<HirExpr>, Span),
    Pipe(Box<HirExpr>, Box<HirExpr>, Span),
    Spawn(Box<HirExpr>, Span),
    With(Box<HirExpr>, Box<HirExpr>, Span),
    Jsx(HirJsxElement),
    JsxSelfClosing(HirJsxSelfClosing),
    Block(Vec<HirStmt>, Span),
}

#[derive(Debug, Clone)]
pub struct HirArg {
    pub name: Option<String>,
    pub value: HirExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinOp {
    Add, Sub, Mul, Div,
    Lt, Gt, Lte, Gte,
    And, Or, Is, Isnt,
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnOp {
    Not, Neg,
}

#[derive(Debug, Clone)]
pub struct HirMatchArm {
    pub pattern: HirPattern,
    pub guard: Option<Box<HirExpr>>,
    pub body: Box<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirPattern {
    Ident(String, Span),
    Tuple(Vec<HirPattern>, Span),
    Constructor(String, Vec<HirPattern>, Span),
    Wildcard(Span),
    Literal(Box<HirExpr>, Span),
}

#[derive(Debug, Clone)]
pub enum HirStmt {
    Let {
        pattern: HirPattern,
        type_ann: Option<HirType>,
        value: HirExpr,
        mutable: bool,
        span: Span,
    },
    Assign {
        target: HirExpr,
        value: HirExpr,
        span: Span,
    },
    Return {
        value: Option<HirExpr>,
        span: Span,
    },
    Expr {
        expr: HirExpr,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct HirJsxElement {
    pub tag: String,
    pub attributes: Vec<HirJsxAttr>,
    pub children: Vec<HirExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirJsxSelfClosing {
    pub tag: String,
    pub attributes: Vec<HirJsxAttr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirJsxAttr {
    pub name: String,
    pub value: HirExpr,
}

/// ADT / type definition in HIR.
#[derive(Debug, Clone)]
pub struct HirTypeDef {
    pub id: DefId,
    pub name: String,
    pub variants: Vec<HirVariant>,
    pub is_pub: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirVariant {
    pub name: String,
    pub fields: Vec<(String, HirType)>,
    pub span: Span,
}

/// HTTP route in HIR.
#[derive(Debug, Clone)]
pub struct HirRoute {
    pub method: HirHttpMethod,
    pub path: String,
    pub return_type: Option<HirType>,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirHttpMethod {
    Get, Post, Put, Delete,
}

/// Actor definition in HIR.
#[derive(Debug, Clone)]
pub struct HirActor {
    pub id: DefId,
    pub name: String,
    pub handlers: Vec<HirActorHandler>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirActorHandler {
    pub event_name: String,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

/// Workflow definition in HIR.
#[derive(Debug, Clone)]
pub struct HirWorkflow {
    pub id: DefId,
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

/// Activity definition in HIR.
#[derive(Debug, Clone)]
pub struct HirActivity {
    pub id: DefId,
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

/// A server function — callable from the frontend, auto-generates API route + fetch wrapper.
#[derive(Debug, Clone)]
pub struct HirServerFn {
    pub id: DefId,
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Option<HirType>,
    pub body: Vec<HirStmt>,
    pub route_path: String,
    pub span: Span,
}

/// Table definition — a persistent record type.
#[derive(Debug, Clone)]
pub struct HirTable {
    pub id: DefId,
    pub name: String,
    pub fields: Vec<HirTableField>,
    pub is_pub: bool,
    pub span: Span,
}

/// A field within a table definition.
#[derive(Debug, Clone)]
pub struct HirTableField {
    pub name: String,
    pub type_ann: HirType,
    pub span: Span,
}

/// Index definition for a table.
#[derive(Debug, Clone)]
pub struct HirIndex {
    pub table_name: String,
    pub index_name: String,
    pub columns: Vec<String>,
    pub span: Span,
}

/// MCP tool declaration — a function exposed via the Model Context Protocol.
#[derive(Debug, Clone)]
pub struct HirMcpTool {
    pub description: String,
    pub func: HirFn,
}
