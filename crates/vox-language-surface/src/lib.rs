//! Single source of truth for **keyword** and **decorator** surface strings exposed
//! to the lexer-introspection layer, LSP, MCP, and grammar docs.
//!
//! This crate has **zero logic dependencies** so that both `vox-compiler` (which owns
//! the lexer/parser) and `vox-grammar-export` (which renders `GRAMMAR_SSOT.md`) can
//! depend on it without forming a cycle — `vox-compiler` depends on
//! `vox-grammar-export`, so the surface SSOT cannot live in `vox-compiler`.
//! `vox-compiler::language_surface` re-exports everything here.
//!
//! Keep lexer-derived lists aligned with `vox_compiler::lexer::token::Token`.

/// LSP keyword completions: `(label, snippet)`.
/// Includes parser-level keywords that may still lex as identifiers until dedicated tokens exist.
pub const LSP_KEYWORD_SNIPPETS: &[(&str, &str)] = &[
    ("fn", "fn $1($2) { \n\t$0 \n}"),
    ("let", "let $1 = $0"),
    ("mut", "mut $1 = $0"),
    ("if", "if $1 { \n\t$0 \n}"),
    ("else", "else { \n\t$0 \n}"),
    ("for", "for $1 in $2 { \n\t$0 \n}"),
    ("match", "match $1 { \n\t$2 => $0 \n}"),
    ("return", "return $0"),
    ("while", "while $1 { \n\t$0 \n}"),
    ("loop", "loop { \n\t$0 \n}"),
    ("break", "break"),
    ("continue", "continue"),
    ("type", "type $1 = $2"),
    ("import", "import \"$1\""),
    ("actor", "actor $1($2) { \n\t$0 \n}"),
    ("workflow", "workflow $1($2) { \n\t$0 \n}"),
    ("activity", "activity $1($2) { \n\t$0 \n}"),
    ("spawn", "spawn $0"),
    ("http", "http $1 { \n\t$0 \n}"),
    ("pub", "pub $0"),
    ("with", "with $1 { \n\t$0 \n}"),
    ("on", "on $1($2) { \n\t$0 \n}"),
    // ── Parser-only identifiers (lexed as Ident) ──
    ("struct", "struct $1 { \n\t$0 \n}"),
    ("enum", "enum $1 { \n\t$0 \n}"),
    ("trait", "trait $1 { \n\t$0 \n}"),
    ("impl", "impl $1 for $2 { \n\t$0 \n}"),
    ("const", "const $1 = $0"),
    ("message", "message $1($2)"),
    ("state", "state $1: $2"),
    ("routes", "routes { \n\t$0 \n}"),
    ("to", "to $0"),
    ("from", "from $0"),
    ("use", "use $0"),
];

/// Decorators with dedicated lexer tokens — `(spelling, LSP doc)`.
pub const LSP_DECORATOR_DOCS: &[(&str, &str)] = &[
    (
        "@loading",
        "Route suspense UI (`fn` → `*.tsx`); TanStack Router `pendingComponent` when `routes:` exists.",
    ),
    (
        "@server",
        "Server function (Axum route + TS client wrapper).",
    ),
    ("@table", "Declares a persistent database table."),
    ("@index", "Declares a database index."),
    ("@query", "Declares a database query function."),
    ("@mutation", "Declares a database mutation function."),
    ("@tool", "Exposes a function as an MCP tool."),
    (
        "@resource",
        "Read-only MCP resource (URI + description; nullary fn body).",
    ),
    ("@test", "Marks a function as a test case."),
    (
        "@example",
        "Marks a function as a reference example for corpus / docs tooling.",
    ),
    ("@placeholder", "Placeholder for generated UI hook."),
    (
        "@require",
        "Adds a runtime validation guard (precondition).",
    ),
    (
        "@ensure",
        "Adds a runtime validation guard (postcondition).",
    ),
    (
        "@invariant",
        "Adds a runtime validation guard evaluated on both bounds.",
    ),
    ("@forall", "Marks a test for property-based generation."),
    ("@fuzz", "Marks a test for fuzzing iteration bounds."),
    (
        "@pure",
        "Marks a function as side-effect free (optimization / tooling contracts).",
    ),
    (
        "@traced",
        "Emits a tracing span around the function body, seeded with the active trace-id from the distributed trace context.",
    ),
    (
        "@scheduled",
        "Declares a periodic job with an interval or cron string before `fn`.",
    ),
    (
        "@deprecated",
        "Marks a declaration as deprecated for diagnostics and documentation.",
    ),
    (
        "@ai",
        "Marks a function as being implemented by an LLM (agentic body).",
    ),
];

