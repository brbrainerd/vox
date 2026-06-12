// `@form` declaration parsing.

use super::super::Parser;
use super::head_types::{Decl, FieldConstraint, FormDecl, FormField};
use crate::lexer::token::Token;
use crate::parser::error::{ParseError, ParseErrorClass};

impl Parser {
    /// Parse a `@form Name { field ... on_submit: ... }` declaration.
    pub(crate) fn parse_form_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @form
        let name = self.parse_ident_name()?;
        self.expect(&Token::LBrace)?;

        let mut fields: Vec<FormField> = Vec::new();
        let mut on_submit: Option<String> = None;
        let mut success_redirect: Option<String> = None;
        let mut error_message: Option<String> = None;

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Token::RBrace | Token::Eof => break,
                Token::Ident(ref kw) if kw == "field" => {
                    self.advance(); // eat `field`
                    fields.push(self.parse_form_field()?);
                }
                Token::Ident(ref kw) if kw == "on_submit" => {
                    self.advance(); // eat `on_submit`
                    self.expect(&Token::Colon)?;
                    on_submit = Some(self.parse_ident_name()?);
                }
                Token::Ident(ref kw) if kw == "success_redirect" => {
                    self.advance(); // eat `success_redirect`
                    self.expect(&Token::Colon)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            success_redirect = Some(s);
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal for success_redirect",
                                vec!["\"...\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                }
                Token::Ident(ref kw) if kw == "error_message" => {
                    self.advance(); // eat `error_message`
                    self.expect(&Token::Colon)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            error_message = Some(s);
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal for error_message",
                                vec!["\"...\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Unexpected token inside @form block: {other}; expected `field`, `on_submit`, `success_redirect`, or `}}`"),
                        vec!["field".into(), "on_submit".into(), "success_redirect".into(), "}".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            }
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;

        Ok(Decl::Form(FormDecl {
            name,
            fields,
            on_submit,
            success_redirect,
            error_message,
            span: start.merge(self.span()),
        }))
    }

    /// Parse a single `field name: Type [constraints...] [required|optional] [hidden]` line
    /// inside a `@form` block. Cursor is positioned after the `field` keyword.
    fn parse_form_field(&mut self) -> Result<FormField, ()> {
        let start = self.span();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_type_expr()?;

        let mut constraints: Vec<FieldConstraint> = Vec::new();
        let mut required = false;
        let mut hidden = false;
        let mut label: Option<String> = None;
        let mut default: Option<crate::ast::expr::Expr> = None;

        // Parse optional modifiers on the same line until newline or `}`
        loop {
            match self.peek().clone() {
                Token::Newline | Token::RBrace | Token::Eof => break,
                Token::Ident(ref kw) if kw == "required" => {
                    self.advance();
                    required = true;
                }
                Token::Ident(ref kw) if kw == "optional" => {
                    self.advance();
                    required = false;
                }
                Token::Ident(ref kw) if kw == "hidden" => {
                    self.advance();
                    hidden = true;
                }
                Token::Ident(ref kw) if kw == "label" => {
                    self.advance(); // eat `label`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            label = Some(s);
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal inside label(...)",
                                vec!["\"label text\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "range" => {
                    self.advance(); // eat `range`
                    self.expect(&Token::LParen)?;
                    // Parse `lo..hi` — lexed as IntLit, Dot, Dot, IntLit
                    let lo_span = self.span();
                    let lo = match self.peek().clone() {
                        Token::IntLit(v) => {
                            let span = self.span();
                            self.advance();
                            crate::ast::expr::Expr::IntLit { value: v, span }
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal for range lower bound",
                                vec!["1".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    // Consume two dots: `..`
                    self.expect(&Token::Dot)?;
                    self.expect(&Token::Dot)?;
                    let hi = match self.peek().clone() {
                        Token::IntLit(v) => {
                            let span = self.span();
                            self.advance();
                            crate::ast::expr::Expr::IntLit { value: v, span }
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal for range upper bound",
                                vec!["10".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    self.expect(&Token::RParen)?;
                    let _ = lo_span;
                    constraints.push(FieldConstraint::Range(lo, hi));
                }
                Token::Ident(ref kw) if kw == "max_len" => {
                    self.advance(); // eat `max_len`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::IntLit(v) => {
                            self.advance();
                            constraints.push(FieldConstraint::MaxLen(v as usize));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal inside max_len(...)",
                                vec!["280".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "min_len" => {
                    self.advance(); // eat `min_len`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::IntLit(v) => {
                            self.advance();
                            constraints.push(FieldConstraint::MinLen(v as usize));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal inside min_len(...)",
                                vec!["1".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "pattern" => {
                    self.advance(); // eat `pattern`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            constraints.push(FieldConstraint::Pattern(s));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal inside pattern(...)",
                                vec!["\"^[a-z]+$\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "default" => {
                    self.advance(); // eat `default`
                    self.expect(&Token::LParen)?;
                    default = Some(self.parse_expr()?);
                    self.expect(&Token::RParen)?;
                }
                _ => break,
            }
        }

        Ok(FormField {
            name,
            ty,
            label,
            required,
            hidden,
            default,
            constraints,
            span: start.merge(self.span()),
        })
    }
}
