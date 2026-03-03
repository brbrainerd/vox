use tower_lsp_server::ls_types::SemanticTokenType;
use vox_lexer::token::Token;

/// Maps a Vox lexer token to an LSP SemanticTokenType.
/// This allows for syntax-aware highlighting in the IDE.
pub fn token_to_semantic_type(token: &Token) -> Option<u32> {
    match token {
        Token::Fn
        | Token::Let
        | Token::Mut
        | Token::Ret
        | Token::If
        | Token::Else
        | Token::For
        | Token::While
        | Token::Match
        | Token::TypeKw
        | Token::Actor
        | Token::Workflow
        | Token::Activity
        | Token::Const
        | Token::Import
        | Token::FromKw
        | Token::Use
        | Token::As
        | Token::True
        | Token::False
        | Token::And
        | Token::Or
        | Token::Not
        | Token::Async
        | Token::Await
        | Token::Spawn
        | Token::On
        | Token::To => Some(2), // KEYWORD

        Token::IntLit(_) | Token::FloatLit(_) => Some(5), // NUMBER
        Token::StringLit(_) | Token::SingleQuoteStringLit(_) => Some(4), // STRING
        Token::Comment => Some(6), // COMMENT

        Token::AtComponent
        | Token::AtTable
        | Token::AtCollection
        | Token::AtIndex
        | Token::AtVectorIndex
        | Token::AtSearchIndex
        | Token::AtTest
        | Token::AtFixture
        | Token::AtMock
        | Token::AtDeprecated
        | Token::AtPure
        | Token::AtRequire
        | Token::AtTrace
        | Token::AtHealth
        | Token::AtMetric
        | Token::AtQuery
        | Token::AtMutation
        | Token::AtAction
        | Token::AtMcpTool
        | Token::AtMcpResource
        | Token::AtScheduled
        | Token::AtServer
        | Token::AtAgentDef
        | Token::AtSkill
        | Token::AtLayout
        | Token::AtLoading
        | Token::AtNotFound
        | Token::AtErrorBoundary
        | Token::AtKeyframes
        | Token::AtTheme
        | Token::AtPyImport
        | Token::AtV0 => Some(7), // DECORATOR

        Token::Ident(_) => Some(1), // VARIABLE
        Token::TypeIdent(_) => Some(3), // TYPE

        _ => None,
    }
}

/// The list of semantic token types supported by the Vox LSP.
/// The index in this slice matches the number returned by `token_to_semantic_type`.
pub const SEMANTIC_TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,  // 0
    SemanticTokenType::VARIABLE,  // 1
    SemanticTokenType::KEYWORD,   // 2
    SemanticTokenType::TYPE,      // 3
    SemanticTokenType::STRING,    // 4
    SemanticTokenType::NUMBER,    // 5
    SemanticTokenType::COMMENT,   // 6
    SemanticTokenType::DECORATOR, // 7
    SemanticTokenType::PARAMETER, // 8
];
