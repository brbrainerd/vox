use crate::pattern::Pattern;
use crate::span::Span;
use crate::types::TypeExpr;

/// A function/method argument, potentially named.
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    /// Named argument label (e.g., `json=` in `HTTP.post("/api", json={x})`)
    pub name: Option<String>,
    /// The argument value expression
    pub value: Expr,
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,  // +
    Sub,  // -
    Mul,  // *
    Div,  // /
    Lt,   // <
    Gt,   // >
    Lte,  // <=
    Gte,  // >=
    And,  // and
    Or,   // or
    Is,   // is (==)
    Isnt, // isnt (!=)
    Pipe, // |>
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not, // not
    Neg, // - (prefix)
}

/// A match arm: pattern [if guard] -> body
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
    pub span: Span,
}

/// A JSX element: <tag attrs...>children</tag>
#[derive(Debug, Clone, PartialEq)]
pub struct JsxElement {
    pub tag: String,
    pub attributes: Vec<JsxAttribute>,
    pub children: Vec<Expr>,
    pub span: Span,
}

/// A self-closing JSX element: <tag attrs.../>
#[derive(Debug, Clone, PartialEq)]
pub struct JsxSelfClosingElement {
    pub tag: String,
    pub attributes: Vec<JsxAttribute>,
    pub span: Span,
}

/// A JSX attribute: name={expr} or name="string"
#[derive(Debug, Clone, PartialEq)]
pub struct JsxAttribute {
    pub name: String,
    pub value: Expr,
}

/// Parameter for functions and lambdas.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub default: Option<Expr>,
    pub span: Span,
}

/// Parts of a string interpolation.
#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Literal(String),
    Interpolation(Box<Expr>),
}

/// All expression types in Vox.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Integer literal: `42`
    IntLit { value: i64, span: Span },
    /// Float literal: `3.14`
    FloatLit { value: f64, span: Span },
    /// String literal: `"hello"`
    StringLit { value: String, span: Span },
    /// Boolean literal: `true` / `false`
    BoolLit { value: bool, span: Span },
    /// Identifier reference: `x`, `foo`
    Ident { name: String, span: Span },
    /// Object literal: `{key: value, ...}`
    ObjectLit {
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    /// List literal: `[a, b, c]`
    ListLit { elements: Vec<Expr>, span: Span },
    /// Tuple literal: `(a, b)`
    TupleLit { elements: Vec<Expr>, span: Span },
    /// Binary expression: `a + b`, `a and b`, `a is b`
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// Unary expression: `not x`, `-x`
    Unary {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// Function call: `f(args)`, `f(a, name=value)`
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
        span: Span,
    },
    /// Method call: `obj.method(args)`
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Arg>,
        span: Span,
    },
    /// Field access: `obj.field`
    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    /// Match expression
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// If expression
    If {
        condition: Box<Expr>,
        then_body: Vec<crate::stmt::Stmt>,
        else_body: Option<Vec<crate::stmt::Stmt>>,
        span: Span,
    },
    /// For expression (used in JSX): `for x in list: <elem>`
    For {
        binding: String,
        iterable: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    /// Lambda: `fn(params) to body`
    Lambda {
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        body: Box<Expr>,
        span: Span,
    },
    /// Pipe expression: `a |> b`
    Pipe {
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    /// Spawn an actor: `spawn(X)`
    Spawn { target: Box<Expr>, span: Span },
    /// With expression: `expr with options`
    With {
        operand: Box<Expr>,
        options: Box<Expr>,
        span: Span,
    },
    /// JSX element with children: `<div ...>children</div>`
    Jsx(JsxElement),
    /// Self-closing JSX element: `<input .../>`
    JsxSelfClosing(JsxSelfClosingElement),
    /// String interpolation: `"text {expr} more"`
    StringInterp { parts: Vec<StringPart>, span: Span },
    /// Block expression (a sequence of statements, last expression is the value)
    Block {
        stmts: Vec<crate::stmt::Stmt>,
        span: Span,
    },
}

impl Expr {
    /// Get the source span for any expression variant.
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::BoolLit { span, .. }
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
            | Expr::StringInterp { span, .. }
            | Expr::Block { span, .. } => *span,
            Expr::Jsx(el) => el.span,
            Expr::JsxSelfClosing(el) => el.span,
        }
    }
}
