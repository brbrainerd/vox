use super::Parser;
use vox_ast::expr::*;
use vox_lexer::token::Token;

impl Parser {
    pub(crate) fn parse_expr(&mut self) -> Result<Expr, ()> {
        self.parse_expr_bp(0)
    }

    pub(crate) fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ()> {
        let start = self.span();
        let mut lhs = match self.peek().clone() {
            Token::Not | Token::Minus | Token::Bnot => {
                let op = match self.peek() {
                    Token::Not => UnOp::Not,
                    Token::Minus => UnOp::Neg,
                    _ => UnOp::BitNot,
                };
                self.advance();
                let operand = self.parse_expr_bp(13)?; // High binding power for prefix
                Expr::Unary { op, operand: Box::new(operand), span: start.merge(self.span()) }
            }
            _ => self.parse_primary()?,
        };

        loop {
            let (op, l_bp, r_bp) = match self.peek() {
                Token::PipeOp => (Some(BinOp::Pipe), 1, 2),
                Token::With => {
                    let l_bp = 2;
                    if l_bp < min_bp { break; }
                    self.advance();
                    let options = self.parse_expr_bp(1)?;
                    lhs = Expr::With { operand: Box::new(lhs), options: Box::new(options), span: start.merge(self.span()) };
                    continue;
                }
                Token::Or => (Some(BinOp::Or), 3, 4),
                Token::And => (Some(BinOp::And), 5, 6),
                Token::Is => (Some(BinOp::Is), 7, 8),
                Token::Isnt => (Some(BinOp::Isnt), 7, 8),
                Token::Lt => {
                    // Disambiguate: `<Ident` is JSX, not a comparison op.
                    let next = self.tokens.get(self.pos + 1).map(|s| &s.token);
                    if matches!(next, Some(Token::Ident(_)) | Some(Token::TypeIdent(_))) {
                        break;
                    }
                    (Some(BinOp::Lt), 7, 8)
                }
                Token::Gt => (Some(BinOp::Gt), 7, 8),
                Token::Lte => (Some(BinOp::Lte), 7, 8),
                Token::Gte => (Some(BinOp::Gte), 7, 8),
                Token::Plus => (Some(BinOp::Add), 9, 10),
                Token::Minus => (Some(BinOp::Sub), 9, 10),
                Token::Star => (Some(BinOp::Mul), 11, 12),
                Token::Slash => (Some(BinOp::Div), 11, 12),
                Token::Percent => (Some(BinOp::Mod), 11, 12),
                Token::Band => (Some(BinOp::BitAnd), 11, 12),
                Token::Bor => (Some(BinOp::BitOr), 11, 12),
                Token::Bxor => (Some(BinOp::BitXor), 11, 12),
                Token::Shl => (Some(BinOp::Shl), 11, 12),
                Token::Shr => (Some(BinOp::Shr), 11, 12),
                Token::DotDot => (Some(BinOp::Range), 1, 2),
                Token::DotDotEq => (Some(BinOp::RangeInclusive), 1, 2),
                Token::DoubleQuestion => (Some(BinOp::NullCoalesce), 1, 2),
                Token::As => {
                    if 1 < min_bp { break; }
                    self.advance();
                    let target_type = self.parse_type_expr()?;
                    lhs = Expr::TypeCast { expr: Box::new(lhs), target_type, span: start.merge(self.span()) };
                    continue;
                }
                _ => break,
            };

            if l_bp < min_bp { break; }
            self.advance();
            if let Some(op) = op {
                let rhs = self.parse_expr_bp(r_bp)?;
                let span = lhs.span().merge(rhs.span());
                lhs = match op {
                    BinOp::Pipe => Expr::Pipe { left: Box::new(lhs), right: Box::new(rhs), span },
                    _ => Expr::Binary { op, left: Box::new(lhs), right: Box::new(rhs), span },
                };
            }
        }

        if self.eat(&Token::Question) {
            lhs = Expr::TryOp { operand: Box::new(lhs), span: start.merge(self.span()) };
        }

        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        let mut expr = match self.peek().clone() {
            Token::IntLit(v) => { self.advance(); Expr::IntLit { value: v, span: start } }
            Token::FloatLit(v) => { self.advance(); Expr::FloatLit { value: v, span: start } }
            Token::StringLit(v) | Token::SingleQuoteStringLit(v) => { self.advance(); Expr::StringLit { value: v, span: start } }
            Token::True => { self.advance(); Expr::BoolLit { value: true, span: start } }
            Token::False => { self.advance(); Expr::BoolLit { value: false, span: start } }
            Token::Spawn => {
                self.advance();
                let has_paren = self.eat(&Token::LParen);
                let target = if has_paren {
                    let expr = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    expr
                } else {
                    self.parse_expr_bp(14)?
                };
                Expr::Spawn { target: Box::new(target), span: start.merge(self.span()) }
            }
            Token::Await => {
                self.advance();
                let operand = self.parse_expr_bp(14)?;
                Expr::Await { operand: Box::new(operand), span: start.merge(self.span()) }
            }
            Token::Ident(_) | Token::Message | Token::Version | Token::Migrate | Token::On | Token::FromKw | Token::As | Token::Get | Token::Post | Token::Put | Token::Delete => {
                let name = self.parse_ident_name()?;
                Expr::Ident { name, span: start }
            }
            Token::TypeIdent(ref name) => {
                let name = name.clone();
                self.advance();
                if self.eat(&Token::Colon) {
                    let mut fields = Vec::new();
                    let had_indent = self.eat(&Token::Indent);
                    loop {
                        self.skip_newlines();
                        if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
                        let fname = self.parse_ident_name()?;
                        self.expect(&Token::Colon)?;
                        fields.push((fname, self.parse_expr()?));
                        if !self.eat(&Token::Comma) { break; }
                    }
                    if had_indent { self.eat(&Token::Dedent); }
                    Expr::ObjectLit { fields, span: start.merge(self.span()) }
                } else {
                    Expr::Ident { name, span: start }
                }
            }
            Token::LParen => {
                self.advance();
                let mut exprs = Vec::new();
                self.skip_newlines_and_indent();
                if !self.eat(&Token::RParen) {
                    loop {
                        exprs.push(self.parse_expr()?);
                        self.skip_newlines_and_indent();
                        if !self.eat(&Token::Comma) { break; }
                        self.skip_newlines_and_indent();
                    }
                    self.skip_newlines_and_indent();
                    self.expect(&Token::RParen)?;
                }
                if exprs.len() == 1 { exprs.remove(0) }
                else { Expr::TupleLit { elements: exprs, span: start.merge(self.span()) } }
            }
            Token::LBracket => {
                self.advance();
                let mut exprs = Vec::new();
                self.skip_newlines_and_indent();
                while !matches!(self.peek(), Token::RBracket | Token::Eof) {
                    exprs.push(self.parse_expr()?);
                    self.skip_newlines_and_indent();
                    if !self.eat(&Token::Comma) { break; }
                    self.skip_newlines_and_indent();
                }
                self.skip_newlines_and_indent();
                self.expect(&Token::RBracket)?;
                Expr::ListLit { elements: exprs, span: start.merge(self.span()) }
            }
            Token::LBrace => self.parse_object_lit_internal()?,
            Token::Match => self.parse_match()?,
            Token::If => self.parse_if()?,
            Token::For => self.parse_for()?,
            Token::While => self.parse_while()?,
            Token::Loop => self.parse_loop()?,
            Token::Stream => {
                self.advance();
                self.expect(&Token::Colon)?;
                let body = self.parse_block()?;
                Expr::StreamBlock { body, span: start.merge(self.span()) }
            }
            Token::Try => {
                self.advance();
                self.expect(&Token::Colon)?;
                let body = self.parse_block()?;
                self.skip_newlines();
                self.expect(&Token::Catch)?;
                let catch_binding = self.parse_ident_name()?;
                self.expect(&Token::Colon)?;
                let catch_body = self.parse_block()?;
                Expr::TryCatch { body, catch_binding, catch_body, span: start.merge(self.span()) }
            }
            Token::Fn => self.parse_lambda()?,
            Token::Lt => self.parse_jsx()?,
            _ => { self.report_error("Expected expression"); return Err(()); }
        };

        loop {
            match self.peek() {
                Token::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    self.skip_newlines_and_indent();
                    while !matches!(self.peek(), Token::RParen | Token::Eof) {
                        let mut name = None;
                        if let Token::Ident(n) = self.peek().clone() {
                            let next_token = self.tokens.get(self.pos + 1).map(|t| &t.token);
                            if next_token == Some(&Token::Eq) || next_token == Some(&Token::Colon) {
                                name = Some(n);
                                self.advance();
                                self.advance();
                            }
                        }
                        args.push(Arg { name, value: self.parse_expr()? });
                        if !self.eat(&Token::Comma) { break; }
                        self.skip_newlines_and_indent();
                    }
                    self.skip_newlines_and_indent();
                    self.expect(&Token::RParen)?;
                    expr = Expr::Call { callee: Box::new(expr), args, span: start.merge(self.span()) };
                }
                Token::Dot | Token::QuestionDot => {
                    let is_optional = self.peek() == &Token::QuestionDot;
                    self.advance();
                    let field = self.parse_ident_name()?;
                    if self.peek() == &Token::LParen {
                        self.advance();
                        let mut args = Vec::new();
                        self.skip_newlines_and_indent();
                        while !matches!(self.peek(), Token::RParen | Token::Eof) {
                            let mut name = None;
                            if let Token::Ident(n) = self.peek().clone() {
                                let next_token = self.tokens.get(self.pos + 1).map(|t| &t.token);
                                if next_token == Some(&Token::Eq) || next_token == Some(&Token::Colon) {
                                    name = Some(n);
                                    self.advance();
                                    self.advance();
                                }
                            }
                            args.push(Arg { name, value: self.parse_expr()? });
                            if !self.eat(&Token::Comma) { break; }
                            self.skip_newlines_and_indent();
                        }
                        self.skip_newlines_and_indent();
                        self.expect(&Token::RParen)?;
                        expr = Expr::MethodCall { object: Box::new(expr), method: field, args, span: start.merge(self.span()) };
                    } else if is_optional {
                        expr = Expr::OptionalChain { object: Box::new(expr), field, span: start.merge(self.span()) };
                    } else {
                        expr = Expr::FieldAccess { object: Box::new(expr), field, span: start.merge(self.span()) };
                    }
                }
                Token::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::IndexAccess { object: Box::new(expr), index: Box::new(index), span: start.merge(self.span()) };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_object_lit_internal(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        let mut fields = Vec::new();
        self.skip_newlines_and_indent();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            let fname = match self.peek().clone() {
                Token::StringLit(s) => { self.advance(); s }
                Token::Ident(_) | Token::Message | Token::Version | Token::Migrate | Token::On | Token::FromKw | Token::As | Token::Get | Token::Post | Token::Put | Token::Delete | Token::TypeIdent(_) => {
                    self.parse_ident_name()?
                }
                _ => { self.report_error("Expected field name"); return Err(()); }
            };
            self.expect(&Token::Colon)?;
            fields.push((fname, self.parse_expr()?));
            self.skip_newlines_and_indent();
            if !self.eat(&Token::Comma) { break; }
            self.skip_newlines_and_indent();
        }
        self.skip_newlines_and_indent();
        self.expect(&Token::RBrace)?;
        Ok(Expr::ObjectLit { fields, span: start.merge(self.span()) })
    }

    fn parse_match(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        let target = self.parse_expr()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        self.eat(&Token::Indent);
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            self.eat(&Token::Bar); // optional leading | before each arm
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            let pattern = self.parse_pattern()?;
            self.expect(&Token::Arrow)?;
            let body = if self.eat(&Token::Colon) {
                let bstart = self.span();
                Expr::Block { stmts: self.parse_block()?, span: bstart.merge(self.span()) }
            } else if self.peek() == &Token::Newline && matches!(self.peek_at(1), Token::Indent) {
                self.advance(); // Newline
                let bstart = self.span();
                Expr::Block { stmts: self.parse_block()?, span: bstart.merge(self.span()) }
            } else {
                self.parse_expr()?
            };
            arms.push(MatchArm { pattern, body: Box::new(body), guard: None, span: start.merge(self.span()) });
        }
        self.eat(&Token::Dedent);
        Ok(Expr::Match { subject: Box::new(target), arms, span: start.merge(self.span()) })
    }

