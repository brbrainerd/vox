use crate::features::{ExtractedFeatures, LiteralContext};
use crate::rules::{DriftRule, WorkspaceContext};
use vox_code_audit::rules::{Finding, FindingConfidence, Language, Severity};

/// Flags `<something>.join(".vox")` chained directly off a bare `current_dir()`
/// call — the shape fixed by `vox_config::paths::repo_dot_vox_dir()` after it
/// minted stray `.vox/` trees wherever the shell happened to be invoked from
/// (see that function's doc comment).
pub struct VoxDirUnanchoredRule;

/// Anchor-function names that, if mentioned in the same line window as a
/// `current_dir()` call, mean the `.vox` join is already routed through a
/// repo-root resolver rather than a bare cwd. Prevents false positives on
/// code that legitimately calls `current_dir()` on one line and then anchors
/// it (e.g. `find_repo_root(&cwd)`) before joining `.vox` nearby.
const ANCHOR_MARKERS: &[&str] = &["repo_dot_vox_dir", "find_repo_root", "dot_vox_user_dir"];

impl DriftRule for VoxDirUnanchoredRule {
    fn id(&self) -> &'static str {
        "drift/vox-dir-unanchored"
    }
    fn severity(&self) -> Severity {
        Severity::Warning
    }
    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn check(&self, features: &ExtractedFeatures, _ctx: &WorkspaceContext) -> Vec<Finding> {
        // `ExtractedFeatures::call_sites` only records function-call-style
        // expressions (`syn::ExprCall` with a path callee, e.g.
        // `reqwest::Client::new()`). `x.join(y)` is a `syn::ExprMethodCall`,
        // which `extractors/rust.rs::RustVisitor` never visits — so there is no
        // structural record of a `current_dir().join(".vox")` chain (or the
        // `let cwd = current_dir(); cwd.join(".vox")` split-statement form) in
        // `ExtractedFeatures` to match against. Falling back to a textual
        // re-read of the `.vox` literal's source line (and the line
        // immediately before it) is the precedented approach `vox_path_literal`
        // and this crate's other rules do not need, but that this shape does.
        let Ok(content) = std::fs::read_to_string(&features.file) else {
            return vec![];
        };
        let lines: Vec<&str> = content.lines().collect();

        features
            .string_literals
            .iter()
            // Exact match, not `starts_with(".vox")`: the bug is specifically
            // `.join(".vox")` as one path segment. A raw `.vox/`-prefixed
            // literal is `drift/vox-path-literal`'s territory (different fix:
            // use the constant, not anchor the API call).
            .filter(|lit| matches!(lit.ctx, LiteralContext::Code) && lit.value == ".vox")
            .filter(|lit| !crate::extractor::is_allowed_at(features, self.id(), lit.loc.line))
            .filter(|lit| line_window_is_unanchored_current_dir(&lines, lit.loc.line))
            .map(|lit| Finding {
                rule_id: self.id().to_string(),
                rule_name: "Unanchored .vox Directory Join".into(),
                severity: self.severity(),
                file: features.file.clone(),
                line: lit.loc.line,
                column: lit.loc.col,
                message: "`.join(\".vox\")` off a bare `current_dir()` mints a stray .vox tree \
                    wherever the shell happens to be invoked from, instead of anchoring to the \
                    repository root"
                    .into(),
                suggestion: Some(
                    "Use `vox_config::paths::repo_dot_vox_dir()`, or \
                     `vox_config::paths::find_repo_root(&cwd)` + `.join(\".vox\")`, instead of \
                     joining `.vox` onto a bare `current_dir()`"
                        .into(),
                ),
                context: format!("crate: {}", features.crate_name.as_deref().unwrap_or("")),
                confidence: Some(FindingConfidence::Medium),
                evidence: None,
                diagnostic_id: None,
                alternatives: vec![],
                rationale: None,
            })
            .collect()
    }
}

