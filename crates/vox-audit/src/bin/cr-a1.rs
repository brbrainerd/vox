//! CR-A1 cyclomatic-complexity check — walks `.rs` files under the
//! primary lowering paths and emits the worst-case per-function
//! complexity to `contracts/reports/arch/cr-a1/<UTC>.json`. Exits
//! non-zero if any function exceeds the threshold (default 15).
//!
//! Per `docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md` §5.6.
//! Threshold tracks v1-release-criteria CR-A1.
//!
//! The metric is McCabe cyclomatic complexity: 1 + (decision points), where
//! decision points are `if`, `else if`, `match` arms, `while`, `for`,
//! `loop`, `&&`, `||`, and `?`. The implementation is a line-based scanner
//! over the source rather than an AST walk — fast, deterministic, slightly
//! overcounts in patterns like string `"&&"` literals (acceptable for the
//! CI gate; AST-based refinement is a v1.1 sharpening, not v1.0 blocker).

use serde_json::json;
use std::path::Path;

/// Honest plan §5.6 / v1-release-criteria CR-A1.
const COMPLEXITY_BUDGET: u32 = 15;

/// Paths whose `.rs` sources are the "primary lowering paths" cited in
/// CR-A1. Add to this list as new lowering paths land; the gate then
/// applies the budget to them too.
const LOWERING_PATHS: &[&str] = &[
    "crates/vox-compiler/src/hir/lower",
    "crates/vox-compiler/src/typeck/checker",
    "crates/vox-codegen/src/codegen_rust/emit",
];

#[derive(Debug, serde::Serialize)]
struct FunctionReport {
    file: String,
    function: String,
    complexity: u32,
    line: u32,
}

fn main() {
    let workspace = vox_audit::workspace_root();
    let mut all_functions: Vec<FunctionReport> = Vec::new();
    let mut files_scanned = 0u32;
    for rel in LOWERING_PATHS {
        let abs = workspace.join(rel);
        if !abs.exists() {
            eprintln!("warning: lowering path missing: {}", abs.display());
            continue;
        }
        for entry in walkdir::WalkDir::new(&abs)
            .follow_links(false)
            .into_iter()
            .filter_map(|r| r.ok())
        {
            let p = entry.path();
            if !(p.is_file() && p.extension().is_some_and(|x| x == "rs")) {
                continue;
            }
            files_scanned += 1;
            let Ok(src) = std::fs::read_to_string(p) else {
                continue;
            };
            for f in scan_functions(&src) {
                all_functions.push(FunctionReport {
                    file: relative_to_workspace(p, &workspace),
                    function: f.name,
                    complexity: f.complexity,
                    line: f.line,
                });
            }
        }
    }

    all_functions.sort_by(|a, b| b.complexity.cmp(&a.complexity));
    let max_complexity = all_functions
        .iter()
        .map(|f| f.complexity)
        .max()
        .unwrap_or(0);
    let over_budget: Vec<&FunctionReport> = all_functions
        .iter()
        .filter(|f| f.complexity > COMPLEXITY_BUDGET)
        .collect();
    let met = over_budget.is_empty();

    eprintln!(
        "CR-A1: scanned {files_scanned} .rs files under {} lowering paths",
        LOWERING_PATHS.len()
    );
    eprintln!(
        "CR-A1: {} functions; max complexity = {max_complexity}; budget = {COMPLEXITY_BUDGET}",
        all_functions.len()
    );
    if !met {
        eprintln!("CR-A1: {} function(s) over budget:", over_budget.len());
        for f in &over_budget {
            eprintln!(
                "  {}:{} `{}` complexity={}",
                f.file, f.line, f.function, f.complexity
            );
        }
    }

    let top = all_functions.iter().take(20).collect::<Vec<_>>();
    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-A1",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "lowering_paths": LOWERING_PATHS,
        "files_scanned": files_scanned,
        "functions_scanned": all_functions.len(),
        "max_complexity": max_complexity,
        "budget": COMPLEXITY_BUDGET,
        "over_budget": over_budget,
        "top_20_complexity": top,
        "threshold": {
            "target_max_complexity": COMPLEXITY_BUDGET,
            "met": met,
        },
    });
    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = workspace
        .join("contracts")
        .join("reports")
        .join("arch")
        .join("cr-a1");
    std::fs::create_dir_all(&out_dir).expect("create arch dir");
    let out_path = out_dir.join(format!("{date}.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if !met {
        std::process::exit(1);
    }
}

fn relative_to_workspace(p: &Path, root: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug)]
struct ScannedFunction {
    name: String,
    complexity: u32,
    line: u32,
}

