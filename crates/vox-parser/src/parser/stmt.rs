use super::Parser;
use vox_ast::stmt::Stmt;
use vox_lexer::token::Token;

impl Parser {
    pub(crate) fn parse_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.span();
        match self.peek().clone() {
            Token::Let => self.parse_let_stmt(),
            Token::Use => self.parse_use_stmt(),
            Token::Ret => {
                self.advance();
                let value = if matches!(self.peek(), Token::Newline | Token::Dedent | Token::Eof | Token::Comma | Token::RBracket | Token::RParen | Token::RBrace) { None }
                else { Some(self.parse_expr()?) };
                Ok(Stmt::Return { value, span: start.merge(self.span()) })
            }
            Token::Break => {
                self.advance();
                let label = if let Token::Ident(l) = self.peek().clone() { self.advance(); Some(l) } else { None };
                let value = if matches!(self.peek(), Token::Newline | Token::Dedent | Token::Eof | Token::Comma | Token::RBracket | Token::RParen | Token::RBrace) { None }
                else { Some(self.parse_expr()?) };
                Ok(Stmt::Break { label, value, span: start.merge(self.span()) })
            }
            Token::Continue => {
                self.advance();
                let label = if let Token::Ident(l) = self.peek().clone() { self.advance(); Some(l) } else { None };
                Ok(Stmt::Continue { label, span: start.merge(self.span()) })
            }
            Token::Emit => {
                self.advance();
                let value = self.parse_expr()?;
                Ok(Stmt::Emit { value, span: start.merge(self.span()) })
            }
            Token::Fn if matches!(self.tokens.get(self.pos + 1).map(|t| &t.token), Some(Token::Ident(_))) => {
                self.advance(); // eat 'fn'
                let fn_name = self.parse_ident_name()?;
                let p_start = self.span();
                self.expect(&Token::LParen)?;
                let params = self.parse_params()?;
                self.expect(&Token::RParen)?;
                let return_type = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
                let body = if self.eat(&Token::Colon) {
                    let bstart = self.span();
                    vox_ast::expr::Expr::Block { stmts: self.parse_block()?, span: bstart.merge(self.span()) }
                } else if self.peek() == &Token::LParen {
                    self.parse_expr()?
                } else {
                    self.parse_expr()?
                };
                let lambda_span = start.merge(self.span());
                let lambda = vox_ast::expr::Expr::Lambda { params, return_type, body: Box::new(body), span: lambda_span };
                Ok(Stmt::Let {
                    pattern: vox_ast::pattern::Pattern::Ident { name: fn_name, span: p_start },
                    type_ann: None,
                    value: lambda,
                    mutable: false,
                    span: lambda_span
                })
            }
            _ => {
                let e = self.parse_expr()?;
                if self.eat(&Token::Eq) {
                    let value = self.parse_expr()?;
                    Ok(Stmt::Assign { target: e, value, span: start.merge(self.span()) })
                } else if matches!(self.peek(), Token::PlusEq | Token::MinusEq | Token::StarEq | Token::SlashEq | Token::PercentEq) {
                    let op = match self.peek() {
                        Token::PlusEq => vox_ast::expr::BinOp::Add,
                        Token::MinusEq => vox_ast::expr::BinOp::Sub,
                        Token::StarEq => vox_ast::expr::BinOp::Mul,
                        Token::SlashEq => vox_ast::expr::BinOp::Div,
                        Token::PercentEq => vox_ast::expr::BinOp::Mod,
                        _ => unreachable!(),
                    };
                    self.advance();
                    let value = self.parse_expr()?;
                    let span = start.merge(self.span());
                    // Desugar augmented assignment: a += b => a = a + b
                    let expanded_value = vox_ast::expr::Expr::Binary {
                        op,
                        left: Box::new(e.clone()),
                        right: Box::new(value),
                        span,
                    };
                    Ok(Stmt::Assign { target: e, value: expanded_value, span })
                } else {
                    Ok(Stmt::Expr { expr: e, span: start.merge(self.span()) })
                }
            }
        }
    }

    pub(crate) fn parse_let_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.span();
        self.advance();
        let mutable = self.eat(&Token::Mut);
        let pattern = self.parse_pattern()?;
        let type_ann = if self.eat(&Token::Colon) { Some(self.parse_type_expr()?) } else { None };
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Let { pattern, type_ann, value, mutable, span: start.merge(self.span()) })
    }

    pub(crate) fn parse_use_stmt(&mut self) -> Result<Stmt, ()> {
        let start = self.span();
        self.advance();
        let path = self.parse_import_path()?;
        Ok(Stmt::Use { path, span: start.merge(self.span()) })
    }

    pub(crate) fn parse_block(&mut self) -> Result<Vec<Stmt>, ()> {
        self.skip_newlines();
        if !self.eat(&Token::Indent) { return Ok(vec![]); }
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        self.eat(&Token::Dedent);
        Ok(stmts)
    }
}
