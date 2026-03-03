use crate::rules::{Finding, Language, SourceFile};

pub const REVIEW_SYSTEM_PROMPT: &str =
    "You are an expert senior code reviewer specializing in safe, high-performance systems programming in Rust, TypeScript, Python, GDScript, \
     and the Vox AI-native programming language. \
     \n\n### VOX LANGUAGE IDIOMS & RULES:\
     \n1. SAFETY FIRST: .unwrap() is strictly forbidden in production code. Always use .expect(\"rich context\") or, preferably, Result propagation with .map_err(|e| anyhow!(...)).\
     \n2. EXPLICIT NULL-SAFETY: Null does not exist in Vox. Everything optional is an Option<T>. Flag any code that attempts to access a value without a 'match' or 'if let'.\
     \n3. RESOURCE DISCIPLINE: Lowering passes MUST maintain scope integrity. push_scope() and pop_scope() must be balanced. Check for early returns that bypass pop_scope().\
     \n4. NO COMPILER LEAKS: TypeVar(0) is an internal sentinel. It must never appear in generated source code or user-facing output.\
     \n5. ASYNC CONCURRENCY: Async executors are sensitive. NEVER call blocking functions (std::thread::sleep, recv_blocking, fs sync calls) inside an async fn. Use tokio equivalents.\
     \n6. DEADLOCK PREVENTION: Mutex guards must NEVER be held across an .await point. This is the #1 cause of deadlocks in the Vox orchestrator. Use message passing or tokio::sync::Mutex.\
     \n7. ACTOR INTEGRITY: Actors should mutate state through actions/events to maintain log durability. Direct field mutation should be flagged if it bypasses the event system.\
     \n8. ERROR VISIBILITY: Use the 'tracing' crate (error!, warn!, info!) for logging. Avoid println!/eprintln! in library crates.\
     \n9. CLEAN EXITS: todo!() and unimplemented!() are production defects. Replace them with proper error handling or complete the implementation.\
     \n10. MEMORY SAFETY: Identify potential Send/Sync violations in multi-threaded code. Check for raw pointer misuse or unsafe blocks without documented safety invariants.\
     \n11. API DESIGN: Flag redundant allocations (e.g., .to_string() on &str literals when &str suffices). Identify excessive use of Box/Arc where stack allocation or simple references would work.\
     \n12. DOCUMENTATION: Public functions and structs MUST have doc comments. Flag missing documentation for exported APIs.\
     \n\n### GENERAL QUALITY GATE:\
     \n- Report only HIGH-CONFIDENCE issues (≥80%). If uncertain, ignore.\
     \n- Severity Scale: info (nit) < warning (potential bug) < error (defect) < critical (security/crash).\
     \n- prioritisation: Correctness > Security > Reliability > Performance > Style.\
     \n- Verification: Check all line numbers against the provided source. Hallucinations result in systemic failure.";

/// Build the full review prompt, capped at `max_tokens` chars of source code.
pub fn build_review_prompt(
    file: &SourceFile,
    static_findings: &[Finding],
    _lang: Language,
    max_context_chars: usize,
) -> String {
    build_review_prompt_inner(file, static_findings, max_context_chars, None)
}

/// Build a prompt focused on a git diff hunk — only the changed lines are reviewed.
pub fn build_diff_review_prompt(
    file: &SourceFile,
    static_findings: &[Finding],
    max_context_chars: usize,
    diff_hunk: &str,
) -> String {
    build_review_prompt_inner(file, static_findings, max_context_chars, Some(diff_hunk))
}

fn build_review_prompt_inner(
    file: &SourceFile,
    static_findings: &[Finding],
    max_context_chars: usize,
    diff_hunk: Option<&str>,
) -> String {
    let static_summary = if static_findings.is_empty() {
        "None (static analysis clean).".to_string()
    } else {
        static_findings
            .iter()
            .take(20)
            .map(|f| {
                format!(
                    "  - L{}: [{}] {} ({})",
                    f.line, f.rule_id, f.message, f.severity
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let code_snippet = if file.content.len() > max_context_chars {
        &file.content[..max_context_chars]
    } else {
        &file.content
    };

    let focus_section = if let Some(hunk) = diff_hunk {
        let trimmed_hunk = if hunk.len() > max_context_chars / 2 {
            &hunk[..max_context_chars / 2]
        } else {
            hunk
        };
        format!(
            "\nGIT DIFF (focus your review on THESE changed lines — lines prefixed with + are additions, - are deletions):\n```diff\n{}\n```\n",
            trimmed_hunk
        )
    } else {
        String::new()
    };

    format!(
        r#"Review the following source file for issues.

FILE: {path}
LANGUAGE: {lang}
LINES: {line_count}
{focus_section}
STATIC ANALYSIS FINDINGS (already known — do NOT repeat these):
{static_summary}

SOURCE CODE:
```
{code}
```

Review for ALL of the following categories. For EACH issue found, output EXACTLY this format (one per line):
ISSUE|<line>|<severity>|<category>|<confidence 0-100>|<message>|<suggestion>

Where:
- <line> = 1-indexed line number (0 if file-level)
- <severity> = info | warning | error | critical
- <category> = logic | security | error-handling | performance | dead-code | style | vox | deps
- <confidence> = integer 0-100 (only report if >=80)
- <message> = concise description of the issue
- <suggestion> = optional fix hint (or "-" if none)

REVIEW CATEGORIES:
1. LOGIC: Off-by-one errors, incorrect conditions, wrong operator, flawed control flow, race conditions, null/option dereferences, integer overflow
2. SECURITY: SQL injection, XSS, secret leakage, insecure deserialization, path traversal, SSRF, weak crypto, insecure direct object reference
3. ERROR-HANDLING: Unchecked Results (.unwrap() in production without .expect()), empty catch blocks, missing error propagation, swallowed exceptions
4. PERFORMANCE: N+1 queries, unnecessary clones, quadratic algorithms, excessive allocations, blocking calls in async context, Mutex held across .await
5. DEAD-CODE: Unreachable branches, todo!()/unimplemented!() in non-test code, functions that never return, stubbed implementations
6. STYLE: Inconsistent naming, misleading variable names, overly complex expressions that should be refactored
7. VOX: Violation of null-safety (must use Option/Result, never null), missing push_scope/pop_scope discipline, TypeVar(0) literals in codegen, .unwrap() without expect message, actor handlers holding Mutex across .await
8. DEPS: Missing imports, incorrect module paths, circular dependencies

If no issues are found in a category, skip it. If no issues at all, output: CLEAN
Do NOT repeat static analysis findings. Do NOT explain your reasoning. Do NOT hallucinate line numbers."#,
        path = file.path.display(),
        lang = file.language,
        line_count = file.lines.len(),
        focus_section = focus_section,
        static_summary = static_summary,
        code = code_snippet,
    )
}
