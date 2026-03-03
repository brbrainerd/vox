use crate::cursor::lex;
use crate::token::Token;

/// Compacts Vox source code to be more token-efficient for LLMs.
/// Removes comments, minimizes whitespace, and preserves only essential indentation.
pub fn compact(source: &str) -> String {
    let tokens = lex(source);
    let mut output = String::with_capacity(source.len());
    let mut last_token: Option<Token> = None;
    let mut indent_level = 0;
    let mut pending_newline = false;

    for spanned in tokens {
        let token = spanned.token;

        match &token {
            Token::Eof | Token::Comment => continue,
            Token::Indent => {
                indent_level += 1;
                continue;
            }
            Token::Dedent => {
                indent_level -= 1;
                continue;
            }
            Token::Newline => {
                pending_newline = true;
                continue;
            }
            _ => {}
        }

        if pending_newline {
            output.push('\n');
            output.push_str(&" ".repeat(indent_level));
            pending_newline = false;
            // When starting a new line, we don't need a space before the first token
            last_token = Some(Token::Newline);
        }

        // Handle spacing between tokens
        if let Some(last) = &last_token {
            if needs_space(last, &token) {
                output.push(' ');
            }
        }

        // Special case for StringLit to avoid escaping issues if not handled by Display
        // Token::to_string() for StringLit(s) is format!("\"{s}\"") which is correct.
        output.push_str(&token.to_string());
        last_token = Some(token);
    }

    output.trim().to_string()
}

/// Determines if a space is needed between two tokens to prevent them from merging.
fn needs_space(left: &Token, right: &Token) -> bool {
    // If last token was Newline or start of absolute line, no space needed
    if matches!(left, Token::Newline) {
        return false;
    }

    // Numbers followed by '.' (Dot) should probably have a space if it's not a float member access
    // But IntLit(10) followed by Dot(.) should be 10. (which might be a range or something)
    // Actually, in Vox, 10.foo is field access.

    let left_is_word = is_word(left);
    let right_is_word = is_word(right);

    // Keyword/Ident followed by Keyword/Ident needs space
    if left_is_word && right_is_word {
        return true;
    }

    // Number followed by a word (ident/keyword) needs space (e.g. "ret 10" -> "ret 10")
    if matches!(left, Token::IntLit(_) | Token::FloatLit(_)) && right_is_word {
        return true;
    }

    // Word followed by Number needs space (e.g. "x = 10")
    // Wait, "=" is not a word. "let x = 10"
    // "let x" -> space. "x =" -> no space. "= 10" -> no space.
    // So "let x=10" is fine.

    false
}

fn is_word(t: &Token) -> bool {
    match t {
        Token::Fn
        | Token::Let
        | Token::Mut
        | Token::If
        | Token::Else
        | Token::Match
        | Token::For
        | Token::In
        | Token::To
        | Token::Ret
        | Token::TypeKw
        | Token::Const
        | Token::Import
        | Token::Use
        | Token::Actor
        | Token::Workflow
        | Token::Activity
        | Token::Spawn
        | Token::Http
        | Token::Pub
        | Token::With
        | Token::On
        | Token::Trait
        | Token::Impl
        | Token::While
        | Token::Break
        | Token::Continue
        | Token::Try
        | Token::Catch
        | Token::Async
        | Token::Await
        | Token::Agent
        | Token::Stream
        | Token::Emit
        | Token::Message
        | Token::Version
        | Token::Migrate
        | Token::FromKw
        | Token::And
        | Token::Or
        | Token::Not
        | Token::Is
        | Token::Isnt
        | Token::True
        | Token::False
        | Token::Ident(_)
        | Token::TypeIdent(_) => true,
        // Numbers are handled separately in needs_space but count as "words" for spacing between them
        Token::IntLit(_) | Token::FloatLit(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_basic() {
        let src = "fn main():\n    let x = 10\n    ret x";
        let compacted = compact(src);
        println!("Compacted:\n'{}'", compacted);
        // Should look like: "fn main():\n let x=10\n ret x"
        assert!(compacted.contains("fn main():"));
        assert!(compacted.contains("\n let x=10"));
        assert!(compacted.contains("\n ret x"));
    }
}
