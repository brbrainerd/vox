use crate::expr::Expr;
use crate::pattern::Pattern;
use crate::span::Span;
use crate::types::TypeExpr;

/// All statement types in Vox.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Let binding: `let x = 5` or `let mut x = 0`
    Let {
        pattern: Pattern,
        type_ann: Option<TypeExpr>,
        value: Expr,
        mutable: bool,
        span: Span,
    },
    /// Assignment: `x = x + 1` (only valid for mut bindings)
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    /// Return statement: `ret value`
    Return { value: Option<Expr>, span: Span },
    /// Expression statement (an expression evaluated for its side effects)
    Expr { expr: Expr, span: Span },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Expr { span, .. } => *span,
        }
    }
}