    fn parse_if(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        let condition = self.parse_expr()?;
        self.expect(&Token::Colon)?;
        let then_body = self.parse_block()?;
        self.skip_newlines();
        let else_body = if self.eat(&Token::Else) {
            if self.eat(&Token::Colon) {
                Some(self.parse_block()?)
            } else if self.peek() == &Token::If {
                Some(vec![vox_ast::stmt::Stmt::Expr { expr: self.parse_if()?, span: self.span() }])
            } else {
                self.report_error("Expected : or if after else");
                return Err(());
            }
        } else { None };
        Ok(Expr::If { condition: Box::new(condition), then_body, else_body, span: start.merge(self.span()) })
    }

    fn parse_for(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        let mut bindings = vec![self.parse_pattern()?];
        while self.eat(&Token::Comma) {
            bindings.push(self.parse_pattern()?);
        }
        let binding = if bindings.len() == 1 {
            bindings.pop().unwrap()
        } else {
            let start_span = bindings.first().unwrap().span();
            let end_span = bindings.last().unwrap().span();
            vox_ast::pattern::Pattern::Tuple { elements: bindings, span: start_span.merge(end_span) }
        };
        self.expect(&Token::In)?;
        let iterable = self.parse_expr()?;

        // Optionally parse `key <expr>` — "key" is contextual (not a reserved keyword).
        // e.g. `for item in items key item.id:` emits React key={item.id} in codegen.
        let key = if let Token::Ident(ref s) = self.peek().clone() {
            if s == "key" {
                self.advance(); // consume the "key" ident
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            }
        } else {
            None
        };

        self.expect(&Token::Colon)?;

        self.skip_newlines();
        let body = if self.peek() == &Token::Indent {
            let bstart = self.span();
            Expr::Block { stmts: self.parse_block()?, span: bstart.merge(self.span()) }
        } else {
            self.parse_expr()?
        };

        Ok(Expr::For { binding, iterable: Box::new(iterable), key, body: Box::new(body), span: start.merge(self.span()) })
    }

