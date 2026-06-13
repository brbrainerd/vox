// ADT typedefs and actor / workflow / HTTP declarations.

use super::super::Parser;
use crate::ast::decl::*;
use crate::lexer::token::Token;
use crate::parser::error::{ParseError, ParseErrorClass};

impl Parser {
    /// Parse `@json_as(TypeName, naming: "...", strict: true, ...)`
    /// followed by a `type` declaration. Attaches the parsed annotation to
    /// the produced TypeDefDecl. Per-field attributes (`@field_name`,
    /// `@default`, `@skip_if_none`) are parsed in `parse_typedef`'s field
    /// loops (Phase M Step 1 completion).
    ///
    /// RFC: docs/src/architecture/json-as-rfc-2026-05-24.md §4.
    pub(crate) fn parse_json_as(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @json_as
        self.expect(&Token::LParen)?;

        // First positional argument: type name.
        let type_name = self.parse_ident_name()?;

        // Optional named parameters: `naming: "..."`, `strict: true`,
        // `defaults: true`, `tag: "..."`.
        let mut naming = "snake_case".to_string();
        let mut strict = false;
        let mut defaults = false;
        let mut tag: Option<String> = None;

        while self.eat(&Token::Comma) {
            self.skip_newlines();
            if matches!(self.peek(), Token::RParen | Token::Eof) {
                break;
            }
            let key_name = match self.peek().clone() {
                Token::Ident(name) => {
                    self.advance();
                    name
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!(
                            "Expected `@json_as` parameter name (one of: naming, strict, defaults, tag); got `{other}`"
                        ),
                        vec!["naming".into(), "strict".into(), "defaults".into(), "tag".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };
            self.expect(&Token::Colon)?;
            match key_name.as_str() {
                "naming" => {
                    if let Token::StringLit(s) = self.peek().clone() {
                        self.advance();
                        naming = s;
                    } else {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "`naming` expects a string literal (\"snake_case\" / \"camelCase\" / \"kebab-case\" / \"PascalCase\")",
                            vec!["\"snake_case\"".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                }
                "strict" => {
                    strict = self.parse_bool_literal()?;
                }
                "defaults" => {
                    defaults = self.parse_bool_literal()?;
                }
                "tag" => {
                    if let Token::StringLit(s) = self.peek().clone() {
                        self.advance();
                        tag = Some(s);
                    } else {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "`tag` expects a string literal (e.g. \"kind\")",
                            vec!["\"kind\"".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!(
                            "Unknown `@json_as` parameter `{other}`. Allowed: naming, strict, defaults, tag."
                        ),
                        vec!["naming".into(), "strict".into(), "defaults".into(), "tag".into()],
                        Some(other.into()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            }
        }
        self.expect(&Token::RParen)?;
        let annotation_span = start.merge(self.span());

        // The decorator must immediately precede a type declaration.
        self.skip_newlines();
        let is_pub = self.eat(&Token::Pub);
        if !matches!(self.peek(), Token::TypeKw) {
            self.errors.push(ParseError::classified(
                self.span(),
                "`@json_as(...)` must precede a `type` declaration.",
                vec!["type".into()],
                Some(self.peek().to_string()),
                ParseErrorClass::Declaration,
            ));
            return Err(());
        }
        let mut decl = self.parse_typedef(is_pub)?;
        if let Decl::TypeDef(ref mut td) = decl {
            // Sanity check: the positional type name in @json_as should
            // match the actual `type Name` that follows. Mismatch = author
            // error; emit a parse error pointing at both.
            if td.name != type_name {
                self.errors.push(ParseError::classified(
                    annotation_span,
                    format!(
                        "`@json_as({type_name})` does not match the following type `{}`. Use `@json_as({})` or rename the type.",
                        td.name, td.name
                    ),
                    vec![td.name.clone()],
                    Some(type_name.clone()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
            td.json_as = Some(JsonAsAnnotation {
                type_name,
                naming,
                strict,
                defaults,
                tag,
                span: annotation_span,
            });
        }
        Ok(decl)
    }

    /// Consume a bare `true` / `false` token; error otherwise.
    fn parse_bool_literal(&mut self) -> Result<bool, ()> {
        match self.peek().clone() {
            Token::True => {
                self.advance();
                Ok(true)
            }
            Token::False => {
                self.advance();
                Ok(false)
            }
            other => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    format!("Expected `true` or `false`, got `{other}`"),
                    vec!["true".into(), "false".into()],
                    Some(other.to_string()),
                    ParseErrorClass::Declaration,
                ));
                Err(())
            }
        }
    }

    pub(crate) fn parse_typedef(&mut self, is_pub: bool) -> Result<Decl, ()> {
        // After the type name, peek to disambiguate:
        //   `type Foo { f: T, ... }` — struct (product type, brace body)
        //   `type Foo = | A | B(...)` — ADT / alias (existing path)
        let start = self.span();
        self.advance(); // eat 'type'
        let name = self.parse_ident_name()?;

        // Struct branch: `type Name { f: T, ... }`
        if matches!(self.peek(), Token::LBrace) {
            self.advance(); // eat `{`
            let mut struct_fields = Vec::new();
            loop {
                self.skip_newlines();
                if self.eat(&Token::RBrace) {
                    break;
                }
                if matches!(self.peek(), Token::Eof) {
                    break;
                }
                let fstart = self.span();
                // Phase M Step 1 (completed): per-field @json_as attributes.
                // RFC §4.3: @field_name("key"), @default(expr), @skip_if_none.
                let json_as_attr = self.parse_json_as_field_attrs()?;
                let fname = self.parse_ident_name()?;
                self.expect(&Token::Colon)?;
                let ftype = self.parse_type_expr()?;
                struct_fields.push(VariantField {
                    name: fname,
                    type_ann: ftype,
                    json_as_attr,
                    span: fstart.merge(self.span()),
                });
                self.eat(&Token::Comma);
            }
            return Ok(Decl::TypeDef(TypeDefDecl {
                name,
                generics: vec![],
                variants: vec![],
                fields: struct_fields,
                type_alias: None,
                json_layout: None,
                is_pub,
                is_deprecated: false,
                json_as: None,
                span: start.merge(self.span()),
            }));
        }

        self.expect(&Token::Eq)?;
        self.skip_newlines();
        // Variants may appear inline (| A | B) or on separate lines
        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if !self.eat(&Token::Bar) {
                break;
            }
            let vstart = self.span();
            let vname = self.parse_ident_name()?;
            let mut fields = Vec::new();
            if self.eat(&Token::LBrace) {
                // Struct-shaped variant: `| Search { query: str }` (RFC §4.4).
                loop {
                    self.skip_newlines();
                    if matches!(self.peek(), Token::RBrace | Token::Eof) {
                        break;
                    }
                    let json_as_attr = self.parse_json_as_field_attrs()?;
                    let fname = self.parse_ident_name()?;
                    self.expect(&Token::Colon)?;
                    let ftype = self.parse_type_expr()?;
                    fields.push(VariantField {
                        name: fname,
                        type_ann: ftype,
                        json_as_attr,
                        span: vstart.merge(self.span()),
                    });
                    self.eat(&Token::Comma);
                }
                self.expect(&Token::RBrace)?;
            } else if self.eat(&Token::LParen) {
                // Tuple-style variant: `| Compute(expr: str, precision: int)`.
                loop {
                    if matches!(self.peek(), Token::RParen) {
                        break;
                    }
                    let json_as_attr = self.parse_json_as_field_attrs()?;
                    let fname = self.parse_ident_name()?;
                    self.expect(&Token::Colon)?;
                    let ftype = self.parse_type_expr()?;
                    fields.push(VariantField {
                        name: fname,
                        type_ann: ftype,
                        json_as_attr,
                        span: vstart.merge(self.span()),
                    });
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RParen)?;
            }
            variants.push(Variant {
                name: vname,
                fields,
                literal_value: None,
                span: vstart.merge(self.span()),
            });
        }
        Ok(Decl::TypeDef(TypeDefDecl {
            name,
            generics: vec![],
            variants,
            fields: vec![],
            type_alias: None,
            json_layout: None,
            is_pub,
            is_deprecated: false,
            json_as: None,
            span: start.merge(self.span()),
        }))
    }
    /// Parse zero or more per-field `@json_as` attributes that may precede a
    /// field name inside an `@json_as`-annotated type definition (RFC §4.3).
    ///
    /// Supported attributes:
    /// - `@field_name("key")` — override the JSON key name for this field
    /// - `@default(expr)` — source-text default expression when key is absent
    /// - `@skip_if_none` — omit this field when serialising and the value is None
    ///
    /// Returns the accumulated [`JsonAsFieldAttr`]. All attrs default to their
    /// zero values (`None` / `false`) when none are present.
    fn parse_json_as_field_attrs(
        &mut self,
    ) -> Result<crate::ast::decl::typedef::JsonAsFieldAttr, ()> {
        use crate::ast::decl::typedef::JsonAsFieldAttr;

        let mut attr = JsonAsFieldAttr::default();

        loop {
            if self.eat(&Token::AtFieldName) {
                // @field_name("json_key")
                self.expect(&Token::LParen)?;
                let name_str = match self.peek().clone() {
                    Token::StringLit(s) => {
                        self.advance();
                        s
                    }
                    other => {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            format!("`@field_name` expects a string literal, got `{other}`"),
                            vec!["\"json_key\"".into()],
                            Some(other.to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                };
                self.expect(&Token::RParen)?;
                attr.field_name = Some(name_str);
            } else if self.eat(&Token::AtDefault) {
                // @default(expr) — capture the source text between the parens.
                // We count paren depth to handle nested calls like @default(foo(1)).
                self.expect(&Token::LParen)?;
                let mut depth: usize = 0;
                let mut text = String::new();
                loop {
                    match self.peek().clone() {
                        Token::Eof => break,
                        Token::RParen if depth == 0 => break,
                        Token::LParen => {
                            depth += 1;
                            text.push('(');
                            self.advance();
                        }
                        Token::RParen => {
                            depth -= 1;
                            text.push(')');
                            self.advance();
                        }
                        tok => {
                            text.push_str(&tok.to_string());
                            self.advance();
                        }
                    }
                }
                self.expect(&Token::RParen)?;
                attr.default_expr = Some(text.trim().to_string());
            } else if self.eat(&Token::AtSkipIfNone) {
                // @skip_if_none — no arguments
                attr.skip_if_none = true;
            } else {
                break;
            }
            // Allow optional newline between consecutive field attrs
            self.skip_newlines();
        }
        Ok(attr)
    }

    /// Push a parse error for a duplicated `@table(...)` parameter (e.g.
    /// `@table(pk: a, pk: b)`); duplicates must error rather than silently
    /// last-wins.
    fn push_duplicate_table_param_error(&mut self, param: &str) {
        use crate::parser::error::{ParseError, ParseErrorClass};
        self.errors.push(ParseError::classified(
            self.span(),
            format!("Duplicate `{param}` parameter in `@table(...)` — each parameter may appear at most once."),
            vec![format!("remove the extra `{param}`")],
            Some(self.peek().to_string()),
            ParseErrorClass::Declaration,
        ));
    }

    pub(crate) fn parse_table(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @table

        // Optional `(pk: <ident>)` argument — names the primary-key column
        // when it differs from the default `"id"`. The typeck enforces
        // that the named column actually exists on the table (E1041);
        // when the decorator is bare, the typeck enforces an `id` field
        // exists (E1042).
        let mut primary_key: Option<String> = None;
        let mut is_extern = false;
        let mut source: Option<String> = None;
        if matches!(self.peek(), Token::LParen) {
            self.advance(); // eat `(`
            loop {
                self.skip_newlines();
                if self.eat(&Token::RParen) {
                    break;
                }
                match self.peek().clone() {
                    Token::Extern => {
                        if is_extern {
                            self.push_duplicate_table_param_error("extern");
                            return Err(());
                        }
                        self.advance();
                        is_extern = true;
                    }
                    Token::Ident(k) if k == "extern" => {
                        if is_extern {
                            self.push_duplicate_table_param_error("extern");
                            return Err(());
                        }
                        self.advance();
                        is_extern = true;
                    }
                    Token::Ident(k) if k == "pk" => {
                        if primary_key.is_some() {
                            self.push_duplicate_table_param_error("pk");
                            return Err(());
                        }
                        self.advance(); // eat `pk`
                        self.expect(&Token::Colon)?;
                        primary_key = Some(self.parse_ident_name()?);
                    }
                    Token::Ident(k) if k == "source" => {
                        if source.is_some() {
                            self.push_duplicate_table_param_error("source");
                            return Err(());
                        }
                        self.advance(); // eat `source`
                        self.expect(&Token::Colon)?;
                        source = match self.peek().clone() {
                            Token::StringLit(s) | Token::SingleStringLit(s) => {
                                self.advance();
                                Some(s)
                            }
                            Token::Ident(name) | Token::TypeIdent(name) => {
                                self.advance();
                                Some(name)
                            }
                            other => {
                                use crate::parser::error::{ParseError, ParseErrorClass};
                                self.errors.push(ParseError::classified(
                                    self.span(),
                                    "Expected `source: <table_name>` or `source: \"table_name\"` inside `@table(...)`.",
                                    vec!["source: users".into(), "source: \"users\"".into()],
                                    Some(other.to_string()),
                                    ParseErrorClass::Declaration,
                                ));
                                return Err(());
                            }
                        };
                    }
                    _ => {
                        use crate::parser::error::{ParseError, ParseErrorClass};
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected `extern`, `pk: <field_name>`, or `source: <name>` inside `@table(...)`.",
                            vec!["extern".into(), "pk: id".into(), "source: users".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                }
                if self.eat(&Token::Comma) {
                    continue;
                }
                self.skip_newlines();
                if self.eat(&Token::RParen) {
                    break;
                }
                use crate::parser::error::{ParseError, ParseErrorClass};
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Expected `,` or `)` in `@table(...)` argument list.",
                    vec![",".into(), ")".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
        }

        self.expect(&Token::TypeKw)?;
        let name = self.parse_ident_name()?;
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat(&Token::RBrace) {
                break;
            }
            if matches!(self.peek(), Token::Eof) {
                break;
            }
            let fstart = self.span();
            let fname = self.parse_ident_name()?;
            self.expect(&Token::Colon)?;
            let ftype = self.parse_type_expr()?;
            fields.push(crate::ast::decl::TableField {
                name: fname,
                type_ann: ftype,
                description: None,
                span: fstart.merge(self.span()),
            });
            self.eat(&Token::Comma);
        }
        Ok(Decl::Table(crate::ast::decl::TableDecl {
            name,
            fields,
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            is_pub: false,
            is_deprecated: false,
            primary_key,
            is_extern,
            source,
            span: start.merge(self.span()),
        }))
    }

    /// Parse `url Name { Variant; Variant(arg: Type); Variant(?opt: Type) }`.
    pub(crate) fn parse_url_decl(&mut self, is_pub: bool) -> Result<Decl, ()> {
        use crate::parser::error::{ParseError, ParseErrorClass};
        let start = self.span();
        self.advance(); // eat `url`
        let name = self.parse_ident_name()?;
        self.expect(&Token::LBrace)?;
        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat(&Token::RBrace) {
                break;
            }
            if matches!(self.peek(), Token::Eof) {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Unexpected EOF inside `url` block",
                    vec!["}".into()],
                    None,
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
            let vstart = self.span();
            let vname = self.parse_ident_name()?;
            let mut args = Vec::new();
            if self.eat(&Token::LParen) {
                loop {
                    self.skip_newlines();
                    if matches!(self.peek(), Token::RParen) {
                        break;
                    }
                    let astart = self.span();
                    let optional = self.eat(&Token::Question);
                    let aname = self.parse_ident_name()?;
                    self.expect(&Token::Colon)?;
                    let atype = self.parse_type_expr()?;
                    args.push(crate::ast::decl::UrlArg {
                        name: aname,
                        optional,
                        type_ann: atype,
                        span: astart.merge(self.span()),
                    });
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RParen)?;
            }
            variants.push(crate::ast::decl::UrlVariant {
                name: vname,
                args,
                span: vstart.merge(self.span()),
            });
            // Allow an optional comma between variants; newlines are skipped at loop top
            self.eat(&Token::Comma);
        }
        Ok(Decl::Url(crate::ast::decl::UrlDecl {
            name,
            variants,
            is_pub,
            span: start.merge(self.span()),
        }))
    }

    /// Parse `[partial] state_machine Name { state … on … }`.
    ///
    /// Called with the cursor on the `state_machine` ident token (or on `partial`
    /// if `is_partial` was set by the caller after consuming `partial`).
    pub(crate) fn parse_state_machine_decl(
        &mut self,
        is_pub: bool,
        is_partial: bool,
    ) -> Result<Decl, ()> {
        use crate::ast::decl::{SmState, SmTransition, StateMachineDecl};
        use crate::parser::error::{ParseError, ParseErrorClass};

        let start = self.span();
        self.advance(); // eat `state_machine` ident

        let name = self.parse_ident_name()?;
        self.expect(&Token::LBrace)?;

        let mut states: Vec<SmState> = Vec::new();
        let mut transitions: Vec<SmTransition> = Vec::new();

        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }

            match self.peek().clone() {
                // `state Name` or `state Name(field: Type, …)`
                Token::State => {
                    let ss = self.parse_sm_state(false)?;
                    states.push(ss);
                }
                // `terminal state Name(…)`
                Token::Ident(ref kw) if kw == "terminal" => {
                    self.advance(); // eat `terminal`
                    if !matches!(self.peek(), Token::State) {
                        self.errors.push(ParseError::classified(
                            self.span(),
                            "Expected `state` after `terminal`",
                            vec!["state".into()],
                            Some(self.peek().to_string()),
                            ParseErrorClass::Declaration,
                        ));
                        return Err(());
                    }
                    let ss = self.parse_sm_state(true)?;
                    states.push(ss);
                }
                // `on Event(params) from State -> Target`
                Token::On => {
                    let tr = self.parse_sm_transition()?;
                    transitions.push(tr);
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!(
                            "Expected `state`, `terminal state`, or `on` inside state_machine block; got {other}"
                        ),
                        vec!["state".into(), "terminal".into(), "on".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            }

            // Allow optional commas or newlines as separators.
            self.eat(&Token::Comma);
        }

        self.expect(&Token::RBrace)?;

        Ok(Decl::StateMachine(StateMachineDecl {
            name,
            states,
            transitions,
            is_partial,
            is_pub,
            span: start.merge(self.span()),
        }))
    }

    fn parse_sm_state(&mut self, is_terminal: bool) -> Result<crate::ast::decl::SmState, ()> {
        use crate::ast::decl::{SmField, SmState};

        let start = self.span();
        self.advance(); // eat `state`
        let name = self.parse_ident_name()?;

        let fields = if self.eat(&Token::LParen) {
            let mut fs = Vec::new();
            loop {
                self.skip_newlines();
                if matches!(self.peek(), Token::RParen | Token::Eof) {
                    break;
                }
                let fstart = self.span();
                let fname = self.parse_ident_name()?;
                let type_ann = if self.eat(&Token::Colon) {
                    Some(self.parse_type_expr()?)
                } else {
                    None
                };
                fs.push(SmField {
                    name: fname,
                    type_ann,
                    span: fstart.merge(self.span()),
                });
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::RParen)?;
            fs
        } else {
            Vec::new()
        };

        Ok(SmState {
            name,
            fields,
            is_terminal,
            span: start.merge(self.span()),
        })
    }

    fn parse_sm_transition(&mut self) -> Result<crate::ast::decl::SmTransition, ()> {
        use crate::ast::decl::{SmFromPattern, SmTransition};
        use crate::parser::error::{ParseError, ParseErrorClass};

        let start = self.span();
        self.advance(); // eat `on`

        let event_name = self.parse_ident_name()?;

        // Optional event params: `on Assign(t, r)` or `on Resume` (no parens).
        let event_params = if self.eat(&Token::LParen) {
            let mut params = Vec::new();
            loop {
                self.skip_newlines();
                if matches!(self.peek(), Token::RParen | Token::Eof) {
                    break;
                }
                // Accept `_` as wildcard or any ident name.
                let pname = self.parse_ident_name()?;
                params.push(pname);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::RParen)?;
            params
        } else {
            Vec::new()
        };

        // `from State` or `from any`
        let from_kw_ok = matches!(self.peek(), Token::Ident(n) if n == "from");
        if !from_kw_ok {
            self.errors.push(ParseError::classified(
                self.span(),
                "Expected `from` after event in transition",
                vec!["from".into()],
                Some(self.peek().to_string()),
                ParseErrorClass::Declaration,
            ));
            return Err(());
        }
        self.advance(); // eat `from`

        let from = match self.peek().clone() {
            Token::Ident(ref n) if n == "any" => {
                self.advance();
                SmFromPattern::Any
            }
            // `from Working(_)` — consume the state name and skip any parens/wildcards.
            _ => {
                let state_name = self.parse_ident_name()?;
                // Consume optional `(_)` wildcard pattern (e.g. `from Working(_)`).
                if self.eat(&Token::LParen) {
                    let mut depth = 1usize;
                    loop {
                        match self.peek() {
                            Token::LParen => {
                                depth += 1;
                                self.advance();
                            }
                            Token::RParen => {
                                depth -= 1;
                                self.advance();
                                if depth == 0 {
                                    break;
                                }
                            }
                            Token::Eof => break,
                            _ => {
                                self.advance();
                            }
                        }
                    }
                }
                SmFromPattern::Named(state_name)
            }
        };

        // `->`
        self.expect(&Token::Arrow)?;

        let to_state = self.parse_ident_name()?;

        // Consume optional target args `(t)` without deep parsing.
        if self.eat(&Token::LParen) {
            let mut depth = 1usize;
            loop {
                match self.peek() {
                    Token::LParen => {
                        depth += 1;
                        self.advance();
                    }
                    Token::RParen => {
                        depth -= 1;
                        self.advance();
                        if depth == 0 {
                            break;
                        }
                    }
                    Token::Eof => break,
                    _ => {
                        self.advance();
                    }
                }
            }
        }

        Ok(SmTransition {
            event_name,
            event_params,
            from,
            to_state,
            span: start.merge(self.span()),
        })
    }

    // ── TASK-2.6 Path A: workflow / activity / actor ──────────────────────────

    /// Parse `workflow Name(params) [uses ...] [to ReturnType] { body }`.
    pub(crate) fn parse_workflow_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat `workflow`
        self.finish_workflow_decl(start, None, None)
    }

    /// `@distributed_train(strategy = ..., peers = N) workflow Name(...) { ... }`
    pub(crate) fn parse_distributed_train_workflow_decl(&mut self) -> Result<Decl, ()> {
        use crate::parser::error::{ParseError, ParseErrorClass};
        let outer_start = self.span();
        self.advance(); // `@distributed_train`
        self.expect(&Token::LParen)?;
        let mut strategy: Option<String> = None;
        let mut peers: Option<u64> = None;
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RParen | Token::Eof) {
                break;
            }
            if let Token::Ident(key) = self.peek().clone() {
                let key = key.clone();
                self.advance();
                self.expect(&Token::Eq)?;
                match key.as_str() {
                    "strategy" => match self.peek().clone() {
                        Token::StringLit(s) | Token::SingleStringLit(s) => {
                            self.advance();
                            strategy = Some(s);
                        }
                        Token::Ident(v) => {
                            self.advance();
                            strategy = Some(v);
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string or identifier for `strategy` in `@distributed_train`",
                                vec!["\"data_parallel\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    },
                    "peers" => {
                        if let Token::IntLit(n) = self.peek().clone() {
                            self.advance();
                            if n > 0 {
                                peers = Some(n as u64);
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
        self.skip_newlines();
        if !matches!(self.peek(), Token::Workflow) {
            self.errors.push(ParseError::classified(
                self.span(),
                "Expected `workflow` after `@distributed_train(...)`",
                vec!["workflow".into()],
                Some(self.peek().to_string()),
                ParseErrorClass::Declaration,
            ));
            return Err(());
        }
        self.advance(); // `workflow`
        self.finish_workflow_decl(outer_start, strategy, peers)
    }

    fn finish_workflow_decl(
        &mut self,
        span_start: crate::ast::span::Span,
        distributed_train_strategy: Option<String>,
        distributed_train_peers: Option<u64>,
    ) -> Result<Decl, ()> {
        use crate::ast::decl::WorkflowDecl;
        let name = self.parse_ident_name()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = if self.eat_return_arrow() {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        Ok(Decl::Workflow(WorkflowDecl {
            name,
            params,
            return_type,
            body,
            is_traced: false,
            is_deprecated: false,
            distributed_train_strategy,
            distributed_train_peers,
            span: span_start.merge(self.span()),
        }))
    }

    /// Parse `activity Name(params) [uses ...] [to ReturnType] { body }`.
    pub(crate) fn parse_activity_decl(&mut self) -> Result<Decl, ()> {
        use crate::ast::decl::ActivityDecl;
        let start = self.span();
        self.advance(); // eat `activity`
        let name = self.parse_ident_name()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let return_type = if self.eat_return_arrow() {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        Ok(Decl::Activity(ActivityDecl {
            name,
            params,
            return_type,
            body,
            options: None,
            prompt: None,
            is_traced: false,
            is_deprecated: false,
            span: start.merge(self.span()),
        }))
    }

    /// Parse `actor Name { on event(params) [to ReturnType] { body } … }`.
    pub(crate) fn parse_actor_decl(&mut self) -> Result<Decl, ()> {
        use crate::ast::decl::logic::{ActorDecl, ActorHandler};
        use crate::parser::error::{ParseError, ParseErrorClass};
        let start = self.span();
        self.advance(); // eat `actor`
        let name = self.parse_ident_name()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();
        let mut handlers = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            if !matches!(self.peek(), Token::On) {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Expected `on` handler inside actor block",
                    vec!["on".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
            let h_start = self.span();
            self.advance(); // eat `on`
            let event_name = self.parse_ident_name()?;
            self.expect(&Token::LParen)?;
            let params = self.parse_params()?;
            self.expect(&Token::RParen)?;
            let return_type = if self.eat_return_arrow() {
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            self.expect(&Token::LBrace)?;
            let body = self.parse_block()?;
            handlers.push(ActorHandler {
                event_name,
                params,
                return_type,
                body,
                is_traced: false,
                span: h_start.merge(self.span()),
            });
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;
        Ok(Decl::Actor(ActorDecl {
            name,
            state_fields: vec![],
            handlers,
            is_deprecated: false,
            span: start.merge(self.span()),
        }))
    }
}
