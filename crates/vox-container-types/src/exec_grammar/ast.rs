//! Core AST types and POSIX-shell tokeniser for parsed command invocations.

use serde::{Deserialize, Serialize};

/// A fully parsed command invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecAst {
    /// The primary executable or cmdlet name (e.g. `cargo`, `Get-ChildItem`).
    pub command: String,
    /// Positional arguments (non-flag tokens after the command).
    pub args: Vec<Arg>,
    /// Named flags / switches (e.g. `--release`, `-p foo`).
    pub flags: Vec<Flag>,
    /// I/O redirects (`>`, `>>`, `2>`, `|`).
    pub redirects: Vec<Redirect>,
    /// Assessed risk level (populated by the risk classifier after parsing).
    pub risk: super::RiskLevel,
}

/// A positional argument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arg(pub String);

/// A named flag with an optional value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flag {
    /// Flag name without leading dashes (e.g. `release`, `p`, `Recurse`).
    pub name: String,
    /// Value if the flag was written as `--flag=value` or `-f value`.
    pub value: Option<String>,
}

/// An I/O redirect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Redirect {
    pub kind: RedirectKind,
    /// Target filename or command name (for pipes).
    pub target: String,
}

/// The kind of redirect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RedirectKind {
    /// `>`  — stdout truncate
    Stdout,
    /// `>>` — stdout append
    StdoutAppend,
    /// `2>` — stderr truncate
    Stderr,
    /// `|`  — pipe to next command
    Pipe,
}

// ────────────────────────────── tokeniser ──────────────────────────────────

/// Tokenise `raw` into shell words, honouring single/double quoting and
/// backslash escapes.  Returns `Err` on unmatched quotes.
pub(crate) fn tokenise(raw: &str) -> Result<Vec<String>, super::ParseError> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = raw.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    // Set when an empty quoted string (`""` or `''`) is closed so the empty
    // token is preserved even if `current` is still empty at a word boundary.
    let mut had_quoted_empty = false;

    while let Some(ch) = chars.next() {
        match ch {
            // Backslash escape — only meaningful inside double-quotes (POSIX `\"`, `\\`).
            // Outside quotes, treat `\` as a literal character so Windows paths
            // like `C:\foo` are not mangled.
            '\\' if in_double => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            // Single-quote toggle
            '\'' if !in_double => {
                if in_single && current.is_empty() {
                    had_quoted_empty = true;
                }
                in_single = !in_single;
            }
            // Double-quote toggle
            '"' if !in_single => {
                if in_double && current.is_empty() {
                    had_quoted_empty = true;
                }
                in_double = !in_double;
            }
            // Redirect / pipe — only significant outside quotes
            '|' if !in_single && !in_double => {
                if !current.is_empty() || had_quoted_empty {
                    tokens.push(std::mem::take(&mut current));
                    had_quoted_empty = false;
                }
                tokens.push("|".to_owned());
            }
            '2' if !in_single && !in_double => {
                // Peek for `2>`
                if chars.peek() == Some(&'>') {
                    chars.next();
                    if !current.is_empty() || had_quoted_empty {
                        tokens.push(std::mem::take(&mut current));
                        had_quoted_empty = false;
                    }
                    tokens.push("2>".to_owned());
                } else {
                    current.push('2');
                }
            }
            '>' if !in_single && !in_double => {
                if !current.is_empty() || had_quoted_empty {
                    tokens.push(std::mem::take(&mut current));
                    had_quoted_empty = false;
                }
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(">>".to_owned());
                } else {
                    tokens.push(">".to_owned());
                }
            }
            // Whitespace — word boundary outside quotes
            c if c.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() || had_quoted_empty {
                    tokens.push(std::mem::take(&mut current));
                    had_quoted_empty = false;
                }
            }
            c => current.push(c),
        }
    }

    if in_single || in_double {
        return Err(super::ParseError::UnmatchedQuote(raw.to_owned()));
    }
    if !current.is_empty() || had_quoted_empty {
        tokens.push(current);
    }
    Ok(tokens)
}

