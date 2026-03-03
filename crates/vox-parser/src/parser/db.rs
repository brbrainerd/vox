use super::Parser;
use vox_ast::decl::*;
use vox_lexer::token::Token;

impl Parser {
    pub(crate) fn parse_collection(&mut self, is_pub: bool) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        self.eat(&Token::TypeKw);
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        let had_indent = self.eat(&Token::Indent);
        let mut fields = Vec::new();
        let mut has_spread = false;
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof | Token::RBrace) { break; }

            if self.eat(&Token::DotDotDot) {
                has_spread = true;
                self.skip_newlines();
                continue;
            }

            let fstart = self.span();
            let mut f_description = None;

            // Per-field decorators
            while self.peek() == &Token::AtDescribe {
                self.advance();
                self.expect(&Token::LParen)?;
                if let Token::StringLit(s) = self.peek().clone() { f_description = Some(s); self.advance(); }
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            }

            let fname = self.parse_ident_name()?;
            self.expect(&Token::Colon)?;
            fields.push(TableField {
                name: fname,
                type_ann: self.parse_type_expr()?,
                description: f_description,
                span: fstart.merge(self.span())
            });
        }
        if had_indent { self.eat(&Token::Dedent); }
        Ok(Decl::Collection(CollectionDecl {
            name,
            fields,
            description: None,
            is_pub,
            has_spread,
            span: start.merge(self.span())
        }))
    }

    pub(crate) fn parse_table(&mut self, is_pub: bool) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        self.eat(&Token::TypeKw);
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        let had_indent = self.eat(&Token::Indent);
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof | Token::RBrace) { break; }

            let fstart = self.span();
            let mut f_description = None;

            // Per-field decorators
            while self.peek() == &Token::AtDescribe {
                self.advance();
                self.expect(&Token::LParen)?;
                if let Token::StringLit(s) = self.peek().clone() { f_description = Some(s); self.advance(); }
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            }

            let fname = self.parse_ident_name()?;
            self.expect(&Token::Colon)?;
            fields.push(TableField {
                name: fname,
                type_ann: self.parse_type_expr()?,
                description: f_description,
                span: fstart.merge(self.span())
            });
        }
        if had_indent { self.eat(&Token::Dedent); }
        Ok(Decl::Table(TableDecl {
            name,
            fields,
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            is_pub,
            is_deprecated: false,
            span: start.merge(self.span())
        }))
    }

    pub(crate) fn parse_index(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let table_name = self.parse_ident_name()?;
        self.expect(&Token::Dot)?;
        let index_name = self.parse_ident_name()?;
        self.expect(&Token::On)?;
        self.expect(&Token::LParen)?;
        let mut columns = Vec::new();
        loop {
            columns.push(self.parse_ident_name()?);
            if !self.eat(&Token::Comma) { break; }
        }
        self.expect(&Token::RParen)?;
        Ok(Decl::Index(IndexDecl { table_name, index_name, columns, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_vector_index(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let table_name = self.parse_ident_name()?;
        self.expect(&Token::Dot)?;
        let index_name = self.parse_ident_name()?;
        self.expect(&Token::On)?;
        self.expect(&Token::LParen)?;
        let column = self.parse_ident_name()?;
        self.expect(&Token::RParen)?;
        let mut dimensions = 0;
        if self.eat(&Token::Ident("dimensions".into())) {
            if let Token::IntLit(d) = self.peek().clone() { dimensions = d as u32; self.advance(); }
        }
        let filter_fields = if self.eat(&Token::Ident("filter".into())) {
            self.expect(&Token::LParen)?;
            let mut f = Vec::new();
            loop { f.push(self.parse_ident_name()?); if !self.eat(&Token::Comma) { break; } }
            self.expect(&Token::RParen)?;
            f
        } else { vec![] };
        Ok(Decl::VectorIndex(VectorIndexDecl { table_name, index_name, column, dimensions, filter_fields, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_search_index(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let table_name = self.parse_ident_name()?;
        self.expect(&Token::Dot)?;
        let index_name = self.parse_ident_name()?;
        self.expect(&Token::On)?;
        self.expect(&Token::LParen)?;
        let search_field = self.parse_ident_name()?;
        self.expect(&Token::RParen)?;
        let filter_fields = if self.eat(&Token::Ident("filter".into())) {
            self.expect(&Token::LParen)?;
            let mut f = Vec::new();
            loop { f.push(self.parse_ident_name()?); if !self.eat(&Token::Comma) { break; } }
            self.expect(&Token::RParen)?;
            f
        } else { vec![] };
        Ok(Decl::SearchIndex(SearchIndexDecl { table_name, index_name, search_field, filter_fields, span: start.merge(self.span()) }))
    }
}
