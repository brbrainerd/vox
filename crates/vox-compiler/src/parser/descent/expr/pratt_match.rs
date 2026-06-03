// Primary expressions, postfix, control/lambda forms.

use super::super::Parser;
use crate::ast::expr::{
    Arg, Expr, JsxAttribute, JsxElement, JsxSelfClosingElement, MatchArm, StringPart, UnOp,
    WorkflowVersionCall,
};
use crate::ast::stmt::Stmt;
use crate::lexer::token::Token;
use crate::parser::error::{ParseError, ParseErrorClass};

impl Parser {
    pub(crate) fn parse_primary(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        let mut expr = match self.peek().clone() {
            Token::IntLit(v) => {
                self.advance();
                Expr::IntLit {
                    value: v,
                    span: start,
                }
            }
            Token::FloatLit(v) => {
                self.advance();
                Expr::FloatLit {
                    value: v,
                    span: start,
                }
            }
            Token::StringLit(s) | Token::SingleStringLit(s) | Token::RawStringLit(s) => {
                self.advance();
                Expr::StringLit {
                    value: s,
                    span: start,
                }
            }
            Token::TemplateStringLit(s) => {
                self.advance();
                // Parse template string with interpolation
                let parts = self.parse_template_string_parts(s, start)?;
                Expr::StringInterp { parts, span: start }
            }
            Token::DecLit(s) => {
                self.advance();
                Expr::DecimalLit {
                    value: s,
                    span: start,
                }
            }

            Token::True => {
                self.advance();
                Expr::BoolLit {
                    value: true,
                    span: start,
                }
            }
            Token::False => {
                self.advance();
                Expr::BoolLit {
                    value: false,
                    span: start,
                }
            }
            Token::Not => {
                self.advance();
                let operand = self.parse_primary()?;
                Expr::Unary {
                    op: UnOp::Not,
                    operand: Box::new(operand),
                    span: start.merge(self.span()),
                }
            }
            Token::BangInvalid => {
                // `!` is not a valid Vox operator. Vox uses phonetic operators
                // (`not`, `and`, `or`, `is`, `isnt`). Emit a clear error pointing
                // at the canonical form, then advance past the `!` so the parser
                // can keep going and report any other issues in the same pass.
                self.errors.push(ParseError::classified(
                    start,
                    "`!` is not a valid operator in Vox; use `not` instead. \
                     (Vox uses phonetic operators: `not`, `and`, `or`, `is`, `isnt`.)",
                    vec!["not".to_string()],
                    Some("!".to_string()),
                    ParseErrorClass::Expression,
                ));
                self.advance();
                return Err(());
            }
            Token::Minus => {
                self.advance();
                let operand = self.parse_primary()?;
                Expr::Unary {
                    op: UnOp::Neg,
                    operand: Box::new(operand),
                    span: start.merge(self.span()),
                }
            }
            Token::LParen => {
                self.advance();
                let e = self.parse_expr()?;
                let paren_expr = if self.eat(&Token::Comma) {
                    let mut elems = vec![e];
                    loop {
                        self.skip_newlines();
                        if matches!(self.peek(), Token::RParen) {
                            break;
                        }
                        elems.push(self.parse_expr()?);
                        if !self.eat(&Token::Comma) {
                            break;
                        }
                    }
                    self.skip_newlines();
                    if self.expect(&Token::RParen).is_err() {
                        return Err(());
                    }
                    Expr::TupleLit {
                        elements: elems,
                        span: start.merge(self.span()),
                    }
                } else {
                    self.skip_newlines();
                    if self.expect(&Token::RParen).is_err() {
                        return Err(());
                    }
                    e
                };

                if self.eat(&Token::FatArrow) {
                    let mut params = Vec::new();
                    match paren_expr {
                        Expr::Ident { name, span } => {
                            params.push(crate::ast::expr::Param {
                                name,
                                type_ann: None,
                                default: None,
                                span,
                            });
                        }
                        Expr::TupleLit { elements, .. } => {
                            for elem in elements {
                                match elem {
                                    Expr::Ident { name, span } => {
                                        params.push(crate::ast::expr::Param {
                                            name,
                                            type_ann: None,
                                            default: None,
                                            span,
                                        });
                                    }
                                    _ => {
                                        self.errors.push(ParseError::classified(
                                            elem.span(),
                                            "Expected identifier in lambda parameters",
                                            vec![],
                                            None,
                                            ParseErrorClass::Expression,
                                        ));
                                        return Err(());
                                    }
                                }
                            }
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                paren_expr.span(),
                                "Expected identifier or tuple in lambda parameters",
                                vec![],
                                None,
                                ParseErrorClass::Expression,
                            ));
                            return Err(());
                        }
                    }
                    let body = self.parse_expr()?;
                    Expr::Lambda {
                        params,
                        return_type: None,
                        body: Box::new(body),
                        cancellable: false,
                        span: start.merge(self.span()),
                    }
                } else {
                    paren_expr
                }
            }
            Token::LBracket => {
                self.advance();
                let mut elems = Vec::new();
                while !matches!(self.peek(), Token::RBracket | Token::Eof) {
                    self.skip_newlines();
                    if matches!(self.peek(), Token::RBracket | Token::Eof) {
                        break;
                    }
                    elems.push(self.parse_expr()?);
                    self.skip_newlines();
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.skip_newlines();
                self.expect(&Token::RBracket)?;
                Expr::ListLit {
                    elements: elems,
                    span: start.merge(self.span()),
                }
            }
            Token::LBrace => self.parse_brace_expr()?,
            Token::Match => self.parse_match()?,
            Token::If => self.parse_if()?,
            Token::For => self.parse_for()?,
            Token::Fn => self.parse_lambda()?,
            Token::AtCancellable => {
                self.advance(); // eat '@cancellable'
                self.parse_lambda_with_cancellable(true)?
            }
            Token::Spawn => {
                self.advance();
                self.expect(&Token::LParen)?;
                let target = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Expr::Spawn {
                    target: Box::new(target),
                    span: start.merge(self.span()),
                }
            }
            Token::SideEffect => {
                self.advance(); // eat `side_effect`
                self.expect(&Token::LBrace)?;
                let stmts = self.parse_block()?;
                Expr::SideEffect {
                    stmts,
                    span: start.merge(self.span()),
                }
            }
            // P2-T2: `workflow.version("change-id", min, max)` patch-marker.
            Token::Workflow => {
                self.advance(); // eat `workflow`
                self.expect(&Token::Dot)?;
                let method = self.parse_ident_name()?;
                if method != "version" {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("expected `version` after `workflow.`, got `{method}`"),
                        vec!["version".into()],
                        Some(method),
                        ParseErrorClass::Expression,
                    ));
                    return Err(());
                }
                self.expect(&Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(&Token::RParen)?;
                let span = start.merge(self.span());
                let change_id = match args.first().map(|a| &a.value) {
                    Some(Expr::StringLit { value, .. }) => value.clone(),
                    _ => {
                        self.errors.push(ParseError::classified(
                            span,
                            "workflow.version arg 1 must be a string literal",
                            vec!["string literal".into()],
                            None,
                            ParseErrorClass::Expression,
                        ));
                        return Err(());
                    }
                };
                let min = match args.get(1).map(|a| &a.value) {
                    Some(Expr::IntLit { value, .. }) => match u32::try_from(*value) {
                        Ok(v) => v,
                        Err(_) => {
                            self.errors.push(ParseError::classified(
                                span,
                                "workflow.version arg 2 must be a non-negative int",
                                vec!["non-negative integer".into()],
                                None,
                                ParseErrorClass::Expression,
                            ));
                            return Err(());
                        }
                    },
                    _ => {
                        self.errors.push(ParseError::classified(
                            span,
                            "workflow.version arg 2 must be a non-negative int",
                            vec!["non-negative integer".into()],
                            None,
                            ParseErrorClass::Expression,
                        ));
                        return Err(());
                    }
                };
                let max = match args.get(2).map(|a| &a.value) {
                    Some(Expr::IntLit { value, .. }) => match u32::try_from(*value) {
                        Ok(v) => v,
                        Err(_) => {
                            self.errors.push(ParseError::classified(
                                span,
                                "workflow.version arg 3 must be a non-negative int",
                                vec!["non-negative integer".into()],
                                None,
                                ParseErrorClass::Expression,
                            ));
                            return Err(());
                        }
                    },
                    _ => {
                        self.errors.push(ParseError::classified(
                            span,
                            "workflow.version arg 3 must be a non-negative int",
                            vec!["non-negative integer".into()],
                            None,
                            ParseErrorClass::Expression,
                        ));
                        return Err(());
                    }
                };
                Expr::WorkflowVersion(WorkflowVersionCall {
                    change_id,
                    min,
                    max,
                    span,
                })
            }
            // VUV: angle-bracket JSX (`<tag attr=...>`) was retired as a parser entry point.
            // View calls are now `Ident(kwargs) { children }`. Hitting `<` here is a real
            // less-than usage in expression context — fall through to the error path so we
            // don't silently consume HTML-shaped source.
            Token::Ident(name) | Token::TypeIdent(name) => {
                self.advance();
                if self.eat(&Token::FatArrow) {
                    let body = self.parse_expr()?;
                    Expr::Lambda {
                        params: vec![crate::ast::expr::Param {
                            name: name.clone(),
                            type_ann: None,
                            default: None,
                            span: start,
                        }],
                        return_type: None,
                        body: Box::new(body),
                        cancellable: false,
                        span: start.merge(self.span()),
                    }
                } else {
                    Expr::Ident { name, span: start }
                }
            }
            Token::Env => {
                self.advance();
                Expr::Ident {
                    name: "env".to_string(),
                    span: start,
                }
            }
            Token::To => {
                self.advance();
                Expr::Ident {
                    name: "to".to_string(),
                    span: start,
                }
            }
            Token::Http => {
                self.advance();
                Expr::Ident {
                    name: "http".to_string(),
                    span: start,
                }
            }
            _ => {
                self.errors.push(ParseError::classified(
                    start,
                    format!("Unexpected token in expression: {}", self.peek()),
                    vec![],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Expression,
                ));
                return Err(());
            }
        };
        // Postfix: calls, field access, method calls
        loop {
            match self.peek() {
                Token::LParen => {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(&Token::RParen)?;
                    // VUV view-call form: `Ident(args) { children }` parses as JSX.
                    // Trigger requires (a) callee is a bare Ident (no method chains, no field access)
                    // and (b) next non-newline token is `{`. Sugars to Expr::Jsx so HIR / web_ir /
                    // codegen are untouched. Positional args are rejected — view calls are kw-only.
                    //
                    // Capitalized-Ident calls without a trailing block are also sugared into
                    // Expr::JsxSelfClosing — `Component()` ≡ `Component() {}` ≡ `<Component/>`.
                    // This matches React's tag-naming convention and avoids forcing every
                    // self-closing component invocation to write `() {}`.
                    if let Expr::Ident { name: tag, .. } = &expr {
                        let mut peek_pos = self.pos;
                        while peek_pos < self.tokens.len()
                            && matches!(self.tokens[peek_pos].token, Token::Newline)
                        {
                            peek_pos += 1;
                        }
                        let is_brace_next = matches!(
                            self.tokens.get(peek_pos).map(|t| &t.token),
                            Some(Token::LBrace)
                        );
                        let starts_uppercase =
                            tag.chars().next().is_some_and(|c| c.is_ascii_uppercase());
                        let is_view_callee = starts_uppercase
                            || crate::lowering_shared::primitive_tags::is_primitive(tag)
                            || is_known_html_view_tag(tag);
                        let all_named = args.iter().all(|a| a.name.is_some());
                        // Trailing-block-as-children sugar fires only when the call shape is
                        // unambiguously a view-call: callee is a recognized view-callee
                        // (capitalized component, primitive, or known HTML/SVG tag), all args
                        // are named, AND a `{` follows. Otherwise the `{` belongs to an outer
                        // construct (e.g. `if !has_capability(cap) {`), and we must NOT consume
                        // it as children.
                        if is_brace_next && is_view_callee && all_named {
                            let tag = tag.clone();
                            let attributes = self.view_args_to_attrs(args)?;
                            self.skip_newlines();
                            self.expect(&Token::LBrace)?;
                            let children = self.parse_view_children()?;
                            self.expect(&Token::RBrace)?;
                            expr = Expr::Jsx(JsxElement {
                                tag,
                                attributes,
                                children,
                                span: start.merge(self.span()),
                            });
                            continue;
                        } else if (starts_uppercase
                            || crate::lowering_shared::primitive_tags::is_primitive(tag)
                            || is_known_html_view_tag(tag))
                            && args.iter().all(|a| a.name.is_some())
                        {
                            // Three view-call self-closing triggers, all requiring all-named args:
                            //   1. Capitalized callee (`Component(...)`) — React component shape.
                            //   2. Recognized UI primitive (`row(...)`, `panel(...)`).
                            //   3. Recognized raw HTML element from a curated allowlist
                            //      (`input(attr_type="checkbox")`, `select(...)`, etc.). The
                            //      allowlist guards against ordinary lowercase function calls
                            //      like `fetch(timeout=5)` accidentally lowering to JSX.
                            // Positional args (enum/type constructors like `Some(x)`) always fall
                            // through to a regular Expr::Call below.
                            let tag = tag.clone();
                            let attributes = self.view_args_to_attrs(args)?;
                            expr = Expr::JsxSelfClosing(JsxSelfClosingElement {
                                tag,
                                attributes,
                                span: start.merge(self.span()),
                            });
                            continue;
                        }
                    }
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        span: start.merge(self.span()),
                    };
                }
                Token::Dot => {
                    self.advance();
                    let field = self.parse_ident_name()?;
                    if self.eat(&Token::LParen) {
                        let args = self.parse_args()?;
                        self.expect(&Token::RParen)?;
                        expr = Expr::MethodCall {
                            object: Box::new(expr),
                            method: field,
                            args,
                            span: start.merge(self.span()),
                        };
                    } else {
                        expr = Expr::FieldAccess {
                            object: Box::new(expr),
                            field,
                            span: start.merge(self.span()),
                        };
                    }
                }
                Token::LBracket => {
                    let start_span = expr.span();
                    self.advance(); // consume [
                    let index_expr = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    let end_span = self.span();
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index: Box::new(index_expr),
                        span: start_span.merge(end_span),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    pub(crate) fn parse_args(&mut self) -> Result<Vec<Arg>, ()> {
        let mut args = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek(), Token::RParen | Token::Eof) {
            // Check for named arg: name=value
            if let Token::Ident(name) = self.peek().clone() {
                let saved = self.pos;
                self.advance();
                if self.eat(&Token::Eq) {
                    let value = self.parse_expr()?;
                    args.push(Arg {
                        name: Some(name),
                        value,
                    });
                    self.skip_newlines();
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                    self.skip_newlines();
                    continue;
                }
                self.pos = saved; // backtrack
            }
            let value = self.parse_expr()?;
            args.push(Arg { name: None, value });
            self.skip_newlines();
            if !self.eat(&Token::Comma) {
                break;
            }
            self.skip_newlines();
        }
        Ok(args)
    }

    pub(crate) fn parse_brace_expr(&mut self) -> Result<Expr, ()> {
        let start = self.span();

        // Peek past { and newlines
        let mut i = self.pos + 1;
        while i < self.tokens.len() && matches!(self.tokens[i].token, Token::Newline) {
            i += 1;
        }

        if matches!(self.tokens.get(i).map(|t| &t.token), Some(Token::RBrace)) {
            self.advance(); // {
            self.skip_newlines();
            self.advance(); // }
            return Ok(Expr::ObjectLit {
                fields: Vec::new(),
                span: start.merge(self.span()),
            });
        }

        let is_object = if let Some(
            Token::Ident(_)
            | Token::TypeIdent(_)
            | Token::StringLit(_)
            | Token::SingleStringLit(_)
            // Phonetic keywords usable as object keys (db query-predicate
            // vocabulary: `{ and: [..] }`, `{ or: [..] }`, `{ not: {..} }`,
            // `{ field: { in: [..] } }`). Disambiguated as an object literal —
            // not a block — by the trailing `:` check below.
            | Token::And
            | Token::Or
            | Token::Not
            | Token::In,
        ) = self.tokens.get(i).map(|t| &t.token)
        {
            let mut j = i + 1;
            while j < self.tokens.len() && matches!(self.tokens[j].token, Token::Newline) {
                j += 1;
            }
            matches!(self.tokens.get(j).map(|t| &t.token), Some(Token::Colon))
        } else {
            false
        };

        if is_object {
            self.parse_object_lit()
        } else {
            self.advance(); // {
            let stmts = self.parse_block()?;
            Ok(Expr::Block {
                stmts,
                span: start.merge(self.span()),
            })
        }
    }

    pub(crate) fn parse_object_lit(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance(); // {
        let mut fields = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            let key = match self.peek().clone() {
                Token::Ident(n) | Token::TypeIdent(n) => {
                    self.advance();
                    n
                }
                Token::StringLit(s) | Token::SingleStringLit(s) => {
                    self.advance();
                    s
                }
                // Phonetic keyword combinators/operators are valid object keys —
                // the db query-predicate vocabulary uses them
                // (`{ and: [..] }`, `{ or: [..] }`, `{ not: {..} }`,
                // `{ field: { in: [..] } }`). Without this they lex to keyword
                // tokens and fail key parsing, leaving the lowering's
                // and/or/not/in predicate branches unreachable from source.
                Token::And => {
                    self.advance();
                    "and".to_string()
                }
                Token::Or => {
                    self.advance();
                    "or".to_string()
                }
                Token::Not => {
                    self.advance();
                    "not".to_string()
                }
                Token::In => {
                    self.advance();
                    "in".to_string()
                }
                _ => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        "Expected identifier or string as object key",
                        vec![],
                        None,
                        ParseErrorClass::Expression,
                    ));
                    return Err(());
                }
            };
            self.expect(&Token::Colon)?;
            let value = self.parse_expr()?;
            fields.push((key, value));
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.skip_newlines();
        self.expect(&Token::RBrace)?;
        Ok(Expr::ObjectLit {
            fields,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_match(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance(); // eat 'match'
        let subject = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            let arm_start = self.span();
            let pattern = self.parse_pattern()?;
            if self.eat(&Token::Arrow) {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "The '->' syntax is deprecated for match arms. Use '=>'.",
                    vec![],
                    None,
                    ParseErrorClass::Expression,
                ));
            } else {
                self.expect(&Token::FatArrow)?;
            }
            // Match-arm bodies accept statement-leading constructs (return / break /
            // continue) by wrapping a single statement in an `Expr::Block`. The
            // expression path is unchanged. Regular `{ ... }` blocks are picked up
            // by the existing `parse_brace_expr` route via `parse_expr`.
            let body_start = self.span();
            let body = match self.peek() {
                Token::Return | Token::Break | Token::Continue => {
                    let stmt = self.parse_stmt()?;
                    Expr::Block {
                        stmts: vec![stmt],
                        span: body_start.merge(self.span()),
                    }
                }
                _ => self.parse_expr()?,
            };
            arms.push(MatchArm {
                pattern,
                guard: None,
                body: Box::new(body),
                span: arm_start.merge(self.span()),
            });
            self.skip_newlines();
            // Rust-style separators: `arm => expr,` (including trailing comma before `}`).
            let _ = self.eat(&Token::Comma);
            self.skip_newlines();
        }
        self.eat(&Token::RBrace);
        Ok(Expr::Match {
            subject: Box::new(subject),
            arms,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_if(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance(); // eat 'if'
        let condition = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let then_body = self.parse_block()?;
        self.skip_newlines();
        let else_body = if self.eat(&Token::Else) {
            self.skip_newlines();
            if matches!(self.peek(), Token::If) {
                // `else if` chain: recurse and wrap as a single expression statement
                let else_if_expr = self.parse_if()?;
                let span = else_if_expr.span();
                Some(vec![Stmt::Expr {
                    expr: else_if_expr,
                    span,
                }])
            } else {
                self.expect(&Token::LBrace)?;
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(Expr::If {
            condition: Box::new(condition),
            then_body,
            else_body,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_for(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance(); // eat 'for'
        // Accept `_` as a wildcard binding (e.g. `for _ in range(0, n)`)
        let binding = if self.eat(&Token::Underscore) {
            "_".to_string()
        } else {
            self.parse_ident_name()?
        };
        // Optional index variable: `for x, i in ...`
        let index = if self.eat(&Token::Comma) {
            if self.eat(&Token::Underscore) {
                Some("_".to_string())
            } else {
                Some(self.parse_ident_name()?)
            }
        } else {
            None
        };
        self.expect(&Token::In)?;
        let iterable = self.parse_expr()?;
        // Optional `key=expr` clause before the body.
        let key = if matches!(self.peek(), Token::Ident(n) if n == "key") {
            self.advance(); // eat 'key'
            self.expect(&Token::Eq)?;
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let body = self.parse_expr()?;
        Ok(Expr::For {
            binding,
            index,
            iterable: Box::new(iterable),
            body: Box::new(body),
            key,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_lambda(&mut self) -> Result<Expr, ()> {
        self.parse_lambda_with_cancellable(false)
    }

    pub(crate) fn parse_lambda_with_cancellable(&mut self, cancellable: bool) -> Result<Expr, ()> {
        let start = self.span();
        self.advance(); // eat 'fn'
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = if self.eat_return_arrow() {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        let body = self.parse_expr()?;
        Ok(Expr::Lambda {
            params,
            return_type,
            body: Box::new(body),
            cancellable,
            span: start.merge(self.span()),
        })
    }

    // Curated allowlist of raw HTML elements that may appear as view-calls outside the primitive
    // set. Only tags that genuinely belong in a view tree are listed; ordinary function names
    // like `fetch`, `query`, `request` — even when called with named args only — must NOT be
    // sugared into JSX, because that breaks them at the type-check layer.
}

#[must_use]
fn is_known_html_view_tag(tag: &str) -> bool {
    matches!(
        tag,
        // Form / interactive
        "input" | "label" | "textarea" | "select" | "option" | "form" | "fieldset" | "legend"
        // Tabular
        | "table" | "thead" | "tbody" | "tfoot" | "tr" | "th" | "td" | "caption"
        // Media
        | "audio" | "video" | "source" | "track" | "canvas" | "svg" | "img"
        // Inline structural
        | "span" | "br" | "hr" | "i" | "b" | "em" | "strong" | "small" | "code" | "pre"
        | "kbd" | "abbr" | "cite" | "blockquote" | "q"
        // Document
        | "head" | "meta" | "title" | "link" | "script" | "style" | "noscript" | "main" | "section"
        | "article" | "aside" | "header" | "footer" | "nav" | "details" | "summary" | "dialog"
        | "figure" | "figcaption"
        // Embedded
        | "iframe" | "embed" | "object" | "param"
        // SVG child elements (passthrough — no Tailwind base classes).
        // snake_case forms are accepted here; map_jsx_tag normalises them to camelCase
        // (radial_gradient → radialGradient, etc.) at emit time.
        | "g" | "defs" | "path" | "circle" | "rect" | "ellipse" | "line" | "polyline"
        | "polygon" | "text" | "tspan" | "use" | "symbol" | "marker" | "mask"
        | "pattern" | "stop" | "filter"
        | "radial_gradient" | "linear_gradient" | "clip_path" | "foreign_object"
        | "fe_gaussian_blur"
    )
}

impl super::super::Parser {
    /// VUV: convert positional/named call args into JSX attributes. Positional args are rejected
    /// because view calls are keyword-only by design (props need names like HTML attributes).
    ///
    /// Reserved-keyword escape: kwarg names beginning with `attr_` have the prefix stripped so
    /// HTML attributes whose names collide with Vox keywords can still be expressed. Example:
    /// `attr_type="checkbox"` → JSX `type="checkbox"`. The escape is needed because `type`,
    /// `for`, and similar HTML attribute names are reserved Vox identifiers and cannot appear
    /// as bare kwarg names without a parse error.
    pub(crate) fn view_args_to_attrs(&mut self, args: Vec<Arg>) -> Result<Vec<JsxAttribute>, ()> {
        let mut attrs = Vec::with_capacity(args.len());
        for arg in args {
            match arg.name {
                Some(mut name) => {
                    if let Some(rest) = name.strip_prefix("attr_") {
                        name = rest.to_string();
                    }
                    attrs.push(JsxAttribute {
                        name,
                        value: arg.value,
                    });
                }
                None => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        "Positional argument in view-call form. View calls are keyword-only — give every argument a name.",
                        vec![],
                        None,
                        ParseErrorClass::Expression,
                    ));
                    return Err(());
                }
            }
        }
        Ok(attrs)
    }

    /// VUV: parse the trailing `{ … }` children block of a view call. Each statement-position
    /// expression is one child; separators (newline, comma, semicolon) are all accepted.
    /// Caller has already consumed the opening `{`; this stops at (but does not consume) `}`.
    pub(crate) fn parse_view_children(&mut self) -> Result<Vec<Expr>, ()> {
        let mut children = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            let child = self.parse_expr()?;
            children.push(child);
            self.skip_newlines();
            self.eat(&Token::Comma);
            self.skip_newlines();
        }
        Ok(children)
    }

    /// Parse template string parts from a raw string with {expr} interpolation markers.
    /// This is a simple parser that splits on { and } and parses expressions between them.
    fn parse_template_string_parts(
        &mut self,
        s: String,
        start_span: crate::ast::span::Span,
    ) -> Result<Vec<StringPart>, ()> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = s.chars().peekable();
        let mut in_interp = false;

        while let Some(c) = chars.next() {
            if c == '{' && !in_interp {
                // Start of interpolation
                if !current.is_empty() {
                    parts.push(StringPart::Literal(current));
                    current = String::new();
                }
                in_interp = true;
            } else if c == '}' && in_interp {
                // End of interpolation - parse the expression
                in_interp = false;
                let expr_str = current.clone();
                current = String::new();

                // Parse the expression from the string
                // For now, we'll use a simple approach: tokenize and parse the expression
                // This is a simplified implementation - a full implementation would need
                // to properly handle nested braces and other edge cases
                let expr = self.parse_expression_from_string(&expr_str)?;
                parts.push(StringPart::Interpolation(Box::new(expr)));
            } else {
                current.push(c);
            }
        }

        // Add remaining literal part
        if !current.is_empty() {
            parts.push(StringPart::Literal(current));
        }

        // If we're still in interpolation at the end, that's an error
        if in_interp {
            self.errors.push(ParseError::classified(
                start_span,
                "Unclosed interpolation in template string",
                vec!["}".into()],
                None,
                ParseErrorClass::Expression,
            ));
            return Err(());
        }

        Ok(parts)
    }

    /// Parse an expression from a string (simplified for template string interpolation).
    /// This is a temporary implementation - a full implementation would need to handle
    /// the token stream properly rather than parsing from a string.
    fn parse_expression_from_string(&mut self, s: &str) -> Result<Expr, ()> {
        // For now, treat simple identifiers as expressions
        // A full implementation would need to tokenize and parse the expression properly
        if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
            Ok(Expr::Ident {
                name: s.to_string(),
                span: self.span(),
            })
        } else {
            // For more complex expressions, we'd need to tokenize and parse properly
            // For now, return an error
            self.errors.push(ParseError::classified(
                self.span(),
                "Complex expressions in template strings not yet supported",
                vec!["identifier".into()],
                Some(s.to_string()),
                ParseErrorClass::Expression,
            ));
            Err(())
        }
    }
}