/// Parse `raw` into an [`ExecAst`].  Risk level is left as `Unknown`; call
/// `super::risk::classify` to populate it.
pub(crate) fn parse_raw(raw: &str) -> Result<ExecAst, super::ParseError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(super::ParseError::Empty);
    }

    let tokens = tokenise(raw)?;
    let mut iter = tokens.into_iter().peekable();

    let command = iter.next().unwrap_or_default();
    let mut args: Vec<Arg> = Vec::new();
    let mut flags: Vec<Flag> = Vec::new();
    let mut redirects: Vec<Redirect> = Vec::new();

    let mut after_double_dash = false; // `--` stops flag parsing

    while let Some(token) = iter.next() {
        match token.as_str() {
            // Redirect operators
            "|" => {
                let target = iter.next().unwrap_or_default();
                redirects.push(Redirect {
                    kind: RedirectKind::Pipe,
                    target,
                });
            }
            ">" => {
                let target = iter.next().unwrap_or_default();
                redirects.push(Redirect {
                    kind: RedirectKind::Stdout,
                    target,
                });
            }
            ">>" => {
                let target = iter.next().unwrap_or_default();
                redirects.push(Redirect {
                    kind: RedirectKind::StdoutAppend,
                    target,
                });
            }
            "2>" => {
                let target = iter.next().unwrap_or_default();
                redirects.push(Redirect {
                    kind: RedirectKind::Stderr,
                    target,
                });
            }
            // Double-dash: everything after is positional
            "--" => {
                after_double_dash = true;
            }
            tok if !after_double_dash && tok.starts_with("--") => {
                // Long flag: --name or --name=value
                let body = &tok[2..];
                if let Some((name, val)) = body.split_once('=') {
                    flags.push(Flag {
                        name: name.to_owned(),
                        value: Some(val.to_owned()),
                    });
                } else {
                    // Peek: if next token is not a flag and not a redirect, treat as value
                    let value = match iter.peek() {
                        Some(next)
                            if !next.starts_with('-')
                                && !matches!(next.as_str(), "|" | ">" | ">>" | "2>") =>
                        {
                            Some(iter.next().unwrap())
                        }
                        _ => None,
                    };
                    flags.push(Flag {
                        name: body.to_owned(),
                        value,
                    });
                }
            }
            tok if !after_double_dash
                && tok.starts_with('-')
                && tok.len() > 1
                && !tok.starts_with("--") =>
            {
                // Short flags: -f, -fVALUE (POSIX attached), -abc (bundled), or -Parameter val (PowerShell).
                let body = &tok[1..];
                let mut body_chars = body.chars();
                let first = body_chars.next().unwrap_or_default();

                if body.len() == 1 {
                    // Single char: peek for a separate value token.
                    let value = match iter.peek() {
                        Some(next)
                            if !next.starts_with('-')
                                && !matches!(next.as_str(), "|" | ">" | ">>" | "2>") =>
                        {
                            Some(iter.next().unwrap())
                        }
                        _ => None,
                    };
                    flags.push(Flag {
                        name: body.to_owned(),
                        value,
                    });
                } else if first.is_ascii_lowercase() {
                    let rest: String = body_chars.collect();
                    if rest.chars().all(|c| c.is_ascii_lowercase()) {
                        // POSIX bundled lowercase flags: `-rf`, `-abc` → one flag each.
                        flags.push(Flag {
                            name: first.to_string(),
                            value: None,
                        });
                        for ch in rest.chars() {
                            flags.push(Flag {
                                name: ch.to_string(),
                                value: None,
                            });
                        }
                    } else {
                        // POSIX attached value: `-p22`, `-C/tmp`, `-oout.log` → name="p", value="22".
                        flags.push(Flag {
                            name: first.to_string(),
                            value: Some(rest),
                        });
                    }
                } else {
                    // PowerShell / GNU long-word style: -Recurse, -Path foo, -J4.
                    let value = match iter.peek() {
                        Some(next)
                            if !next.starts_with('-')
                                && !matches!(next.as_str(), "|" | ">" | ">>" | "2>") =>
                        {
                            Some(iter.next().unwrap())
                        }
                        _ => None,
                    };
                    flags.push(Flag {
                        name: body.to_owned(),
                        value,
                    });
                }
            }
            tok => {
                args.push(Arg(tok.to_owned()));
            }
        }
    }

    Ok(ExecAst {
        command,
        args,
        flags,
        redirects,
        risk: super::RiskLevel::Unknown,
    })
}

/// Split `raw` on unquoted command separators and return the trimmed segments.
///
/// Recognised separators (all treated equally for policy enforcement):
/// - `|` — pipe
/// - `||` — short-circuit OR
/// - `;` — sequential compose
/// - `&` — backgrounding
/// - `&&` — short-circuit AND
///
/// Respects single/double quoting so `echo "a && b"` is one segment.
fn split_on_command_separators(raw: &str) -> Result<Vec<String>, super::ParseError> {
    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            // Track quoting; preserve quote chars so parse_raw sees them.
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            // Two-char separators take priority over single-char.
            '&' if !in_single && !in_double => {
                if chars.peek() == Some(&'&') {
                    chars.next(); // consume the second '&'
                }
                segments.push(std::mem::take(&mut current));
            }
            '|' if !in_single && !in_double => {
                if chars.peek() == Some(&'|') {
                    chars.next(); // consume the second '|'
                }
                segments.push(std::mem::take(&mut current));
            }
            ';' if !in_single && !in_double => {
                segments.push(std::mem::take(&mut current));
            }
            c => current.push(c),
        }
    }
    if in_single || in_double {
        return Err(super::ParseError::UnmatchedQuote(raw.to_owned()));
    }
    segments.push(current);
    Ok(segments)
}

