// Import declaration parsing.

use super::super::Parser;
use super::head_types::{
    Decl, ImportDecl, ImportPath, ImportPathKind, ReactNamedImport, RustCrateImport,
};
use crate::ast::decl::ReactBinding;
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
}
