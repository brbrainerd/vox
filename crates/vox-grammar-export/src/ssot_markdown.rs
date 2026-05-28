// Hardcoded tokens to avoid circular dependency with vox-compiler.
// These MUST be kept in sync with vox-compiler/src/language_surface.rs.
const LEXER_KEYWORDS: &[&str] = &[
    "fn",
    "let",
    "mut",
    "if",
    "else",
    "match",
    "for",
    "in",
    "to",
    "return",
    "while",
    "loop",
    "break",
    "continue",
    "type",
    "import",
    "actor",
    "workflow",
    "activity",
    "spawn",
    "http",
    "pub",
    "with",
    "on",
    "state",
    "derived",
    "effect",
    "mount",
    "cleanup",
    "view",
    "component",
    "and",
    "or",
    "not",
    "is",
    "isnt",
    "true",
    "false",
    "get",
    "post",
    "put",
    "delete",
];

const LEXER_DECORATORS: &[&str] = &[
    "@deprecated",
    "@mcp.tool",
    "@mcp.resource",
    "@pure",
    "@require",
    "@scheduled",
    "@ensure",
    "@invariant",
    "@forall",
    "@fuzz",
    "@test",
    "@server",
    "@query",
    "@mutation",
    "@table",
    "@index",
    "@v0",
    "@mobile.native",
    "@loading",
];

pub fn emit_ssot_markdown() -> String {
    let mut g = String::with_capacity(4096);
    g.push_str("# Vox Grammar SSOT\n\n");
    g.push_str("This document defines the canonical vocabulary for the Vox programming language. Both `tree-sitter-vox` and `apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` must align with these tokens.\n\n");

    g.push_str("## Keywords\n\n");

    g.push_str("### Control Flow\n");
    g.push_str(&format!("`{}`\n\n", LEXER_KEYWORDS[..19].join("`, `")));

    g.push_str("### Declaration\n");
    g.push_str(&format!("`{}`\n\n", LEXER_KEYWORDS[19..36].join("`, `")));

    g.push_str("### Web & Reactive (Path C)\n");
    g.push_str(&format!("`{}`\n\n", LEXER_KEYWORDS[36..].join("`, `")));

    g.push_str("## Primitive Types\n");
    g.push_str("`int`, `str`, `bool`, `float`, `Unit`, `Element`\n\n");

    g.push_str("## Collection Types\n");
    g.push_str("`List[T]`, `Map[K, V]`, `Set[T]`, `Result[T, E]`, `Option[T]`\n\n");

    g.push_str("## Constants\n");
    g.push_str("`true`, `false`\n\n");

    g.push_str("## Decorators\n");
    g.push_str(&format!("`{}`\n\n", LEXER_DECORATORS.join("`, `")));

    g.push_str("## Operators\n");
    g.push_str("`->`, `|>`, `==`, `!=`, `<=`, `>=`, `<`, `>`, `=`, `+=`, `-=`, `*=`, `/=`, `+`, `-`, `*`, `/`, `%`\n\n");

    g.push_str("## Comments\n");
    g.push_str("- Single line: `//`\n");

    g
}
