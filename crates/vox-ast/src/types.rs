use crate::span::Span;

/// Type expressions in Vox source code.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TypeExpr {
    /// A simple named type: `int`, `str`, `bool`, `Element`, `Unit`
    Named { name: String, span: Span },
    /// A generic/parameterized type: `list[T]`, `Result[str]`, `Option[int]`
    Generic {
        name: String,
        args: Vec<TypeExpr>,
        span: Span,
    },
    /// A function type: `fn(int, str) -> bool`
    Function {
        params: Vec<TypeExpr>,
        return_type: Box<TypeExpr>,
        span: Span,
    },
    /// A tuple type: `(int, str)`
    Tuple { elements: Vec<TypeExpr>, span: Span },
    /// Unit type (void)
    Unit { span: Span },
    /// Infer: `_` in type position; triggers type inference.
    Infer { span: Span },
    /// Fixed-point decimal type: `dec`
    Decimal { span: Span },
}

impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named { span, .. }
            | TypeExpr::Generic { span, .. }
            | TypeExpr::Function { span, .. }
            | TypeExpr::Tuple { span, .. }
            | TypeExpr::Unit { span }
            | TypeExpr::Infer { span }
            | TypeExpr::Decimal { span } => *span,
        }
    }
}
