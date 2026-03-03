use super::Parser;
use vox_ast::expr::Expr;
use vox_ast::pattern::Pattern;
use vox_lexer::token::Token;

impl Parser {
    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, ()> {
        self.eat(&Token::Bar); // optional leading bar
        let mut pats = vec![self.parse_primary_pattern()?];
        while self.eat(&Token::Bar) {
            pats.push(self.parse_primary_pattern()?);
        }
        if pats.len() == 1 {
            Ok(pats.into_iter().next().ok_or(())?)
        } else {
            let (first, last) = (pats.first().ok_or(())?, pats.last().ok_or(())?);
            let span = first.span().merge(last.span());
            Ok(Pattern::Or { patterns: pats, span })
        }
    }

    pub(crate) fn parse_primary_pattern(&mut self) -> Result<Pattern, ()> {
        let start_span = self.span();
        let mut pattern = match self.peek().clone() {
            Token::Underscore => { self.advance(); Pattern::Wildcard { span: start_span } }
            Token::LParen => {
                self.advance();
                let mut elems = Vec::new();
                loop {
                    if matches!(self.peek(), Token::RParen) { break; }
                    elems.push(self.parse_pattern()?);
                    if !self.eat(&Token::Comma) { break; }
                }
                self.expect(&Token::RParen)?;
                Pattern::Tuple { elements: elems, span: start_span.merge(self.span()) }
            }
            Token::TypeIdent(name) => {
                self.advance();
                if self.eat(&Token::LParen) {
                    let mut fields = Vec::new();
                    while !matches!(self.peek(), Token::RParen | Token::Eof) {
                        fields.push(self.parse_pattern()?);
                        if !self.eat(&Token::Comma) { break; }
                    }
                    self.expect(&Token::RParen)?;
                    Pattern::Constructor { name, fields, span: start_span.merge(self.span()) }
                } else if self.eat(&Token::At) {
                    let inner = self.parse_pattern()?;
                    Pattern::Binding { name, pattern: Box::new(inner), span: start_span.merge(self.span()) }
                } else {
                    Pattern::Ident { name, span: start_span.merge(self.span()) }
                }
            }
            Token::Ident(_) | Token::Message | Token::Version | Token::Migrate | Token::On | Token::FromKw | Token::As | Token::Get | Token::Post | Token::Put | Token::Delete => {
                let name = self.parse_ident_name()?;
                if self.eat(&Token::At) {
                    let inner = self.parse_pattern()?;
                    Pattern::Binding { name, pattern: Box::new(inner), span: start_span.merge(self.span()) }
                } else {
                    Pattern::Ident { name, span: start_span.merge(self.span()) }
                }
            }
            Token::LBrace => {
                self.advance();
                let mut fields = Vec::new();
                while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                    let fname = self.parse_ident_name()?;
                    let pat = if self.eat(&Token::Colon) { Some(self.parse_pattern()?) } else { None };
                    fields.push((fname, pat));
                    if !self.eat(&Token::Comma) { break; }
                }
                self.expect(&Token::RBrace)?;
                Pattern::Record { fields, span: start_span.merge(self.span()) }
            }
            Token::DotDotDot => {
                self.advance();
                let name = if let Token::Ident(n) = self.peek().clone() { self.advance(); Some(n) } else { None };
                Pattern::Rest { name, span: start_span.merge(self.span()) }
            }
            Token::IntLit(v) => {
                self.advance();
                Pattern::Literal { value: Box::new(Expr::IntLit { value: v, span: start_span }), span: start_span }
            }
            Token::StringLit(s) => {
                self.advance();
                Pattern::Literal { value: Box::new(Expr::StringLit { value: s, span: start_span }), span: start_span }
            }
            _ => { self.report_error("Expected pattern"); return Err(()); }
        };

        if self.eat(&Token::DotDot) || self.eat(&Token::DotDotEq) {
            let is_inclusive = self.peek() == &Token::DotDotEq;
            let start_expr = match pattern {
                Pattern::Literal { value, .. } => value,
                Pattern::Ident { name, span } => Box::new(Expr::Ident { name, span }),
                _ => { self.report_error("Invalid range start"); return Err(()); }
            };
            let end_expr = self.parse_expr()?;
            let full_span = start_expr.span().merge(end_expr.span());
            pattern = Pattern::Range { start: start_expr, end: Box::new(end_expr), inclusive: is_inclusive, span: full_span };
        }
        Ok(pattern)
    }
}
