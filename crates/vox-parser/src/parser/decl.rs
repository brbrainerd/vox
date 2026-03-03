use super::Parser;
use vox_ast::decl::*;
use vox_ast::expr::Param;
use vox_lexer::token::Token;
use crate::parser::did_you_mean;

impl Parser {
    pub(crate) fn parse_decl(&mut self) -> Result<Decl, ()> {
        self.skip_newlines();

        let mut is_deprecated = false;
        let mut is_traced = false;
        let mut is_llm = false;
        let mut llm_model = None;
        let mut is_pure = false;
        let mut preconditions = Vec::new();
        let mut json_layout = None;
        let mut auth_provider = None;
        let mut roles = Vec::new();
        let mut cors = None;
        let mut is_metric = false;
        let mut metric_name = None;
        let mut is_health = false;
        let mut description = None;

        loop {
            if self.eat(&Token::AtDeprecated) { is_deprecated = true; self.skip_newlines(); }
            else if self.eat(&Token::AtTrace) { is_traced = true; self.skip_newlines(); }
            else if self.eat(&Token::AtPure) { is_pure = true; self.skip_newlines(); }
            else if self.eat(&Token::AtJson) {
                self.expect(&Token::LParen)?;
                if let Token::StringLit(s) = self.peek().clone() { json_layout = Some(s); self.advance(); }
                else { self.report_error("Expected layout string after @json"); return Err(()); }
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            } else if self.eat(&Token::AtRequire) {
                self.expect(&Token::LParen)?;
                preconditions.push(self.parse_expr()?);
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            } else if self.eat(&Token::AtAuth) {
                self.expect(&Token::LParen)?;
                if let Token::StringLit(s) = self.peek().clone() { auth_provider = Some(s); self.advance(); }
                else { self.report_error("Expected string after @auth"); return Err(()); }
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            } else if self.eat(&Token::AtRole) {
                self.expect(&Token::LParen)?;
                if let Token::StringLit(s) = self.peek().clone() { roles.push(s); self.advance(); }
                else { self.report_error("Expected string after @role"); return Err(()); }
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            } else if self.eat(&Token::AtCors) {
                self.expect(&Token::LParen)?;
                if let Token::StringLit(s) = self.peek().clone() { cors = Some(s); self.advance(); }
                else { self.report_error("Expected string after @cors"); return Err(()); }
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            } else if self.eat(&Token::AtLlm) {
                is_llm = true;
                if self.eat(&Token::LParen) {
                    if let Token::StringLit(s) = self.peek().clone() { llm_model = Some(s); self.advance(); }
                    else { self.report_error("Expected string literal for llm model"); return Err(()); }
                    self.expect(&Token::RParen)?;
                }
                self.skip_newlines();
            } else if self.eat(&Token::AtMetric) {
                is_metric = true;
                if self.eat(&Token::LParen) {
                    if let Token::StringLit(s) = self.peek().clone() { metric_name = Some(s); self.advance(); }
                    else { self.report_error("Expected metric name string after @metric("); return Err(()); }
                    self.expect(&Token::RParen)?;
                }
                self.skip_newlines();
            } else if self.eat(&Token::AtDescribe) {
                self.expect(&Token::LParen)?;
                if let Token::StringLit(s) = self.peek().clone() { description = Some(s); self.advance(); }
                else { self.report_error("Expected string after @describe"); return Err(()); }
                self.expect(&Token::RParen)?;
                self.skip_newlines();
            } else if self.eat(&Token::AtHealth) { is_health = true; self.skip_newlines(); }
            else { break; }
        }

        let mut decl = self.parse_decl_inner()?;
        decl.set_decorators(is_deprecated, is_pure, is_traced, is_llm, llm_model, false, is_metric, metric_name, is_health);
        if let Some(desc) = description { decl.set_description(desc); }
        if let Some(layout) = json_layout { decl.set_json_layout(layout); }
        decl.set_security(auth_provider, roles, cors);
        if !preconditions.is_empty() { if let Decl::Function(ref mut f) = decl { f.preconditions = preconditions; } }
        Ok(decl)
    }