/// Parse `raw` as a pipeline or compound command — one [`ExecAst`] per segment.
///
/// Splits on unquoted `|` (pipe) and `;` (sequential compose).  Every segment
/// is evaluated independently by policy checks so `curl https://evil.com;
/// cargo build` and `curl https://evil.com | cargo build` are both fully checked.
pub(crate) fn parse_pipeline_raw(raw: &str) -> Result<Vec<super::ExecAst>, super::ParseError> {
    let segments = split_on_command_separators(raw)?;
    let mut result = Vec::new();
    for seg in segments {
        let trimmed = seg.trim();
        if !trimmed.is_empty() {
            result.push(parse_raw(trimmed)?);
        }
    }
    if result.is_empty() {
        return Err(super::ParseError::Empty);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_command() {
        let ast = parse_raw("cargo build --release").unwrap();
        assert_eq!(ast.command, "cargo");
        assert_eq!(ast.args, vec![Arg("build".into())]);
        assert_eq!(ast.flags[0].name, "release");
        assert!(ast.flags[0].value.is_none());
    }

    #[test]
    fn flag_with_value() {
        let ast = parse_raw("cargo test -p vox-cli").unwrap();
        assert_eq!(ast.command, "cargo");
        assert_eq!(ast.flags[0].name, "p");
        assert_eq!(ast.flags[0].value.as_deref(), Some("vox-cli"));
    }

    #[test]
    fn long_flag_equals() {
        let ast = parse_raw("git log --format=%H").unwrap();
        assert_eq!(ast.flags[0].name, "format");
        assert_eq!(ast.flags[0].value.as_deref(), Some("%H"));
    }

    #[test]
    fn redirect_stdout() {
        let ast = parse_raw("cargo build > build.log").unwrap();
        assert_eq!(ast.redirects.len(), 1);
        assert_eq!(ast.redirects[0].kind, RedirectKind::Stdout);
        assert_eq!(ast.redirects[0].target, "build.log");
    }

    #[test]
    fn pipe_detected() {
        let ast = parse_raw("Get-ChildItem | Select-Object Name").unwrap();
        assert_eq!(ast.redirects[0].kind, RedirectKind::Pipe);
        assert_eq!(ast.redirects[0].target, "Select-Object");
    }

    #[test]
    fn quoted_argument() {
        let ast = parse_raw(r#"echo "hello world""#).unwrap();
        assert_eq!(ast.args[0].0, "hello world");
    }

    #[test]
    fn unmatched_quote_error() {
        assert!(matches!(
            parse_raw(r#"echo "hello"#),
            Err(super::super::ParseError::UnmatchedQuote(_))
        ));
    }

    #[test]
    fn empty_error() {
        assert!(matches!(
            parse_raw("   "),
            Err(super::super::ParseError::Empty)
        ));
    }

    #[test]
    fn double_dash_stops_flag_parsing() {
        let ast = parse_raw("cargo run -- --not-a-flag").unwrap();
        assert!(ast.flags.is_empty());
        assert_eq!(ast.args[1].0, "--not-a-flag");
    }

    #[test]
    fn powershell_style_flag() {
        let ast = parse_raw("Get-ChildItem -Recurse -Path C:\\foo").unwrap();
        assert_eq!(ast.command, "Get-ChildItem");
        assert_eq!(ast.flags[0].name, "Recurse");
        assert_eq!(ast.flags[1].name, "Path");
        assert_eq!(ast.flags[1].value.as_deref(), Some("C:\\foo"));
    }

    #[test]
    fn posix_attached_value() {
        // `-p22` → name="p", value="22"
        let ast = parse_raw("ssh -p22 host").unwrap();
        assert_eq!(ast.flags[0].name, "p");
        assert_eq!(ast.flags[0].value.as_deref(), Some("22"));
    }

    #[test]
    fn posix_bundled_flags() {
        // `-rf` → two flags: r, f (no values)
        let ast = parse_raw("rm -rf dir").unwrap();
        assert_eq!(ast.flags.len(), 2);
        assert_eq!(ast.flags[0].name, "r");
        assert_eq!(ast.flags[1].name, "f");
    }

    #[test]
    fn empty_double_quoted_arg() {
        // `echo ""` → one empty arg
        let ast = parse_raw(r#"echo """#).unwrap();
        assert_eq!(ast.args.len(), 1);
        assert_eq!(ast.args[0].0, "");
    }

    #[test]
    fn empty_single_quoted_arg() {
        // `echo ''` → one empty arg
        let ast = parse_raw("echo ''").unwrap();
        assert_eq!(ast.args.len(), 1);
        assert_eq!(ast.args[0].0, "");
    }

    // ── pipeline tests ───────────────────────────────────────────────────────

    #[test]
    fn pipeline_two_segments() {
        let asts = parse_pipeline_raw("curl https://example.com | cargo build").unwrap();
        assert_eq!(asts.len(), 2);
        assert_eq!(asts[0].command, "curl");
        assert_eq!(asts[1].command, "cargo");
        assert_eq!(asts[1].args[0].0, "build");
    }

    #[test]
    fn pipeline_three_segments() {
        let asts = parse_pipeline_raw("cat file.txt | grep foo | wc -l").unwrap();
        assert_eq!(asts.len(), 3);
        assert_eq!(asts[0].command, "cat");
        assert_eq!(asts[1].command, "grep");
        assert_eq!(asts[2].command, "wc");
    }

    #[test]
    fn pipeline_quoted_pipe_not_split() {
        // A pipe inside quotes must NOT split the pipeline.
        let asts = parse_pipeline_raw(r#"echo "a | b""#).unwrap();
        assert_eq!(asts.len(), 1);
        assert_eq!(asts[0].args[0].0, "a | b");
    }

    #[test]
    fn pipeline_single_command_is_one_segment() {
        let asts = parse_pipeline_raw("cargo build --release").unwrap();
        assert_eq!(asts.len(), 1);
        assert_eq!(asts[0].command, "cargo");
    }

    #[test]
    fn pipeline_unmatched_quote_error() {
        assert!(matches!(
            parse_pipeline_raw(r#"echo "hello | world"#),
            Err(super::super::ParseError::UnmatchedQuote(_))
        ));
    }

    // ── semicolon separator tests ─────────────────────────────────────────────

    #[test]
    fn semicolon_two_segments() {
        let asts = parse_pipeline_raw("cargo build; cargo test").unwrap();
        assert_eq!(asts.len(), 2);
        assert_eq!(asts[0].command, "cargo");
        assert_eq!(asts[1].command, "cargo");
        assert_eq!(asts[1].args[0].0, "test");
    }

    #[test]
    fn semicolon_and_pipe_mixed() {
        // `cmd1 | cmd2; cmd3` → three segments
        let asts = parse_pipeline_raw("curl https://a.com | grep ok; echo done").unwrap();
        assert_eq!(asts.len(), 3);
        assert_eq!(asts[0].command, "curl");
        assert_eq!(asts[1].command, "grep");
        assert_eq!(asts[2].command, "echo");
    }

    #[test]
    fn semicolon_in_quotes_not_split() {
        let asts = parse_pipeline_raw(r#"echo "a; b""#).unwrap();
        assert_eq!(asts.len(), 1);
        assert_eq!(asts[0].args[0].0, "a; b");
    }

    // ── conditional / background separator tests ─────────────────────────────

    #[test]
    fn double_and_two_segments() {
        let asts = parse_pipeline_raw("cargo build && cargo test").unwrap();
        assert_eq!(asts.len(), 2);
        assert_eq!(asts[0].command, "cargo");
        assert_eq!(asts[1].command, "cargo");
        assert_eq!(asts[1].args[0].0, "test");
    }

    #[test]
    fn double_or_two_segments() {
        let asts = parse_pipeline_raw("cargo test || echo failed").unwrap();
        assert_eq!(asts.len(), 2);
        assert_eq!(asts[0].command, "cargo");
        assert_eq!(asts[1].command, "echo");
    }

    #[test]
    fn single_ampersand_backgrounds() {
        let asts = parse_pipeline_raw("long-running & follow-up").unwrap();
        assert_eq!(asts.len(), 2);
        assert_eq!(asts[0].command, "long-running");
        assert_eq!(asts[1].command, "follow-up");
    }

    #[test]
    fn mixed_separators_all_split() {
        // All five separators in one expression.
        let asts = parse_pipeline_raw("a | b && c; d || e & f").unwrap();
        assert_eq!(asts.len(), 6);
        let cmds: Vec<&str> = asts.iter().map(|a| a.command.as_str()).collect();
        assert_eq!(cmds, vec!["a", "b", "c", "d", "e", "f"]);
    }

    #[test]
    fn double_and_in_quotes_not_split() {
        let asts = parse_pipeline_raw(r#"echo "a && b""#).unwrap();
        assert_eq!(asts.len(), 1);
        assert_eq!(asts[0].args[0].0, "a && b");
    }

    #[test]
    fn double_or_in_quotes_not_split() {
        let asts = parse_pipeline_raw(r#"echo "a || b""#).unwrap();
        assert_eq!(asts.len(), 1);
        assert_eq!(asts[0].args[0].0, "a || b");
    }
}
