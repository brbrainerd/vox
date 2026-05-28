//! Pretty-print / minimally format Vox source by round-tripping parse → string.
//!
//! - [`format()`] returns the original buffer when parsing fails (editor-safe soft mode).
//! - [`try_format`] is **fail-closed**: returns parse/round-trip errors for tooling (`vox fmt`).

mod expr;
mod printer;
mod stmt;

use crate::lexer::lex;
use crate::parser::{ParseError, parse};

/// Format `source` when it parses cleanly; otherwise return `source` unchanged.
pub fn format(source: &str) -> String {
    let tokens = lex(source);
    let module = match parse(tokens) {
        Ok(m) => m,
        Err(_) => return source.to_string(), // Incomplete/invalid source - return as-is
    };

    let mut printer = printer::Printer::new();
    printer.print_module(&module);
    printer.finish().trim_end().to_string() + "\n"
}

/// Parse, print, and **re-parse** output. Fails if the source is invalid or the printer loses validity.
pub fn try_format(source: &str) -> Result<String, Vec<ParseError>> {
    let tokens = lex(source);
    let module = parse(tokens)?;
    let mut printer = printer::Printer::new();
    printer.print_module(&module);
    let out = printer.finish().trim_end().to_string() + "\n";
    parse(lex(&out)).map(|_| ())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert formatting is idempotent: format(format(x)) == format(x)
    fn assert_idempotent(source: &str) {
        let once = format(source);
        let twice = format(&once);
        assert_eq!(once, twice, "Formatting is not idempotent for:\n{source}");
    }

    #[test]
    fn test_format_environment() {
        let source = r#"
@environment production:
    base: "node:22-alpine"
    workdir: "/app"
    packages: ["curl"]
    env:
        NODE_ENV: "production"
    expose: [3000]
    volumes: ["/data"]
    copy:
        "nginx.conf" to "/etc/nginx/nginx.conf"
    run: ["echo 'Building...'"]
    cmd: ["npm", "start"]
"#;
        let formatted = format(source);
        let expected = r#"@environment production:
    base: "node:22-alpine"
    workdir: "/app"
    packages: ["curl"]
    env:
        NODE_ENV: "production"
    expose: [3000]
    volumes: ["/data"]
    copy:
        "nginx.conf" to "/etc/nginx/nginx.conf"
    run: ["echo 'Building...'"]
    cmd: ["npm", "start"]
"#;
        assert_eq!(formatted.trim(), expected.trim());
    }

    #[test]
    fn idempotent_simple_fn() {
        assert_idempotent(
            "fn add(x: int, y: int) to int {
    return x + y
}\n",
        );
    }

    #[test]
    fn try_format_matches_soft_format_when_valid() {
        let src = "fn add(x: int, y: int) to int {\n    return x + y\n}\n";
        let soft = format(src);
        let strict = try_format(src).expect("try_format");
        assert_eq!(soft, strict);
    }

    #[test]
    fn idempotent_table_decl() {
        assert_idempotent(
            "@table type Note {
    title: str
    content: str
    created_at: str
}\n",
        );
    }

    #[test]
    fn idempotent_server_fn() {
        assert_idempotent(
            "@server fn greet(name: str) to str {
    return \"hello\"
}\n",
        );
    }

    #[test]
    fn idempotent_query_fn() {
        assert_idempotent(
            "@query fn list_items() to list[Item] {
    return []
}\n",
        );
    }

    #[test]
    fn idempotent_mutation_fn() {
        assert_idempotent(
            "@mutation fn add_item(name: str) to Result[str] {
    return Ok(name)
}\n",
        );
    }

    #[test]
    fn idempotent_import() {
        assert_idempotent("import react.use_state\n");
    }

    #[test]
    fn idempotent_const() {
        assert_idempotent("const MAX: int = 100\n");
    }

    #[test]
    fn idempotent_for_loop() {
        assert_idempotent(
            "fn process(items: list[str]) to int {
    for item in items {
        return 0
    }
    return 1
}\n",
        );
    }

    #[test]
    fn idempotent_workflow() {
        assert_idempotent(
            "workflow my_flow(input: str) to Result[str] {
    return Ok(input)
}\n",
        );
    }

    #[test]
    fn idempotent_actor() {
        assert_idempotent(
            "actor Counter {
    on increment(n: int) to int {
        return n
    }
}\n",
        );
    }

    #[test]
    fn idempotent_routes() {
        assert_idempotent(
            "routes {
    \"/\" to Home
    \"/about\" to About
}\n",
        );
    }

    #[test]
    fn table_uses_brace_syntax() {
        let out = format(
            "@table type User {
    name: str
    age: int
}\n",
        );
        assert!(
            out.contains("@table type User {"),
            "expected brace block, got: {out}"
        );
        assert!(out.contains('{'), "must use brace syntax, got: {out}");
    }

    #[test]
    fn format_invalid_source_returns_original() {
        let broken = "fn () { !!! }";
        let out = format(broken);
        assert_eq!(out, broken, "Invalid source should be returned unchanged");
    }
}