/// True when the `.vox`-literal's `.join(...)` call is textually unanchored:
/// either it chains directly off a `current_dir()` call on the same line, or
/// its receiver identifier is bound to a `current_dir()` call on the line
/// immediately above — and neither line mentions one of [`ANCHOR_MARKERS`].
///
/// Deliberately a *narrow* (same-line-or-adjacent-line) textual window, not a
/// data-flow analysis: a local variable named `cwd` that was itself derived
/// from `repo_dot_vox_dir()` several lines earlier would be missed by this
/// check. That under-flagging is the safe direction to err in — chasing
/// multi-statement data flow risks false positives on the many correctly
/// anchored call sites in the tree (e.g. `repo_root.join(".vox")` where
/// `repo_root` came from a secret or a `discover_repository_or_fallback` call
/// several lines above).
///
/// The adjacent-line case additionally requires an identifier binding between
/// the two lines (the `.join(...)` receiver must be the same name a
/// `current_dir()` call bound on the line above) rather than merely "the word
/// `current_dir` appears somewhere nearby" — otherwise an unrelated
/// `current_dir()` call captured into an unrelated variable on the adjacent
/// line (e.g. a `_cwd_for_log` binding used only for logging) would falsely
/// implicate an already-anchored `.join(".vox")` on the next line.
fn line_window_is_unanchored_current_dir(lines: &[&str], literal_line_1_indexed: usize) -> bool {
    if literal_line_1_indexed == 0 {
        return false;
    }
    let idx = literal_line_1_indexed - 1; // 0-indexed
    let Some(current_line) = lines.get(idx).copied() else {
        return false;
    };
    let above_line = idx.checked_sub(1).and_then(|i| lines.get(i).copied());

    let window: Vec<&str> = [above_line, Some(current_line)]
        .into_iter()
        .flatten()
        .collect();
    let mentions_anchor = window
        .iter()
        .any(|l| ANCHOR_MARKERS.iter().any(|m| l.contains(m)));
    if mentions_anchor {
        return false;
    }

    // Isolate the statement that actually contains the flagged `.vox`
    // literal. Regression for a false positive found in code review: an
    // unrelated statement earlier on the same physical line (e.g.
    // `let _ = std::env::current_dir(); let p = already_anchored.join(".vox");`)
    // must not implicate an already-anchored join later on the line just
    // because the whole line contains the substring `current_dir()`.
    let statement = statement_containing_vox_literal(current_line);

    // Same-line chain: `current_dir().join(".vox")`, with or without
    // intervening `.unwrap()`/`.unwrap_or_default()` calls. No identifier
    // binding to check — the call is used directly.
    if statement.contains("current_dir()") {
        return true;
    }

    // Adjacent-line split-statement form: `let cwd = ...current_dir()...;`
    // followed by `cwd.join(".vox")`. Require the `.join(...)` receiver to be
    // the *same* identifier the line above binds a `current_dir()` call to.
    let Some(above_line) = above_line else {
        return false;
    };
    if !above_line.contains("current_dir()") {
        return false;
    }
    match receiver_ident_before_join(statement) {
        Some(ident) => line_binds_ident_before_current_dir(above_line, ident),
        None => false,
    }
}

/// Returns the `;`-delimited statement segment of `line` that contains the
/// literal text `.vox` (i.e. the statement the flagged string literal
/// actually lives in), or the whole line if no `;` splits it — a cheap,
/// column-free stand-in for "which statement is this on" that keeps a
/// same-line multi-statement false positive from bleeding across an earlier,
/// unrelated statement. Also fixes a false NEGATIVE in
/// `receiver_ident_before_join`: without this isolation, a line with more
/// than one `.join(` call (e.g. `a.join("bin").join(cwd.join(".vox"))`) would
/// hand the whole line to that function, which only looks at the first
/// `.join(` occurrence — this isolates to the statement first, so callers
/// only ever see the `.join(` calls relevant to the flagged literal's own
/// statement.
fn statement_containing_vox_literal(line: &str) -> &str {
    if !line.contains(';') {
        return line;
    }
    line.split(';')
        .find(|seg| seg.contains(".vox"))
        .unwrap_or(line)
}

/// Extracts the identifier immediately preceding a `.join(` call on `line`,
/// e.g. `"cwd"` from `out.push(cwd.join(".vox"))`. Returns `None` when the
/// receiver isn't a bare identifier (a chained call, an index expression,
/// etc.) — those are handled by the same-line-chain path instead.
///
/// Uses `rfind`, not `find`: `line` is already isolated to the statement
/// containing the flagged `.vox` literal (see `statement_containing_vox_literal`),
/// so the *last* `.join(` on it is the innermost/rightmost one — the one
/// whose argument is the `.vox` literal itself, e.g. in
/// `already_anchored.join("bin").join(cwd.join(".vox"))` the last `.join(`
/// is `cwd.join(`, not the outer `already_anchored.join(`. A prior `find`
/// (first occurrence) picked the wrong receiver whenever more than one
/// `.join(` appeared before the flagged one, producing a false negative.
fn receiver_ident_before_join(line: &str) -> Option<&str> {
    let join_pos = line.rfind(".join(")?;
    let before = &line[..join_pos];
    let start = before
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map_or(0, |i| i + 1);
    let ident = &before[start..];
    if ident.is_empty() { None } else { Some(ident) }
}

