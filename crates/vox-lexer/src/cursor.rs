use logos::Logos;
use crate::token::Token;

/// A located token with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub span: std::ops::Range<usize>,
}

/// Lex source code into a flat vector of spanned tokens.
/// Handles indentation tracking: raw newlines from logos are processed
/// to emit synthetic Indent, Dedent, and Newline tokens based on
/// leading whitespace at each line start.
pub fn lex(source: &str) -> Vec<Spanned> {
    // Step 1: Run logos to get raw tokens
    let raw_tokens: Vec<Spanned> = Token::lexer(source)
        .spanned()
        .filter_map(|(result, span)| {
            match result {
                Ok(token) => {
                    // Skip comments
                    if matches!(token, Token::Comment) {
                        return None;
                    }
                    Some(Spanned { token, span })
                }
                Err(_) => None, // Skip unrecognized tokens
            }
        })
        .collect();

    // Step 2: Process indentation
    inject_indentation(source, raw_tokens)
}

/// Process raw tokens and inject Indent/Dedent/Newline tokens
/// based on leading whitespace at each line start.
fn inject_indentation(source: &str, raw_tokens: Vec<Spanned>) -> Vec<Spanned> {
    let mut result: Vec<Spanned> = Vec::new();
    let mut indent_stack: Vec<usize> = vec![0];
    let mut i = 0;

    while i < raw_tokens.len() {
        let spanned = &raw_tokens[i];

        if spanned.token == Token::RawNewline {
            // Find the indentation level of the next non-empty, non-comment line
            let newline_pos = spanned.span.end;

            // Count leading whitespace on the next line
            let next_indent = count_leading_whitespace(source, newline_pos);

            // Skip blank lines (lines that are just whitespace followed by another newline)
            let next_non_ws_pos = newline_pos + next_indent;
            if next_non_ws_pos < source.len() {
                let next_char = source.as_bytes().get(next_non_ws_pos);
                if next_char == Some(&b'\n') || next_char == Some(&b'\r') {
                    // Blank line — skip
                    i += 1;
                    continue;
                }
            }

            let current_indent = *indent_stack.last().unwrap();
            let span = spanned.span.clone();

            if next_indent > current_indent {
                // Deeper indentation → emit Indent
                indent_stack.push(next_indent);
                result.push(Spanned {
                    token: Token::Newline,
                    span: span.clone(),
                });
                result.push(Spanned {
                    token: Token::Indent,
                    span,
                });
            } else if next_indent < current_indent {
                // Shallower indentation → emit Dedent(s) then Newline
                while indent_stack.len() > 1 && *indent_stack.last().unwrap() > next_indent {
                    indent_stack.pop();
                    result.push(Spanned {
                        token: Token::Dedent,
                        span: span.clone(),
                    });
                }
                result.push(Spanned {
                    token: Token::Newline,
                    span,
                });
            } else {
                // Same level → emit Newline
                result.push(Spanned {
                    token: Token::Newline,
                    span,
                });
            }
        } else {
            result.push(spanned.clone());
        }
        i += 1;
    }

    // At EOF, close all remaining indentation levels
    let eof_pos = source.len();
    while indent_stack.len() > 1 {
        indent_stack.pop();
        result.push(Spanned {
            token: Token::Dedent,
            span: eof_pos..eof_pos,
        });
    }

    result.push(Spanned {
        token: Token::Eof,
        span: eof_pos..eof_pos,
    });

    result
}

