use crate::ast::span::Span;

/// Unique identifier for definitions within a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DefId(
    /// Opaque numeric id assigned during HIR lowering.
    pub u32,
);
/// A function parameter in HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirParam {
    /// Parameter definition id.
    pub id: DefId,
    /// Binding name.
    pub name: String,
    /// Optional type annotation.
    pub type_ann: Option<HirType>,
    /// Default value expression.
    pub default: Option<HirExpr>,
    /// Span covering this parameter.
    pub span: Span,
}

/// Canonical lowered operation for `db.<Table>.<method>(...)` (Vox Native DB IR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HirDbTableOp {
    Insert,
    /// Primary key fetch by `_id` (alias: `find`).
    Get,
    Delete,
    /// Safe full-table scan (`SELECT * FROM t`).
    All,
    /// Typed equality predicates: `db.T.filter({ col: value, ... })` → parameterized `AND`.
    FilterRecord,
    /// `SELECT COUNT(*) FROM t` (safe aggregate).
    Count,
    /// Appends a caller-controlled fragment after `SELECT * FROM t` — avoid unless necessary.
    UnsafeQueryRawClause,
}

/// Predicate algebra used by native DB query plans.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HirDbPredicate {
    Eq { field: String },
    Neq { field: String },
    Lt { field: String },
    Lte { field: String },
    Gt { field: String },
    Gte { field: String },
    In { field: String, arity: usize },
    Contains { field: String },
    IsNull { field: String },
    And(Vec<HirDbPredicate>),
    Or(Vec<HirDbPredicate>),
    Not(Box<HirDbPredicate>),
}

/// Retrieval strategy hint for Turso-backed plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HirDbRetrievalMode {
    Fts,
    Vector,
    Hybrid,
}

/// Capability metadata carried with lowered DB plans.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HirDbPlanCapabilities {
    pub requires_sync: bool,
    pub emits_change_log: bool,
    pub live_topic: Option<String>,
    pub retrieval_mode: Option<HirDbRetrievalMode>,
    pub orchestration_scope: Option<String>,
}

/// Backend-neutral query/mutation plan representation attached to DB expressions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HirDbQueryPlan {
    pub table: String,
    pub op: HirDbTableOp,
    pub predicate: Option<HirDbPredicate>,
    pub projection: Option<Vec<String>>,
    pub order_by: Option<(String, bool)>,
    pub has_limit: bool,
    pub capabilities: HirDbPlanCapabilities,
}

/// Type representation in HIR (resolved from [`crate::ast::types::TypeExpr`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HirType {
    /// Named scalar or path type (`int`, `User`).
    Named(String),
    /// Generic application (`list[T]`).
    Generic(String, Vec<HirType>),
    /// Function type (`fn(a, b) -> c`).
    Function(Vec<HirType>, Box<HirType>),
    /// Tuple type.
    Tuple(Vec<HirType>),
    /// Unit / void.
    Unit,
    /// Fixed-point decimal.
    Decimal,
}

/// Try expression (`x?`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirTry {
    /// Target expression being tried.
    pub target: Box<HirExpr>,
    /// Span covering the `?`.
    pub span: Span,
}

/// Expression in HIR (mirrors AST but with resolved names).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HirExpr {
    /// Integer literal.
    IntLit(i64, Span),
    /// Floating-point literal.
    FloatLit(f64, Span),
    /// String literal.
    StringLit(String, Span),
    /// Boolean literal.
    BoolLit(bool, Span),
    /// Fixed-point decimal literal.
    DecimalLit(String, Span),
    /// Local or global identifier.
    Ident(String, Span),
    /// Object literal `{ k: v }`.
    ObjectLit(Vec<(String, HirExpr)>, Span),
    /// List literal.
    ListLit(Vec<HirExpr>, Span),
    /// Tuple literal.
    TupleLit(Vec<HirExpr>, Span),
    /// Binary operator application.
    Binary(HirBinOp, Box<HirExpr>, Box<HirExpr>, Span),
    /// Unary operator application.
    Unary(HirUnOp, Box<HirExpr>, Span),
    /// Call; `bool` indicates tail-call hint when used by backend.
    Call(Box<HirExpr>, Vec<HirArg>, bool, Span),
    /// Method call `obj.method(args)`.
    MethodCall(
        Box<HirExpr>,
        String,
        Vec<HirArg>,
        Option<Box<HirDbQueryPlan>>,
        Span,
    ),
    /// Field projection.
    FieldAccess(Box<HirExpr>, String, Span),
    /// `match` expression.
    Match(Box<HirExpr>, Vec<HirMatchArm>, Span),
    /// `if` with optional else.
    If(Box<HirExpr>, Vec<HirStmt>, Option<Vec<HirStmt>>, Span),
    /// `for` loop expression (often JSX). The second field is the optional index variable name.
    /// The fifth field is the optional React key expression (`key=expr`).
    For(
        String,
        Option<String>,
        Box<HirExpr>,
        Box<HirExpr>,
        Option<Box<HirExpr>>,
        Span,
    ),
    /// Lambda / closure. The `bool` is `true` when annotated `@cancellable fn(…)`.
    Lambda(Vec<HirParam>, Option<HirType>, Box<HirExpr>, bool, Span),
    /// `spawn` actor constructor.
    Spawn(Box<HirExpr>, Span),
    /// `with` options wrapper.
    With(Box<HirExpr>, Box<HirExpr>, Span),
    /// JSX element with children.
    Jsx(HirJsxElement),
    /// Self-closing JSX.
    JsxSelfClosing(HirJsxSelfClosing),
    /// JSX fragment: `<>children</>`.
    JsxFragment(Vec<HirExpr>, Span),
    /// Statement block used as expression.
    Block(Vec<HirStmt>, Span),
    /// Error propagation (`?`).
    Try(HirTry),
    /// Subscript / index expression: `object[index]`.
    Index(Box<HirExpr>, Box<HirExpr>, Span),
    /// `Async[T]` view — `when async_value { fetching => … empty => … error e => … ok x => … }`.
    AsyncView(Box<crate::hir::nodes::async_view::HirAsyncView>),
    /// `workflow.version("change-id", min, max)` patch-marker (P2-T2).
    WorkflowVersion(HirWorkflowVersion),
}

