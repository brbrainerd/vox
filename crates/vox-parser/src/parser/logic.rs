use super::Parser;
use vox_ast::decl::*;
use vox_lexer::token::Token;

impl Parser {
    pub(crate) fn parse_actor(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        self.eat(&Token::Indent);
        let mut handlers = Vec::new();
        let mut state_fields = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            let mut h_traced = false;
            while self.eat(&Token::AtTrace) {
                h_traced = true;
                self.skip_newlines();
            }
            if self.eat(&Token::On) {
                let hstart = self.span();
                let event = self.parse_ident_name()?;
                self.expect(&Token::LParen)?;
                let params = self.parse_params()?;
                self.expect(&Token::RParen)?;
                let ret = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
                self.expect(&Token::Colon)?;
                handlers.push(ActorHandler {
                    event_name: event,
                    params,
                    return_type: ret,
                    body: self.parse_block()?,
                    is_traced: h_traced,
                    span: hstart.merge(self.span()),
                });
            } else if let Token::Ident(s) = self.peek().clone() {
                if s == "state" {
                    self.advance();
                    let fstart = self.span();
                    let fname = self.parse_ident_name()?;
                    self.expect(&Token::Colon)?;
                    let type_ann = self.parse_type_expr()?;
                    if self.eat(&Token::Eq) {
                        let _ = self.parse_expr()?;
                    }
                    state_fields.push(VariantField { name: fname, type_ann, span: fstart.merge(self.span()) });
                } else { break; }
            } else { break; }
        }
        self.eat(&Token::Dedent);
        Ok(Decl::Actor(ActorDecl { name, handlers, state_fields, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_agent_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        self.eat(&Token::Indent);

        let mut state_fields = Vec::new();
        let mut handlers = Vec::new();
        let mut version = None;
        let mut migrations = Vec::new();

        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            if self.eat(&Token::Version) {
                if let Token::StringLit(v) = self.peek().clone() {
                    version = Some(v); self.advance();
                } else { self.report_error("Expected version string"); return Err(()); }
                self.skip_newlines();
            } else if self.eat(&Token::Migrate) {
                let mstart = self.span();
                self.expect(&Token::FromKw)?;
                let from_version = if let Token::StringLit(v) = self.peek().clone() {
                    self.advance(); v
                } else { self.report_error("Expected version string"); return Err(()); };
                self.expect(&Token::Colon)?;
                migrations.push(MigrationRule { from_version, body: self.parse_block()?, span: mstart.merge(self.span()) });
            } else if self.peek() == &Token::AtTrace || self.peek() == &Token::On {
                let mut h_traced = false;
                while self.eat(&Token::AtTrace) {
                    h_traced = true;
                    self.skip_newlines();
                }
                if !self.eat(&Token::On) { break; }
                let hstart = self.span();
                let event = self.parse_ident_name()?;
                self.expect(&Token::LParen)?;
                let params = self.parse_params()?;
                self.expect(&Token::RParen)?;
                let ret = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
                self.expect(&Token::Colon)?;
                handlers.push(AgentHandler {
                    event_name: event,
                    params,
                    return_type: ret,
                    body: self.parse_block()?,
                    is_traced: h_traced,
                    span: hstart.merge(self.span()),
                });
            } else {
                let fstart = self.span();
                let fname = self.parse_ident_name()?;
                self.expect(&Token::Colon)?;
                state_fields.push(VariantField { name: fname, type_ann: self.parse_type_expr()?, span: fstart.merge(self.span()) });
                self.skip_newlines();
            }
        }
        self.eat(&Token::Dedent);
        Ok(Decl::Agent(AgentDecl { name, version, state_fields, handlers, migrations, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_message_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        self.eat(&Token::Indent);
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            let fstart = self.span();
            let fname = self.parse_ident_name()?;
            self.expect(&Token::Colon)?;
            fields.push(VariantField { name: fname, type_ann: self.parse_type_expr()?, span: fstart.merge(self.span()) });
            self.skip_newlines();
        }
        self.eat(&Token::Dedent);
        Ok(Decl::Message(MessageDecl { name, fields, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_workflow(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let ret = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
        self.expect(&Token::Colon)?;
        Ok(Decl::Workflow(WorkflowDecl { name, params, return_type: ret, body: self.parse_block()?, is_traced: false, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_activity(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let ret = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
        self.expect(&Token::Colon)?;

        let mut options = None;
        let mut prompt = None;
        let mut body = Vec::new();

        self.skip_newlines();
        if self.eat(&Token::Indent) {
            loop {
                self.skip_newlines();
                if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }

                // metadata: with { ... }
                if self.peek() == &Token::With && options.is_none() {
                    self.advance();
                    options = Some(self.parse_expr()?);
                    self.skip_newlines();
                    continue;
                }

                // metadata: prompt: "..."
                if let Token::Ident(ref id) = self.peek() {
                    if id == "prompt" && self.tokens.get(self.pos + 1).map(|t| &t.token) == Some(&Token::Colon) && prompt.is_none() {
                        self.advance(); // prompt
                        self.advance(); // :
                        if let Token::StringLit(s) = self.peek().clone() {
                            self.advance();
                            prompt = Some(s);
                        } else if let Token::SingleQuoteStringLit(s) = self.peek().clone() {
                            self.advance();
                            prompt = Some(s);
                        } else {
                            self.report_error("Expected string literal for prompt");
                        }
                        self.skip_newlines();
                        continue;
                    }
                }

                body.push(self.parse_stmt()?);
                self.skip_newlines();
            }
            self.eat(&Token::Dedent);
        }

        Ok(Decl::Activity(ActivityDecl {
            name,
            params,
            return_type: ret,
            body,
            options,
            prompt,
            is_traced: false,
            is_deprecated: false,
            span: start.merge(self.span())
        }))
    }

    pub(crate) fn parse_skill_decl(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @skill
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Skill(SkillDecl { func: f }))
    }

    pub(crate) fn parse_scheduled_decl(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @scheduled
        let interval = if let Token::StringLit(s) = self.peek().clone() { self.advance(); s }
        else { self.report_error("Expected interval string"); return Err(()); };
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Scheduled(ScheduledDecl { interval, func: f }))
    }

    pub(crate) fn parse_agent_def_decl(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @agent_def
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::AgentDef(AgentDefDecl { func: f }))
    }

    pub(crate) fn parse_context_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @context
        let name = self.parse_ident_name()?;
        let state_type = if self.eat(&Token::Colon) { Some(self.parse_type_expr()?) } else { None };
        let default_expr = if self.eat(&Token::Eq) { Some(self.parse_expr()?) } else { None };
        Ok(Decl::Context(ContextDecl { name, state_type, default_expr, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_provider_decl(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @provider
        let context_name = if self.eat(&Token::LParen) {
            let name = self.parse_ident_name()?;
            self.expect(&Token::RParen)?;
            name
        } else { String::new() };
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        let span = f.span;
        Ok(Decl::Provider(ProviderDecl { context_name, func: f, span }))
    }

    pub(crate) fn parse_mock_decl(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @mock
        let target = if self.eat(&Token::LParen) {
            let t = if let Token::StringLit(s) = self.peek().clone() { self.advance(); s }
            else { self.report_error("Expected string after @mock("); return Err(()); };
            self.expect(&Token::RParen)?;
            t
        } else { String::new() };
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Mock(MockDecl { target, func: f }))
    }
}
