use super::Parser;
use vox_ast::decl::*;
use vox_lexer::token::Token;

impl Parser {
    pub(crate) fn parse_v0_component(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let (prompt, image_path) = match self.peek().clone() {
            Token::StringLit(s) => { self.advance(); (s, None) }
            Token::FromKw => {
                self.advance();
                match self.peek().clone() {
                    Token::StringLit(s) => { self.advance(); (String::new(), Some(s)) }
                    _ => { self.report_error("Expected image path string after 'from'"); return Err(()); }
                }
            }
            _ => { self.report_error("Expected prompt string or 'from' after @v0"); return Err(()); }
        };
        self.expect(&Token::Fn)?;
        let name = self.parse_ident_name()?;
        self.expect(&Token::LParen)?;
        self.expect(&Token::RParen)?;
        let return_type = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
        Ok(Decl::V0Component(V0ComponentDecl { prompt, image_path, name, return_type, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_style_blocks(&mut self) -> Vec<StyleBlock> {
        let mut styles = Vec::new();
        self.skip_newlines();
        while let Token::Ident(ref name) = self.peek().clone() {
            if name != "style" { break; }
            self.advance();
            if !self.eat(&Token::Colon) { break; }
            self.skip_newlines();
            let had_indent = self.eat(&Token::Indent);
            loop {
                self.skip_newlines();
                match self.peek().clone() {
                    Token::Dot => {
                        let sel_start = self.span();
                        self.advance();
                        let class_name = match self.parse_ident_name() { Ok(n) => n, Err(_) => break };
                        let selector = format!(".{}", class_name);
                        if !self.eat(&Token::Colon) { break; }
                        self.skip_newlines();
                        let had_prop_indent = self.eat(&Token::Indent);
                        let mut properties = Vec::new();
                        loop {
                            self.skip_newlines();
                            match self.peek().clone() {
                                Token::Ident(prop_name) => {
                                    self.advance();
                                    if !self.eat(&Token::Colon) { break; }
                                    match self.peek().clone() {
                                        Token::StringLit(val) => { self.advance(); properties.push((prop_name, val)); }
                                        _ => break,
                                    }
                                }
                                _ => break,
                            }
                        }
                        if had_prop_indent { self.eat(&Token::Dedent); }
                        styles.push(StyleBlock { selector, properties, span: sel_start.merge(self.span()) });
                    }
                    _ => break,
                }
            }
            if had_indent { self.eat(&Token::Dedent); }
        }
        styles
    }

    pub(crate) fn parse_keyframes(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        let had_indent = self.eat(&Token::Indent);
        let mut steps = Vec::new();
        loop {
            self.skip_newlines();
            let selector = match self.peek().clone() {
                Token::Ident(s) if s == "from" || s == "to" => { self.advance(); s }
                Token::FromKw => { self.advance(); "from".to_string() }
                Token::To => { self.advance(); "to".to_string() }
                Token::StringLit(s) => { self.advance(); s }
                _ => break,
            };
            if !self.eat(&Token::Colon) { break; }
            self.skip_newlines();
            let had_prop_indent = self.eat(&Token::Indent);
            let mut properties = Vec::new();
            loop {
                self.skip_newlines();
                match self.peek().clone() {
                    Token::Ident(prop_name) => {
                        self.advance();
                        if !self.eat(&Token::Colon) { break; }
                        match self.peek().clone() {
                            Token::StringLit(val) => { self.advance(); properties.push((prop_name, val)); }
                            _ => break,
                        }
                    }
                    _ => break,
                }
            }
            if had_prop_indent { self.eat(&Token::Dedent); }
            steps.push(KeyframeStep { selector, properties });
        }
        if had_indent { self.eat(&Token::Dedent); }
        Ok(Decl::Keyframes(KeyframeDecl { name, steps, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_theme(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        let had_indent = self.eat(&Token::Indent);
        let mut light = Vec::new();
        let mut dark = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Token::Ident(variant) if variant == "light" || variant == "dark" => {
                    self.advance();
                    if !self.eat(&Token::Colon) { break; }
                    self.skip_newlines();
                    let had_var_indent = self.eat(&Token::Indent);
                    let target = if variant == "light" { &mut light } else { &mut dark };
                    loop {
                        self.skip_newlines();
                        match self.peek().clone() {
                            Token::Ident(prop_name) => {
                                self.advance();
                                if !self.eat(&Token::Colon) { break; }
                                match self.peek().clone() {
                                    Token::StringLit(val) => { self.advance(); target.push((prop_name, val)); }
                                    _ => break,
                                }
                            }
                            _ => break,
                        }
                    }
                    if had_var_indent { self.eat(&Token::Dedent); }
                }
                _ => break,
            }
        }
        if had_indent { self.eat(&Token::Dedent); }
        Ok(Decl::Theme(ThemeDecl { name, light, dark, span: start.merge(self.span()) }))
    }
}
