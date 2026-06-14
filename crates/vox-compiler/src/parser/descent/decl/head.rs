// Top-level and declaration parsing.

use super::super::Parser;
use super::head_types::{
    AstColorToken, AstFontToken, AstScalarToken, BackButtonDecl, Decl, DeepLinkDecl, EndpointDecl,
    EndpointKind, ExampleDecl, FnDecl, ForallDecl, LoadingDecl, McpResourceDecl, McpToolDecl,
    PushDecl, ScheduledDecl, TestDecl, TokensDecl,
};
use crate::lexer::token::Token;
use crate::parser::error::{ParseError, ParseErrorClass};

impl Parser {
    /// `@loading fn Name() to Element { ... }` — TanStack Router `pendingComponent` / suspense UI.
    pub(crate) fn parse_loading(&mut self) -> Result<Decl, ()> {
        self.advance(); // @loading
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Loading(LoadingDecl { func: f }))
    }

    /// One v0 component prop line: `name`, optional `?`, `:`, type (OP-0006).
    pub(crate) fn parse_v0_prop_line(&mut self) -> Result<crate::ast::decl::V0Prop, ()> {
        let pname = self.parse_ident_name()?;
        if std::env::var_os("VOX_PARSER_DEBUG").is_some() {
            eprintln!(
                "[vox-compiler:v0.prop] name={pname:?} next={:?}",
                self.peek()
            );
        }
        let is_optional = self.eat(&Token::Question);
        self.expect(&Token::Colon)?;
        let ty = self.parse_type_expr()?;
        Ok(crate::ast::decl::V0Prop {
            name: pname,
            ty,
            is_optional,
        })
    }

    pub(crate) fn parse_mcp_tool(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @mcp.tool
        let desc = if let Token::StringLit(s) = self.peek().clone() {
            self.advance();
            s
        } else {
            String::new()
        };
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::McpTool(McpToolDecl {
            description: desc,
            func: f,
        }))
    }

    /// `@mcp.resource ("uri", "desc") fn ...` or `@mcp.resource "uri" "desc" fn ...`.
    pub(crate) fn parse_mcp_resource(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @mcp.resource
        let (uri, description) = match self.peek().clone() {
            Token::LParen => {
                self.advance();
                let u = match self.peek().clone() {
                    Token::StringLit(s) => {
                        self.advance();
                        s
                    }
                    _ => {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected string literal for resource URI",
                            vec!["\"...\"".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                };
                self.expect(&Token::Comma)?;
                let d = match self.peek().clone() {
                    Token::StringLit(s) => {
                        self.advance();
                        s
                    }
                    _ => {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected string literal for resource description",
                            vec!["\"...\"".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                };
                self.expect(&Token::RParen)?;
                (u, d)
            }
            Token::StringLit(_) => {
                let u = match self.peek().clone() {
                    Token::StringLit(s) => {
                        self.advance();
                        s
                    }
                    _ => unreachable!(),
                };
                let d = match self.peek().clone() {
                    Token::StringLit(s) => {
                        self.advance();
                        s
                    }
                    _ => {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected second string literal (description) after resource URI",
                            vec!["\"...\"".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                };
                (u, d)
            }
            _ => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Expected `(` or string literal after @mcp.resource",
                    vec!["(\"uri\", \"desc\")".into(), "\"uri\"".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
        };
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::McpResource(McpResourceDecl {
            uri,
            description,
            func: f,
        }))
    }

    pub(crate) fn parse_test(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @test
        let mut label = String::new();
        if self.eat(&Token::LParen) {
            if let Token::StringLit(s) = self.peek().clone() {
                self.advance();
                label = s;
            }
            let _ = self.eat(&Token::RParen);
        }
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Test(TestDecl { label, func: f }))
    }

    pub(crate) fn parse_example(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @example
        let mut label = String::new();
        if self.eat(&Token::LParen) {
            if let Token::StringLit(s) = self.peek().clone() {
                self.advance();
                label = s;
            }
            let _ = self.eat(&Token::RParen);
        }
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Example(ExampleDecl { label, func: f }))
    }

    pub(crate) fn parse_forall(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @forall
        let mut label = String::new();
        if self.eat(&Token::LParen) {
            match self.peek().clone() {
                Token::StringLit(s) => {
                    self.advance();
                    label = s;
                }
                _ => {
                    while !self.eat(&Token::RParen) && !matches!(self.peek(), Token::Eof) {
                        self.advance();
                    }
                }
            }
            let _ = self.eat(&Token::RParen);
        }
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Forall(ForallDecl {
            label,
            iterations: 1000,
            func: f,
        }))
    }

    /// `@scheduled("1h") fn name(...) { ... }` — interval string is retained on [`ScheduledDecl`].
    pub(crate) fn parse_scheduled(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @scheduled
        self.skip_newlines();
        let interval = if self.eat(&Token::LParen) {
            let s = match self.peek().clone() {
                Token::StringLit(s) => {
                    self.advance();
                    s
                }
                _ => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        "Expected string literal schedule in @scheduled(\"...\")",
                        vec!["@scheduled(\"1h\") fn tick() -> Unit { return Unit }".into()],
                        Some(self.peek().to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };
            self.expect(&Token::RParen)?;
            s
        } else {
            self.errors.push(ParseError::classified(
                self.span(),
                "Expected `(` after @scheduled",
                vec!["@scheduled(\"1h\") fn tick() -> Unit { return Unit }".into()],
                Some(self.peek().to_string()),
                ParseErrorClass::Declaration,
            ));
            return Err(());
        };
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Scheduled(ScheduledDecl { interval, func: f }))
    }

    /// Parse `@query fn ...` — first-class GET-style endpoint, no kind param.
    /// Equivalent to `@endpoint(kind: query) fn ...` but lower K-complexity
    /// (audit doc §11.2). Introduced 2026-05-23.
    pub(crate) fn parse_query(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @query
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Endpoint(EndpointDecl {
            kind: EndpointKind::Query,
            func: f,
        }))
    }

    /// Parse `@mutation fn ...` — first-class POST/PUT/DELETE-style endpoint.
    pub(crate) fn parse_mutation(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @mutation
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Endpoint(EndpointDecl {
            kind: EndpointKind::Mutation,
            func: f,
        }))
    }

    /// Parse `@server fn ...` — first-class server-only endpoint (no client emit).
    pub(crate) fn parse_server_endpoint(&mut self) -> Result<Decl, ()> {
        self.advance(); // eat @server
        self.skip_newlines();
        let f = self.parse_fn_decl(false)?;
        Ok(Decl::Endpoint(EndpointDecl {
            kind: EndpointKind::Server,
            func: f,
        }))
    }

    // `parse_endpoint` (the `@endpoint(kind: …)` decorator parser) was retired
    // in v0.6.0 per `vox-stdlib-gap-audit-2026-05-23.md §Phase H step 18`.
    // The canonical bare-form decorators `@query` / `@mutation` / `@server`
    // — parsed by `parse_query`, `parse_mutation`, `parse_server_endpoint`
    // above — produce the same `EndpointDecl` AST node.  Any remaining
    // `@endpoint` text in user source fails to lex; the
    // `retired/decorator-usage` lint surfaces a friendly migration
    // suggestion before that point.

    /// Parse `extern fn name(args) to T = "./module"` (TS-source FFI, plan 6).
    /// The body is empty; codegen-TS emits `import { name } from "./module"`.
    pub(crate) fn parse_extern_fn(&mut self) -> Result<crate::ast::decl::Decl, ()> {
        use crate::parser::error::{ParseError, ParseErrorClass};
        let start = self.span();
        self.advance(); // eat `extern`
        self.expect(&Token::Fn)?;
        let name = self.parse_ident_name()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = if self.eat_return_arrow() {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        self.expect(&Token::Eq)?;
        let module = match self.peek().clone() {
            Token::StringLit(s) | Token::SingleStringLit(s) => {
                self.advance();
                s
            }
            other => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Expected string literal module path after `=` in extern fn",
                    vec!["\"./module\"".into()],
                    Some(other.to_string()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
        };
        Ok(crate::ast::decl::Decl::Function(FnDecl {
            name,
            generics: vec![],
            params,
            return_type,
            body: vec![],
            is_async: false,
            is_deprecated: false,
            is_pure: false,
            is_reactive: false,
            is_versioned: false,
            is_remote: false,
            effects: vec![],
            is_traced: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output_type: None,
            ai_max_iterations: 3,
            ai_task_category: None,
            ai_strengths: vec![],
            ai_tier_max: None,
            ai_cost_ceiling_usd_per_call: None,
            prompt_stage: None,
            prompt_schema: None,
            prompt_redact: vec![],
            subagent_policy: None,
            subagent_max_depth: None,
            subagent_budget_usd: None,
            subagent_description: None,
            subagent_parallel: false,
            subagent_complexity: None,
            search_corpus: None,
            search_query: None,
            search_into: None,
            search_top_k: None,
            search_policy: None,
            hole_spec: None,
            hole_reviewer: None,
            hole_cache_key: None,
            hole_constraints: vec![],
            embed: None,
            is_pub: true,
            auth_provider: None,
            roles: vec![],
            cors: None,
            webhook: None,
            cors_spec: None,
            rate_limit: None,
            pii: None,
            layer: None,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            verify_mode: crate::ast::decl::fundecl::VerifyMode::Off,
            test_strategy: None,
            is_mobile_native: false,
            ts_extern_module: Some(module),
            inference_model: None,
            training_step: false,
            is_auth_exempt: false,
            is_offline_capable: false,
            is_collaborative: false,
            span: start.merge(self.span()),
        }))
    }

    pub(crate) fn parse_ident_name(&mut self) -> Result<String, ()> {
        match self.peek().clone() {
            Token::Ident(n) | Token::TypeIdent(n) => {
                self.advance();
                Ok(n)
            }
            Token::TypeKw => {
                self.advance();
                Ok("type".to_string())
            }
            Token::On => {
                self.advance();
                Ok("on".to_string())
            }
            Token::State => {
                self.advance();
                Ok("state".to_string())
            }
            Token::Derived => {
                self.advance();
                Ok("derived".to_string())
            }
            Token::Effect => {
                self.advance();
                Ok("effect".to_string())
            }
            Token::Mount => {
                self.advance();
                Ok("mount".to_string())
            }
            Token::Cleanup => {
                self.advance();
                Ok("cleanup".to_string())
            }
            Token::View => {
                self.advance();
                Ok("view".to_string())
            }
            Token::Component => {
                self.advance();
                Ok("component".to_string())
            }
            Token::Http => {
                self.advance();
                Ok("http".to_string())
            }
            Token::Env => {
                self.advance();
                Ok("env".to_string())
            }
            Token::To => {
                self.advance();
                Ok("to".to_string())
            }
            Token::In => {
                self.advance();
                Ok("in".to_string())
            }
            _ => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Expected identifier",
                    vec!["identifier".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                Err(())
            }
        }
    }

    /// Parse an optional `uses <effect-list>` clause after `)` in a function signature.
    ///
    /// Grammar: `uses (<effect-name> | mcp(<tool-name>)) (',' (<effect-name> | mcp(<tool-name>)))*`
    /// where `effect-name` ∈ {net, db, fs, env, clock, random, spawn, nothing}.
    ///
    /// Returns an empty vec when no `uses` keyword is present (unannotated = unconstrained).
    pub(crate) fn parse_uses_clause(&mut self) -> Vec<crate::ast::decl::EffectAnnotation> {
        // `uses` is a contextual keyword — check by value, not token type.
        let is_uses = matches!(self.peek(), Token::Ident(n) if n == "uses");
        if !is_uses {
            return Vec::new();
        }
        self.advance(); // eat `uses`

        let mut effects = Vec::new();
        loop {
            let eff = match self.peek().clone() {
                Token::Ident(ref name) => {
                    let name = name.clone();
                    if name == "mcp" {
                        self.advance(); // eat `mcp`
                        if self.eat(&Token::LParen) {
                            let tool = match self.peek().clone() {
                                Token::Ident(t) | Token::TypeIdent(t) => {
                                    self.advance();
                                    t
                                }
                                _ => {
                                    self.errors.push(ParseError::classified(
                                        self.span(),
                                        "Expected MCP tool name inside `mcp(...)`",
                                        vec!["tool_name".into()],
                                        Some(self.peek().to_string()),
                                        ParseErrorClass::Declaration,
                                    ));
                                    return effects;
                                }
                            };
                            let _ = self.expect(&Token::RParen);
                            crate::ast::decl::EffectAnnotation::Mcp(tool)
                        } else {
                            crate::ast::decl::EffectAnnotation::Mcp(String::new())
                        }
                    } else if let Some(eff) =
                        crate::ast::decl::EffectAnnotation::from_keyword(&name)
                    {
                        self.advance();
                        eff
                    } else {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            format!("Unknown effect `{name}`; expected one of: net, db, fs, env, clock, random, spawn, mcp(…), nothing"),
                            vec!["net".into(), "db".into(), "fs".into(), "env".into(), "clock".into(), "random".into(), "spawn".into(), "mcp(…)".into(), "nothing".into()],
                            Some(name),
                            ParseErrorClass::Declaration,
                        ));
                        return effects;
                    }
                }
                // `env` is a keyword token, allow it here.
                Token::Env => {
                    self.advance();
                    crate::ast::decl::EffectAnnotation::Env
                }
                _ => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        "Expected effect name after `uses`",
                        vec!["net".into(), "db".into(), "nothing".into()],
                        Some(self.peek().to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return effects;
                }
            };
            effects.push(eff);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        effects
    }

    // ── Mobile primitives (Tasks D2-D4) ───────────────────────────────────

    /// Parse `@back_button { on_press: handler [fallback: handler] }`.
    pub(crate) fn parse_back_button_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @back_button
        self.expect(&Token::LBrace)?;
        let mut on_press = String::new();
        let mut fallback: Option<String> = None;
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            let key = match self.peek().clone() {
                Token::Ident(k) => {
                    self.advance();
                    k
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Expected field name inside @back_button block, got `{other}`"),
                        vec!["on_press".into(), "fallback".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };
            self.expect(&Token::Colon)?;
            let val = self.parse_ident_name()?;
            match key.as_str() {
                "on_press" => on_press = val,
                "fallback" => fallback = Some(val),
                _ => {}
            }
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;
        Ok(Decl::BackButton(BackButtonDecl {
            on_press,
            fallback,
            span: start.merge(self.span()),
        }))
    }

    /// Parse `@deep_link { scheme: "…" on_link: handler [universal_link: "…"] }`.
    pub(crate) fn parse_deep_link_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @deep_link
        self.expect(&Token::LBrace)?;
        let mut scheme = String::new();
        let mut universal_link: Option<String> = None;
        let mut on_link = String::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            let key = match self.peek().clone() {
                Token::Ident(k) => {
                    self.advance();
                    k
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Expected field name inside @deep_link block, got `{other}`"),
                        vec!["scheme".into(), "on_link".into(), "universal_link".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };
            self.expect(&Token::Colon)?;
            // Values are either string literals or identifiers.
            let val = match self.peek().clone() {
                Token::StringLit(s) => {
                    self.advance();
                    s
                }
                Token::Ident(_) | Token::TypeIdent(_) => self.parse_ident_name()?,
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Expected string or identifier as value in @deep_link block, got `{other}`"),
                        vec!["\"…\"".into(), "identifier".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };
            match key.as_str() {
                "scheme" => scheme = val,
                "universal_link" => universal_link = Some(val),
                "on_link" => on_link = val,
                _ => {}
            }
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;
        Ok(Decl::DeepLink(DeepLinkDecl {
            scheme,
            universal_link,
            on_link,
            span: start.merge(self.span()),
        }))
    }

    /// Parse `@tokens { color <name> light: "<hex>" dark: "<hex>" ... }`.
    ///
    /// Grammar per CC-23 / GA-20:
    /// ```text
    /// @tokens {
    ///   color <name>   light: "<hex>" dark: "<hex>"
    ///   spacing <name>: "<css-value>"
    ///   radius  <name>: "<css-value>"
    ///   shadow  <name>: "<css-value>"
    ///   font    <name> family: "<stack>"
    /// }
    /// ```
    pub(crate) fn parse_tokens_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @tokens
        self.expect(&Token::LBrace)?;

        let mut colors: Vec<AstColorToken> = Vec::new();
        let mut spacing: Vec<AstScalarToken> = Vec::new();
        let mut radius: Vec<AstScalarToken> = Vec::new();
        let mut shadows: Vec<AstScalarToken> = Vec::new();
        let mut fonts: Vec<AstFontToken> = Vec::new();

        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            let kw = match self.peek().clone() {
                Token::Ident(k) => {
                    self.advance();
                    k
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Expected token category (color/spacing/radius/shadow/font) inside @tokens block, got `{other}`"),
                        vec!["color".into(), "spacing".into(), "radius".into(), "shadow".into(), "font".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };
            let entry_start = self.span();
            match kw.as_str() {
                "color" => {
                    let name = self.parse_ident_name()?;
                    // `light: "<hex>"`
                    let light_kw = self.parse_ident_name()?;
                    if light_kw != "light" {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected `light:` keyword in color token entry",
                            vec!["light".into()],
                            Some(light_kw),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                    self.expect(&Token::Colon)?;
                    let light = match self.peek().clone() {
                        Token::StringLit(s) | Token::SingleStringLit(s) => {
                            self.advance();
                            s
                        }
                        other => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected hex string after `light:`",
                                vec!["\"#RRGGBB\"".into()],
                                Some(other.to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    let dark_kw = self.parse_ident_name()?;
                    if dark_kw != "dark" {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected `dark:` keyword in color token entry",
                            vec!["dark".into()],
                            Some(dark_kw),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                    self.expect(&Token::Colon)?;
                    let dark = match self.peek().clone() {
                        Token::StringLit(s) | Token::SingleStringLit(s) => {
                            self.advance();
                            s
                        }
                        other => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected hex string after `dark:`",
                                vec!["\"#RRGGBB\"".into()],
                                Some(other.to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    let pair_bg = if matches!(self.peek(), Token::On) {
                        self.advance(); // eat `on`
                        self.expect(&Token::Colon)?;
                        Some(self.parse_ident_name()?)
                    } else {
                        None
                    };
                    colors.push(AstColorToken {
                        name,
                        light,
                        dark,
                        pair_bg,
                        span: entry_start.merge(self.span()),
                    });
                }
                "spacing" | "radius" | "shadow" => {
                    let name = self.parse_ident_name()?;
                    self.expect(&Token::Colon)?;
                    let value = match self.peek().clone() {
                        Token::StringLit(s) | Token::SingleStringLit(s) => {
                            self.advance();
                            s
                        }
                        other => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected CSS value string",
                                vec!["\"8px\"".into()],
                                Some(other.to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    let tok = AstScalarToken {
                        name,
                        value,
                        span: entry_start.merge(self.span()),
                    };
                    match kw.as_str() {
                        "spacing" => spacing.push(tok),
                        "radius" => radius.push(tok),
                        "shadow" => shadows.push(tok),
                        _ => unreachable!(),
                    }
                }
                "font" => {
                    let name = self.parse_ident_name()?;
                    let fam_kw = self.parse_ident_name()?;
                    if fam_kw != "family" {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected `family:` keyword in font token entry",
                            vec!["family".into()],
                            Some(fam_kw),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                    self.expect(&Token::Colon)?;
                    let family = match self.peek().clone() {
                        Token::StringLit(s) | Token::SingleStringLit(s) => {
                            self.advance();
                            s
                        }
                        other => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected font family string",
                                vec!["\"Inter, sans-serif\"".into()],
                                Some(other.to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    fonts.push(AstFontToken {
                        name,
                        family,
                        span: entry_start.merge(self.span()),
                    });
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Unknown token category `{other}`; expected color, spacing, radius, shadow, or font"),
                        vec!["color".into(), "spacing".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            }
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;
        Ok(Decl::Tokens(TokensDecl {
            span: start.merge(self.span()),
            colors,
            spacing,
            radius,
            shadows,
            fonts,
        }))
    }

    /// Parse `@push { [on_register: handler] [on_notification: handler] [on_action: handler] }`.
    pub(crate) fn parse_push_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @push
        self.expect(&Token::LBrace)?;
        let mut on_register: Option<String> = None;
        let mut on_notification: Option<String> = None;
        let mut on_action: Option<String> = None;
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            let key = match self.peek().clone() {
                Token::Ident(k) => {
                    self.advance();
                    k
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Expected field name inside @push block, got `{other}`"),
                        vec![
                            "on_register".into(),
                            "on_notification".into(),
                            "on_action".into(),
                        ],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };
            self.expect(&Token::Colon)?;
            let val = self.parse_ident_name()?;
            match key.as_str() {
                "on_register" => on_register = Some(val),
                "on_notification" => on_notification = Some(val),
                "on_action" => on_action = Some(val),
                _ => {}
            }
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;
        Ok(Decl::Push(PushDecl {
            on_register,
            on_notification,
            on_action,
            span: start.merge(self.span()),
        }))
    }
}
