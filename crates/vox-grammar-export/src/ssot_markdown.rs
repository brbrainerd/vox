//! Renders `tree-sitter-vox/GRAMMAR_SSOT.md` from the single source of truth in
//! [`vox_language_surface`]. There are no longer any hardcoded copies of the
//! keyword/decorator lists here — the category arrays ARE the source, so the doc can
//! never silently drift from the compiler's surface, and the former magic slice
//! indices (`[..19]`/`[19..36]`/`[36..]`) are gone.

use vox_language_surface::{
    CONTROL_FLOW_KEYWORDS, DECLARATION_KEYWORDS, LEXER_DECORATORS, WEB_REACTIVE_KEYWORDS,
};

pub fn emit_ssot_markdown() -> String {
    let mut g = String::with_capacity(4096);
    g.push_str("# Vox Grammar SSOT\n\n");
    g.push_str("This document defines the canonical vocabulary for the Vox programming language. Both `tree-sitter-vox` and `apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` must align with these tokens.\n\n");

    g.push_str("## Keywords\n\n");

    g.push_str("### Control Flow\n");
    g.push_str(&format!("`{}`\n\n", CONTROL_FLOW_KEYWORDS.join("`, `")));

    g.push_str("### Declaration\n");
    g.push_str(&format!("`{}`\n\n", DECLARATION_KEYWORDS.join("`, `")));

    g.push_str("### Web & Reactive (Path C)\n");
    g.push_str(&format!("`{}`\n\n", WEB_REACTIVE_KEYWORDS.join("`, `")));

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