/// Count leading whitespace characters (spaces) starting at the given byte offset.
fn count_leading_whitespace(source: &str, offset: usize) -> usize {
    let bytes = source.as_bytes();
    let mut count = 0;
    let mut pos = offset;
    while pos < bytes.len() {
        match bytes[pos] {
            b' ' => {
                count += 1;
                pos += 1;
            }
            b'\t' => {
                // Treat tabs as 4 spaces
                count += 4;
                pos += 1;
            }
            _ => break,
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::Token;

    fn lex_tokens(source: &str) -> Vec<Token> {
        lex(source).into_iter().map(|s| s.token).collect()
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
        let tokens = lex_tokens("fn let mut if else match for in to ret type import");
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
            Token::Ret,
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

    #[test]
    fn test_string_literals() {
        let tokens = lex_tokens(r#""hello world""#);
        assert_eq!(
            tokens,
            vec![Token::StringLit("hello world".into()), Token::Eof]
        );
    }

    #[test]
    fn test_single_quote_strings() {
        let tokens = lex_tokens("'user'");
        assert_eq!(
            tokens,
            vec![Token::SingleQuoteStringLit("user".into()), Token::Eof]
        );
    }

    #[test]
    fn test_numeric_literals() {
        let tokens = lex_tokens("42 3.14");
        assert_eq!(
            tokens,
            vec![Token::IntLit(42), Token::FloatLit(3.14), Token::Eof]
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
        let tokens = lex_tokens("@component @mcp.tool @external");
        assert_eq!(
            tokens,
            vec![Token::AtComponent, Token::AtMcpTool, Token::AtExternal, Token::Eof]
        );
    }

    #[test]
    fn test_symbols() {
        let tokens = lex_tokens("( ) [ ] { } : , . = -> |> | < >");
        let expected = vec![
            Token::LParen, Token::RParen,
            Token::LBracket, Token::RBracket,
            Token::LBrace, Token::RBrace,
            Token::Colon, Token::Comma, Token::Dot, Token::Eq,
            Token::Arrow, Token::PipeOp, Token::Bar,
            Token::Lt, Token::Gt,
            Token::Eof,
        ];
        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_indentation_basic() {
        let source = "fn foo() to int:\n    ret 5";
        let tokens = lex_tokens(source);
        assert_eq!(
            tokens,
            vec![
                Token::Fn,
                Token::Ident("foo".into()),
                Token::LParen,
                Token::RParen,
                Token::To,
                Token::Ident("int".into()),
                Token::Colon,
                Token::Newline,
                Token::Indent,
                Token::Ret,
                Token::IntLit(5),
                Token::Dedent,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_indentation_nested() {
        let source = "fn foo():\n    if true:\n        ret 1\n    ret 0";
        let tokens = lex_tokens(source);
        // fn foo ( ) : NL INDENT if true : NL INDENT ret 1 DEDENT NL ret 0 DEDENT EOF
        assert!(tokens.contains(&Token::Indent));
        // Should have 2 indents (one for fn body, one for if body)
        let indent_count = tokens.iter().filter(|t| **t == Token::Indent).count();
        assert_eq!(indent_count, 2);
        let dedent_count = tokens.iter().filter(|t| **t == Token::Dedent).count();
        assert_eq!(dedent_count, 2);
    }

    #[test]
    fn test_indentation_dedent_multiple() {
        let source = "a:\n    b:\n        c\nd";
        let tokens = lex_tokens(source);
        let dedent_count = tokens.iter().filter(|t| **t == Token::Dedent).count();
        assert_eq!(dedent_count, 2, "Should emit 2 dedents when jumping from indent 8 to 0");
    }

    #[test]
    fn test_jsx_tokens() {
        let tokens = lex_tokens("<div></div>");
        assert!(tokens.contains(&Token::Lt));
        assert!(tokens.contains(&Token::JsxCloseStart));
    }

    #[test]
    fn test_component_decorator() {
        let tokens = lex_tokens("@component fn Chat() to Element:");
        assert_eq!(tokens[0], Token::AtComponent);
        assert_eq!(tokens[1], Token::Fn);
        assert_eq!(tokens[2], Token::TypeIdent("Chat".to_string()));
    }

    #[test]
    fn test_match_expression() {
        let source = "match x:\n    Ok(r) -> r\n    Error(e) -> e";
        let tokens = lex_tokens(source);
        assert!(tokens.contains(&Token::Match));
        assert!(tokens.contains(&Token::Arrow));
    }

    #[test]
    fn test_http_route() {
        let tokens = lex_tokens("http post \"/api/chat\" to Result:");
        assert_eq!(tokens[0], Token::Http);
        assert_eq!(tokens[1], Token::Post);
    }

    #[test]
    fn test_pipe_operator() {
        let tokens = lex_tokens("x |> transform |> render");
        let pipe_count = tokens.iter().filter(|t| **t == Token::PipeOp).count();
        assert_eq!(pipe_count, 2);
    }

    #[test]
    fn test_chatbot_tokenizes() {
        let source = r#"import react.use_state, network.HTTP, llm.Claude

@component fn Chat() to Element:
    let (msgs, set_msgs) = use_state([])
    let (input_val, set_input) = use_state("")
    fn send(_) to Unit:
        set_msgs(msgs.append({role: "user", text: input_val}))
        match HTTP.post("/api/chat", json={input: input_val}):
            Ok(r) -> set_msgs(msgs.append({role: "ai", text: r.text}))
            Error(e) -> set_msgs(msgs.append({role: "error", text: e.message}))
    <div class="flex flex-col h-screen bg-gray-900 text-white">
        <div class="flex-1 overflow-y-auto p-4">
            for msg in msgs:
                <div class="mb-2 p-2 rounded">
                    {msg.text}
                </div>
        </div>
        <div class="flex p-4 border-t border-gray-700">
            <input class="flex-1 bg-gray-800 p-2 rounded-l" on_change={fn(e) to set_input(e.value)} value={input_val}/>
            <button class="bg-blue-600 hover:bg-blue-700 px-4 rounded-r" on_click={send}>"Send"</button>
        </div>
    </div>

http post "/api/chat" to Result:
    ret spawn(Claude).send(request.json().input)"#;

        let tokens = lex(source);
        // Should not have any errors (all tokens recognized)
        assert!(!tokens.is_empty());
        // Should end with EOF
        assert_eq!(tokens.last().unwrap().token, Token::Eof);
        // Should contain key tokens from the chatbot
        let token_types: Vec<&Token> = tokens.iter().map(|s| &s.token).collect();
        assert!(token_types.contains(&&Token::Import));
        assert!(token_types.contains(&&Token::AtComponent));
        assert!(token_types.contains(&&Token::Fn));
        assert!(token_types.contains(&&Token::Match));
        assert!(token_types.contains(&&Token::Http));
        assert!(token_types.contains(&&Token::Post));
        assert!(token_types.contains(&&Token::Spawn));
    }

    #[test]
    fn test_durable_execution_keywords() {
        let tokens = lex_tokens("activity with workflow");
        assert_eq!(
            tokens,
            vec![
                Token::Activity,
                Token::With,
                Token::Workflow,
                Token::Eof,
            ]
        );
    }
}