/// HIR form of `workflow.version("change-id", min_supported, max_supported)`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HirWorkflowVersion {
    pub change_id: String,
    pub min: u32,
    pub max: u32,
    pub span: Span,
}

/// Named or positional call argument.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirArg {
    /// Label for named args (`json=`); `None` for positional.
    pub name: Option<String>,
    /// Argument value.
    pub value: HirExpr,
}

/// Binary operators in HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HirBinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Lte,
    /// `>=`
    Gte,
    /// `and`
    And,
    /// `or`
    Or,
    /// `is`
    Is,
    /// `isnt`
    Isnt,
    /// `%`
    Mod,
    /// `|>`
    Pipe,
}

/// Unary operators in HIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HirUnOp {
    /// `not`
    Not,
    /// `-`
    Neg,
}

/// Single `match` arm.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirMatchArm {
    /// Matched pattern.
    pub pattern: HirPattern,
    /// Optional guard.
    pub guard: Option<Box<HirExpr>>,
    /// Arm body.
    pub body: Box<HirExpr>,
    /// Span covering the arm.
    pub span: Span,
}

/// Pattern after lowering (names resolved where applicable).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HirPattern {
    /// Bind identifier.
    Ident(String, Span),
    /// Tuple destructuring.
    Tuple(Vec<HirPattern>, Span),
    /// ADT constructor pattern.
    Constructor(String, Vec<HirPattern>, Span),
    /// `_` wildcard.
    Wildcard(Span),
    /// Literal pattern.
    Literal(Box<HirExpr>, Span),
}

/// Statement in HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HirStmt {
    /// `let` / `let mut` binding.
    Let {
        /// Bound pattern.
        pattern: HirPattern,
        /// Optional type ascription.
        type_ann: Option<HirType>,
        /// Initializer.
        value: HirExpr,
        /// `mut` flag.
        mutable: bool,
        /// Span.
        span: Span,
    },
    /// Assignment.
    Assign {
        /// Target l-value.
        target: HirExpr,
        /// New value.
        value: HirExpr,
        /// Span covering the assignment.
        span: Span,
    },
    /// `return` statement.
    Return {
        /// Return value, if any.
        value: Option<HirExpr>,
        /// Span covering `return` / value.
        span: Span,
    },
    /// Expression statement.
    Expr {
        /// Evaluated expression.
        expr: HirExpr,
        /// Span covering the statement.
        span: Span,
    },
    /// While loop.
    While {
        /// Loop condition.
        condition: HirExpr,
        /// Loop body.
        body: Vec<HirStmt>,
        /// Span.
        span: Span,
    },
    /// Infinite loop.
    Loop {
        /// Loop body.
        body: Vec<HirStmt>,
        /// Span.
        span: Span,
    },
    /// Break.
    Break {
        /// Span.
        span: Span,
    },
    /// Continue.
    Continue {
        /// Span.
        span: Span,
    },
}

/// JSX element with children.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirJsxElement {
    /// Tag name.
    pub tag: String,
    /// Attributes.
    pub attributes: Vec<HirJsxAttr>,
    /// Child expressions.
    pub children: Vec<HirExpr>,
    /// Span covering the element.
    pub span: Span,
}

/// Self-closing JSX element.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirJsxSelfClosing {
    /// Tag name.
    pub tag: String,
    /// Attributes.
    pub attributes: Vec<HirJsxAttr>,
    /// Span covering `<tag .../>`.
    pub span: Span,
}

/// JSX attribute.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirJsxAttr {
    /// Attribute name.
    pub name: String,
    /// Attribute value expression.
    pub value: HirExpr,
}
