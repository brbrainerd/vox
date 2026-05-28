//! Expression AST: literals, control flow, calls, JSX, blocks.
//!
//! Expressions may contain [`crate::ast::stmt::Stmt`] sequences only inside `Expr::Block`, `Expr::If`
//! branches, and similar nodes. The parser does not distinguish “statement context” vs “expression
//! context” in the type system—that distinction is enforced during typechecking.
//!
//! **Tooling:** Prefer matching on [`Expr`](crate::ast::expr::Expr) variants and using embedded
//! [`Span`](crate::ast::span) fields for
//! diagnostics; do not assume expression shapes mirror HIR 1:1 until after lowering.

use crate::ast::pattern::Pattern;
use crate::ast::span::Span;
use crate::ast::types::TypeExpr;

/// Call argument: optional label (`name = value`) plus value expression.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Arg {
    /// Named argument label (e.g., `json=` in `HTTP.post("/api", json={x})`)
    pub name: Option<String>,
    /// The argument value expression
    pub value: Expr,
}

/// Infix operators in surface syntax (including `is` / `isnt` for reference equality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BinOp {
    /// Addition (`+`).
    Add,
    /// Subtraction (`-`).
    Sub,
    /// Multiplication (`*`).
    Mul,
    /// Division (`/`).
    Div,
    /// Less than (`<`).
    Lt,
    /// Greater than (`>`).
    Gt,
    /// Less than or equal (`<=`).
    Lte,
    /// Greater than or equal (`>=`).
    Gte,
    /// Logical AND (`and`).
    And,
    /// Logical OR (`or`).
    Or,
    /// Reference equality (`is`).
    Is,
    /// Reference inequality (`isnt`).
    Isnt,
    /// Modulo (`%`).
    Mod,
    /// Pipeline application (`|>`).
    Pipe,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnOp {
    /// Logical negation (`not`).
    Not,
    /// Numeric negation (prefix `-`).
    Neg,
}

/// A match arm: pattern [if guard] -> body
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MatchArm {
    /// Pattern being matched.
    pub pattern: Pattern,
    /// Optional `if` guard expression.
    pub guard: Option<Box<Expr>>,
    /// Arm body expression.
    pub body: Box<Expr>,
    /// Span covering the entire arm.
    pub span: Span,
}

/// A JSX element: <tag attrs...>children</tag>
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JsxElement {
    /// Element tag name (lower-case component name).
    pub tag: String,
    /// JSX attributes (`name={expr}`).
    pub attributes: Vec<JsxAttribute>,
    /// Child expressions / nested elements.
    pub children: Vec<Expr>,
    /// Span covering the opening tag through closing tag.
    pub span: Span,
}

/// Self-closing JSX: tag and attributes, then `/>` with no separate closing tag.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JsxSelfClosingElement {
    /// Element tag name.
    pub tag: String,
    /// JSX attributes.
    pub attributes: Vec<JsxAttribute>,
    /// Span covering the self-closing tag through `/>`.
    pub span: Span,
}

/// A JSX attribute: name={expr} or name="string"
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JsxAttribute {
    /// Attribute identifier (`className`, `onClick`, …).
    pub name: String,
    /// Attribute value expression or string literal.
    pub value: Expr,
}

/// Parameter for functions and lambdas.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Param {
    /// Parameter binding name.
    pub name: String,
    /// Optional type annotation.
    pub type_ann: Option<TypeExpr>,
    /// Default value when omitted at call sites.
    pub default: Option<Expr>,
    /// Span covering the parameter in the signature.
    pub span: Span,
}

/// Parts of a string interpolation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StringPart {
    /// Raw string fragment between interpolations.
    Literal(String),
    /// `{expression}` segment inside an interpolated string.
    Interpolation(Box<Expr>),
}

