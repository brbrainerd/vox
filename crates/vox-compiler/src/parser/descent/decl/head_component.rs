// Reactive component / fragment / module-scope reactive parsing.

use super::super::Parser;
use crate::ast::decl::{
    Decl, EffectDecl, OnCleanupDecl, OnMountDecl, ReactiveComponentDecl, ReactiveMemberDecl,
};
use crate::ast::span::Span;
use crate::lexer::token::Token;
use crate::parser::error::{ParseError, ParseErrorClass};

impl Parser {
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
            layer: None,
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
}
