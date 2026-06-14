// Function declaration parsing (parse_fn_decl).

use super::super::Parser;
use super::head_types::FnDecl;
use crate::ast::decl::PostCondition;
use crate::lexer::token::Token;

impl Parser {
    pub(crate) fn parse_fn_decl(&mut self, is_pub: bool) -> Result<FnDecl, ()> {
        let start = self.span();
        let mut is_pub = is_pub;
        let mut preconditions = Vec::new();
        let mut postconditions = Vec::new();
        let mut invariants = Vec::new();
        let mut is_mobile_native = false;
        let mut is_pure = false;
        let mut is_reactive = false;
        let mut is_versioned = false;
        let mut is_remote = false;
        let mut is_deprecated = false;
        let mut is_llm = false;
        let mut llm_model = None;
        let mut ai_structured_output_type: Option<String> = None;
        let mut ai_max_iterations: u32 = 3;
        let mut ai_task_category: Option<String> = None;
        let mut ai_strengths: Vec<String> = Vec::new();
        let mut ai_tier_max: Option<String> = None;
        let mut ai_cost_ceiling_usd_per_call: Option<f64> = None;
        let mut prompt_stage: Option<String> = None;
        let mut prompt_schema: Option<String> = None;
        let mut prompt_redact: Vec<String> = Vec::new();
        let mut subagent_policy: Option<String> = None;
        let mut subagent_max_depth: Option<u32> = None;
        let mut subagent_budget_usd: Option<f64> = None;
        let mut subagent_description: Option<String> = None;
        let mut subagent_parallel = false;
        let mut subagent_complexity: Option<u8> = None;
        let mut search_corpus: Option<String> = None;
        let mut search_query: Option<String> = None;
        let mut search_into: Option<String> = None;
        let mut search_top_k: Option<u32> = None;
        let mut search_policy: Option<String> = None;
        let mut hole_spec: Option<String> = None;
        let mut hole_reviewer: Option<String> = None;
        let mut hole_cache_key: Option<String> = None;
        let mut hole_constraints: Vec<String> = Vec::new();
        let mut embed_model: Option<String> = None;
        let mut embed_dimensions: usize = 0;
        let mut embed_source_field: Option<String> = None;
        let mut embed_span: Option<crate::ast::span::Span> = None;
        let mut inference_model: Option<String> = None;
        let mut training_step = false;
        let mut decorator_effects: Vec<crate::ast::decl::effect::EffectAnnotation> = Vec::new();
        let mut auth_provider: Option<String> = None;
        let mut webhook: Option<crate::ast::decl::webhook::AstWebhookSpec> = None;
        let mut cors_spec: Option<crate::ast::decl::http_decorators::AstCorsSpec> = None;
        let mut rate_limit: Option<crate::ast::decl::http_decorators::AstRateLimitSpec> = None;
        let mut pii: Option<crate::ast::decl::http_decorators::AstPiiSpec> = None;
        let mut layer: Option<crate::ast::decl::layer_decorator::AstLayerSpec> = None;

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Token::AtRequire => {
                    self.advance();
                    self.expect(&Token::LParen)?;
                    preconditions.push(self.parse_expr()?);
                    self.expect(&Token::RParen)?;
                }
                Token::AtEnsure => {
                    self.advance();
                    self.expect(&Token::LParen)?;
                    let condition = self.parse_expr()?;
                    let mut fallback = None;
                    if self.eat(&Token::Comma)
                        && let Token::Ident(k) = self.peek().clone()
                        && k == "fallback"
                    {
                        self.advance();
                        self.expect(&Token::Colon)?;
                        fallback = Some(self.parse_ident_name()?);
                    }
                    postconditions.push(PostCondition {
                        condition,
                        fallback,
                    });
                    self.expect(&Token::RParen)?;
                }
                Token::AtInvariant => {
                    self.advance();
                    self.expect(&Token::LParen)?;
                    invariants.push(self.parse_expr()?);
                    self.expect(&Token::RParen)?;
                }
                Token::AtPure => {
                    self.advance();
                    is_pure = true;
                }
                Token::AtReactive => {
                    self.advance();
                    is_reactive = true;
                }
                Token::AtVersioned | Token::AtTracked => {
                    self.advance();
                    is_versioned = true;
                }
                Token::AtRemote => {
                    self.advance();
                    is_remote = true;
                }
                Token::AtDeprecated => {
                    self.advance();
                    is_deprecated = true;
                }
                Token::AtFuzz | Token::AtNative => {
                    self.advance();
                    is_mobile_native = true;
                }
                Token::AtInference => {
                    self.advance();
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                let key = key.clone();
                                self.advance();
                                self.eat(&Token::Eq);
                                if key == "model"
                                    && let Token::StringLit(m) = self.peek().clone()
                                {
                                    self.advance();
                                    inference_model = Some(m);
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                }
                Token::AtTrainingStep => {
                    self.advance();
                    training_step = true;
                }
                Token::AtPrompt => {
                    self.advance();
                    is_llm = true;
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                self.eat(&Token::Eq);
                                match key.as_str() {
                                    "stage" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            prompt_stage = Some(v);
                                        }
                                    }
                                    "schema" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            prompt_schema = Some(v);
                                        }
                                    }
                                    "redact" => {
                                        if self.eat(&Token::LBracket) {
                                            let mut redact: Vec<String> = vec![];
                                            loop {
                                                self.skip_newlines();
                                                if matches!(
                                                    self.peek(),
                                                    Token::RBracket | Token::Eof
                                                ) {
                                                    break;
                                                }
                                                match self.peek().clone() {
                                                    Token::Ident(v)
                                                    | Token::TypeIdent(v)
                                                    | Token::StringLit(v) => {
                                                        self.advance();
                                                        redact.push(v);
                                                    }
                                                    _ => {
                                                        self.advance();
                                                    }
                                                }
                                                if !self.eat(&Token::Comma) {
                                                    break;
                                                }
                                            }
                                            let _ = self.expect(&Token::RBracket);
                                            prompt_redact = redact;
                                        }
                                    }
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect(&Token::RParen)?;
                    }
                }
                Token::AtSubagent => {
                    self.advance();
                    is_llm = true;
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                self.eat(&Token::Eq);
                                match key.as_str() {
                                    "policy" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            subagent_policy = Some(v);
                                        }
                                    }
                                    "max_depth" => {
                                        if let Token::IntLit(v) = self.peek().clone() {
                                            self.advance();
                                            if v >= 0 {
                                                subagent_max_depth = Some(v as u32);
                                            }
                                        }
                                    }
                                    "budget_usd" => match self.peek().clone() {
                                        Token::FloatLit(v) => {
                                            self.advance();
                                            subagent_budget_usd = Some(v);
                                        }
                                        Token::IntLit(v) => {
                                            self.advance();
                                            subagent_budget_usd = Some(v as f64);
                                        }
                                        _ => {}
                                    },
                                    "description" => {
                                        if let Token::StringLit(v) = self.peek().clone() {
                                            self.advance();
                                            subagent_description = Some(v);
                                        }
                                    }
                                    "parallel" => match self.peek().clone() {
                                        Token::True => {
                                            self.advance();
                                            subagent_parallel = true;
                                        }
                                        Token::False => {
                                            self.advance();
                                            subagent_parallel = false;
                                        }
                                        _ => {}
                                    },
                                    "complexity" => {
                                        if let Token::IntLit(v) = self.peek().clone() {
                                            self.advance();
                                            if (0..=10).contains(&v) {
                                                subagent_complexity = Some(v as u8);
                                            }
                                        }
                                    }
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect(&Token::RParen)?;
                    }
                }
                Token::AtSearch => {
                    self.advance();
                    is_llm = true;
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                self.eat(&Token::Eq);
                                match key.as_str() {
                                    "corpus" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            search_corpus = Some(v);
                                        }
                                    }
                                    "query" => match self.peek().clone() {
                                        Token::StringLit(v)
                                        | Token::Ident(v)
                                        | Token::TypeIdent(v) => {
                                            self.advance();
                                            search_query = Some(v);
                                        }
                                        _ => {}
                                    },
                                    "into" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            search_into = Some(v);
                                        }
                                    }
                                    "top_k" => {
                                        if let Token::IntLit(v) = self.peek().clone() {
                                            self.advance();
                                            if v >= 0 {
                                                search_top_k = Some(v as u32);
                                            }
                                        }
                                    }
                                    "policy" => match self.peek().clone() {
                                        Token::StringLit(v)
                                        | Token::Ident(v)
                                        | Token::TypeIdent(v) => {
                                            self.advance();
                                            search_policy = Some(v);
                                        }
                                        _ => {}
                                    },
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect(&Token::RParen)?;
                    }
                }
                Token::AtHole => {
                    self.advance();
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                self.eat(&Token::Eq);
                                match key.as_str() {
                                    "spec" => match self.peek().clone() {
                                        Token::StringLit(v)
                                        | Token::Ident(v)
                                        | Token::TypeIdent(v) => {
                                            self.advance();
                                            hole_spec = Some(v);
                                        }
                                        _ => {}
                                    },
                                    "reviewer" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            hole_reviewer = Some(v);
                                        }
                                    }
                                    "cache_key" => match self.peek().clone() {
                                        Token::StringLit(v)
                                        | Token::Ident(v)
                                        | Token::TypeIdent(v) => {
                                            self.advance();
                                            hole_cache_key = Some(v);
                                        }
                                        _ => {}
                                    },
                                    "constraints" => {
                                        if self.eat(&Token::LBracket) {
                                            let mut constraints: Vec<String> = vec![];
                                            loop {
                                                self.skip_newlines();
                                                if matches!(
                                                    self.peek(),
                                                    Token::RBracket | Token::Eof
                                                ) {
                                                    break;
                                                }
                                                match self.peek().clone() {
                                                    Token::Ident(v)
                                                    | Token::TypeIdent(v)
                                                    | Token::StringLit(v) => {
                                                        self.advance();
                                                        constraints.push(v);
                                                    }
                                                    _ => {
                                                        self.advance();
                                                    }
                                                }
                                                if !self.eat(&Token::Comma) {
                                                    break;
                                                }
                                            }
                                            let _ = self.expect(&Token::RBracket);
                                            hole_constraints = constraints;
                                        }
                                    }
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect(&Token::RParen)?;
                    }
                }
                Token::AtAi => {
                    self.advance();
                    is_llm = true;
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                let key = key.clone();
                                self.advance();
                                self.eat(&Token::Eq);
                                match key.as_str() {
                                    "model" => {
                                        if let Token::StringLit(m) = self.peek().clone() {
                                            self.advance();
                                            llm_model = Some(m);
                                        }
                                    }
                                    "structured_output" => {
                                        let ty_opt = match self.peek().clone() {
                                            Token::Ident(ty) | Token::TypeIdent(ty) => Some(ty),
                                            _ => None,
                                        };
                                        if let Some(ty) = ty_opt {
                                            self.advance();
                                            ai_structured_output_type = Some(ty);
                                        }
                                    }
                                    "max_iterations" => {
                                        if let Token::IntLit(n) = self.peek().clone() {
                                            self.advance();
                                            if n > 0 {
                                                ai_max_iterations = n as u32;
                                            }
                                        }
                                    }
                                    "task_category" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            ai_task_category = Some(v);
                                        }
                                    }
                                    "strengths" => {
                                        if self.eat(&Token::LBracket) {
                                            let mut strengths: Vec<String> = vec![];
                                            loop {
                                                self.skip_newlines();
                                                if matches!(
                                                    self.peek(),
                                                    Token::RBracket | Token::Eof
                                                ) {
                                                    break;
                                                }
                                                if let Token::Ident(v) | Token::TypeIdent(v) =
                                                    self.peek().clone()
                                                {
                                                    self.advance();
                                                    strengths.push(v);
                                                } else {
                                                    self.advance();
                                                }
                                                if !self.eat(&Token::Comma) {
                                                    break;
                                                }
                                            }
                                            let _ = self.expect(&Token::RBracket);
                                            ai_strengths = strengths;
                                        }
                                    }
                                    "tier_max" => {
                                        if let Token::Ident(v) | Token::TypeIdent(v) =
                                            self.peek().clone()
                                        {
                                            self.advance();
                                            match v.as_str() {
                                                "Local" | "Light" | "Pro" | "Elite" => {
                                                    ai_tier_max = Some(v);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    "cost_ceiling_usd_per_call" => match self.peek().clone() {
                                        Token::FloatLit(v) => {
                                            self.advance();
                                            ai_cost_ceiling_usd_per_call = Some(v);
                                        }
                                        Token::IntLit(v) => {
                                            self.advance();
                                            ai_cost_ceiling_usd_per_call = Some(v as f64);
                                        }
                                        _ => {}
                                    },
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect(&Token::RParen)?;
                    }
                }
                Token::AtUses => {
                    self.advance();
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            match self.peek().clone() {
                                Token::Ident(ref name) => {
                                    let name = name.clone();
                                    self.advance();
                                    if let Some(eff) =
                                        crate::ast::decl::effect::EffectAnnotation::from_keyword(
                                            &name,
                                        )
                                    {
                                        if name == "mcp" && self.eat(&Token::LParen) {
                                            if let Token::Ident(tool) = self.peek().clone() {
                                                self.advance();
                                                decorator_effects.push(
                                                    crate::ast::decl::effect::EffectAnnotation::Mcp(
                                                        tool,
                                                    ),
                                                );
                                            }
                                            let _ = self.expect(&Token::RParen);
                                        } else {
                                            decorator_effects.push(eff);
                                        }
                                    }
                                }
                                // `env` lexes as a keyword (`Token::Env`) — mirror `parse_uses_clause`.
                                Token::Env => {
                                    self.advance();
                                    decorator_effects
                                        .push(crate::ast::decl::effect::EffectAnnotation::Env);
                                }
                                _ => {
                                    self.advance();
                                }
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                }
                Token::AtWebhook => {
                    let wh_start = self.span();
                    self.advance();
                    let mut provider = crate::ast::decl::webhook::AstWebhookProvider::Custom {
                        secret_var: String::new(),
                    };
                    let mut replay_window_secs: u64 = 300;
                    let mut idempotent = true;
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                let _ = self.expect(&Token::Colon);
                                match key.as_str() {
                                    "provider" => {
                                        if let Token::Ident(v) = self.peek().clone() {
                                            self.advance();
                                            match v.as_str() {
                                                "stripe" => provider = crate::ast::decl::webhook::AstWebhookProvider::Stripe,
                                                "github" => provider = crate::ast::decl::webhook::AstWebhookProvider::Github,
                                                "slack" => provider = crate::ast::decl::webhook::AstWebhookProvider::Slack,
                                                "custom" => {} // keep current Custom (secret_var possibly empty)
                                                _ => {}
                                            }
                                        }
                                    }
                                    "secret" => {
                                        if let Token::StringLit(s) = self.peek().clone() {
                                            self.advance();
                                            provider = crate::ast::decl::webhook::AstWebhookProvider::Custom { secret_var: s };
                                        }
                                    }
                                    "replay_window_secs" => {
                                        if let Token::IntLit(n) = self.peek().clone() {
                                            self.advance();
                                            if n >= 0 {
                                                replay_window_secs = n as u64;
                                            }
                                        }
                                    }
                                    "idempotent" => match self.peek().clone() {
                                        Token::True => {
                                            self.advance();
                                            idempotent = true;
                                        }
                                        Token::False => {
                                            self.advance();
                                            idempotent = false;
                                        }
                                        Token::Ident(v) => {
                                            self.advance();
                                            idempotent = v == "true";
                                        }
                                        _ => {}
                                    },
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                    webhook = Some(crate::ast::decl::webhook::AstWebhookSpec {
                        provider,
                        replay_window_secs,
                        idempotent,
                        span: wh_start.merge(self.span()),
                    });
                }
                Token::AtCors => {
                    let cs_start = self.span();
                    self.advance();
                    let mut origins: Vec<String> = vec![];
                    let mut allow_credentials = false;
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                let _ = self.expect(&Token::Colon);
                                match key.as_str() {
                                    "origins" => {
                                        if self.eat(&Token::LBracket) {
                                            loop {
                                                self.skip_newlines();
                                                if matches!(
                                                    self.peek(),
                                                    Token::RBracket | Token::Eof
                                                ) {
                                                    break;
                                                }
                                                if let Token::StringLit(s) = self.peek().clone() {
                                                    self.advance();
                                                    origins.push(s);
                                                } else {
                                                    self.advance();
                                                }
                                                if !self.eat(&Token::Comma) {
                                                    break;
                                                }
                                            }
                                            let _ = self.expect(&Token::RBracket);
                                        } else if let Token::StringLit(s) = self.peek().clone() {
                                            self.advance();
                                            origins.push(s);
                                        }
                                    }
                                    "allow_credentials" => match self.peek().clone() {
                                        Token::True => {
                                            self.advance();
                                            allow_credentials = true;
                                        }
                                        Token::False => {
                                            self.advance();
                                            allow_credentials = false;
                                        }
                                        Token::Ident(v) => {
                                            self.advance();
                                            allow_credentials = v == "true";
                                        }
                                        _ => {}
                                    },
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                    cors_spec = Some(crate::ast::decl::http_decorators::AstCorsSpec {
                        origins,
                        allow_credentials,
                        span: cs_start.merge(self.span()),
                    });
                }
                Token::AtRateLimit => {
                    let rl_start = self.span();
                    self.advance();
                    let mut by = crate::ast::decl::http_decorators::AstRateLimitBy::Ip;
                    let mut window_secs: u64 = 60;
                    let mut max_requests: u64 = 100;
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                let _ = self.expect(&Token::Colon);
                                match key.as_str() {
                                    "by" => {
                                        if let Token::Ident(v) = self.peek().clone() {
                                            self.advance();
                                            by = match v.as_str() {
                                                "user_id" | "user" => crate::ast::decl::http_decorators::AstRateLimitBy::UserId,
                                                "api_key" => crate::ast::decl::http_decorators::AstRateLimitBy::ApiKey,
                                                _ => crate::ast::decl::http_decorators::AstRateLimitBy::Ip,
                                            };
                                        }
                                    }
                                    "window_secs" | "window" => {
                                        if let Token::IntLit(n) = self.peek().clone() {
                                            self.advance();
                                            if n > 0 {
                                                window_secs = n as u64;
                                            }
                                        }
                                    }
                                    "max" | "max_requests" => {
                                        if let Token::IntLit(n) = self.peek().clone() {
                                            self.advance();
                                            max_requests = n.max(0) as u64;
                                        }
                                    }
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                    rate_limit = Some(crate::ast::decl::http_decorators::AstRateLimitSpec {
                        by,
                        window_secs,
                        max_requests,
                        span: rl_start.merge(self.span()),
                    });
                }
                Token::AtPii => {
                    let pii_start = self.span();
                    self.advance();
                    let mut class =
                        crate::ast::decl::http_decorators::AstPiiClass::Other("unknown".into());
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                if key == "class" {
                                    let _ = self.expect(&Token::Colon);
                                    if let Token::Ident(v) = self.peek().clone() {
                                        self.advance();
                                        class = crate::ast::decl::http_decorators::AstPiiClass::from_str(&v);
                                    }
                                } else {
                                    self.advance();
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                    pii = Some(crate::ast::decl::http_decorators::AstPiiSpec {
                        class,
                        span: pii_start.merge(self.span()),
                    });
                }
                Token::AtLayer => {
                    let l_start = self.span();
                    self.advance();
                    let mut tier = String::from("content");
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                self.advance();
                                let _ = self.expect(&Token::Colon);
                                if key == "tier" {
                                    if let Token::Ident(v) = self.peek().clone() {
                                        self.advance();
                                        tier = v;
                                    }
                                } else {
                                    self.advance();
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                    }
                    layer = Some(crate::ast::decl::layer_decorator::AstLayerSpec {
                        tier,
                        span: l_start.merge(self.span()),
                    });
                }
                Token::AtEmbed => {
                    let e_start = self.span();
                    self.advance();
                    if self.eat(&Token::LParen) {
                        loop {
                            self.skip_newlines();
                            if matches!(self.peek(), Token::RParen | Token::Eof) {
                                break;
                            }
                            if let Token::Ident(key) = self.peek().clone() {
                                let key = key.clone();
                                self.advance();
                                self.eat(&Token::Colon);
                                match key.as_str() {
                                    "model" => {
                                        if let Token::StringLit(m) = self.peek().clone() {
                                            self.advance();
                                            embed_model = Some(m);
                                        }
                                    }
                                    "dimensions" => {
                                        if let Token::IntLit(n) = self.peek().clone() {
                                            self.advance();
                                            if n >= 0 {
                                                embed_dimensions = n as usize;
                                            }
                                        }
                                    }
                                    "source_field" => {
                                        if let Token::StringLit(f) = self.peek().clone() {
                                            self.advance();
                                            embed_source_field = Some(f);
                                        }
                                    }
                                    _ => {
                                        self.advance();
                                    }
                                }
                            } else {
                                self.advance();
                            }
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        let _ = self.expect(&Token::RParen);
                        embed_span = Some(e_start.merge(self.span()));
                    }
                }
                Token::AtPublic => {
                    self.advance();
                    is_pub = true;
                }
                Token::AtAuth => {
                    self.advance();
                    if self.eat(&Token::LParen) {
                        self.skip_paren_args_inner();
                    }
                    auth_provider = Some(String::new());
                }
                Token::AtOfflineCapable | Token::AtCollaborative => {
                    self.advance();
                    if self.eat(&Token::LParen) {
                        self.skip_paren_args_inner();
                    }
                }
                _ => break,
            }
        }

        self.expect(&Token::Fn)?;
        let name = self.parse_ident_name()?;

        let generics = if self.eat(&Token::Lt) {
            let mut gs = Vec::new();
            loop {
                gs.push(self.parse_ident_name()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::Gt)?;
            gs
        } else {
            Vec::new()
        };

        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let clause_effects = self.parse_uses_clause();
        let effects = if decorator_effects.is_empty() {
            clause_effects
        } else {
            let mut all = decorator_effects;
            all.extend(clause_effects);
            all
        };
        let return_type = if self.eat_return_arrow() {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        let body = if is_llm && !matches!(self.peek(), Token::LBrace) {
            vec![]
        } else {
            self.expect(&Token::LBrace)?;
            self.parse_block()?
        };
        Ok(FnDecl {
            name,
            generics,
            params,
            return_type,
            body,
            is_async: false,
            is_deprecated,
            is_pure,
            is_reactive,
            is_versioned,
            is_remote,
            is_llm,
            llm_model,
            ai_structured_output_type,
            ai_max_iterations,
            ai_task_category,
            ai_strengths,
            ai_tier_max,
            ai_cost_ceiling_usd_per_call,
            prompt_stage,
            prompt_schema,
            prompt_redact,
            subagent_policy,
            subagent_max_depth,
            subagent_budget_usd,
            subagent_description,
            subagent_parallel,
            subagent_complexity,
            search_corpus,
            search_query,
            search_into,
            search_top_k,
            search_policy,
            hole_spec,
            hole_reviewer,
            hole_cache_key,
            hole_constraints,
            embed: embed_span.map(|sp| crate::ast::decl::embed_decorator::AstEmbedSpec {
                model: embed_model.unwrap_or_default(),
                dimensions: embed_dimensions,
                source_field: embed_source_field.unwrap_or_default(),
                span: sp,
            }),
            is_traced: false,
            is_pub,
            auth_provider,
            roles: vec![],
            cors: None,
            webhook,
            cors_spec,
            rate_limit,
            pii,
            layer,
            preconditions,
            postconditions,
            invariants,
            verify_mode: crate::ast::decl::fundecl::VerifyMode::Off,
            test_strategy: None,
            is_mobile_native,
            ts_extern_module: None,
            effects,
            inference_model,
            training_step,
            span: start.merge(self.span()),
        })
    }
}
