use crate::lexer::token::Token;
use logos::Logos;

/// A located token with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub span: std::ops::Range<usize>,
}

/// Lex source code into a flat vector of spanned tokens, **preserving** comment
/// tokens so that callers can reconstruct the original byte sequence from the
/// token spans plus the inter-token gaps (skipped horizontal whitespace).
///
/// Unlike [`lex`], this function does **not** strip [`Token::Comment`] entries.
/// It is intended for source-rewriting tools that must preserve every byte of
/// the original file (whitespace, comments, newlines).
///
/// A final [`Token::Eof`] sentinel is always appended.
pub fn lex_preserving(source: &str) -> Vec<Spanned> {
    let mut result: Vec<Spanned> = Token::lexer(source)
        .spanned()
        .filter_map(|(result, span)| match result {
            Ok(token) => Some(Spanned { token, span }),
            Err(_) => None, // skip unrecognized characters (still preserves via gap-fill)
        })
        .collect();

    let eof_pos = source.len();
    result.push(Spanned {
        token: Token::Eof,
        span: eof_pos..eof_pos,
    });

    result
}

/// Lex source code into a flat vector of spanned tokens.
///
/// Block structure is delimited by `{` / `}` tokens — **no** synthetic
/// `Indent`/`Dedent` tokens are emitted. Comments are stripped. A final
/// [`Token::Eof`] sentinel is always appended.
pub fn lex(source: &str) -> Vec<Spanned> {
    let mut result: Vec<Spanned> = Token::lexer(source)
        .spanned()
        .filter_map(|(result, span)| match result {
            Ok(token) => {
                if matches!(token, Token::Comment) {
                    None // strip comments
                } else {
                    Some(Spanned { token, span })
                }
            }
            Err(_) => None, // skip unrecognized characters
        })
        .collect();

    let eof_pos = source.len();
    result.push(Spanned {
        token: Token::Eof,
        span: eof_pos..eof_pos,
    });

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::Token;

    fn lex_tokens(source: &str) -> Vec<Token> {
        lex(source).into_iter().map(|s| s.token).collect()
    }

    #[test]
    fn lexes_versioned_and_tracked_decorators() {
        let toks = lex_tokens("@versioned @tracked");
        assert!(
            toks.contains(&Token::AtVersioned),
            "expected AtVersioned, got {toks:?}"
        );
        assert!(
            toks.contains(&Token::AtTracked),
            "expected AtTracked, got {toks:?}"
        );
    }

    #[test]
    fn lexes_at_traced() {
        let toks = lex_tokens("@traced fn f() to int { return 1 }");
        assert!(
            toks.iter().any(|t| matches!(t, Token::AtTraced)),
            "@traced must lex to AtTraced"
        );
    }

    #[test]
    fn test_simple_let_binding() {
        let tokens = lex_tokens("let x = 5");
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("x".into()),
                Token::Eq,
                Token::IntLit(5),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_keywords() {
        let tokens = lex_tokens("fn let mut if else match for in to return type import");
        let expected = vec![
            Token::Fn,
            Token::Let,
            Token::Mut,
            Token::If,
            Token::Else,
            Token::Match,
            Token::For,
            Token::In,
            Token::To,
            Token::Return,
            Token::TypeKw,
            Token::Import,
            Token::Eof,
        ];
        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_phonetic_operators() {
        let tokens = lex_tokens("and or not is isnt true false");
        let expected = vec![
            Token::And,
            Token::Or,
            Token::Not,
            Token::Is,
            Token::Isnt,
            Token::True,
            Token::False,
            Token::Eof,
        ];
        assert_eq!(tokens, expected);
    }

    /// `!` must lex as `Token::BangInvalid` (a distinct token whose only
    /// purpose is to let the parser emit a clear "use `not`" error). Prior
    /// to this token, the bare `!` was silently dropped by the lexer,
    /// causing `if !x` to parse as `if x` — a silent wrong-output bug.
    /// Vox commits to phonetic operators only (`not`, `and`, `or`,
    /// `is`, `isnt`) per the K-complexity / phonetic-aesthetic decision
    /// in docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md §8.
    #[test]
    fn test_bang_lexes_as_invalid_distinct_from_not() {
        let tokens = lex_tokens("!true");
        assert_eq!(
            tokens,
            vec![Token::BangInvalid, Token::True, Token::Eof],
            "`!` must lex as Token::BangInvalid so the parser can emit a clear error"
        );
    }

    /// `@query` and `@mutation` are first-class decorators replacing the
    /// awkward `@endpoint(kind: query)` / `@endpoint(kind: mutation)`
    /// spellings — ~65 % K-complexity reduction per call site (audit doc
    /// §11.2). The legacy `@endpoint` token was retired in v0.6.0 per
    /// `vox-stdlib-gap-audit-2026-05-23.md §Phase H step 18`; `@endpoint`
    /// text no longer lexes as a known decorator and the parser reports
    /// it as an unknown token (the `retired/decorator-usage` lint surfaces
    /// a friendly migration suggestion before that point).
    #[test]
    fn test_query_and_mutation_lex_as_distinct_tokens() {
        let tokens = lex_tokens("@query @mutation @server");
        assert_eq!(
            tokens,
            vec![
                Token::AtQuery,
                Token::AtMutation,
                Token::AtServer,
                Token::Eof,
            ],
        );
    }

    /// `!=` must still lex as `NotEq` (one token), not `!` + `=`.
    /// Logos longest-match makes this work, but lock it in with a test.
    #[test]
    fn test_bang_eq_still_one_token() {
        let tokens = lex_tokens("a != b");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("a".into()),
                Token::NotEq,
                Token::Ident("b".into()),
                Token::Eof,
            ],
        );
    }

    #[test]
    fn test_string_literals() {
        let tokens = lex_tokens(r#""hello world""#);
        assert_eq!(
            tokens,
            vec![Token::StringLit("hello world".into()), Token::Eof]
        );
    }

    #[test]
    fn test_string_literal_with_backticks_and_colon() {
        // Regression: markdown-style `…` inside strings must not break lexing (match arms, prints).
        let tokens = lex_tokens(r#""  [err] `cargo clean` failed to spawn: ""#);
        assert_eq!(
            tokens,
            vec![
                Token::StringLit("  [err] `cargo clean` failed to spawn: ".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_numeric_literals() {
        let tokens = lex_tokens("42 2.75");
        assert_eq!(
            tokens,
            vec![Token::IntLit(42), Token::FloatLit(2.75), Token::Eof]
        );
    }

    #[test]
    fn test_identifiers() {
        let tokens = lex_tokens("foo bar_baz MyType Result");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("foo".into()),
                Token::Ident("bar_baz".into()),
                Token::TypeIdent("MyType".into()),
                Token::TypeIdent("Result".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_decorators() {
        let tokens = lex_tokens("@component @tool @mcp.tool @resource @mcp.resource @native");
        assert_eq!(
            tokens,
            vec![
                Token::AtComponent,
                Token::AtTool,
                Token::AtMcpTool,
                Token::AtResource,
                Token::AtMcpResource,
                Token::AtNative,
                Token::Eof
            ]
        );
    }

    #[test]
    fn test_pure_scheduled_deprecated_tokens() {
        let tokens = lex_tokens("@pure @scheduled @deprecated");
        assert_eq!(
            tokens,
            vec![
                Token::AtPure,
                Token::AtScheduled,
                Token::AtDeprecated,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_symbols() {
        let tokens = lex_tokens("( ) [ ] { } : ? , . = -> |> | < >");
        let expected = vec![
            Token::LParen,
            Token::RParen,
            Token::LBracket,
            Token::RBracket,
            Token::LBrace,
            Token::RBrace,
            Token::Colon,
            Token::Question,
            Token::Comma,
            Token::Dot,
            Token::Eq,
            Token::Arrow,
            Token::PipeOp,
            Token::Bar,
            Token::Lt,
            Token::Gt,
            Token::Eof,
        ];
        assert_eq!(tokens, expected);
    }

    /// Brace syntax: no Indent/Dedent in output — braces carry block structure.
    #[test]
    fn test_brace_block_no_indent_tokens() {
        let source = "fn foo() to int { return 5 }";
        let tokens = lex_tokens(source);
        assert!(
            tokens.contains(&Token::Eof),
            "lexer output must include trailing Eof"
        );
        // Fn, Ident(foo), LParen, RParen, To, Ident(int), LBrace, Return, IntLit(5), RBrace, Eof
        assert!(tokens.contains(&Token::LBrace), "Should have LBrace");
        assert!(tokens.contains(&Token::RBrace), "Should have RBrace");
        assert!(tokens.contains(&Token::Return), "Should have Return");
    }

    /// Newlines are now cosmetic — they are emitted but not structurally significant.
    #[test]
    fn test_newlines_emitted_but_not_structural() {
        let source = "fn foo() to int {\n    return 5\n}";
        let tokens = lex_tokens(source);
        // Should contain Newline tokens but no Indent/Dedent
        assert!(
            tokens.contains(&Token::Newline),
            "Newlines should be emitted"
        );
        assert!(tokens.contains(&Token::LBrace), "Should have LBrace");
        assert!(tokens.contains(&Token::RBrace), "Should have RBrace");
    }

    #[test]
    fn test_jsx_tokens() {
        let tokens = lex_tokens("<div></div>");
        assert!(tokens.contains(&Token::Lt));
        assert!(tokens.contains(&Token::JsxCloseStart));
    }

    #[test]
    fn test_component_decorator() {
        let tokens = lex_tokens("@component fn Chat() to Element {");
        assert_eq!(tokens[0], Token::AtComponent);
        assert_eq!(tokens[1], Token::Fn);
        assert_eq!(tokens[2], Token::TypeIdent("Chat".to_string()));
        // Opening brace is now the block delimiter
        assert!(tokens.contains(&Token::LBrace));
    }

    #[test]
    fn test_match_expression() {
        let source = "match x {\n    Ok(r) -> r\n    Error(e) -> e\n}";
        let tokens = lex_tokens(source);
        assert!(tokens.contains(&Token::Match));
        assert!(tokens.contains(&Token::Arrow));
        assert!(tokens.contains(&Token::LBrace));
        assert!(tokens.contains(&Token::RBrace));
    }

    #[test]
    fn test_http_route() {
        let tokens = lex_tokens("http post \"/api/chat\" to Result {");
        assert_eq!(tokens[0], Token::Http);
        assert_eq!(tokens[1], Token::Ident("post".into()));
    }

    #[test]
    fn test_double_slash_line_comment_skipped() {
        let source = "let x = 1 // trailing\nlet y = 2";
        let tokens = lex(source);
        let kinds: Vec<&Token> = tokens.iter().map(|s| &s.token).collect();
        assert!(
            !kinds.iter().any(|t| **t == Token::Comment),
            "Comment tokens should be filtered: {:?}",
            kinds
        );
        assert!(matches!(kinds.last(), Some(t) if **t == Token::Eof));
    }

    #[test]
    fn test_agent_environment_tokens() {
        let tokens = lex_tokens("agent env migrate");
        assert!(tokens.contains(&Token::Agent));
        assert!(tokens.contains(&Token::Env));
        assert!(tokens.contains(&Token::Migrate));
    }

    #[test]
    fn test_pipe_operator() {
        let tokens = lex_tokens("x |> transform |> render");
        let pipe_count = tokens.iter().filter(|t| **t == Token::PipeOp).count();
        assert_eq!(pipe_count, 2);
    }

    #[test]
    fn test_chatbot_tokenizes() {
        // Brace-syntax chatbot source
        let source = r#"import react.use_state, network.HTTP, llm.Claude

@component fn Chat() to Element {
    let (msgs, set_msgs) = use_state([])
    let (input_val, set_input) = use_state("")
    fn send(_) to Unit {
        set_msgs(msgs.append({role: "user", text: input_val}))
        match HTTP.post("/api/chat", json={input: input_val}) {
            Ok(r) -> set_msgs(msgs.append({role: "ai", text: r.text}))
            Error(e) -> set_msgs(msgs.append({role: "error", text: e.message}))
        }
    }
    <div class="flex flex-col h-screen bg-gray-900 text-white">
        <button on_click={send}>"Send"</button>
    </div>
}

http post "/api/chat" to Result {
    return spawn(Claude).send(request.json().input)
}"#;

        let tokens = lex(source);
        assert!(!tokens.is_empty());
        assert_eq!(tokens.last().unwrap().token, Token::Eof);
        let token_types: Vec<&Token> = tokens.iter().map(|s| &s.token).collect();
        assert!(token_types.contains(&&Token::Import));
        assert!(token_types.contains(&&Token::AtComponent));
        assert!(token_types.contains(&&Token::Fn));
        assert!(token_types.contains(&&Token::Match));
        assert!(token_types.contains(&&Token::Http));
        assert!(token_types.contains(&&Token::Ident("post".into())));
        assert!(token_types.contains(&&Token::Spawn));
        assert!(token_types.contains(&&Token::LBrace));
        assert!(token_types.contains(&&Token::RBrace));
    }

    #[test]
    fn test_durable_execution_keywords() {
        let tokens = lex_tokens("activity with workflow");
        assert_eq!(
            tokens,
            vec![Token::Activity, Token::With, Token::Workflow, Token::Eof,]
        );
    }
}