    fn parse_decl_inner(&mut self) -> Result<Decl, ()> {
        match self.peek().clone() {
            Token::Import => self.parse_import(),
            Token::AtPyImport => self.parse_py_import(),
            Token::AtComponent => self.parse_component(),
            Token::AtPage => self.parse_page(),
            Token::AtTest => self.parse_test(),
            Token::AtFixture => self.parse_fixture(),
            Token::AtServer => self.parse_server_fn(),
            Token::AtV0 => self.parse_v0_component(),
            Token::AtMcpTool => self.parse_mcp_tool(),
            Token::AtMcpResource => self.parse_mcp_resource(),
            Token::AtQuery => self.parse_query_decl(),
            Token::AtMutation => self.parse_mutation_decl(),
            Token::AtAction => self.parse_action_decl(),
            Token::AtSkill => self.parse_skill_decl(),
            Token::AtAgentDef => self.parse_agent_def_decl(),
            Token::AtScheduled => self.parse_scheduled_decl(),
            Token::AtConfig => self.parse_config(),
            Token::AtEnvironment => self.parse_environment_decl(),
            Token::AtHook => { self.advance(); let f = self.parse_fn_decl(false)?; Ok(Decl::Hook(HookDecl { func: f })) }
            Token::AtTheme => self.parse_theme(),
            Token::AtMock => self.parse_mock_decl(),
            Token::AtContext => self.parse_context_decl(),
            Token::AtProvider => self.parse_provider_decl(),
            Token::AtKeyframes => self.parse_keyframes(),
            Token::AtLayout => { self.advance(); self.skip_newlines(); let f = self.parse_fn_decl(false)?; Ok(Decl::Layout(LayoutDecl { func: f })) }
            Token::AtLoading => { self.advance(); self.skip_newlines(); let f = self.parse_fn_decl(false)?; Ok(Decl::Loading(LoadingDecl { func: f })) }
            Token::AtNotFound => { self.advance(); self.skip_newlines(); let f = self.parse_fn_decl(false)?; Ok(Decl::NotFound(NotFoundDecl { func: f })) }
            Token::AtErrorBoundary => { self.advance(); self.skip_newlines(); let f = self.parse_fn_decl(false)?; Ok(Decl::ErrorBoundary(ErrorBoundaryDecl { func: f })) }
            Token::AtAnimate => { self.advance(); self.skip_newlines(); Ok(Decl::Function(self.parse_fn_decl(false)?)) }
            Token::AtBuildConst => { self.advance(); let is_pub = self.eat(&Token::Pub); self.parse_const(is_pub, true) }
            Token::Fn | Token::Async => Ok(Decl::Function(self.parse_fn_decl(false)?)),
            Token::Pub => {
                self.advance();
                match self.peek().clone() {
                    Token::Fn | Token::Async => Ok(Decl::Function(self.parse_fn_decl(true)?)),
                    Token::TypeKw => self.parse_typedef(true),
                    Token::Const => self.parse_const(true, false),
                    Token::AtBuildConst => { self.advance(); self.parse_const(true, true) }
                    _ => { self.report_error("Expected fn, type, or const after pub"); Err(()) }
                }
            }
            Token::TypeKw => self.parse_typedef(false),
            Token::Const => self.parse_const(false, false),
            Token::Actor => self.parse_actor(),
            Token::Workflow => self.parse_workflow(),
            Token::Activity => self.parse_activity(),
            Token::Http => self.parse_http_route(),
            Token::AtTable => self.parse_table(false),
            Token::AtCollection => self.parse_collection(false),
            Token::AtIndex => self.parse_index(),
            Token::AtVectorIndex => self.parse_vector_index(),
            Token::AtSearchIndex => self.parse_search_index(),
            Token::Trait => self.parse_trait(),
            Token::Impl => self.parse_impl(),
            Token::Agent => self.parse_agent_decl(),
            Token::Message => self.parse_message_decl(),
            Token::Ident(ref name) if name == "routes" => self.parse_routes(),
            Token::Ident(ref name) if name == "style" => {
                self.report_error("A `style:` block must immediately follow the `@component` it styles, without any other declarations in between.");
                Err(())
            },
            _ => {
                let found_str = self.peek().to_string();
                const TOP_LEVEL: &[&str] = &["fn", "async", "import", "@py.import", "type", "const", "pub", "actor", "agent", "message", "workflow", "activity", "@component", "@test", "@server", "@mcp.tool", "@query", "@mutation", "@action", "@skill", "@agent_def", "@scheduled", "@table", "@index", "@vector_index", "@search_index", "trait", "impl", "routes"];
                let _lang_hint = match found_str.trim_matches('@') {
                    "function" | "def" | "func" | "fun" => Some("did you mean 'fn'?".to_string()),
                    "class" | "struct" => Some("did you mean 'type'? (Vox uses 'type' for ADTs)".to_string()),
                    "return" => Some("did you mean 'ret'? (Vox uses 'ret' for return)".to_string()),
                    "var" | "mut" => Some("did you mean 'let mut'?".to_string()),
                    "val" => Some("did you mean 'let'?".to_string()),
                    "interface" => Some("did you mean 'trait'?".to_string()),
                    "extends" | "implements" => Some("did you mean 'impl'?".to_string()),
                    "object" | "record" => Some("did you mean 'type'? (Vox uses 'type' for record types)".to_string()),
                    _ => did_you_mean(&found_str, TOP_LEVEL),
                };
                self.report_error(&format!("Unexpected token at top level: {found_str}"));
                Err(())
            }
        }
    }