/// True when `line` binds `ident` (as a whole word, on the left-hand side of
/// an assignment/pattern binding — a `let <ident> = ...`, `let mut <ident> =
/// ...`, `Ok(<ident>) = ...`, or `Some(<ident>) = ...` shape) somewhere before
/// its first `=`, and the line also calls `current_dir()`.
fn line_binds_ident_before_current_dir(line: &str, ident: &str) -> bool {
    if !line.contains("current_dir()") {
        return false;
    }
    let Some(eq_pos) = line.find('=') else {
        return false;
    };
    let binding_side = &line[..eq_pos];
    contains_whole_word(binding_side, ident)
}

/// True when `haystack` contains `word` bounded on both sides by a
/// non-identifier character (or the string edge) — a cheap stand-in for a
/// regex word-boundary match without pulling in a `regex` dependency.
fn contains_whole_word(haystack: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let wlen = word.len();
    let mut search_from = 0usize;
    while let Some(rel_pos) = haystack[search_from..].find(word) {
        let pos = search_from + rel_pos;
        let before_ok =
            pos == 0 || !(bytes[pos - 1] as char).is_alphanumeric() && bytes[pos - 1] != b'_';
        let after_idx = pos + wlen;
        let after_ok = after_idx >= bytes.len()
            || !(bytes[after_idx] as char).is_alphanumeric() && bytes[after_idx] != b'_';
        if before_ok && after_ok {
            return true;
        }
        search_from = pos + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::*;
    use crate::rules::WorkspaceContext;
    use std::path::PathBuf;
    use vox_code_audit::rules::Language;

    fn ctx() -> WorkspaceContext {
        WorkspaceContext {
            workspace_version: "0.5.0".into(),
            workspace_root: PathBuf::from("."),
            layers: crate::layers_manifest::LayersManifest::default(),
        }
    }

    /// Writes `content` to a temp file and returns an `ExtractedFeatures` whose
    /// `file` points at it and whose `string_literals` contain a `.vox` entry
    /// at `vox_literal_line` — mirroring what `RustExtractor` would produce for
    /// that source, without needing a full `syn` parse in the test.
    fn features_for(
        content: &str,
        vox_literal_line: usize,
    ) -> (tempfile::TempDir, ExtractedFeatures) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("site.rs");
        std::fs::write(&path, content).expect("write fixture");

        let mut f = ExtractedFeatures::new(path, Language::Rust);
        f.crate_name = Some("vox-example".into());
        f.string_literals.push(LiteralLoc {
            value: ".vox".into(),
            loc: Loc {
                line: vox_literal_line,
                col: 0,
            },
            ctx: LiteralContext::Code,
        });
        (dir, f)
    }

    #[test]
    fn flags_cwd_join_vox_split_across_two_statements() {
        // The exact bug pattern fixed in Part A of this task
        // (crates/vox-secrets/src/sources/populi_env.rs) and previously in
        // vox_config::paths::repo_dot_vox_dir()'s predecessor: `current_dir()`
        // assigned on one line, `.join(".vox")` used on the very next line.
        let src = "fn f() {\n    if let Ok(cwd) = std::env::current_dir() {\n        out.push(cwd.join(\".vox\"));\n    }\n}\n";
        let (_dir, f) = features_for(src, 3);
        let rule = VoxDirUnanchoredRule;
        let findings = rule.check(&f, &ctx());
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule_id, "drift/vox-dir-unanchored");
        assert_eq!(findings[0].line, 3);
    }

    #[test]
    fn flags_current_dir_join_vox_chained_on_one_line() {
        let src = "fn f() {\n    let p = std::env::current_dir().unwrap().join(\".vox\");\n}\n";
        let (_dir, f) = features_for(src, 2);
        let rule = VoxDirUnanchoredRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }

    #[test]
    fn allows_repo_dot_vox_dir_call() {
        // The correct replacement shape: no bare current_dir().join(".vox") at all.
        let src = "fn f() {\n    let p = vox_config::paths::repo_dot_vox_dir();\n}\n";
        let (_dir, f) = features_for(src, 2);
        let rule = VoxDirUnanchoredRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn allows_find_repo_root_anchored_join_even_when_current_dir_is_nearby() {
        // Precedent shape (crates/vox-ml-cli/.../oratio_cmd.rs and this task's
        // own Part A fix): current_dir() feeds an anchoring call before the
        // `.vox` join, so the join line itself never mentions a bare
        // current_dir(). Also covers the case where current_dir() and the
        // anchor call land in the *same* window as the join.
        let src = "fn f() {\n    let cwd = std::env::current_dir().unwrap_or_default();\n    let root = find_repo_root(&cwd).unwrap();\n    let p = root.join(\".vox\");\n}\n";
        let (_dir, f) = features_for(src, 4);
        let rule = VoxDirUnanchoredRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn allows_join_vox_off_an_unrelated_repo_root_variable() {
        // toestub_tools.rs-shaped precedent: `.join(".vox")` several lines away
        // from any current_dir() call, on a `repo_root` variable resolved
        // through a secret with a cwd fallback in a different branch.
        let src = "fn f() {\n    let repo_root = if let Some(p) = resolve() {\n        PathBuf::from(p)\n    } else {\n        std::env::current_dir().unwrap_or_default()\n    };\n\n    let dot_vox = repo_root.join(\".vox\");\n}\n";
        let (_dir, f) = features_for(src, 8);
        let rule = VoxDirUnanchoredRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn allows_join_vox_when_adjacent_current_dir_binds_an_unrelated_identifier() {
        // Regression for the false positive found in code review: a
        // `current_dir()` call captured into an unrelated variable (here,
        // one only used for logging) on the line immediately above an
        // already-anchored `.join(".vox")` must NOT be flagged just because
        // `current_dir()` is textually adjacent — the two lines share no
        // identifier binding.
        let src = "fn f() {\n    let _cwd_for_log = std::env::current_dir().unwrap_or_default();\n    let path = already_anchored_root.join(\".vox\");\n}\n";
        let (_dir, f) = features_for(src, 3);
        let rule = VoxDirUnanchoredRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn allows_join_vox_when_current_dir_is_an_earlier_unrelated_statement_on_the_same_line() {
        // Regression for a false positive found in code review: two
        // statements on one physical line, where an unrelated `current_dir()`
        // call precedes an already-anchored `.join(".vox")`. Before the fix,
        // `current_line.contains("current_dir()")` matched the whole line and
        // ignored the `;` statement boundary between them.
        let src = "fn f() {\n    let _ = std::env::current_dir(); let p = already_anchored_root.join(\".vox\");\n}\n";
        let (_dir, f) = features_for(src, 2);
        let rule = VoxDirUnanchoredRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }

    #[test]
    fn flags_the_innermost_join_when_the_statement_has_multiple_join_calls() {
        // Regression for a false negative found in code review:
        // `receiver_ident_before_join` used `find` (first `.join(`), so a
        // statement with more than one `.join(` call before the `.vox` one
        // extracted the wrong receiver identifier and silently missed a
        // genuinely unanchored join. `already_anchored.join("bin")` is an
        // anchored, irrelevant outer call; `cwd.join(".vox")` is the real bug.
        let src = "fn f() {\n    let cwd = std::env::current_dir().unwrap();\n    already_anchored.join(\"bin\").join(cwd.join(\".vox\"));\n}\n";
        let (_dir, f) = features_for(src, 3);
        let rule = VoxDirUnanchoredRule;
        assert_eq!(rule.check(&f, &ctx()).len(), 1);
    }

    #[test]
    fn respects_drift_allow_annotation() {
        let src = "fn f() {\n    if let Ok(cwd) = std::env::current_dir() {\n        out.push(cwd.join(\".vox\")); // drift-allow(vox-dir-unanchored): test fixture\n    }\n}\n";
        let (_dir, mut f) = features_for(src, 3);
        let mut allowed = std::collections::HashSet::new();
        allowed.insert(3);
        f.allowed_lines
            .insert("vox-dir-unanchored".to_string(), allowed);
        let rule = VoxDirUnanchoredRule;
        assert!(rule.check(&f, &ctx()).is_empty());
    }
}