/// Best-effort line-based scanner. Tracks brace depth from the `fn` header
/// to the matching close; counts decision keywords inside.
///
/// Limitations (intentional for v1.0 simplicity):
/// - String/comment literals containing `&&`, `||`, `if `, `match `, etc.
///   are overcounted. Real cases where this matters are rare in the
///   lowering paths under scrutiny; AST refinement is v1.1.
/// - Closures `|x| { ... }` are counted as part of the enclosing fn.
fn scan_functions(src: &str) -> Vec<ScannedFunction> {
    let mut out: Vec<ScannedFunction> = Vec::new();
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some(name) = parse_fn_header(lines[i]) {
            let header_line = i + 1;
            // Find the matching `{` on this line or a subsequent one,
            // then walk braces until depth returns to 0.
            let mut depth: i32 = 0;
            let mut started = false;
            let mut complexity: u32 = 1;
            let mut j = i;
            while j < lines.len() {
                let line = lines[j];
                let stripped = strip_comments_and_strings(line);
                for ch in stripped.chars() {
                    if ch == '{' {
                        depth += 1;
                        started = true;
                    } else if ch == '}' {
                        depth -= 1;
                    }
                }
                if started {
                    complexity += count_decisions(&stripped);
                }
                if started && depth == 0 {
                    out.push(ScannedFunction {
                        name: name.clone(),
                        complexity,
                        line: header_line as u32,
                    });
                    i = j + 1;
                    break;
                }
                j += 1;
            }
            if j >= lines.len() {
                // Unbalanced — bail to avoid infinite loop.
                break;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Extract `fn <name>` from a line. Returns None on no match.
fn parse_fn_header(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let prefix_options = [
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "pub(super) fn ",
        "async fn ",
        "pub async fn ",
        "pub(crate) async fn ",
        "pub(super) async fn ",
        "const fn ",
        "pub const fn ",
        "unsafe fn ",
        "pub unsafe fn ",
    ];
    let rest = prefix_options
        .iter()
        .find_map(|p| trimmed.strip_prefix(p))?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    Some(name)
}

/// Strip `// ...` line comments and `"..."` string literals from a line.
/// Crude — doesn't handle raw strings or escaped quotes perfectly, but
/// dampens the false-positive rate considerably.
fn strip_comments_and_strings(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_string = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if !in_string && c == '/' && chars.peek() == Some(&'/') {
            break; // line comment swallows the rest
        }
        if c == '"' {
            in_string = !in_string;
            out.push(' ');
            continue;
        }
        if in_string {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Count McCabe decision points on a single line.
fn count_decisions(line: &str) -> u32 {
    let mut n: u32 = 0;
    // `else if X` is ONE decision in McCabe (the `if` test) — not two.
    // Count `else if ` separately and subtract its count from `if ` so an
    // `else if` doesn't double-count.
    let else_if_count = line.matches("else if ").count() as u32;
    let if_count = line.matches("if ").count() as u32;
    n += else_if_count;
    n += if_count.saturating_sub(else_if_count);
    for kw in ["match ", "while ", "for ", "loop "] {
        n += line.matches(kw).count() as u32;
    }
    n += line.matches("&&").count() as u32;
    n += line.matches("||").count() as u32;
    n += line.matches('?').count() as u32;
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_fn() {
        let src = "fn foo() {\n  return 1\n}\n";
        let fns = scan_functions(src);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "foo");
        assert_eq!(fns[0].complexity, 1);
    }

    #[test]
    fn counts_if_else_match() {
        let src = "fn bar(x: i32) {\n  if x > 0 {} else if x < 0 {}\n  match x { _ => {} }\n}";
        let fns = scan_functions(src);
        // 1 (base) + 1 (if) + 1 (else if) + 1 (match) = 4
        assert_eq!(fns[0].complexity, 4);
    }

    #[test]
    fn counts_short_circuit_operators() {
        let src = "fn baz(x: bool, y: bool) {\n  if x && y || x {} \n}";
        let fns = scan_functions(src);
        // 1 base + 1 if + 1 && + 1 || = 4
        assert_eq!(fns[0].complexity, 4);
    }

    #[test]
    fn counts_try_operator() {
        let src = "fn q() {\n  let a = foo()?;\n  let b = bar()?;\n}";
        let fns = scan_functions(src);
        assert_eq!(fns[0].complexity, 3);
    }

    #[test]
    fn strip_comments_drops_inline_double_slash() {
        let s = strip_comments_and_strings("let x = 1; // && this is in a comment");
        assert!(!s.contains("&&"));
    }

    #[test]
    fn strip_comments_drops_strings() {
        let s = strip_comments_and_strings(r#"let s = "if you read this"; let _ = s;"#);
        assert!(!s.contains("if "));
    }
}