/// LSP decorator snippets: `(label, snippet)`.
pub const LSP_DECORATOR_SNIPPETS: &[(&str, &str)] = &[
    ("@loading", "@loading"),
    ("@server", "@server"),
    ("@table", "@table"),
    ("@index", "@index"),
    ("@query", "@query"),
    ("@mutation", "@mutation"),
    ("@tool", "@tool(\"${1:tool_name}\")"),
    ("@resource", "@resource(\"${1:uri}\")"),
    ("@test", "@test"),
    ("@placeholder", "@placeholder(\"${1:component_id}\")"),
    ("@require", "@require(${1:condition})"),
    ("@ensure", "@ensure(${1:condition})"),
    ("@invariant", "@invariant(${1:condition})"),
    ("@forall", "@forall"),
    ("@fuzz", "@fuzz"),
    ("@pure", "@pure"),
    ("@scheduled", "@scheduled(\"${1:cron}\")"),
    ("@deprecated", "@deprecated(\"${1:reason}\")"),
    (
        "@ai",
        "@ai(model=\"${1:gpt-4o}\") fn $2($3) to $4 {\n\t$0\n}",
    ),
];

// ── Keyword categories (the source of truth; `LEXER_KEYWORDS` is their concatenation).
//    Replaces the former magic `LEXER_KEYWORDS[..19] / [19..36] / [36..]` slices in the
//    grammar exporter — adding a keyword to a category can no longer silently mis-bucket. ──

/// Control-flow and binding keywords.
pub const CONTROL_FLOW_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "match", "for", "in", "to", "return", "while", "loop",
    "break", "continue",
];

/// Declaration / structural keywords.
pub const DECLARATION_KEYWORDS: &[&str] = &[
    "type", "import", "actor", "workflow", "activity", "spawn", "http", "pub", "with", "on",
    "state", "derived", "effect", "mount", "cleanup", "view", "component", "agent", "async",
    "migrate", "env", "dec",
];

/// Web / reactive / operator keywords, HTTP verbs, and the soft (contextual) data-layer
/// keywords (`table`/`index`/`query`/`mutation`/`server`/`tool`/`resource`) — the latter
/// have no logos `#[token]`; they remain `Token::Ident` and are recognized positionally
/// at declaration-head position only.
pub const WEB_REACTIVE_KEYWORDS: &[&str] = &[
    "and", "or", "not", "is", "true", "false", "get", "post", "put", "delete", "table", "index",
    "query", "mutation", "server", "tool", "resource",
];

/// Keywords that have dedicated single-word lexer tokens (speech / strict introspection).
/// The concatenation of [`CONTROL_FLOW_KEYWORDS`], [`DECLARATION_KEYWORDS`], and
/// [`WEB_REACTIVE_KEYWORDS`] — guarded by [`tests::categories_partition_lexer_keywords`].
pub const LEXER_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "match", "for", "in", "to", "return", "while", "loop",
    "break", "continue", "type", "import", "actor", "workflow", "activity", "spawn", "http", "pub",
    "with", "on", "state", "derived", "effect", "mount", "cleanup", "view", "component", "agent",
    "async", "migrate", "env", "dec", "and", "or", "not", "is", "true", "false", "get", "post",
    "put", "delete", "table", "index", "query", "mutation", "server", "tool", "resource",
];

/// `@decorator` spellings from the lexer (stable order).
pub const LEXER_DECORATORS: &[&str] = &[
    "@deprecated",
    "@tool",
    "@resource",
    "@pure",
    "@traced",
    "@require",
    "@scheduled",
    "@ensure",
    "@invariant",
    "@forall",
    "@fuzz",
    "@test",
    "@example",
    "@server",
    "@query",
    "@mutation",
    "@table",
    "@index",
    "@placeholder",
    "@place",
    "@loading",
    "@ai",
];

