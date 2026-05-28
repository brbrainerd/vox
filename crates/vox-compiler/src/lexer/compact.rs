use crate::lexer::cursor::lex;
use crate::lexer::token::Token;

/// Compacts Vox source code to be more token-efficient for LLMs.
///
/// With brace-delimited syntax, compaction is straightforward: remove comments
/// and collapse adjacent newlines/whitespace. The resulting output is valid
/// single-line Vox (braces carry block structure; whitespace is cosmetic).
///
/// # Example
///
/// ```text
/// use crate::lexer::compact::compact; // module is pub(crate)
/// let src = "fn greet(name: str) to str {\n    return \"Hello, \" + name\n}";
/// let out = compact(src);
/// assert!(!out.contains('\n'));
/// ```
pub fn compact(source: &str) -> String {
    let tokens = lex(source);
    let mut output = String::with_capacity(source.len());
    let mut last_token: Option<Token> = None;

    for spanned in tokens {
        let token = spanned.token;
        match &token {
            // Drop EOF and newlines — braces carry structure now.
            Token::Eof | Token::Newline => continue,
            _ => {}
        }

        // Handle spacing between tokens
        if let Some(last) = &last_token
            && needs_space(last, &token)
        {
            output.push(' ');
        }

        output.push_str(&token.to_string());
        last_token = Some(token);
    }

    output.trim().to_string()
}

/// Determines if a space is needed between two adjacent tokens.
fn needs_space(left: &Token, right: &Token) -> bool {
    let left_is_word = is_word(left);
    let right_is_word = is_word(right);

    // Keyword/Ident/Number followed by Keyword/Ident needs space
    if left_is_word && right_is_word {
        return true;
    }

    // Number followed by a word needs space (e.g. "return 10")
    if matches!(left, Token::IntLit(_) | Token::FloatLit(_)) && right_is_word {
        return true;
    }

    false
}

fn is_word(t: &Token) -> bool {
    matches!(
        t,
        Token::Fn
            | Token::Let
            | Token::Mut
            | Token::If
            | Token::Else
            | Token::Match
            | Token::For
            | Token::In
            | Token::To
            | Token::Return
            | Token::TypeKw
            | Token::Import
            | Token::Actor
            | Token::Workflow
            | Token::Activity
            | Token::Spawn
            | Token::Http
            | Token::Pub
            | Token::With
            | Token::On
            | Token::And
            | Token::Or
            | Token::Not
            | Token::Is
            | Token::True
            | Token::False
            | Token::AtComponent
            | Token::AtTool
            | Token::AtResource
            | Token::AtTest
            | Token::AtTable
            | Token::AtIndex
            | Token::AtPure
            | Token::AtScheduled
            | Token::AtDeprecated
            | Token::Ident(_)
            | Token::TypeIdent(_)
            | Token::IntLit(_)
            | Token::FloatLit(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    #[test]
    fn test_compaction_brace_syntax() {
        // Brace syntax: the multi-line form compacts to a single-line form
        let src = "fn main() {\n    let x = 10\n    return x\n}";
        let compacted = compact(src);
        // No newlines in output — single-line serialization achieved
        assert!(
            !compacted.contains('\n'),
            "Compacted output should be single-line"
        );
        assert!(
            compacted.contains("fn main()"),
            "Should preserve function name"
        );
        assert!(compacted.contains('{'), "Should preserve LBrace");
        assert!(compacted.contains('}'), "Should preserve RBrace");
        assert!(
            compacted.contains("return x"),
            "Should preserve return statement"
        );
    }

    #[test]
    fn test_compaction_preserves_braces() {
        let src = "fn f(x: int) to int { return x }";
        let compacted = compact(src);
        // No space before `{` (Ident→LBrace: needs_space=false)
        // No space before `return` (LBrace is not a word, return is: no space added)
        // No space before `x` or `}`
        assert!(compacted.contains("fn f"), "should have fn f");
        assert!(compacted.contains("to int"), "should have return type");
        assert!(compacted.contains('{'), "should have LBrace");
        assert!(compacted.contains('}'), "should have RBrace");
        assert!(compacted.contains("return x"), "should have return x");
        assert!(!compacted.contains('\n'), "should be single line");
    }

    #[test]
    fn test_compaction_strips_comments() {
        let src = "let x = 1 // trailing comment\nlet y = 2";
        let compacted = compact(src);
        assert!(
            !compacted.contains("trailing"),
            "Comment should be stripped"
        );
        assert!(compacted.contains("let x"), "let x should be present");
        assert!(compacted.contains("let y"), "let y should be present");
    }

    #[test]
    fn test_compaction_single_line_serialization() {
        // Demonstrates that brace syntax enables full single-line serialization
        // (essential for Mens training data minification and LLM token budget use)
        let src = r#"fn greet(name: str) to str {
    if name is "" {
        return "Hello, stranger"
    }
    return "Hello, " + name
}"#;
        let compacted = compact(src);
        assert!(
            !compacted.contains('\n'),
            "Full program should serialize to single line, got: {}",
            compacted
        );
        assert!(compacted.contains("fn greet"), "Should have function name");
        assert!(compacted.contains("if name"), "Should have if condition");
    }

    #[test]
    fn test_compaction_golden_output() {
        let src = "fn main() {\n    let x = 10\n    return x\n}";
        let compacted = compact(src);
        assert_eq!(compacted, "fn main(){let x=10 return x}");
    }

    #[test]
    fn test_compaction_roundtrip_parseable() {
        let src = r#"
fn sum(a: int, b: int) to int {
    return a + b
}
"#;
        let compacted = compact(src);
        let parsed = parse(lex(&compacted));
        assert!(
            parsed.is_ok(),
            "compacted output must parse cleanly, got: {}",
            compacted
        );
    }
}