/// All expression types in Vox.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    /// Integer literal: `42`
    IntLit {
        /// Numeric value.
        value: i64,
        /// Span covering the literal token.
        span: Span,
    },
    /// Float literal: `3.14`
    FloatLit {
        /// Numeric value.
        value: f64,
        /// Span covering the literal token.
        span: Span,
    },
    /// String literal: `"hello"`
    StringLit {
        /// Unescaped string contents.
        value: String,
        /// Span covering the quoted literal.
        span: Span,
    },
    /// Boolean literal: `true` / `false`
    BoolLit {
        /// Boolean value.
        value: bool,
        /// Span covering the keyword literal.
        span: Span,
    },
    /// Fixed-point decimal literal: `1.23dec`
    DecimalLit {
        /// Raw decimal string.
        value: String,
        /// Span covering the literal.
        span: Span,
    },
    /// Identifier reference: `x`, `foo`
    Ident {
        /// Resolved textual name as written in source.
        name: String,
        /// Span covering the identifier.
        span: Span,
    },
    /// Object literal: `{key: value, ...}`
    ObjectLit {
        /// Field name / value pairs in source order.
        fields: Vec<(String, Expr)>,
        /// Span covering `{` … `}`.
        span: Span,
    },
    /// List literal: `[a, b, c]`
    ListLit {
        /// Element expressions.
        elements: Vec<Expr>,
        /// Span covering `[` … `]`.
        span: Span,
    },
    /// Tuple literal: `(a, b)`
    TupleLit {
        /// Element expressions.
        elements: Vec<Expr>,
        /// Span covering `(` … `)`.
        span: Span,
    },
    /// Binary expression: `a + b`, `a and b`, `a is b`
    Binary {
        /// Operator token.
        op: BinOp,
        /// Left-hand operand.
        left: Box<Expr>,
        /// Right-hand operand.
        right: Box<Expr>,
        /// Span covering the full binary expression.
        span: Span,
    },
    /// Unary expression: `not x`, `-x`
    Unary {
        /// Unary operator.
        op: UnOp,
        /// Operand expression.
        operand: Box<Expr>,
        /// Span covering operator + operand.
        span: Span,
    },
    /// Function call: `f(args)`, `f(a, name=value)`
    Call {
        /// Callee expression.
        callee: Box<Expr>,
        /// Positional and named arguments.
        args: Vec<Arg>,
        /// Span covering the call postfix.
        span: Span,
    },
    /// Method call: `obj.method(args)`
    MethodCall {
        /// Receiver expression.
        object: Box<Expr>,
        /// Method name.
        method: String,
        /// Call arguments.
        args: Vec<Arg>,
        /// Span covering `.method(` … `)`.
        span: Span,
    },
    /// Field access: `obj.field`
    FieldAccess {
        /// Base expression.
        object: Box<Expr>,
        /// Field identifier.
        field: String,
        /// Span covering `.field`.
        span: Span,
    },
    /// `match subject: arm -> body …` — value is the chosen arm’s body (all arms must be compatible).
    Match {
        /// Scrutinee expression.
        subject: Box<Expr>,
        /// Arms: pattern, optional `if` guard, `->` body expression.
        arms: Vec<MatchArm>,
        /// Span covering `match` … end of block.
        span: Span,
    },
    /// Conditional: `if cond:` then-branch stmts, optional `else:` branch.
    If {
        /// Condition expression.
        condition: Box<Expr>,
        /// `then` branch statements.
        then_body: Vec<crate::ast::stmt::Stmt>,
        /// Optional `else` branch statements.
        else_body: Option<Vec<crate::ast::stmt::Stmt>>,
        /// Span covering the whole `if`.
        span: Span,
    },
    /// Try expression: `x?`
    Try {
        /// The expression being tried.
        target: Box<Expr>,
        /// Span covering `?`.
        span: Span,
    },
    /// For expression (used in JSX): `for x in list: <elem>` or `for x, i in list: <elem>`
    For {
        /// Loop binding name.
        binding: String,
        /// Optional index variable (the `, i` in `for x, i in list`).
        index: Option<String>,
        /// Iterable expression.
        iterable: Box<Expr>,
        /// Body expression (often JSX).
        body: Box<Expr>,
        /// Optional React key expression (`key=expr`). Required for list renders.
        key: Option<Box<Expr>>,
        /// Span covering `for` … body.
        span: Span,
    },
    /// Lambda: `fn(params) to body` or `@cancellable fn(params) to body`
    Lambda {
        /// Parameter list.
        params: Vec<Param>,
        /// Optional return type annotation.
        return_type: Option<TypeExpr>,
        /// Lambda body expression.
        body: Box<Expr>,
        /// True when the lambda was annotated `@cancellable fn(…)`.
        cancellable: bool,
        /// Span covering `fn` … body.
        span: Span,
    },
    /// Pipe expression: `a |> b`
    Pipe {
        /// Left pipeline stage.
        left: Box<Expr>,
        /// Right pipeline stage (often a call).
        right: Box<Expr>,
        /// Span covering `|>`.
        span: Span,
    },
    /// Spawn an actor: `spawn(X)`
    Spawn {
        /// Target actor type / handle expression.
        target: Box<Expr>,
        /// Span covering `spawn`.
        span: Span,
    },
    /// With expression: `expr with options`
    With {
        /// Base expression.
        operand: Box<Expr>,
        /// Options record or builder expression.
        options: Box<Expr>,
        /// Span covering `with`.
        span: Span,
    },
    /// JSX element with children: `<div ...>children</div>`
    Jsx(JsxElement),
    /// Self-closing JSX element: `<input .../>`
    JsxSelfClosing(JsxSelfClosingElement),
    /// JSX fragment: `<>children</>`
    JsxFragment {
        /// Child expressions / nested elements.
        children: Vec<Expr>,
        /// Span covering `<>` through `</>`.
        span: Span,
    },
    /// Subscript / index expression: `obj[index]`
    Index {
        /// Base expression being indexed.
        object: Box<Expr>,
        /// Index expression.
        index: Box<Expr>,
        /// Span covering `object[index]`.
        span: Span,
    },
    /// String interpolation: `"text {expr} more"`
    StringInterp {
        /// Alternating literal and interpolation segments.
        parts: Vec<StringPart>,
        /// Span covering the entire string literal.
        span: Span,
    },
    /// Braced sequence of statements in an expression position (e.g. after `=>`).
    ///
    /// How the block’s value is determined is enforced during typechecking in `vox-compiler`
    /// (typically the tail must be an expression or `return`).
    Block {
        /// Statements executed in order.
        stmts: Vec<crate::ast::stmt::Stmt>,
        /// Span covering the `{` … `}` block.
        span: Span,
    },
    /// Phase 1 P1-T7: `side_effect { stmts }` — sanctioned non-determinism inside
    /// a workflow body. Desugars to a synthesised anonymous inline activity at HIR
    /// lower time; the workflow determinism check is exempt inside the block.
    SideEffect {
        /// Body statements (allowed to call `time.now()`, `random.*`, etc.).
        stmts: Vec<crate::ast::stmt::Stmt>,
        /// Span covering the `side_effect { … }` expression.
        span: Span,
    },
    /// `workflow.version("change-id", min, max)` patch-marker primitive (P2-T2).
    WorkflowVersion(WorkflowVersionCall),
}

/// Arguments for `workflow.version("change-id", min_supported, max_supported)`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowVersionCall {
    pub change_id: String,
    pub min: u32,
    pub max: u32,
    pub span: Span,
}

impl Expr {
    /// Outermost source span for diagnostics (includes JSX and block delimiters).
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::DecimalLit { span, .. }
            | Expr::Ident { span, .. }
            | Expr::ObjectLit { span, .. }
            | Expr::ListLit { span, .. }
            | Expr::TupleLit { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Call { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::Match { span, .. }
            | Expr::If { span, .. }
            | Expr::For { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::Pipe { span, .. }
            | Expr::Spawn { span, .. }
            | Expr::With { span, .. }
            | Expr::Try { span, .. }
            | Expr::StringInterp { span, .. }
            | Expr::Block { span, .. }
            | Expr::SideEffect { span, .. }
            | Expr::Index { span, .. } => *span,
            Expr::Jsx(el) => el.span,
            Expr::JsxSelfClosing(el) => el.span,
            Expr::JsxFragment { span, .. } => *span,
            Expr::WorkflowVersion(c) => c.span,
        }
    }
}
