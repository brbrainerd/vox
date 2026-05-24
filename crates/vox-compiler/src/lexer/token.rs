use logos::Logos;

/// All tokens in the Vox language.
/// Keywords are phonetically distinct English words.
/// Operators use English keywords (and, or, not, is, isnt) instead of symbols.
///
/// Block structure is delimited by `{` / `}` (`LBrace` / `RBrace`).
/// Indentation is cosmetic only; the lexer does **not** emit `Indent` or `Dedent` tokens.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")] // skip horizontal whitespace
pub enum Token {
    // ── Keywords ──────────────────────────────────────────────
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("async")]
    Async,
    #[token("mut")]
    Mut,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("loop")]
    Loop,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("match")]
    Match,
    /// `when` — view-arm discriminant keyword for `Async[T]` rendering (GA-01).
    #[token("when")]
    When,
    /// `fetching` — `Async[T]` view arm: data is in-flight (GA-01).
    #[token("fetching")]
    Fetching,
    /// `empty` — `Async[T]` view arm: data resolved to empty set (GA-01).
    #[token("empty")]
    Empty,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("to")]
    To,
    #[token("return")]
    Return,
    #[token("type")]
    TypeKw,
    #[token("dec")]
    Dec,
    #[token("import")]
    Import,
    #[token("extern")]
    Extern,
    #[token("actor")]
    Actor,
    #[token("workflow")]
    Workflow,
    #[token("activity")]
    Activity,
    /// `side_effect` — sanctioned non-determinism block inside a workflow (P1-T7).
    #[token("side_effect")]
    SideEffect,
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
    #[token("state")]
    State,
    #[token("derived")]
    Derived,
    #[token("effect")]
    Effect,
    #[token("mount")]
    Mount,
    #[token("cleanup")]
    Cleanup,
    #[token("view")]
    View,
    #[token("component")]
    Component,
    #[token("agent")]
    Agent,
    #[token("migrate")]
    Migrate,
    #[token("env")]
    Env,

    // ── Phonetic Operators ────────────────────────────────────
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    /// The bare `!` character. Vox uses phonetic operators (`not`, `and`, `or`,
    /// `is`, `isnt`), so `!` is NOT a valid negation operator. This token exists
    /// only so the parser can emit a clear error pointing the user at `not`.
    /// Prior to this token's existence, `!` was silently dropped by the lexer
    /// — see docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md §2.
    #[token("!")]
    BangInvalid,
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
    #[token("@tool")]
    AtTool,
    #[token("@mcp.tool")]
    AtMcpTool,
    #[token("@resource")]
    AtResource,
    #[token("@mcp.resource")]
    AtMcpResource,
    #[token("@test")]
    AtTest,
    #[token("@endpoint")]
    AtEndpoint,
    /// `@query` — first-class GET-style endpoint, replacing
    /// `@endpoint(kind: query)`. Lower K-complexity (~65 % per call site)
    /// and matches the conceptual hierarchy (the verb is the head, not the
    /// modifier). Added 2026-05-23 per
    /// `docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md` §11.2.
    /// `@endpoint(kind: query)` remains accepted during the deprecation
    /// window; corpus migration via `vox fmt` lands in a separate pass.
    #[token("@query")]
    AtQuery,
    /// `@mutation` — first-class POST/PUT/DELETE-style endpoint, replacing
    /// `@endpoint(kind: mutation)`. See `@query` above.
    #[token("@mutation")]
    AtMutation,
    /// `@server` — first-class server-only endpoint (no client emit),
    /// replacing `@endpoint(kind: server)`. Same K-complexity argument as
    /// `@query`/`@mutation`.
    #[token("@server")]
    AtServer,
    #[token("@table")]
    AtTable,
    #[token("@index")]
    AtIndex,
    #[token("@native")]
    AtNative,
    #[token("@loading")]
    AtLoading,
    #[token("@require")]
    AtRequire,
    #[token("@ensure")]
    AtEnsure,
    #[token("@invariant")]
    AtInvariant,
    #[token("@forall")]
    AtForall,
    #[token("@fuzz")]
    AtFuzz,
    #[token("@pure")]
    AtPure,
    /// `@reactive` — opt-in marker on a free `fn` declaring that its body's
    /// reactive-binding reads should be tracked across calls by the auto-dep
    /// inference pass (Phase E of the Svelte-mineable features plan).
    #[token("@reactive")]
    AtReactive,
    /// `fragment` — typed parametric markup primitive (ADR-033). Body shape
    /// mirrors `view:` (single markup expression). Parsed in Phase F slice 1;
    /// codegen gated on Phase 6 (TASK-6.1) typed primitive surface.
    #[token("fragment")]
    Fragment,
    #[token("@scheduled")]
    AtScheduled,
    #[token("@deprecated")]
    AtDeprecated,
    #[token("@v0")]
    AtV0,
    #[token("@ai")]
    AtAi,
    #[token("@prompt")]
    AtPrompt,
    #[token("@subagent")]
    AtSubagent,
    #[token("@search")]
    AtSearch,
    #[token("@hole")]
    AtHole,
    #[token("@cancellable")]
    AtCancellable,
    #[token("@form")]
    AtForm,
    #[token("@back_button")]
    AtBackButton,
    #[token("@deep_link")]
    AtDeepLink,
    #[token("@push")]
    AtPush,
    /// `@tokens` — project-level design-token block declaration (CC-23 / GA-20).
    #[token("@tokens")]
    AtTokens,
    /// `@cors` — CORS policy decorator on `@endpoint` (Phase 3 HTTP ergonomics / GA-06).
    #[token("@cors")]
    AtCors,
    /// `@rate_limit` — per-IP/key rate-limiting decorator (Phase 3 HTTP ergonomics / GA-06).
    #[token("@rate_limit")]
    AtRateLimit,
    /// `@uses` — effect annotation declaring I/O surfaces of a function (Phase 5 / GA-05).
    #[token("@uses")]
    AtUses,
    /// `@pii` — PII-taint marker on a type field or variable (GA-23).
    #[token("@pii")]
    AtPii,
    /// `@embed` — embedding-generation decorator on a `@table` field (CC-16 / GA-24).
    #[token("@embed")]
    AtEmbed,
    /// `@webhook` — verified-inbound-webhook decorator (CC-04 / GA-16).
    #[token("@webhook")]
    AtWebhook,
    /// `@auth` — OAuth/OIDC auth flow decorator (GA-04).
    #[token("@auth")]
    AtAuth,
    /// `@offline_capable` — service-worker / offline-first decorator (CC-22 / GA-15).
    #[token("@offline_capable")]
    AtOfflineCapable,
    /// `@collaborative` — CRDT-backed collaborative-editing decorator (CC-20 / GA-15).
    #[token("@collaborative")]
    AtCollaborative,
    /// `@layer` — VUV layered-layout tier decorator (GA-26).
    #[token("@layer")]
    AtLayer,
    /// `@remote` — marks a function for cross-node dispatch via the mesh (P1-T3).
    #[token("@remote")]
    AtRemote,
    /// `@inference` — MENS inference routing (Mn-T4).
    #[token("@inference")]
    AtInference,
    /// `@training_step` — one step of a CUDA training loop (Mn-T5).
    #[token("@training_step")]
    AtTrainingStep,
    /// `@distributed_train` — distributed training workflow preamble (Mn-T5).
    #[token("@distributed_train")]
    AtDistributedTrain,

    // ── Symbols ───────────────────────────────────────────────
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    /// Opens a block or an object literal.
    #[token("{")]
    LBrace,
    /// Closes a block or an object literal.
    #[token("}")]
    RBrace,
    #[token(":")]
    Colon,
    #[token("?")]
    Question,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("=")]
    Eq,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
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
    #[token("%")]
    Percent,
    #[token("_")]
    Underscore,

    // ── JSX-specific ──────────────────────────────────────────
    #[token("</")]
    JsxCloseStart,
    #[token("/>")]
    JsxSelfClose,
    /// Fragment open `<>` — shorthand for `<React.Fragment>`.
    #[token("<>")]
    JsxFragmentOpen,
    /// Fragment close `</>` — shorthand for `</React.Fragment>`.
    #[token("</>")]
    JsxFragmentClose,

    // ── Literals ──────────────────────────────────────────────
    #[regex(r"[0-9]+\.[0-9]+(dec)?", |lex| {
        let s = lex.slice();
        if s.ends_with("dec") {
            None // Handled by DecLit
        } else {
            s.parse::<f64>().ok()
        }
    })]
    FloatLit(f64),

    #[regex(r"[0-9]+(\.[0-9]+)?dec", |lex| {
        let s = lex.slice();
        Some(s[..s.len()-3].to_string())
    })]
    DecLit(String),

    #[regex(r"[0-9]+", priority = 2, callback = |lex| lex.slice().parse::<i64>().ok())]
    IntLit(i64),

    // Raw string literal: `r"..."` — no escape processing at all. Closing
    // delimiter is the first unescaped `"`; backslashes are preserved
    // verbatim. Use this for regex patterns, Windows paths, and any
    // string where `\n`/`\t`/`\"` shouldn't get interpreted.
    //
    // Priority 6 so `r"text"` is captured here before the `r` is mis-lexed
    // as an Ident followed by a StringLit. The pattern uses `[^"]*?` (lazy)
    // so the first `"` after the opening `"` closes the literal. For
    // strings containing a `"`, use the regular `"..."` form with `\"`.
    //
    // Added 2026-05-23 to unblock regex-heavy corpus scripts (Phase L.4
    // bucket: extract_table_names.vox, migrate-arrows.vox) that were
    // running their `(...)` capture groups into Vox's `{...}` template
    // regex friction.
    // Hash-padded raw strings — Rust-style `r#"..."#`, `r##"..."##`,
    // `r###"..."###`. Higher priority than the bare `r"..."` so the
    // explicit-delimiter forms are tried first. Bodies can embed `"` as
    // long as it isn't followed by the matching `#`-count. Three levels
    // cover every realistic use (regex with quotes, SQL with quotes, etc.);
    // Rust technically allows arbitrary `#` depth but practical code never
    // exceeds three. Added 2026-05-24 to support patterns like
    // `r#"\)\s*->\s*([A-Z][a-zA-Z0-9_\[\]]*)"#` that the bare form can't
    // express. See audit doc §Phase L.4.
    // For N=3: body allows `"` followed by 0–2 `#`s then non-`#`-non-`"`.
    #[regex(r#####"r###"(?:[^"]|"+([^#"]|#[^#"]|##[^#"]))*"+###"#####, priority = 8, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        Some(s[5..s.len()-4].to_string())
    })]
    // For N=2: body allows `"` followed by 0–1 `#` then non-`#`-non-`"`.
    #[regex(r#####"r##"(?:[^"]|"+([^#"]|#[^#"]))*"+##"#####, priority = 7, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        Some(s[4..s.len()-3].to_string())
    })]
    // For N=1: body allows `"` followed by non-`#`-non-`"`.
    #[regex(r####"r#"(?:[^"]|"+[^#"])*"+#"####, priority = 6, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        Some(s[3..s.len()-2].to_string())
    })]
    #[regex(r#"r"[^"]*""#, priority = 5, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        // Drop leading `r"` and trailing `"`.
        Some(s[2..s.len()-1].to_string())
    })]
    RawStringLit(String),

    // Double-quoted string: any run of non-quote / non-backslash bytes, or standard escapes.
    // Backticks (U+0060) are literal — markdown-style `cmd` fragments do not close the string.
    #[regex(r#""([^"\\]|\\.)*""#, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        let inner = &s[1..s.len()-1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n')  => out.push('\n'),
                    Some('t')  => out.push('\t'),
                    Some('r')  => out.push('\r'),
                    Some('\\') => out.push('\\'),
                    Some('"')  => out.push('"'),
                    Some('\'') => out.push('\''),
                    Some('0')  => out.push('\0'),
                    Some(c)    => { out.push('\\'); out.push(c); }
                    None       => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        Some(out)
    })]
    StringLit(String),

    // Template string: double-quoted string with {expr} interpolation.
    // The regex *requires* at least one `{...}` segment, so plain strings without
    // interpolation are matched by `StringLit` instead. (Earlier attempts let the
    // regex match any `"..."` and returned `None` for non-templates, but Logos
    // emits a lexer error on `None` rather than falling through to a lower-priority
    // pattern — which silently swallowed every plain string literal.)
    //
    // The `{...}` segment must START with an identifier character (letter or
    // underscore, optionally preceded by whitespace) — this excludes JSON
    // literals like `"{\"key\":1}"` which have `{` followed by `\"` / `"`.
    // Without this guard, every embedded JSON string was misclassified as a
    // template, producing "Complex expressions in template strings not yet
    // supported" parse errors. (RFC json-ergonomics-rfc-2026-05-23 §10
    // migration impact — discovered while migrating audit-dependency-layers.)
    #[regex(r#""([^"\\]|\\.)*\{\s*[a-zA-Z_]([^"\\]|\\.)*\}([^"\\]|\\.)*""#, priority = 5, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    TemplateStringLit(String),

    #[regex(r#"'([^'\\]|\\.)*'"#, allow_greedy = true, callback = |lex| {
        let s = lex.slice();
        let inner = &s[1..s.len()-1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n')  => out.push('\n'),
                    Some('t')  => out.push('\t'),
                    Some('r')  => out.push('\r'),
                    Some('\\') => out.push('\\'),
                    Some('"')  => out.push('"'),
                    Some('\'') => out.push('\''),
                    Some('0')  => out.push('\0'),
                    Some(c)    => { out.push('\\'); out.push(c); }
                    None       => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        Some(out)
    })]
    SingleStringLit(String),

    // ── Identifiers ───────────────────────────────────────────
    /// Lower-case identifiers (variables, functions).
    #[regex(r"[a-z_][a-zA-Z0-9_]*", priority = 1, callback = |lex| lex.slice().to_string())]
    Ident(String),

    /// Upper-case identifiers (types, constructors).
    #[regex(r"[A-Z][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    TypeIdent(String),

    // ── Comments ──────────────────────────────────────────────
    /// Line comments: `// …` (JS-style) and `# …` (shell / Vox fixture headers).
    #[regex(r"//[^\r\n]*|#[^\r\n]*", allow_greedy = true, priority = 3)]
    Comment,

    // ── Newlines ─────────────────────────────────────────────
    /// Newline character. Used as a statement separator inside blocks.
    /// Not structural (does not define block nesting — braces do).
    #[regex(r"\n|\r\n")]
    Newline,

    // ── Sentinel ─────────────────────────────────────────────
    /// End-of-file sentinel, injected by [`crate::lexer::cursor::lex`].
    Eof,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Async => write!(f, "async"),
            Token::Fn => write!(f, "fn"),
            Token::Let => write!(f, "let"),
            Token::Mut => write!(f, "mut"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::Match => write!(f, "match"),
            Token::When => write!(f, "when"),
            Token::Fetching => write!(f, "fetching"),
            Token::Empty => write!(f, "empty"),
            Token::For => write!(f, "for"),
            Token::In => write!(f, "in"),
            Token::To => write!(f, "to"),
            Token::Return => write!(f, "return"),
            Token::TypeKw => write!(f, "type"),
            Token::Dec => write!(f, "dec"),
            Token::Import => write!(f, "import"),
            Token::Extern => write!(f, "extern"),
            Token::Actor => write!(f, "actor"),
            Token::Workflow => write!(f, "workflow"),
            Token::Activity => write!(f, "activity"),
            Token::SideEffect => write!(f, "side_effect"),
            Token::Spawn => write!(f, "spawn"),
            Token::Http => write!(f, "http"),
            Token::Pub => write!(f, "pub"),
            Token::With => write!(f, "with"),
            Token::On => write!(f, "on"),
            Token::State => write!(f, "state"),
            Token::Derived => write!(f, "derived"),
            Token::Effect => write!(f, "effect"),
            Token::Mount => write!(f, "mount"),
            Token::Cleanup => write!(f, "cleanup"),
            Token::View => write!(f, "view"),
            Token::Component => write!(f, "component"),
            Token::Agent => write!(f, "agent"),
            Token::Migrate => write!(f, "migrate"),
            Token::Env => write!(f, "env"),
            Token::And => write!(f, "and"),
            Token::Or => write!(f, "or"),
            Token::Not => write!(f, "not"),
            Token::BangInvalid => write!(f, "!"),
            Token::AtQuery => write!(f, "@query"),
            Token::AtMutation => write!(f, "@mutation"),
            Token::AtServer => write!(f, "@server"),
            Token::Is => write!(f, "is"),
            Token::Isnt => write!(f, "isnt"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::AtComponent => write!(f, "@component"),
            Token::AtTool => write!(f, "@tool"),
            Token::AtMcpTool => write!(f, "@mcp.tool"),
            Token::AtResource => write!(f, "@resource"),
            Token::AtMcpResource => write!(f, "@mcp.resource"),
            Token::AtTest => write!(f, "@test"),
            Token::AtEndpoint => write!(f, "@endpoint"),
            Token::AtTable => write!(f, "@table"),
            Token::AtIndex => write!(f, "@index"),
            Token::AtNative => write!(f, "@native"),
            Token::AtLoading => write!(f, "@loading"),
            Token::AtRequire => write!(f, "@require"),
            Token::AtEnsure => write!(f, "@ensure"),
            Token::AtInvariant => write!(f, "@invariant"),
            Token::AtForall => write!(f, "@forall"),
            Token::AtFuzz => write!(f, "@fuzz"),
            Token::AtPure => write!(f, "@pure"),
            Token::AtReactive => write!(f, "@reactive"),
            Token::Fragment => write!(f, "fragment"),
            Token::AtScheduled => write!(f, "@scheduled"),
            Token::AtDeprecated => write!(f, "@deprecated"),
            Token::AtV0 => write!(f, "@v0"),
            Token::AtAi => write!(f, "@ai"),
            Token::AtPrompt => write!(f, "@prompt"),
            Token::AtSubagent => write!(f, "@subagent"),
            Token::AtSearch => write!(f, "@search"),
            Token::AtHole => write!(f, "@hole"),
            Token::AtCancellable => write!(f, "@cancellable"),
            Token::AtForm => write!(f, "@form"),
            Token::AtBackButton => write!(f, "@back_button"),
            Token::AtDeepLink => write!(f, "@deep_link"),
            Token::AtPush => write!(f, "@push"),
            Token::AtTokens => write!(f, "@tokens"),
            Token::AtCors => write!(f, "@cors"),
            Token::AtRateLimit => write!(f, "@rate_limit"),
            Token::AtUses => write!(f, "@uses"),
            Token::AtPii => write!(f, "@pii"),
            Token::AtEmbed => write!(f, "@embed"),
            Token::AtWebhook => write!(f, "@webhook"),
            Token::AtAuth => write!(f, "@auth"),
            Token::AtOfflineCapable => write!(f, "@offline_capable"),
            Token::AtCollaborative => write!(f, "@collaborative"),
            Token::AtLayer => write!(f, "@layer"),
            Token::AtRemote => write!(f, "@remote"),
            Token::AtInference => write!(f, "@inference"),
            Token::AtTrainingStep => write!(f, "@training_step"),
            Token::AtDistributedTrain => write!(f, "@distributed_train"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::Colon => write!(f, ":"),
            Token::Question => write!(f, "?"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::Eq => write!(f, "="),
            Token::EqEq => write!(f, "=="),
            Token::NotEq => write!(f, "!="),
            Token::PlusEq => write!(f, "+="),
            Token::MinusEq => write!(f, "-="),
            Token::StarEq => write!(f, "*="),
            Token::SlashEq => write!(f, "/="),
            Token::Arrow => write!(f, "->"),
            Token::FatArrow => write!(f, "=>"),
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
            Token::Percent => write!(f, "%"),
            Token::Underscore => write!(f, "_"),
            Token::JsxCloseStart => write!(f, "</"),
            Token::JsxSelfClose => write!(f, "/>"),
            Token::JsxFragmentOpen => write!(f, "<>"),
            Token::JsxFragmentClose => write!(f, "</>"),
            Token::IntLit(v) => write!(f, "{v}"),
            Token::FloatLit(v) => write!(f, "{v}"),
            Token::StringLit(s) => write!(f, "\"{s}\""),
            Token::TemplateStringLit(s) => write!(f, "\"{s}\""),
            Token::SingleStringLit(s) => write!(f, "'{s}'"),
            Token::RawStringLit(s) => write!(f, "r\"{s}\""),
            Token::DecLit(s) => write!(f, "{s}dec"),
            Token::Ident(s) => write!(f, "{s}"),
            Token::TypeIdent(s) => write!(f, "{s}"),
            Token::Comment => write!(f, "<comment>"),
            Token::Newline => write!(f, "<newline>"),
            Token::While => write!(f, "while"),
            Token::Loop => write!(f, "loop"),
            Token::Break => write!(f, "break"),
            Token::Continue => write!(f, "continue"),
            Token::Eof => write!(f, "<eof>"),
        }
    }
}
