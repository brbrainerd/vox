// Infix / binding-power expression parsing.

use super::super::Parser;
use crate::ast::expr::{BinOp, Expr};
use crate::lexer::token::Token;

impl Parser {
    pub(crate) fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.skip_newlines();
        self.parse_expr_bp(0)
    }

    pub(crate) fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ()> {
        let mut lhs = self.parse_primary()?;
        loop {
            if matches!(self.peek(), Token::With) {
                let (l_bp, r_bp) = (5, 6);
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let rhs = self.parse_expr_bp(r_bp)?;
                let span = lhs.span().merge(rhs.span());
                lhs = Expr::With {
                    operand: Box::new(lhs),
                    options: Box::new(rhs),
                    span,
                };
                continue;
            }

            if matches!(self.peek(), Token::Question) {
                let l_bp = 100; // tightly bind
                if l_bp < min_bp {
                    break;
                }
                let span = lhs.span().merge(self.span());
                self.advance();
                lhs = Expr::Try {
                    target: Box::new(lhs),
                    span,
                };
                continue;
            }

            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Lte => BinOp::Lte,
                Token::Gte => BinOp::Gte,
                Token::And => BinOp::And,
                Token::Or => BinOp::Or,
                Token::Is | Token::EqEq => BinOp::Is,
                Token::Isnt | Token::NotEq => BinOp::Isnt,
                Token::PipeOp => BinOp::Pipe,
                _ => break,
            };
            let (l_bp, r_bp) = infix_bp(op);
            if l_bp < min_bp {
                break;
            }
            self.advance();
            let rhs = self.parse_expr_bp(r_bp)?;
            let span = lhs.span().merge(rhs.span());
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
                span,
            };
        }
        Ok(lhs)
    }
}

fn infix_bp(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Pipe => (1, 2),
        BinOp::Or => (3, 4),
        BinOp::And => (5, 6),
        BinOp::Is | BinOp::Isnt => (7, 8),
        BinOp::Lt | BinOp::Gt | BinOp::Lte | BinOp::Gte => (9, 10),
        BinOp::Add | BinOp::Sub => (11, 12),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (13, 14),
    }
}

#[cfg(test)]
mod semcov_wave1c_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn infix_bp_encodes_precedence_and_left_associativity() {
        // Exact (left, right) binding powers per operator class.
        assert_eq!(infix_bp(BinOp::Pipe), (1, 2));
        assert_eq!(infix_bp(BinOp::Or), (3, 4));
        assert_eq!(infix_bp(BinOp::And), (5, 6));
        assert_eq!(infix_bp(BinOp::Is), (7, 8));
        assert_eq!(infix_bp(BinOp::Isnt), (7, 8));
        assert_eq!(infix_bp(BinOp::Lt), (9, 10));
        assert_eq!(infix_bp(BinOp::Gt), (9, 10));
        assert_eq!(infix_bp(BinOp::Lte), (9, 10));
        assert_eq!(infix_bp(BinOp::Gte), (9, 10));
        assert_eq!(infix_bp(BinOp::Add), (11, 12));
        assert_eq!(infix_bp(BinOp::Sub), (11, 12));
        assert_eq!(infix_bp(BinOp::Mul), (13, 14));
        assert_eq!(infix_bp(BinOp::Div), (13, 14));
        assert_eq!(infix_bp(BinOp::Mod), (13, 14));

        // Invariant: every operator is left-associative (right bp = left bp + 1).
        for op in [
            BinOp::Pipe,
            BinOp::Or,
            BinOp::And,
            BinOp::Is,
            BinOp::Isnt,
            BinOp::Lt,
            BinOp::Add,
            BinOp::Mul,
        ] {
            let (l, r) = infix_bp(op);
            assert_eq!(r, l + 1, "operator {:?} must be left-associative", op);
        }

        // Invariant: multiplicative binds tighter than additive, which binds
        // tighter than comparison, etc. (left bp strictly increases by class).
        assert!(infix_bp(BinOp::Mul).0 > infix_bp(BinOp::Add).0);
        assert!(infix_bp(BinOp::Add).0 > infix_bp(BinOp::Lt).0);
        assert!(infix_bp(BinOp::Lt).0 > infix_bp(BinOp::Is).0);
        assert!(infix_bp(BinOp::Is).0 > infix_bp(BinOp::And).0);
        assert!(infix_bp(BinOp::And).0 > infix_bp(BinOp::Or).0);
        assert!(infix_bp(BinOp::Or).0 > infix_bp(BinOp::Pipe).0);
    }
}