    fn parse_while(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        let condition = self.parse_expr()?;
        self.expect(&Token::Colon)?;
        let body = self.parse_block()?;
        Ok(Expr::While { label: None, condition: Box::new(condition), body, span: start.merge(self.span()) })
    }

    fn parse_loop(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        self.expect(&Token::Colon)?;
        let body = self.parse_block()?;
        Ok(Expr::Loop { label: None, body, span: start.merge(self.span()) })
    }

    fn parse_lambda(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
        let body = if self.eat(&Token::Colon) {
            let bstart = self.span();
            Expr::Block { stmts: self.parse_block()?, span: bstart.merge(self.span()) }
        } else if self.peek() == &Token::LParen {
            self.parse_expr()?
        } else {
            self.parse_expr()?
        };
        Ok(Expr::Lambda { params, return_type, body: Box::new(body), span: start.merge(self.span()) })
    }

    fn parse_jsx(&mut self) -> Result<Expr, ()> {
        let start = self.span();
        self.advance();
        let tag = self.parse_ident_name()?;
        let mut attrs = Vec::new();
        loop {
            self.skip_newlines();
            while matches!(self.peek(), Token::Indent | Token::Dedent) {
                self.advance();
                self.skip_newlines();
            }
            match self.peek() {
                Token::Gt | Token::JsxSelfClose | Token::Eof => break,
                _ => {
                    let name = self.parse_ident_name()?;
                    self.expect(&Token::Eq)?;
                    let value = if self.eat(&Token::LBrace) {
                        let e = self.parse_expr()?;
                        self.expect(&Token::RBrace)?;
                        e
                    } else if let Token::StringLit(s) = self.peek().clone() {
                        let sp = self.span();
                        self.advance();
                        Expr::StringLit { value: s, span: sp }
                    } else { self.parse_expr()? };
                    attrs.push(JsxAttribute { name, value });
                }
            }
        }
        let self_closing = self.eat(&Token::JsxSelfClose);
        if self_closing {
            Ok(Expr::JsxSelfClosing(JsxSelfClosingElement { tag, attributes: attrs, span: start.merge(self.span()) }))
        } else {
            self.expect(&Token::Gt)?;
            let mut children = Vec::new();
            loop {
                self.skip_newlines();
                while matches!(self.peek(), Token::Indent | Token::Dedent) {
                    self.advance();
                    self.skip_newlines();
                }
                match self.peek().clone() {
                    Token::JsxCloseStart => {
                        self.advance();
                        self.expect(&Token::Ident(tag.clone()))?;
                        self.expect(&Token::Gt)?;
                        break;
                    }
                    Token::Eof => break,
                    // {expr} is JSX interpolation
                    Token::LBrace => {
                        self.advance();
                        let e = self.parse_expr()?;
                        self.expect(&Token::RBrace)?;
                        children.push(e);
                    }
                    // nested JSX element
                    Token::Lt => {
                        children.push(self.parse_jsx()?);
                    }
                    // `for x in y:` inside JSX is a for-expression child
                    Token::For => {
                        children.push(self.parse_for()?);
                    }
                    // Text content tokens (plain identifiers, type-idents, literals) —
                    // collect them as string-like idents; skip rather than calling parse_expr
                    // which can loop via primary → jsx → primary infinitely.
                    Token::Ident(_) | Token::TypeIdent(_) | Token::StringLit(_) | Token::IntLit(_) | Token::FloatLit(_) => {
                        let sp = self.span();
                        let text = match self.peek().clone() {
                            Token::Ident(s) | Token::TypeIdent(s) | Token::StringLit(s) => s,
                            Token::IntLit(v) => v.to_string(),
                            Token::FloatLit(v) => v.to_string(),
                            _ => unreachable!(),
                        };
                        self.advance();
                        // Keep consuming same-line text tokens
                        children.push(Expr::StringLit { value: text, span: sp });
                    }
                    // Any other expression-starting token
                    _ => {
                        // Guard: only attempt if not going to recurse into jsx again
                        let tok = self.peek().clone();
                        if matches!(tok, Token::Newline | Token::Gt) {
                            self.advance(); // skip stray newline / gt inside body
                        } else {
                            // Skip unrecognised tokens to avoid infinite loops
                            self.advance();
                        }
                    }
                }
            }
            Ok(Expr::Jsx(JsxElement { tag, attributes: attrs, children, span: start.merge(self.span()) }))
        }
    }
}