    pub(crate) fn parse_import(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let mut paths = Vec::new();
        loop {
            paths.push(self.parse_import_path()?);
            if !self.eat(&Token::Comma) { break; }
        }
        Ok(Decl::Import(ImportDecl { paths, span: start.merge(self.span()) }))
    }

    /// Parse `@py.import <module> [as <alias>]`
    ///
    /// Examples:
    ///   @py.import torch
    ///   @py.import torch as tc
    ///   @py.import torch.nn as nn
    pub(crate) fn parse_py_import(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @py.import
        self.skip_newlines();

        // Parse the dotted module path: torch, torch.nn, etc.
        // We do NOT use peek_ahead; instead we check the token at pos+1 directly.
        let mut module_parts = Vec::new();
        module_parts.push(self.parse_ident_name()?);
        while self.peek() == &Token::Dot {
            // Only consume the dot if the next token is an identifier.
            let next_is_ident = self.tokens.get(self.pos + 1)
                .map(|s| matches!(s.token, Token::Ident(_)))
                .unwrap_or(false);
            if next_is_ident {
                self.advance(); // eat dot
                module_parts.push(self.parse_ident_name()?);
            } else {
                break;
            }
        }
        let module = module_parts.join(".");

        // Optional alias: `as tc`
        let alias = if self.eat(&Token::As) {
            self.parse_ident_name()?
        } else {
            // default alias = last segment
            module_parts.last().cloned().unwrap_or_else(|| module.clone())
        };

        Ok(Decl::PyImport(PyImportDecl { module, alias, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_import_path(&mut self) -> Result<ImportPath, ()> {
        let start = self.span();
        let mut segments = Vec::new();
        loop {
            segments.push(self.parse_ident_name()?);
            if !self.eat(&Token::Dot) { break; }
        }
        Ok(ImportPath { segments, span: start.merge(self.span()) })
    }

    pub(crate) fn parse_component(&mut self) -> Result<Decl, ()> {
        self.advance();
        let f = self.parse_fn_decl(false)?;
        let styles = self.parse_style_blocks();
        Ok(Decl::Component(ComponentDecl { func: f, styles }))
    }

    pub(crate) fn parse_page(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @page
        self.expect(&Token::LParen)?;
        let path = if let Token::StringLit(s) = self.peek().clone() {
            self.advance();
            s
        } else {
            self.report_error("Expected string literal for page path");
            return Err(());
        };
        self.expect(&Token::RParen)?;
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Page(PageDecl { path, func: f, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_mcp_tool(&mut self) -> Result<Decl, ()> {
        self.advance();
        let mut description = String::new();
        if self.eat(&Token::LParen) {
            if let Token::StringLit(s) = self.peek().clone() {
                description = s;
                self.advance();
            }
            if self.eat(&Token::Comma) {
                if let Token::StringLit(s) = self.peek().clone() {
                    description = s;
                    self.advance();
                }
            }
            self.expect(&Token::RParen)?;
        } else if let Token::StringLit(s) = self.peek().clone() {
            // bare string: @mcp.tool "description" fn ...
            description = s;
            self.advance();
        }
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::McpTool(McpToolDecl { description, func: f }))
    }

    pub(crate) fn parse_mcp_resource(&mut self) -> Result<Decl, ()> {
        self.advance();
        let mut uri = String::new();
        let mut description = String::new();
        if self.eat(&Token::LParen) {
            if let Token::StringLit(s) = self.peek().clone() {
                uri = s;
                self.advance();
            }
            if self.eat(&Token::Comma) {
                if let Token::StringLit(s) = self.peek().clone() {
                    description = s;
                    self.advance();
                }
            }
            self.expect(&Token::RParen)?;
        } else if let Token::StringLit(s) = self.peek().clone() {
            // bare string: @mcp.resource "uri" fn ...
            uri = s;
            self.advance();
            if let Token::StringLit(s) = self.peek().clone() {
                description = s;
                self.advance();
            }
        }
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::McpResource(McpResourceDecl { uri, description, func: f }))
    }


    pub(crate) fn parse_test(&mut self) -> Result<Decl, ()> {
        self.advance();
        let mut _name = "test".to_string();
        if self.eat(&Token::LParen) {
            if let Token::StringLit(s) = self.peek().clone() {
                _name = s;
                self.advance();
            }
            self.expect(&Token::RParen)?;
        } else if let Token::StringLit(s) = self.peek().clone() {
            _name = s;
            self.advance();
        }
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Test(TestDecl { func: f }))
    }

    pub(crate) fn parse_fixture(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Fixture(FixtureDecl { func: f, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_server_fn(&mut self) -> Result<Decl, ()> {
        self.advance();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::ServerFn(ServerFnDecl { func: f }))
    }

    pub(crate) fn parse_query_decl(&mut self) -> Result<Decl, ()> {
        self.advance();
        let is_pub = self.eat(&Token::Pub);
        let f = self.parse_fn_decl(is_pub)?;
        Ok(Decl::Query(QueryDecl { func: f }))
    }

    pub(crate) fn parse_mutation_decl(&mut self) -> Result<Decl, ()> {
        self.advance();
        let is_pub = self.eat(&Token::Pub);
        let f = self.parse_fn_decl(is_pub)?;
        Ok(Decl::Mutation(MutationDecl { func: f }))
    }

    pub(crate) fn parse_action_decl(&mut self) -> Result<Decl, ()> {
        self.advance();
        let is_pub = self.eat(&Token::Pub);
        let f = self.parse_fn_decl(is_pub)?;
        Ok(Decl::Action(ActionDecl { func: f }))
    }

    pub(crate) fn parse_fn_decl(&mut self, is_pub: bool) -> Result<FnDecl, ()> {
        let start = self.span();
        let is_async = self.eat(&Token::Async);
        self.expect(&Token::Fn)?;
        let name = self.parse_ident_name()?;

        let mut generics = Vec::new();
        if self.eat(&Token::LBracket) {
            loop {
                generics.push(self.parse_ident_name()?);
                if !self.eat(&Token::Comma) { break; }
            }
            self.expect(&Token::RBracket)?;
        } else if self.eat(&Token::Lt) {
            loop {
                generics.push(self.parse_ident_name()?);
                if !self.eat(&Token::Comma) { break; }
            }
            self.expect(&Token::Gt)?;
        }

        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
        self.expect(&Token::Colon)?;
        let body = self.parse_block()?;
        Ok(FnDecl {
            name,
            generics,
            params,
            return_type,
            body,
            is_async,
            is_pub,
            is_pure: false,
            is_deprecated: false,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_metric: false,
            metric_name: None,
            is_health: false,
            preconditions: vec![],
            auth_provider: None,
            roles: vec![],
            cors: None,
            span: start.merge(self.span()),
        })
    }

    pub(crate) fn parse_ident_name(&mut self) -> Result<String, ()> {
        match self.peek().clone() {
            Token::Ident(name) | Token::TypeIdent(name) => { self.advance(); Ok(name) }
            // Allow certain keywords to be used as field/param names
            Token::Message => { self.advance(); Ok("message".to_string()) }
            Token::Version => { self.advance(); Ok("version".to_string()) }
            Token::Migrate => { self.advance(); Ok("migrate".to_string()) }
            Token::On => { self.advance(); Ok("on".to_string()) }
            Token::FromKw => { self.advance(); Ok("from".to_string()) }
            Token::As => { self.advance(); Ok("as".to_string()) }
            Token::Get => { self.advance(); Ok("get".to_string()) }
            Token::Post => { self.advance(); Ok("post".to_string()) }
            Token::Put => { self.advance(); Ok("put".to_string()) }
            Token::Delete => { self.advance(); Ok("delete".to_string()) }
            _ => { self.report_error("Expected identifier"); Err(()) }
        }
    }

    pub(crate) fn parse_params(&mut self) -> Result<Vec<Param>, ()> {
        let mut params = Vec::new();
        while !matches!(self.peek(), Token::RParen | Token::Eof) {
            let start = self.span();
            let name = self.parse_ident_name()?;
            let type_ann = if self.eat(&Token::Colon) { Some(self.parse_type_expr()?) } else { None };
            let default_value = if self.eat(&Token::Eq) { Some(self.parse_expr()?) } else { None };
            params.push(Param { name, type_ann, default: default_value, span: start.merge(self.span()) });
            if !self.eat(&Token::Comma) { break; }
        }
        Ok(params)
    }

    pub(crate) fn parse_typedef(&mut self, is_pub: bool) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat 'type'
        let name = self.parse_ident_name()?;

        let generics = if self.eat(&Token::Lt) || self.eat(&Token::LBracket) {
            let mut params = Vec::new();
            loop {
                params.push(self.parse_ident_name()?);
                if !self.eat(&Token::Comma) { break; }
            }
            if self.peek() == &Token::Gt { self.expect(&Token::Gt)?; }
            else { self.expect(&Token::RBracket)?; }
            params
        } else { vec![] };

        if self.eat(&Token::Eq) {
            self.skip_newlines();
            let had_indent = self.eat(&Token::Indent);
            if self.peek() == &Token::PipeOp || self.peek() == &Token::Bar {
                let mut variants = Vec::new();
                while self.eat(&Token::PipeOp) || self.eat(&Token::Bar) {
                    let v_start = self.span();
                    let v_name = self.parse_ident_name()?;
                    let fields = if self.eat(&Token::LParen) {
                        let mut f = Vec::new();
                        while !matches!(self.peek(), Token::RParen | Token::Eof) {
                            let f_start = self.span();
                            let f_name = self.parse_ident_name()?;
                            self.expect(&Token::Colon)?;
                            f.push(VariantField { name: f_name, type_ann: self.parse_type_expr()?, span: f_start.merge(self.span()) });
                            if !self.eat(&Token::Comma) { break; }
                        }
                        self.expect(&Token::RParen)?;
                        f
                    } else if self.eat(&Token::LBrace) {
                        let mut f = Vec::new();
                        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                            let f_start = self.span();
                            let f_name = self.parse_ident_name()?;
                            self.expect(&Token::Colon)?;
                            f.push(VariantField { name: f_name, type_ann: self.parse_type_expr()?, span: f_start.merge(self.span()) });
                            if !self.eat(&Token::Comma) { break; }
                        }
                        self.expect(&Token::RBrace)?;
                        f
                    } else { vec![] };
                    variants.push(Variant { name: v_name, fields, literal_value: None, span: v_start.merge(self.span()) });
                    self.skip_newlines();
                }
                if had_indent { self.eat(&Token::Dedent); }
                return Ok(Decl::TypeDef(TypeDefDecl { name, generics, variants, fields: vec![], type_alias: None, json_layout: None, is_pub, is_deprecated: false, span: start.merge(self.span()) }));
            } else {
                let target = self.parse_type_expr()?;
                if had_indent { self.eat(&Token::Dedent); }
                return Ok(Decl::TypeDef(TypeDefDecl { name, generics, variants: vec![], fields: vec![], type_alias: Some(target), json_layout: None, is_pub, is_deprecated: false, span: start.merge(self.span()) }));
            }
        }

        let mut seen_fields = std::collections::HashSet::new();
        self.expect(&Token::Colon)?;
        let mut fields = Vec::new();
        self.skip_newlines();
        let had_indent = self.eat(&Token::Indent);
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            let f_start = self.span();
            let f_name = self.parse_ident_name()?;
            if !seen_fields.insert(f_name.clone()) {
                self.report_error(&format!("Duplicate field '{}' in struct '{}'", f_name, name));
                return Err(());
            }
            self.expect(&Token::Colon)?;
            fields.push(VariantField { name: f_name, type_ann: self.parse_type_expr()?, span: f_start.merge(self.span()) });
            if !self.eat(&Token::Comma) {
                self.skip_newlines();
                if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            }
        }
        if had_indent { self.eat(&Token::Dedent); }
        Ok(Decl::TypeDef(TypeDefDecl { name: name.clone(), generics, variants: vec![], fields, type_alias: None, json_layout: None, is_pub, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_const(&mut self, is_pub: bool, is_build_const: bool) -> Result<Decl, ()> {
        let start = self.span();
        if !is_build_const { self.expect(&Token::Const)?; }
        let name = self.parse_ident_name()?;
        let type_ann = if self.eat(&Token::Colon) { Some(self.parse_type_expr()?) } else { None };
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        Ok(Decl::Const(ConstDecl { name, type_ann, value, is_pub, is_deprecated: false, is_build_const, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_config(&mut self) -> Result<Decl, ()> {
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
            let f_start = self.span();
            let f_name = self.parse_ident_name()?;
            self.expect(&Token::Colon)?;
            fields.push(TableField {
                name: f_name,
                type_ann: self.parse_type_expr()?,
                description: None,
                span: f_start.merge(self.span())
            });
            if !self.eat(&Token::Comma) {
                self.skip_newlines();
                if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            }
        }
        self.eat(&Token::Dedent);
        Ok(Decl::Config(ConfigDecl { name, fields, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_http_route(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let method = match self.peek().clone() {
            Token::Get => { self.advance(); "GET".to_string() }
            Token::Post => { self.advance(); "POST".to_string() }
            Token::Put => { self.advance(); "PUT".to_string() }
            Token::Delete => { self.advance(); "DELETE".to_string() }
            Token::Ident(m) => { self.advance(); m.to_uppercase() }
            _ => "GET".to_string(),
        };
        let path = if let Token::StringLit(p) = self.peek().clone() { self.advance(); p } else { "/".to_string() };
        self.expect(&Token::To)?;
        let return_type = self.parse_type_expr()?;
        self.expect(&Token::Colon)?;
        let method_enum = match method.as_str() {
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            _ => HttpMethod::Get,
        };
        Ok(Decl::HttpRoute(HttpRouteDecl {
            method: method_enum,
            path,
            params: vec![],
            return_type: Some(return_type),
            body: self.parse_block()?,
            auth_provider: None,
            roles: vec![],
            cors: None,
            is_traced: false,
            is_deprecated: false,
            span: start.merge(self.span()),
        }))
    }

    pub(crate) fn parse_trait(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        self.eat(&Token::Indent);
        let mut methods = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            if self.eat(&Token::Fn) {
                let mstart = self.span();
                let mname = self.parse_ident_name()?;
                self.expect(&Token::LParen)?;
                let params = self.parse_params()?;
                self.expect(&Token::RParen)?;
                let return_type = if self.eat(&Token::To) { Some(self.parse_type_expr()?) } else { None };
                methods.push(TraitMethod { name: mname, params, return_type, is_deprecated: false, span: mstart.merge(self.span()) });
            } else { break; }
        }
        self.eat(&Token::Dedent);
        Ok(Decl::Trait(TraitDecl { name, methods, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_impl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        let trait_name = self.parse_ident_name()?;
        self.expect(&Token::For)?;
        let target_type = self.parse_type_expr()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        let had_indent = self.eat(&Token::Indent);
        let mut methods = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            if matches!(self.peek(), Token::Fn) {
                methods.push(self.parse_fn_decl(false)?);
            } else { break; }
        }
        if had_indent { self.eat(&Token::Dedent); }
        Ok(Decl::Impl(ImplDecl { trait_name, target_type, methods, is_deprecated: false, span: start.merge(self.span()) }))
    }

    pub(crate) fn parse_routes(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance();
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        let had_indent = self.eat(&Token::Indent);
        let mut entries = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Token::StringLit(path) => {
                    let entry_start = self.span();
                    self.advance();
                    self.expect(&Token::To)?;
                    let is_wildcard = path == "*";
                    let (component_name, redirect) = if self.peek() == &Token::Ident("redirect".into()) {
                        self.advance();
                        if let Token::StringLit(target) = self.peek().clone() { self.advance(); (String::new(), Some(target)) }
                        else { (self.parse_ident_name()?, None) }
                    } else { (self.parse_ident_name()?, None) };
                    entries.push(RouteEntry { path, component_name, children: vec![], redirect, is_wildcard, span: entry_start.merge(self.span()) });
                }
                _ => break,
            }
        }
        if had_indent { self.eat(&Token::Dedent); }
        Ok(Decl::Routes(RoutesDecl { entries, span: start.merge(self.span()) }))
    }

    /// Parse `@environment <name>:` blocks.
    ///
    /// ```text
    /// @environment production:
    ///     base: "node:20-alpine"
    ///     packages: ["curl", "git"]
    ///     env:
    ///         PORT: "3000"
    ///     expose: [3000]
    ///     volumes: ["/data"]
    ///     workdir: "/app"
    ///     cmd: ["node", "dist/index.js"]
    /// ```
    pub(crate) fn parse_environment_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @environment
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();
        self.eat(&Token::Indent);

        let mut base_image: Option<String> = None;
        let mut packages: Vec<String> = Vec::new();
        let mut env_vars: Vec<(String, String)> = Vec::new();
        let mut exposed_ports: Vec<u16> = Vec::new();
        let mut volumes: Vec<String> = Vec::new();
        let mut workdir: Option<String> = None;
        let mut cmd: Vec<String> = Vec::new();
        let mut copy_instructions: Vec<(String, String)> = Vec::new();
        let mut run_commands: Vec<String> = Vec::new();

        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            let field_name = match self.parse_ident_name() {
                Ok(n) => n,
                Err(_) => break,
            };
            self.expect(&Token::Colon)?;
            match field_name.as_str() {
                "base" => {
                    if let Token::StringLit(s) = self.peek().clone() {
                        base_image = Some(s);
                        self.advance();
                    }
                }
                "workdir" => {
                    if let Token::StringLit(s) = self.peek().clone() {
                        workdir = Some(s);
                        self.advance();
                    }
                }
                "packages" => { packages = self.parse_string_list()?; }
                "volumes"  => { volumes = self.parse_string_list()?; }
                "cmd"      => { cmd = self.parse_string_list()?; }
                "run"      => { run_commands = self.parse_string_list()?; }
                "copy" => {
                    self.skip_newlines();
                    let had_inner = self.eat(&Token::Indent);
                    loop {
                        self.skip_newlines();
                        if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
                        if let Token::StringLit(src) = self.peek().clone() {
                            self.advance();
                            self.expect(&Token::To)?;
                            if let Token::StringLit(dest) = self.peek().clone() {
                                self.advance();
                                copy_instructions.push((src, dest));
                            } else { break; }
                        } else { break; }
                    }
                    if had_inner { self.eat(&Token::Dedent); }
                }
                "expose" | "ports" => {
                    self.expect(&Token::LBracket)?;
                    loop {
                        self.skip_newlines();
                        if matches!(self.peek(), Token::RBracket | Token::Eof) { break; }
                        if let Token::IntLit(n) = self.peek().clone() {
                            if let Ok(port) = u16::try_from(n) { exposed_ports.push(port); }
                            self.advance();
                        } else { break; }
                        if !self.eat(&Token::Comma) { break; }
                    }
                    self.expect(&Token::RBracket)?;
                }
                "env" => {
                    self.skip_newlines();
                    let had_inner = self.eat(&Token::Indent);
                    loop {
                        self.skip_newlines();
                        if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
                        let key = match self.parse_ident_name() { Ok(k) => k, Err(_) => break };
                        self.expect(&Token::Colon)?;
                        if let Token::StringLit(val) = self.peek().clone() {
                            self.advance();
                            env_vars.push((key, val));
                        } else { break; }
                    }
                    if had_inner { self.eat(&Token::Dedent); }
                }
                _ => { self.parse_expr().ok(); }
            }
        }
        self.eat(&Token::Dedent);

        Ok(Decl::Environment(EnvironmentDecl {
            name,
            base_image,
            packages,
            env_vars,
            exposed_ports,
            volumes,
            workdir,
            cmd,
            copy_instructions,
            run_commands,
            is_deprecated: false,
            span: start.merge(self.span()),
        }))
    }

    fn parse_string_list(&mut self) -> Result<Vec<String>, ()> {
        self.expect(&Token::LBracket)?;
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBracket | Token::Eof) { break; }
            if let Token::StringLit(s) = self.peek().clone() {
                items.push(s);
                self.advance();
            } else { break; }
            if !self.eat(&Token::Comma) { break; }
        }
        self.expect(&Token::RBracket)?;
        Ok(items)
    }
}
