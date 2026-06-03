// Top-level and declaration parsing.

use super::super::Parser;
use crate::ast::decl::{
    AstColorToken, AstFontToken, AstScalarToken, BackButtonDecl, Decl, DeepLinkDecl, EffectDecl,
    EndpointDecl, EndpointKind, ExampleDecl, FieldConstraint, FnDecl, ForallDecl, FormDecl,
    FormField, ImportDecl, ImportPath, ImportPathKind, LoadingDecl, McpResourceDecl, McpToolDecl,
    OnCleanupDecl, OnMountDecl, PostCondition, PushDecl, ReactBinding, ReactNamedImport,
    ReactiveComponentDecl, ReactiveMemberDecl, RustCrateImport, ScheduledDecl, TestDecl,
    TokensDecl,
};
use crate::ast::span::Span;
use crate::lexer::token::Token;
use crate::parser::error::{ParseError, ParseErrorClass};

impl Parser {
    pub(crate) fn parse_import(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat 'import'
        let mut paths = Vec::new();
        loop {
            if self.try_parse_local_file_import(&mut paths)? {
                if !self.eat(&Token::Comma) {
                    break;
                }
                continue;
            }
            if self.try_parse_react_component_import(&mut paths)? {
                if !self.eat(&Token::Comma) {
                    break;
                }
                continue;
            }
            // `rust:` imports use the full parse_import_path handler.
            // All symbol imports go through parse_symbol_import which also accepts
            // `/` as a path separator and `as { name1, name2 }` destructuring.
            let first_is_rust = matches!(self.peek(), Token::Ident(n) if n == "rust");
            if first_is_rust {
                let path = self.parse_import_path()?;
                paths.push(path);
            } else {
                self.parse_symbol_import(&mut paths)?;
            }
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        Ok(Decl::Import(ImportDecl {
            paths,
            span: start.merge(self.span()),
        }))
    }

    /// `import "./helpers/walk_docs.vox" [as alias]` — intra-project Vox file
    /// import. Returns `Ok(false)` without consuming input when the next token
    /// isn't a string literal. Rejects strings that don't end in `.vox` so
    /// authors don't confuse this with arbitrary asset imports.
    /// See `docs/src/architecture/intra-project-imports-rfc-2026-05-23.md`.
    fn try_parse_local_file_import(&mut self, paths: &mut Vec<ImportPath>) -> Result<bool, ()> {
        let seg_start = self.span();
        let path = match self.peek().clone() {
            Token::StringLit(s) | Token::SingleStringLit(s) => s,
            _ => return Ok(false),
        };
        self.advance();
        if !path.ends_with(".vox") {
            self.errors.push(ParseError::classified(
                seg_start,
                format!(
                    "Intra-project import path must end with `.vox` (got `{path}`). Use `import \"./helpers/foo.vox\"`."
                ),
                vec!["\"./relative/path.vox\"".into()],
                Some(path.clone()),
                ParseErrorClass::Declaration,
            ));
            return Err(());
        }
        let alias = if matches!(self.peek(), Token::Ident(w) if w == "as") {
            self.advance();
            Some(self.parse_ident_name()?)
        } else {
            None
        };
        paths.push(ImportPath {
            kind: ImportPathKind::LocalFile { path },
            alias,
            span: seg_start.merge(self.span()),
        });
        Ok(true)
    }

    /// `import react …` — Phase 5 React/TS interop. Three binding shapes:
    ///   `import react X from "<spec>"`                 (default)
    ///   `import react { A, B as C } from "<spec>"`     (named)
    ///   `import react * as Ns from "<spec>"`           (namespace)
    ///
    /// Returns `Ok(false)` without consuming input when this is not a react
    /// import (including `import react.use_state`, which starts with `react`
    /// then `.`, and the bare `import react` symbol form). Once `react` is
    /// followed by `{` or `*` the form is unambiguous, so malformed input there
    /// is a hard parse error rather than a silent bail.
    fn try_parse_react_component_import(
        &mut self,
        paths: &mut Vec<ImportPath>,
    ) -> Result<bool, ()> {
        let save = self.pos;
        let seg_start = self.span();
        let Token::Ident(first) = self.peek().clone() else {
            return Ok(false);
        };
        if first != "react" {
            return Ok(false);
        }
        self.advance();
        let binding = match self.peek().clone() {
            // `import react.use_state` / `import react/foo` → symbol path, not this form.
            Token::Dot | Token::Slash => {
                self.pos = save;
                return Ok(false);
            }
            // Named: `import react { A, B as C } from "<spec>"` (committed once `{` seen).
            Token::LBrace => {
                self.advance(); // eat `{`
                let mut names = Vec::new();
                loop {
                    if matches!(self.peek(), Token::RBrace) {
                        break;
                    }
                    let imported = match self.peek().clone() {
                        Token::Ident(n) | Token::TypeIdent(n) => {
                            self.advance();
                            n
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected an identifier inside `import react { … }`.",
                                vec!["identifier".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    let local = if matches!(self.peek(), Token::Ident(w) if w == "as") {
                        self.advance();
                        self.parse_ident_name()?
                    } else {
                        imported.clone()
                    };
                    names.push(ReactNamedImport { imported, local });
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                if names.is_empty() {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        "Expected at least one name inside `import react { ... }`.",
                        vec!["Dialog".into(), "Dialog as D".into()],
                        Some("}".into()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
                self.expect(&Token::RBrace)?;
                ReactBinding::Named(names)
            }
            // Namespace: `import react * as Ns from "<spec>"` (committed once `*` seen).
            Token::Star => {
                self.advance(); // eat `*`
                if !matches!(self.peek(), Token::Ident(w) if w == "as") {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        "Expected `as <Name>` after `import react *`.",
                        vec!["as".into()],
                        Some(self.peek().to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
                self.advance(); // eat `as`
                let local_name = self.parse_ident_name()?;
                ReactBinding::Namespace { local_name }
            }
            // Default: `import react X from "<spec>"`.
            Token::Ident(local_name) | Token::TypeIdent(local_name) => {
                self.advance();
                ReactBinding::Default { local_name }
            }
            // `import react` alone (or anything else) → let the symbol parser try.
            _ => {
                self.pos = save;
                return Ok(false);
            }
        };
        // All three shapes require `from "<spec>"`.
        let from_ok = matches!(self.peek(), Token::Ident(w) if w == "from");
        if !from_ok {
            // Default form may have been a false positive (e.g. `import react foo`
            // used as a symbol) — only bail for Default; named/namespace are committed.
            if matches!(binding, ReactBinding::Default { .. }) {
                self.pos = save;
                return Ok(false);
            }
            self.errors.push(ParseError::classified(
                self.span(),
                "Expected `from \"<module>\"` in a react import.",
                vec!["from \"module\"".into()],
                Some(self.peek().to_string()),
                ParseErrorClass::Declaration,
            ));
            return Err(());
        }
        self.advance(); // eat `from`
        let module_specifier = match self.peek().clone() {
            Token::StringLit(s) | Token::SingleStringLit(s) => {
                self.advance();
                s
            }
            _ => {
                if matches!(binding, ReactBinding::Default { .. }) {
                    self.pos = save;
                    return Ok(false);
                }
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Expected a module specifier string after `from` in a react import.",
                    vec!["\"@scope/pkg\"".into(), "\"./Component.tsx\"".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
        };
        paths.push(ImportPath {
            kind: ImportPathKind::ReactComponent {
                module_specifier,
                binding,
            },
            alias: None,
            span: seg_start.merge(self.span()),
        });
        Ok(true)
    }

    /// Parse one symbol import declaration, appending zero or more `ImportPath`s to `paths`.
    ///
    /// Handles three forms:
    ///   `import lib.chrome.StateChip`          — dotted single-item
    ///   `import lib/chrome.StateChip`          — slash-separated (equivalent)
    ///   `import lib/chrome as { A, B, C }`     — destructured multi-item (ES6-style)
    ///   `import lib.chrome as Alias`           — whole-module alias
    fn parse_symbol_import(&mut self, paths: &mut Vec<ImportPath>) -> Result<(), ()> {
        let seg_start = self.span();

        // ── collect path segments (separated by '.' or '/') ──────────────────
        let first = match self.peek().clone() {
            Token::Ident(name) | Token::TypeIdent(name) => {
                self.advance();
                name
            }
            Token::Env => {
                self.advance();
                "env".to_string()
            }
            Token::Http => {
                self.advance();
                "http".to_string()
            }
            _ => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Import path must begin with an identifier (for example `lib.chrome.StateChip` or `lib/chrome as { StateChip }`).",
                    vec!["identifier".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
        };

        let mut segments = vec![first];
        loop {
            // Accept both '.' and '/' as path separators.
            if !matches!(self.peek(), Token::Dot | Token::Slash) {
                break;
            }
            self.advance(); // eat '.' or '/'
            match self.peek().clone() {
                Token::Ident(name) | Token::TypeIdent(name) => {
                    segments.push(name);
                    self.advance();
                }
                Token::Env => {
                    self.advance();
                    segments.push("env".to_string());
                }
                Token::Http => {
                    self.advance();
                    segments.push("http".to_string());
                }
                _ => break,
            }
        }

        // ── check for `as …` ──────────────────────────────────────────────────
        let has_as = matches!(self.peek(), Token::Ident(w) if w == "as");
        if has_as {
            self.advance(); // eat 'as'
            if matches!(self.peek(), Token::LBrace) {
                self.advance(); // eat '{'
                // Destructured form: `import lib/chrome as { StateChip, TopBar }`
                // Expand into one ImportPath per item (item appended to segments).
                loop {
                    if matches!(self.peek(), Token::RBrace) {
                        break;
                    }
                    let item = match self.peek().clone() {
                        Token::Ident(name) | Token::TypeIdent(name) => {
                            self.advance();
                            name
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected identifier inside destructured import `as { ... }`.",
                                vec!["identifier".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    // Optional `as item_alias` inside the braces.
                    let item_alias = if matches!(self.peek(), Token::Ident(w) if w == "as") {
                        self.advance();
                        Some(self.parse_ident_name()?)
                    } else {
                        None
                    };
                    let mut full = segments.clone();
                    full.push(item.clone());
                    paths.push(ImportPath {
                        kind: ImportPathKind::SymbolPath { segments: full },
                        alias: item_alias,
                        span: seg_start.merge(self.span()),
                    });
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RBrace)?;
            } else {
                // Single alias: `import lib.chrome as chrome`
                let alias_name = self.parse_ident_name()?;
                paths.push(ImportPath {
                    kind: ImportPathKind::SymbolPath { segments },
                    alias: Some(alias_name),
                    span: seg_start.merge(self.span()),
                });
            }
        } else {
            // No `as` — last segment is the item name.
            paths.push(ImportPath {
                kind: ImportPathKind::SymbolPath { segments },
                alias: None,
                span: seg_start.merge(self.span()),
            });
        }
        Ok(())
    }

    pub(crate) fn parse_import_path(&mut self) -> Result<ImportPath, ()> {
        let start = self.span();
        let mut alias = None;
        let first = match self.peek().clone() {
            Token::Ident(name) | Token::TypeIdent(name) => {
                self.advance();
                name
            }
            Token::Env => {
                self.advance();
                "env".to_string()
            }
            Token::Http => {
                self.advance();
                "http".to_string()
            }
            _ => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Import path must begin with an identifier (for example `react.use_state` or `rust:serde_json`).",
                    vec!["identifier".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                return Err(());
            }
        };

        if first == "rust" && self.eat(&Token::Colon) {
            let crate_name = match self.peek().clone() {
                Token::Ident(name) | Token::TypeIdent(name) => {
                    self.advance();
                    name
                }
                _ => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        "Rust import must include a crate name after `rust:` (for example `import rust:serde_json`).",
                        vec!["crate-name".into()],
                        Some(self.peek().to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            };

            let mut rust_meta = RustCrateImport {
                crate_name,
                version: None,
                path: None,
                git: None,
                rev: None,
            };

            if self.eat(&Token::LParen) {
                loop {
                    match self.peek().clone() {
                        Token::RParen => {
                            self.advance();
                            break;
                        }
                        Token::Ident(key) => {
                            self.advance();
                            self.expect(&Token::Colon)?;
                            let value = match self.peek().clone() {
                                Token::StringLit(v) => {
                                    self.advance();
                                    v
                                }
                                Token::Ident(v) | Token::TypeIdent(v) => {
                                    self.advance();
                                    v
                                }
                                _ => {
                                    self.errors.push(ParseError::classified(
                                        self.span(),
                                        "Rust import metadata values must be string or identifier.",
                                        vec!["string".into(), "identifier".into()],
                                        Some(self.peek().to_string()),
                                        ParseErrorClass::Declaration,
                                    ));
                                    return Err(());
                                }
                            };
                            match key.as_str() {
                                "version" => rust_meta.version = Some(value),
                                "path" => rust_meta.path = Some(value),
                                "git" => rust_meta.git = Some(value),
                                "rev" | "branch" => rust_meta.rev = Some(value),
                                _ => {
                                    self.errors.push(ParseError::classified(
                                        self.span(),
                                        format!(
                                            "Unknown rust import metadata key `{key}`; expected one of version/path/git/rev."
                                        ),
                                        vec![
                                            "version".into(),
                                            "path".into(),
                                            "git".into(),
                                            "rev".into(),
                                        ],
                                        Some(key),
                                        ParseErrorClass::Declaration,
                                    ));
                                    return Err(());
                                }
                            }
                            if self.eat(&Token::Comma) {
                                continue;
                            }
                            if self.eat(&Token::RParen) {
                                break;
                            }
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected `,` or `)` after rust import metadata item.",
                                vec![",".into(), ")".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected metadata key or `)` in rust import metadata list.",
                                vec!["identifier".into(), ")".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                }
            }

            if let Token::Ident(word) = self.peek().clone()
                && word == "as"
            {
                self.advance();
                alias = Some(self.parse_ident_name()?);
            }

            return Ok(ImportPath {
                kind: ImportPathKind::RustCrate(rust_meta),
                alias,
                span: start.merge(self.span()),
            });
        }

        let mut segments = vec![first];
        while self.eat(&Token::Dot) {
            match self.peek().clone() {
                Token::Ident(name) | Token::TypeIdent(name) => {
                    segments.push(name);
                    self.advance();
                }
                Token::Env => {
                    self.advance();
                    segments.push("env".to_string());
                }
                // `http` is a dedicated keyword for route headers, but it must still parse as a
                // path segment after `.` (e.g. `import std.http`, `std.http.get_text(...)`).
                Token::Http => {
                    self.advance();
                    segments.push("http".to_string());
                }
                _ => break,
            }
        }
        if let Token::Ident(word) = self.peek().clone()
            && word == "as"
        {
            self.advance();
            alias = Some(self.parse_ident_name()?);
        }

        Ok(ImportPath {
            kind: ImportPathKind::SymbolPath { segments },
            alias,
            span: start.merge(self.span()),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn parse_component(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @component
        self.skip_newlines();
        match self.peek().clone() {
            Token::Fn => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Retired classic `@component fn`. Use Path C `component Name() { ... }` (or prefix: `@component Name() { ... }`).",
                    vec!["component Counter() { state n: int = 0; view: <span>{n}</span> }".into()],
                    Some("fn".into()),
                    ParseErrorClass::Declaration,
                ));
                Err(())
            }
            Token::Ident(_) | Token::TypeIdent(_) => {
                let name = self.parse_ident_name()?;
                let mut inner = self.finish_reactive_component_after_name(start, name)?;
                inner.styles = self.parse_style_blocks();
                Ok(Decl::ReactiveComponent(inner))
            }
            _ => {
                self.errors.push(ParseError::classified(
                    self.span(),
                    "Unsupported head after `@component`: use an identifier for Path C (`@component Name(...)`). Classic `@component fn` is retired.",
                    vec!["ComponentName".into()],
                    Some(self.peek().to_string()),
                    ParseErrorClass::Declaration,
                ));
                Err(())
            }
        }
    }

    /// `Name(params) { state ... }` — shared by `component` and `@component` reactive forms.
    pub(crate) fn finish_reactive_component_after_name(
        &mut self,
        start: Span,
        name: String,
    ) -> Result<ReactiveComponentDecl, ()> {
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;

        self.expect(&Token::LBrace)?;
        let mut members = Vec::new();
        let mut view = None;

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Token::RBrace | Token::Eof => break,
                Token::State => members.push(ReactiveMemberDecl::State(self.parse_state_decl()?)),
                Token::Derived => {
                    members.push(ReactiveMemberDecl::Derived(self.parse_derived_decl()?))
                }
                Token::Effect => {
                    let eff_start = self.span();
                    self.advance(); // eat `effect`
                    // Optional `depends_on (a, b)` clause.
                    let explicit_deps = if matches!(self.peek(), Token::Ident(n) if n == "depends_on")
                    {
                        self.advance(); // eat `depends_on`
                        self.expect(&Token::LParen)?;
                        let mut deps = Vec::new();
                        while !matches!(self.peek(), Token::RParen | Token::Eof) {
                            deps.push(self.parse_ident_name()?);
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect(&Token::RParen)?;
                        Some(deps)
                    } else {
                        None
                    };
                    self.expect(&Token::Colon)?;
                    let body = if matches!(self.peek(), Token::LBrace) {
                        let b_start = self.span();
                        self.advance(); // eat `{`
                        let stmts = self.parse_block()?;
                        crate::ast::expr::Expr::Block {
                            stmts,
                            span: b_start.merge(self.span()),
                        }
                    } else {
                        self.parse_expr()?
                    };
                    members.push(ReactiveMemberDecl::Effect(EffectDecl {
                        body,
                        explicit_deps,
                        span: eff_start.merge(self.span()),
                    }));
                }
                Token::On => {
                    let on_start = self.span();
                    self.advance();
                    match self.peek().clone() {
                        Token::Mount => {
                            let body = self.parse_reactive_block()?;
                            members.push(ReactiveMemberDecl::OnMount(OnMountDecl {
                                body,
                                span: on_start.merge(self.span()),
                            }));
                        }
                        Token::Cleanup => {
                            let body = self.parse_reactive_block()?;
                            members.push(ReactiveMemberDecl::OnCleanup(OnCleanupDecl {
                                body,
                                span: on_start.merge(self.span()),
                            }));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected `mount` or `cleanup` after `on` in reactive component block.",
                                vec!["mount".into(), "cleanup".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                }
                Token::View => {
                    self.advance();
                    self.expect(&Token::Colon)?;
                    view = Some(self.parse_expr()?);
                }
                _ => {
                    let stmt = self.parse_stmt()?;
                    members.push(ReactiveMemberDecl::Stmt(stmt));
                }
            }
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;

        Ok(ReactiveComponentDecl {
            name,
            params,
            members,
            view,
            styles: vec![],
            span: start.merge(self.span()),
        })
    }

    /// ADR-033: parse a `fragment Name(args) { <markup> }` declaration into a
    /// [`crate::ast::decl::FragmentDecl`]. The body is parsed as a single expression
    /// (matches the `view:` shape inside reactive components). Codegen for fragments
    /// is gated on Phase 6 typed-primitive stabilization per the ADR; for now the
    /// parser accepts the syntax and the AST node carries it through to whatever
    /// future codegen / lowering wants to consume it.
    pub(crate) fn parse_fragment_decl(&mut self) -> Result<crate::ast::decl::Decl, ()> {
        use crate::ast::decl::FragmentDecl;
        use crate::lexer::token::Token;

        let start = self.span();
        self.expect(&Token::Fragment)?;
        let name = self.parse_ident_name()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();
        let body = self.parse_expr()?;
        self.skip_newlines();
        self.expect(&Token::RBrace)?;
        Ok(crate::ast::decl::Decl::Fragment(FragmentDecl {
            name,
            params,
            body,
            span: start.merge(self.span()),
        }))
    }

    /// ADR-032: parse module-scope reactive members in a `.vox.ui` file into a single
    /// synthetic [`ReactiveModuleDecl`]. Consumes consecutive `state` / `derived` /
    /// `effect` / `on mount` / `on cleanup` declarations until it hits a token that
    /// isn't one of those, then returns. Subsequent module-scope reactive members in
    /// the same file would be picked up by another `parse_decl` call and produce a
    /// second `ReactiveModuleDecl` — that's intentional; per-module name disambiguation
    /// is the file's responsibility.
    ///
    /// Caller (`parse_decl`) only invokes this when `self.file_kind ==
    /// FileKind::ReactiveModule` and the next token is a reactive member.
    pub(crate) fn parse_reactive_module_decl(&mut self) -> Result<crate::ast::decl::Decl, ()> {
        use crate::ast::decl::{
            EffectDecl, OnCleanupDecl, OnMountDecl, ReactiveMemberDecl, ReactiveModuleDecl,
        };

        let start = self.span();
        let mut members: Vec<ReactiveMemberDecl> = Vec::new();

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Token::State => members.push(ReactiveMemberDecl::State(self.parse_state_decl()?)),
                Token::Derived => {
                    members.push(ReactiveMemberDecl::Derived(self.parse_derived_decl()?))
                }
                Token::Effect => {
                    let eff_start = self.span();
                    self.advance(); // eat `effect`
                    // Optional `depends_on (a, b)` clause.
                    let explicit_deps = if matches!(self.peek(), Token::Ident(n) if n == "depends_on")
                    {
                        self.advance(); // eat `depends_on`
                        self.expect(&Token::LParen)?;
                        let mut deps = Vec::new();
                        while !matches!(self.peek(), Token::RParen | Token::Eof) {
                            deps.push(self.parse_ident_name()?);
                            if !self.eat(&Token::Comma) {
                                break;
                            }
                        }
                        self.expect(&Token::RParen)?;
                        Some(deps)
                    } else {
                        None
                    };
                    self.expect(&Token::Colon)?;
                    let body = if matches!(self.peek(), Token::LBrace) {
                        let b_start = self.span();
                        self.advance(); // eat `{`
                        let stmts = self.parse_block()?;
                        crate::ast::expr::Expr::Block {
                            stmts,
                            span: b_start.merge(self.span()),
                        }
                    } else {
                        self.parse_expr()?
                    };
                    members.push(ReactiveMemberDecl::Effect(EffectDecl {
                        body,
                        explicit_deps,
                        span: eff_start.merge(self.span()),
                    }));
                }
                Token::On => {
                    let on_start = self.span();
                    self.advance();
                    match self.peek().clone() {
                        Token::Mount => {
                            let body = self.parse_reactive_block()?;
                            members.push(ReactiveMemberDecl::OnMount(OnMountDecl {
                                body,
                                span: on_start.merge(self.span()),
                            }));
                        }
                        Token::Cleanup => {
                            let body = self.parse_reactive_block()?;
                            members.push(ReactiveMemberDecl::OnCleanup(OnCleanupDecl {
                                body,
                                span: on_start.merge(self.span()),
                            }));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected `mount` or `cleanup` after `on` at module scope in a `.vox.ui` file.",
                                vec!["mount".into(), "cleanup".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                }
                _ => break,
            }
        }

        Ok(crate::ast::decl::Decl::ReactiveModule(ReactiveModuleDecl {
            // Module name is filled in later by codegen from the source file basename;
            // the parser doesn't know the path. Empty for now.
            name: String::new(),
            members,
            span: start.merge(self.span()),
        }))
    }

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

    pub(crate) fn parse_fn_decl(&mut self, is_pub: bool) -> Result<FnDecl, ()> {
        let start = self.span();
        let mut preconditions = Vec::new();
        let mut postconditions = Vec::new();
        let mut invariants = Vec::new();
        let mut is_mobile_native = false;
        let mut is_pure = false;
        let mut is_reactive = false;
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
                Token::AtAuth | Token::AtOfflineCapable | Token::AtCollaborative => {
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
            auth_provider: None,
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

    /// Parse a `@form Name { field ... on_submit: ... }` declaration.
    pub(crate) fn parse_form_decl(&mut self) -> Result<Decl, ()> {
        let start = self.span();
        self.advance(); // eat @form
        let name = self.parse_ident_name()?;
        self.expect(&Token::LBrace)?;

        let mut fields: Vec<FormField> = Vec::new();
        let mut on_submit: Option<String> = None;
        let mut success_redirect: Option<String> = None;
        let mut error_message: Option<String> = None;

        loop {
            self.skip_newlines();
            match self.peek().clone() {
                Token::RBrace | Token::Eof => break,
                Token::Ident(ref kw) if kw == "field" => {
                    self.advance(); // eat `field`
                    fields.push(self.parse_form_field()?);
                }
                Token::Ident(ref kw) if kw == "on_submit" => {
                    self.advance(); // eat `on_submit`
                    self.expect(&Token::Colon)?;
                    on_submit = Some(self.parse_ident_name()?);
                }
                Token::Ident(ref kw) if kw == "success_redirect" => {
                    self.advance(); // eat `success_redirect`
                    self.expect(&Token::Colon)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            success_redirect = Some(s);
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal for success_redirect",
                                vec!["\"...\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                }
                Token::Ident(ref kw) if kw == "error_message" => {
                    self.advance(); // eat `error_message`
                    self.expect(&Token::Colon)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            error_message = Some(s);
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal for error_message",
                                vec!["\"...\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                }
                other => {
                    self.errors.push(ParseError::classified(
                        self.span(),
                        format!("Unexpected token inside @form block: {other}; expected `field`, `on_submit`, `success_redirect`, or `}}`"),
                        vec!["field".into(), "on_submit".into(), "success_redirect".into(), "}".into()],
                        Some(other.to_string()),
                        ParseErrorClass::Declaration,
                    ));
                    return Err(());
                }
            }
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;

        Ok(Decl::Form(FormDecl {
            name,
            fields,
            on_submit,
            success_redirect,
            error_message,
            span: start.merge(self.span()),
        }))
    }

    /// Parse a single `field name: Type [constraints...] [required|optional] [hidden]` line
    /// inside a `@form` block. Cursor is positioned after the `field` keyword.
    fn parse_form_field(&mut self) -> Result<FormField, ()> {
        let start = self.span();
        let name = self.parse_ident_name()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_type_expr()?;

        let mut constraints: Vec<FieldConstraint> = Vec::new();
        let mut required = false;
        let mut hidden = false;
        let mut label: Option<String> = None;
        let mut default: Option<crate::ast::expr::Expr> = None;

        // Parse optional modifiers on the same line until newline or `}`
        loop {
            match self.peek().clone() {
                Token::Newline | Token::RBrace | Token::Eof => break,
                Token::Ident(ref kw) if kw == "required" => {
                    self.advance();
                    required = true;
                }
                Token::Ident(ref kw) if kw == "optional" => {
                    self.advance();
                    required = false;
                }
                Token::Ident(ref kw) if kw == "hidden" => {
                    self.advance();
                    hidden = true;
                }
                Token::Ident(ref kw) if kw == "label" => {
                    self.advance(); // eat `label`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            label = Some(s);
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal inside label(...)",
                                vec!["\"label text\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "range" => {
                    self.advance(); // eat `range`
                    self.expect(&Token::LParen)?;
                    // Parse `lo..hi` — lexed as IntLit, Dot, Dot, IntLit
                    let lo_span = self.span();
                    let lo = match self.peek().clone() {
                        Token::IntLit(v) => {
                            let span = self.span();
                            self.advance();
                            crate::ast::expr::Expr::IntLit { value: v, span }
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal for range lower bound",
                                vec!["1".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    // Consume two dots: `..`
                    self.expect(&Token::Dot)?;
                    self.expect(&Token::Dot)?;
                    let hi = match self.peek().clone() {
                        Token::IntLit(v) => {
                            let span = self.span();
                            self.advance();
                            crate::ast::expr::Expr::IntLit { value: v, span }
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal for range upper bound",
                                vec!["10".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    };
                    self.expect(&Token::RParen)?;
                    let _ = lo_span;
                    constraints.push(FieldConstraint::Range(lo, hi));
                }
                Token::Ident(ref kw) if kw == "max_len" => {
                    self.advance(); // eat `max_len`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::IntLit(v) => {
                            self.advance();
                            constraints.push(FieldConstraint::MaxLen(v as usize));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal inside max_len(...)",
                                vec!["280".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "min_len" => {
                    self.advance(); // eat `min_len`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::IntLit(v) => {
                            self.advance();
                            constraints.push(FieldConstraint::MinLen(v as usize));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected integer literal inside min_len(...)",
                                vec!["1".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "pattern" => {
                    self.advance(); // eat `pattern`
                    self.expect(&Token::LParen)?;
                    match self.peek().clone() {
                        Token::StringLit(s) => {
                            self.advance();
                            constraints.push(FieldConstraint::Pattern(s));
                        }
                        _ => {
                            self.errors.push(ParseError::classified(
                                self.span(),
                                "Expected string literal inside pattern(...)",
                                vec!["\"^[a-z]+$\"".into()],
                                Some(self.peek().to_string()),
                                ParseErrorClass::Declaration,
                            ));
                            return Err(());
                        }
                    }
                    self.expect(&Token::RParen)?;
                }
                Token::Ident(ref kw) if kw == "default" => {
                    self.advance(); // eat `default`
                    self.expect(&Token::LParen)?;
                    default = Some(self.parse_expr()?);
                    self.expect(&Token::RParen)?;
                }
                _ => break,
            }
        }

        Ok(FormField {
            name,
            ty,
            label,
            required,
            hidden,
            default,
            constraints,
            span: start.merge(self.span()),
        })
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
                    colors.push(AstColorToken {
                        name,
                        light,
                        dark,
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
