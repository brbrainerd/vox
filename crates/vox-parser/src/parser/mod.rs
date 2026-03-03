pub mod decl;
pub mod expr;
pub mod stmt;
pub mod types;
pub mod pattern;
pub mod db;
pub mod ui;
pub mod logic;

use crate::error::ParseError;
use vox_ast::decl::*;
use vox_ast::span::Span;
use vox_lexer::cursor::Spanned;
use vox_lexer::token::Token;

/// Compute edit distance (Levenshtein) between two strings.
pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] { dp[i - 1][j - 1] }
            else { 1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1]) };
        }
    }
    dp[m][n]
}

/// Return a "did you mean 'X'?" hint if any candidate is close enough.
pub(crate) fn did_you_mean(input: &str, candidates: &[&str]) -> Option<String> {
    let input_lower = input.to_lowercase();
    let best = candidates.iter().filter_map(|&c| {
        let dist = edit_distance(&input_lower, &c.to_lowercase());
        let threshold = (c.len() as f32 * 0.4).ceil() as usize;
        let threshold = threshold.max(2).min(3);
        if dist <= threshold && dist < input.len() { Some((dist, c)) } else { None }
    }).min_by_key(|(d, _)| *d);
    best.map(|(_, c)| format!("did you mean '{c}'?"))
}

pub fn parse(tokens: Vec<Spanned>) -> Result<Module, Vec<ParseError>> {
    let mut p = Parser::new(tokens);
    let (m, errs) = p.parse_module_lossy();
    if errs.is_empty() { Ok(m) } else { Err(errs) }
}

pub fn parse_lossy(tokens: Vec<Spanned>) -> (Module, Vec<ParseError>) {
    let mut p = Parser::new(tokens);
    p.parse_module_lossy()
}

pub(crate) struct Parser {
    pub(crate) tokens: Vec<Spanned>,
    pub(crate) pos: usize,
    pub(crate) errors: Vec<ParseError>,
}

impl Parser {
    pub(crate) fn new(tokens: Vec<Spanned>) -> Self {
        Self { tokens, pos: 0, errors: vec![] }
    }

    pub(crate) fn peek(&self) -> &Token {
        self.tokens.get(self.pos).map(|s| &s.token).unwrap_or(&Token::Eof)
    }

    pub(crate) fn peek_at(&self, n: usize) -> &Token {
        self.tokens.get(self.pos + n).map(|s| &s.token).unwrap_or(&Token::Eof)
    }

    pub(crate) fn span(&self) -> Span {
        self.tokens.get(self.pos).map(|s| Span::new(s.span.start, s.span.end)).unwrap_or(Span::new(0, 0))
    }

    pub(crate) fn advance(&mut self) -> Token {
        let t = self.tokens.get(self.pos).map(|s| s.token.clone()).unwrap_or(Token::Eof);
        self.pos += 1;
        t
    }

    pub(crate) fn expect(&mut self, expected: &Token) -> Result<Span, ()> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            Ok(self.advance_span())
        } else {
            let found_str = self.peek().to_string();
            let expected_str = expected.to_string();
            let hint = did_you_mean(&found_str, &[&expected_str]);
            self.errors.push(ParseError {
                message: format!("Expected {}, found {}", expected_str, found_str),
                span: self.span(),
                expected: vec![expected_str.clone()],
                found: Some(found_str),
                context: hint,
                suggestion: Some(expected_str),
            });
            Err(())
        }
    }

    fn advance_span(&mut self) -> Span {
        let sp = self.span();
        self.advance();
        sp
    }

    pub(crate) fn eat(&mut self, expected: &Token) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn skip_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline) { self.advance(); }
    }

    /// Skip newlines AND indent/dedent tokens - for use inside argument lists and similar contexts.
    pub(crate) fn skip_newlines_and_indent(&mut self) {
        while matches!(self.peek(), Token::Newline | Token::Indent | Token::Dedent) { self.advance(); }
    }

    pub(crate) fn parse_module_lossy(&mut self) -> (Module, Vec<ParseError>) {
        let start = self.span();
        let mut decls = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek(), Token::Eof) {
            match self.parse_decl() {
                Ok(d) => decls.push(d),
                Err(_) => { self.recover_to_top_level(); }
            }
            self.skip_newlines();
        }
        (Module { declarations: decls, span: start.merge(self.span()) }, self.errors.clone())
    }

    fn recover_to_top_level(&mut self) {
        loop {
            match self.peek() {
                Token::Eof => break,
                Token::Fn | Token::AtComponent | Token::Import | Token::TypeKw | Token::Actor | Token::Agent | Token::Message | Token::Workflow | Token::Http | Token::AtTest | Token::AtServer | Token::AtV0 => break,
                _ => { self.advance(); }
            }
        }
    }

    pub(crate) fn report_error(&mut self, message: &str) {
        let found = Some(self.peek().to_string());
        self.errors.push(ParseError {
            message: message.to_string(),
            span: self.span(),
            expected: vec![],
            found,
            context: None,
            suggestion: None,
        });
    }
}