/// Every `@` decorator with a dedicated lexer `Token::At*` variant (56 — mirrors
/// `vox_compiler::feature_matrix::DecoratorFeature::ALL`).
pub const LEXER_AT_DECORATORS: &[&str] = &[
    "@component",
    "@tool",
    "@mcp.tool",
    "@resource",
    "@mcp.resource",
    "@test",
    "@example",
    "@query",
    "@mutation",
    "@server",
    "@json_as",
    "@field_name",
    "@default",
    "@skip_if_none",
    "@table",
    "@index",
    "@place",
    "@loading",
    "@require",
    "@ensure",
    "@invariant",
    "@forall",
    "@fuzz",
    "@pure",
    "@reactive",
    "@versioned",
    "@tracked",
    "@scheduled",
    "@deprecated",
    "@v0",
    "@ai",
    "@prompt",
    "@subagent",
    "@search",
    "@hole",
    "@cancellable",
    "@form",
    "@back_button",
    "@deep_link",
    "@push",
    "@tokens",
    "@cors",
    "@rate_limit",
    "@uses",
    "@pii",
    "@embed",
    "@webhook",
    "@public",
    "@auth",
    "@offline_capable",
    "@collaborative",
    "@layer",
    "@remote",
    "@inference",
    "@training_step",
    "@distributed_train",
];

/// Keywords that are deprecated and will be removed in a future version.
/// LSP and introspection tools should avoid suggesting these.
pub const LEXER_DEPRECATED_KEYWORDS: &[&str] = &["ret", "isnt", "environment"];

/// Decorators that are retired and produce hard compiler errors.
/// LSP and introspection tools should avoid suggesting these.
pub const LEXER_DEPRECATED_DECORATORS: &[&str] = &[
    "@component",
    "@mcp.tool",
    "@mcp.resource",
    "@mobile.native",
    "@v0",
    "@llm",
    // Tier-1 decorators demoted to soft keywords (warning-first; hard error at flip).
    // They stay in DecoratorFeature::ALL and LEXER_AT_DECORATORS (parity lever) — only
    // listed here to suppress LSP suggestion and mark the deprecation.
    "@table",
    "@index",
    "@query",
    "@mutation",
    "@server",
    "@tool",
    "@resource",
];

/// Builtin names for LSP / MCP “surface” introspection (aligned with common runtime helpers).
pub const SURFACE_BUILTIN_NAMES: &[&str] = &[
    "print", "len", "push", "pop", "now", "sleep", "hash", "uuid", "random", "assert",
];

/// LSP builtin snippets: `(name, insertText)`.
pub const LSP_BUILTIN_SNIPPETS: &[(&str, &str)] = &[
    ("print", "print($0)"),
    ("len", "len($0)"),
    ("push", "push($1, $0)"),
    ("pop", "pop($0)"),
    ("now", "now()"),
    ("sleep", "sleep($1)"),
    ("hash", "hash($0)"),
    ("uuid", "uuid()"),
    ("random", "random()"),
    ("assert", "assert($0)"),
];

/// Surface type names shown in completions / introspection.
pub const SURFACE_TYPE_NAMES: &[&str] = &[
    "int",
    "str",
    "bool",
    "float",
    "Unit",
    "Element",
    "List[T]",
    "Map[K, V]",
    "Set[T]",
    "Result[T, E]",
    "Option[T]",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The category arrays must partition `LEXER_KEYWORDS` exactly and in order — this
    /// is what lets the grammar exporter render the three doc sections from named arrays
    /// instead of magic slice indices, with no risk of drift between the two views.
    #[test]
    fn categories_partition_lexer_keywords() {
        let concatenated: Vec<&str> = CONTROL_FLOW_KEYWORDS
            .iter()
            .chain(DECLARATION_KEYWORDS)
            .chain(WEB_REACTIVE_KEYWORDS)
            .copied()
            .collect();
        assert_eq!(
            concatenated, LEXER_KEYWORDS,
            "CONTROL_FLOW + DECLARATION + WEB_REACTIVE must equal LEXER_KEYWORDS exactly"
        );
    }
}
