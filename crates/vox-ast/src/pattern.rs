use crate::span::Span;
use crate::expr::Expr;

/// Pattern matching nodes for let bindings and match arms.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Simple identifier pattern: `x`, `msg`
    Ident {
        name: String,
        span: Span,
    },
    /// Tuple destructuring: `(a, b)`
    Tuple {
        elements: Vec<Pattern>,
        span: Span,
    },
    /// Constructor pattern: `Ok(value)`, `Error(msg)`, `Connecting(attempt)`
    Constructor {
        name: String,
        fields: Vec<Pattern>,
        span: Span,
    },
    /// Wildcard pattern: `_`
    Wildcard {
        span: Span,
    },
    /// Literal pattern in match: `0`, `"hello"`, `true`
    Literal {
        value: Box<Expr>,
        span: Span,
    },
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Ident { span, .. }
            | Pattern::Tuple { span, .. }
            | Pattern::Constructor { span, .. }
            | Pattern::Wildcard { span }
            | Pattern::Literal { span, .. } => *span,
        }
    }
}
