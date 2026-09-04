//! Compose a runnable program from a candidate plus a fixture's assertions.
//!
//! Verified across all 164 fixtures (2026-09-01): every `tests.vox` has exactly
//! one `fn main(`, always last, with no declarations after it — so a suffix cut
//! is exact for the corpus as authored. The candidate side is NOT corpus-shaped,
//! so its `main` is excised by brace matching rather than a suffix cut: a v1
//! design that cut-to-EOF deleted helpers a model wrote after a demo `main`,
//! and deleted the entire solution when `main` came first.

use anyhow::{Result, bail};

/// The fixture's assertion block: from its `fn main(` line to EOF.
pub fn extract_test_block(tests_source: &str) -> Result<String> {
    if tests_source.starts_with("fn main(") {
        return Ok(tests_source.to_string());
    }
    match tests_source.find("\nfn main(") {
        Some(i) => Ok(tests_source[i + 1..].to_string()),
        None => bail!(
            "fixture tests.vox has no `fn main(` block — refusing to grade against an empty test"
        ),
    }
}

/// Remove only the candidate's own `fn main` block, preserving everything else.
///
/// Brace-matched (ignoring braces inside strings and line comments) rather than
/// cut-to-EOF, because helpers routinely follow a model's demo `main`.
#[must_use]
pub fn strip_candidate_main(candidate: &str) -> String {
    let Some(start) = find_main_start(candidate) else {
        return candidate.to_string();
    };
    let Some(open) = candidate[start..].find('{').map(|i| start + i) else {
        return candidate[..start].trim_end().to_string();
    };
    match match_brace(candidate, open) {
        Some(end) => format!("{}{}", &candidate[..start], &candidate[end + 1..])
            .lines()
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
        None => candidate[..start].trim_end().to_string(),
    }
}

fn find_main_start(s: &str) -> Option<usize> {
    if s.starts_with("fn main(") {
        return Some(0);
    }
    s.find("\nfn main(").map(|i| i + 1)
}

/// Index of the `}` closing the `{` at `open`, skipping strings and `//` comments.
fn match_brace(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    let (mut depth, mut i) = (0i32, open);
    let (mut in_str, mut in_cmt) = (false, false);
    while i < b.len() {
        let c = b[i] as char;
        if in_cmt {
            if c == '\n' {
                in_cmt = false;
            }
        } else if in_str {
            if c == '\\' {
                i += 1;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '/' && i + 1 < b.len() && b[i + 1] == b'/' {
            in_cmt = true;
        } else if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Candidate body plus the fixture's assertion block.
pub fn compose_program(candidate: &str, tests_source: &str) -> Result<String> {
    let body = strip_candidate_main(candidate);
    Ok(format!(
        "{}\n\n{}",
        body.trim_end(),
        extract_test_block(tests_source)?.trim_start()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTS: &str = "fn f(n: int) to int {\n    return 0\n}\n\nfn main() to str {\n    assert(f(1) == 2)\n    return \"ok\"\n}\n";

    #[test]
    fn extract_takes_main_and_drops_the_reference() {
        let b = extract_test_block(TESTS).unwrap();
        assert!(b.starts_with("fn main() to str {"));
        assert!(b.contains("assert(f(1) == 2)"));
        assert!(!b.contains("return 0"), "reference body must not survive");
    }

    #[test]
    fn extract_fails_closed_without_a_main() {
        assert!(extract_test_block("fn helper() to int { return 1 }").is_err());
    }

    #[test]
    fn strip_keeps_helpers_written_after_a_demo_main() {
        // v1 deleted `helper` here, turning a correct solution into a compile error.
        let c = "fn f() to int { return helper() }\n\nfn main() to str {\n    return \"demo\"\n}\n\nfn helper() to int {\n    return 7\n}\n";
        let s = strip_candidate_main(c);
        assert!(s.contains("fn f() to int"), "solution kept");
        assert!(
            s.contains("fn helper() to int"),
            "helper after main MUST survive"
        );
        assert!(!s.contains("fn main"), "demo main removed");
    }

    #[test]
    fn strip_keeps_the_solution_when_main_comes_first() {
        // v1 returned an empty string here — a guaranteed 0 for the fixture.
        let c = "fn main() to str {\n    return \"demo\"\n}\n\nfn f() to int {\n    return 1\n}\n";
        let s = strip_candidate_main(c);
        assert!(
            s.contains("fn f() to int"),
            "solution after a leading main MUST survive"
        );
        assert!(!s.contains("fn main"));
    }

    #[test]
    fn strip_is_a_noop_without_a_main() {
        let c = "fn f() to int { return 1 }\n";
        assert_eq!(strip_candidate_main(c).trim(), c.trim());
    }

    #[test]
    fn compose_yields_exactly_one_main() {
        let p = compose_program("fn f(n: int) to int { return 2 }", TESTS).unwrap();
        assert_eq!(p.matches("fn main").count(), 1);
        assert!(p.contains("assert(f(1) == 2)"));
        assert!(
            !p.contains("return 0"),
            "reference body never reaches the program"
        );
    }

    #[test]
    fn strip_handles_braces_inside_strings_and_comments() {
        // A `{` inside a string or comment must not confuse brace matching.
        let c = "fn f() to str {\n    return \"a { b\"\n}\n\nfn main() to str {\n    // trailing { comment\n    return \"demo\"\n}\n\nfn helper() to int {\n    return 1\n}\n";
        let s = strip_candidate_main(c);
        assert!(s.contains("fn f() to str"));
        assert!(s.contains("fn helper() to int"), "helper survives");
        assert!(!s.contains("fn main"));
    }
}
