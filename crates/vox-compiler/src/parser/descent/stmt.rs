// Statement, block, and pattern parsing.

use super::Parser;
use crate::ast::expr::{BinOp, Expr};
use crate::ast::pattern::Pattern;
use crate::ast::stmt::Stmt;
use crate::lexer::token::Token;
use crate::parser::error::{ParseError, ParseErrorClass};

impl Parser {
    pub(crate) fn parse_block(&mut self) -> Result<Vec<Stmt>, ()> {
        self.skip_newlines();
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            match self.parse_stmt() {
                Ok(s) => stmts.push(s),
                Err(()) => {
                    // Recovery: skip to the next statement boundary so we can
                    // collect further errors within the same block.
                    while !matches!(self.peek(), Token::Newline | Token::RBrace | Token::Eof) {
                        self.advance();
                    }
                }
            }
            self.skip_newlines();
        }
        self.skip_newlines();
        self.expect(&Token::RBrace)?;
        Ok(stmts)
    }

    pub(crate) fn parse_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.span();
        match self.peek().clone() {
            Token::Let => self.parse_let_stmt(),
            Token::Return => {
                self.advance();
                let value = if matches!(self.peek(), Token::Newline | Token::RBrace | Token::Eof) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                Ok(Stmt::Return {
                    value,
                    span: start.merge(self.span()),
                })
            }
            Token::While => self.parse_while_stmt(),
            Token::Loop => self.parse_loop_stmt(),
            Token::Break => {
                self.advance();
                Ok(Stmt::Break { span: start })
            }
            Token::Continue => {
                self.advance();
                Ok(Stmt::Continue { span: start })
            }
            _ => {
                let expr = self.parse_expr()?;
                if self.eat(&Token::Eq) {
                    let value = self.parse_expr()?;
                    Ok(Stmt::Assign {
                        target: expr,
                        value,
                        span: start.merge(self.span()),
                    })
                } else if let Some(op) = match self.peek().clone() {
                    Token::PlusEq => Some(BinOp::Add),
                    Token::MinusEq => Some(BinOp::Sub),
                    Token::StarEq => Some(BinOp::Mul),
                    Token::SlashEq => Some(BinOp::Div),
                    _ => None,
                } {
                    self.advance();
                    let rhs = self.parse_expr()?;
                    let span = start.merge(self.span());
                    // x += e => x = x + e desugaring at AST level
                    let target_expr = expr.clone();
                    let binary_expr = Expr::Binary {
                        op,
                        left: Box::new(expr),
                        right: Box::new(rhs),
                        span,
                    };
                    Ok(Stmt::Assign {
                        target: target_expr,
                        value: binary_expr,
                        span,
                    })
                } else {
                    Ok(Stmt::Expr {
                        expr: expr.clone(),
                        span: expr.span(),
                    })
                }
            }
        }
    }

    pub(crate) fn parse_while_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.span();
        self.advance(); // while
        let condition = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        Ok(Stmt::While {
            condition,
            body,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_loop_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.span();
        self.advance(); // loop
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        Ok(Stmt::Loop {
            body,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_let_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.span();
        self.advance(); // eat 'let'
        let mutable = self.eat(&Token::Mut);
        let pattern = self.parse_pattern()?;
        let type_ann = if self.eat(&Token::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Let {
            pattern,
            type_ann,
            value,
            mutable,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, ()> {
        let start = self.span();
        match self.peek().clone() {
            Token::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard { span: start })
            }
            Token::LParen => {
                self.advance();
                let mut elems = Vec::new();
                loop {
                    if matches!(self.peek(), Token::RParen) {
                        break;
                    }
                    elems.push(self.parse_pattern()?);
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RParen)?;
                Ok(Pattern::Tuple {
                    elements: elems,
                    span: start.merge(self.span()),
                })
            }
            Token::TypeIdent(name) => {
                self.advance();
                if self.eat(&Token::LParen) {
                    let mut fields = Vec::new();
                    loop {
                        if matches!(self.peek(), Token::RParen) {
                            break;
                        }
                        fields.push(self.parse_pattern()?);
                        if !self.eat(&Token::Comma) {
                            break;
                        }
                    }
                    self.expect(&Token::RParen)?;
                    Ok(Pattern::Constructor {
                        name,
                        fields,
                        span: start.merge(self.span()),
                    })
                } else {
                    // A bare capitalized name (`TypeIdent`) with no `(...)` is a
                    // *nullary constructor* pattern, not a binding. Vox reserves
                    // capitalized identifiers for types/variants, so `Red` in
                    // `match c { Red => .. Green => .. }` must match only the
                    // `Red` variant — never bind the scrutinee unconditionally.
                    // (Lowercase `Token::Ident` below remains a binding.)
                    // Before this, `Red` lowered to `Pattern::Ident`, making the
                    // first arm an unconditional catch-all — the nullary-variant
                    // match regression.
                    Ok(Pattern::Constructor {
                        name,
                        fields: Vec::new(),
                        span: start.merge(self.span()),
                    })
                }
            }
            Token::Ident(name) => {
                self.advance();
                Ok(Pattern::Ident {
                    name,
                    span: start.merge(self.span()),
                })
            }
            Token::IntLit(v) => {
                self.advance();
                Ok(Pattern::Literal {
                    value: Box::new(Expr::IntLit {
                        value: v,
                        span: start,
                    }),
                    span: start,
                })
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(Pattern::Literal {
                    value: Box::new(Expr::StringLit {
                        value: s,
                        span: start,
                    }),
                    span: start,
                })
            }
            _ => {
                self.errors.push(ParseError::classified(
                    start,
                    "Expected pattern",
                    vec!["identifier".into(), "_".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Statement,
                ));
                Err(())
            }
        }
    }
}
