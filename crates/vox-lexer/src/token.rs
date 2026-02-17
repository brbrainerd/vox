use logos::Logos;

/// All tokens in the Vox language.
/// Keywords are phonetically distinct English words.
/// Operators use English keywords (and, or, not, is, isnt) instead of symbols.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token {
    // ── Keywords ──────────────────────────────────────────────
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("match")]
    Match,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("to")]
    To,
    #[token("ret")]
    Ret,
    #[token("type")]
    TypeKw,
    #[token("import")]
    Import,
    #[token("actor")]
    Actor,
    #[token("workflow")]
    Workflow,
    #[token("activity")]
    Activity,
    #[token("spawn")]
    Spawn,
    #[token("http")]
    Http,
    #[token("pub")]
    Pub,
    #[token("with")]
    With,
    #[token("on")]
    On,

    // ── Phonetic Operators ────────────────────────────────────
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("is")]
    Is,
    #[token("isnt")]
    Isnt,
    #[token("true")]
    True,
    #[token("false")]
    False,

    // ── Decorators ────────────────────────────────────────────
    #[token("@component")]
    AtComponent,
    #[token("@mcp.tool")]
    AtMcpTool,
    #[token("@external")]
    AtExternal,
    #[token("@test")]
    AtTest,
    #[token("@server")]
    AtServer,
    #[token("@table")]
    AtTable,
    #[token("@index")]
    AtIndex,
    #[token("@v0")]
    AtV0,

    // ── HTTP Methods (contextual, used after `http` keyword) ─
    #[token("get")]
    Get,
    #[token("post")]
    Post,
    #[token("put")]
    Put,
    #[token("delete")]
    Delete,

    // ── Symbols ───────────────────────────────────────────────
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("=")]
    Eq,
    #[token("->")]
    Arrow,
    #[token("|>")]
    PipeOp,
    #[token("|")]
    Bar,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    Lte,
    #[token(">=")]
    Gte,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("_")]
    Underscore,

    // ── JSX-specific ──────────────────────────────────────────
    #[token("</")]
    JsxCloseStart,
    #[token("/>")]
    JsxSelfClose,

    // ── Literals ──────────────────────────────────────────────
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLit(f64),

    #[regex(r"[0-9]+", priority = 2, callback = |lex| lex.slice().parse::<i64>().ok())]
    IntLit(i64),

    #[regex(r#""([^"\\]|\\.)*""#, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    StringLit(String),

    #[regex(r#"'([^'\\]|\\.)*'"#, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    SingleQuoteStringLit(String),

    // ── Identifiers ───────────────────────────────────────────
    // Lower-case identifiers (variables, functions)
    #[regex(r"[a-z_][a-zA-Z0-9_]*", priority = 1, callback = |lex| lex.slice().to_string())]
    Ident(String),

    // Upper-case identifiers (types, constructors)
    #[regex(r"[A-Z][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    TypeIdent(String),

    // ── Comments ──────────────────────────────────────────────
    #[regex(r"#[^\r\n]*", allow_greedy = true)]
    Comment,

    // ── Newlines (captured, not skipped) ──────────────────────
    #[regex(r"\n|\r\n")]
    RawNewline,

    // ── Synthetic tokens (injected by indentation tracker) ────
    Newline,
    Indent,
    Dedent,
    Eof,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Fn => write!(f, "fn"),
            Token::Let => write!(f, "let"),
            Token::Mut => write!(f, "mut"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::Match => write!(f, "match"),
            Token::For => write!(f, "for"),
            Token::In => write!(f, "in"),
            Token::To => write!(f, "to"),
            Token::Ret => write!(f, "ret"),
            Token::TypeKw => write!(f, "type"),
            Token::Import => write!(f, "import"),
            Token::Actor => write!(f, "actor"),
            Token::Workflow => write!(f, "workflow"),
            Token::Activity => write!(f, "activity"),
            Token::Spawn => write!(f, "spawn"),
            Token::Http => write!(f, "http"),
            Token::Pub => write!(f, "pub"),
            Token::With => write!(f, "with"),
            Token::On => write!(f, "on"),
            Token::And => write!(f, "and"),
            Token::Or => write!(f, "or"),
            Token::Not => write!(f, "not"),
            Token::Is => write!(f, "is"),
            Token::Isnt => write!(f, "isnt"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::AtComponent => write!(f, "@component"),
            Token::AtMcpTool => write!(f, "@mcp.tool"),
            Token::AtExternal => write!(f, "@external"),
            Token::AtTest => write!(f, "@test"),
            Token::AtServer => write!(f, "@server"),
            Token::AtTable => write!(f, "@table"),
            Token::AtIndex => write!(f, "@index"),
            Token::AtV0 => write!(f, "@v0"),
            Token::Get => write!(f, "get"),
            Token::Post => write!(f, "post"),
            Token::Put => write!(f, "put"),
            Token::Delete => write!(f, "delete"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::Eq => write!(f, "="),
            Token::Arrow => write!(f, "->"),
            Token::PipeOp => write!(f, "|>"),
            Token::Bar => write!(f, "|"),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::Lte => write!(f, "<="),
            Token::Gte => write!(f, ">="),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Underscore => write!(f, "_"),
            Token::JsxCloseStart => write!(f, "</"),
            Token::JsxSelfClose => write!(f, "/>"),
            Token::IntLit(v) => write!(f, "{v}"),
            Token::FloatLit(v) => write!(f, "{v}"),
            Token::StringLit(s) => write!(f, "\"{s}\""),
            Token::SingleQuoteStringLit(s) => write!(f, "'{s}'"),
            Token::Ident(s) => write!(f, "{s}"),
            Token::TypeIdent(s) => write!(f, "{s}"),
            Token::Comment => write!(f, "<comment>"),
            Token::RawNewline => write!(f, "<rawNewline>"),
            Token::Newline => write!(f, "<newline>"),
            Token::Indent => write!(f, "<indent>"),
            Token::Dedent => write!(f, "<dedent>"),
            Token::Eof => write!(f, "<eof>"),
        }
    }
}
