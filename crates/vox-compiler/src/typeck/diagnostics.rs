use crate::ast::span::Span;

/// Function / call arity mismatch (SSOT message for Checker + check).
#[must_use]
pub fn msg_arg_count_mismatch(expected: usize, found: usize) -> String {
    format!("Argument count mismatch: expected {expected} arguments, found {found}")
}

/// Tuple arity mismatch (SSOT for Checker + unification).
#[must_use]
pub fn msg_tuple_size_mismatch(expected: usize, found: usize) -> String {
    format!("Tuple size mismatch: expected {expected}, found {found}")
}

/// Function type arity mismatch during unification.
#[must_use]
pub fn msg_function_arity_mismatch(expected: usize, found: usize) -> String {
    format!("Function arity mismatch: expected {expected}, found {found}")
}

/// Record field-count mismatch during unification.
#[must_use]
pub fn msg_record_size_mismatch(expected: usize, found: usize) -> String {
    format!("Record size mismatch: expected {expected}, found {found}")
}

/// Type checking diagnostic severity (distinct from lint / TOESTUB severities).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeckSeverity {
    Error,
    Warning,
}

/// Machine-applicable edit (LSP / MCP repair loops).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticFix {
    pub label: String,
    pub span: Span,
    pub replacement: String,
}

/// Which compiler / pipeline stage produced a diagnostic (taxonomy for tooling and docs).
///
/// See `docs/src/reference/diagnostic-taxonomy.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCategory {
    /// Surface parse failures (typically surfaced before HIR).
    Parse,
    /// AST → HIR lowering or IR-shape issues not covered by type rules.
    Lowering,
    /// Principal type checker / inference (default for historical diagnostics).
    #[default]
    Typecheck,
    /// Structural HIR invariants ([`crate::hir::validate::validate_module`]).
    HirInvariant,
    /// Host / runtime contracts (embed checks, deploy guards).
    RuntimeContract,
    /// Optional lints and style rules.
    Lint,
    /// `uses` clause effect propagation violations.
    EffectViolation,
    /// `state_machine` structural and exhaustiveness violations.
    StateMachineCheck,
}

/// Line/column enrichment added on demand by machine consumers (LSP, healing loop).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LineCol {
    pub line_start: usize,
    pub col_start: usize,
    pub line_end: usize,
    pub col_end: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpanPayload {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuggestedFix {
    pub label: String,
    pub replacement: String,
    pub span: SpanPayload,
}

/// Minimal reproducible excerpt for LLM repair loops.
///
/// The smallest contiguous source slice that contains the diagnostic span
/// plus a small ring of surrounding context. The audit doc names this
/// "the single biggest delta between Vox-as-LLM-target and a typical
/// compiler" — without it, agents must ship the entire file to a model
/// to fix a one-line error.
///
/// Coordinates in `local_span` are relative to `excerpt`, not the original
/// file, so consumers can highlight the offending region without
/// re-resolving file-absolute positions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MinimalRepro {
    /// Excerpt of source containing the diagnostic span plus surrounding
    /// context lines.
    pub excerpt: String,
    /// 1-based line number in the original file of the first line in
    /// `excerpt`. Lets consumers map back to the file when needed.
    pub excerpt_first_line: usize,
    /// Span of the offending region in coordinates relative to `excerpt`
    /// (1-based line, 1-based column, like [`SpanPayload`]).
    pub local_span: SpanPayload,
}

/// Structured diagnostic payload for machine consumers (LLM healing loops).
///
/// Research proves that exact, localized, structured errors are the single
/// highest-leverage improvement for LLM code generation quality.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VoxCompilerDiagnosticPayload {
    pub error_code: String,
    pub severity: TypeckSeverity,
    pub message: String,
    pub file_path: String,
    pub span: SpanPayload,
    pub ast_node_kind: Option<String>,
    pub missing_cases: Vec<String>,
    pub expected_type: Option<String>,
    pub found_type: Option<String>,
    pub correction_hints: Vec<String>,
    pub suggested_fixes: Vec<SuggestedFix>,
    pub related_spans: Vec<SpanPayload>,
    /// Minimal contiguous source excerpt around the diagnostic for LLM
    /// repair consumption. `None` when source is empty or unavailable.
    /// Forward-compat: omitted on serialize when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimal_repro: Option<MinimalRepro>,
}

impl VoxCompilerDiagnosticPayload {
    pub fn from_diagnostic(diag: &Diagnostic, file_path: &str, source: &str) -> Self {
        let compute = |sp: Span| -> SpanPayload {
            let mut line = 1usize;
            let mut col = 1usize;
            for (i, ch) in source.char_indices() {
                if i == sp.start {
                    break;
                }
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            let start_line = line;
            let start_col = col;

            // Reset/Continue for end
            for (i, ch) in source.char_indices().skip(sp.start) {
                if i == sp.end {
                    break;
                }
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            SpanPayload {
                start_line,
                start_col,
                end_line: line,
                end_col: col,
            }
        };

        Self {
            error_code: diag.code.clone().unwrap_or_else(|| "E0000".to_string()),
            severity: diag.severity,
            message: diag.message.clone(),
            file_path: file_path.to_string(),
            span: compute(diag.span),
            ast_node_kind: diag.ast_node_kind.clone(),
            missing_cases: diag.missing_cases.clone(),
            expected_type: diag.expected_type.clone(),
            found_type: diag.found_type.clone(),
            correction_hints: diag.suggestions.clone(),
            suggested_fixes: diag
                .fixes
                .iter()
                .map(|f| SuggestedFix {
                    label: f.label.clone(),
                    replacement: f.replacement.clone(),
                    span: compute(f.span),
                })
                .collect(),
            related_spans: vec![],
            minimal_repro: compute_minimal_repro(diag.span, source),
        }
    }
}

/// Build a [`MinimalRepro`] for `span` from `source`.
///
/// Returns `None` when source is empty. Context window: 3 lines before
/// the start line, 3 lines after the end line, clipped at file
/// boundaries.
pub(crate) fn compute_minimal_repro(span: Span, source: &str) -> Option<MinimalRepro> {
    if source.is_empty() {
        return None;
    }
    const CONTEXT_LINES: usize = 3;

    // Resolve span to (start_line, start_col, end_line, end_col) in 1-based
    // file coordinates. Mirrors `compute` inside from_diagnostic but stays
    // independent so it can be called by other paths.
    let (start_line, start_col, end_line, end_col) = resolve_line_col(span, source);

    // Split source into lines, preserving 1-based indexing.
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let total = lines.len();
    // Clip start_line / end_line into [1, total] in case the span sits at the
    // very end of input without a trailing newline.
    let start_line_clamped = start_line.clamp(1, total);
    let end_line_clamped = end_line.clamp(start_line_clamped, total);

    let first = start_line_clamped.saturating_sub(CONTEXT_LINES).max(1);
    let last = (end_line_clamped + CONTEXT_LINES).min(total);

    let excerpt = lines[(first - 1)..last].join("\n");
    let local_span = SpanPayload {
        start_line: start_line_clamped + 1 - first,
        start_col,
        end_line: end_line_clamped + 1 - first,
        end_col,
    };

    Some(MinimalRepro {
        excerpt,
        excerpt_first_line: first,
        local_span,
    })
}

fn resolve_line_col(span: Span, source: &str) -> (usize, usize, usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    let byte = source
        .char_indices()
        .find_map(|(i, ch)| {
            if i >= span.start {
                Some(i)
            } else {
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                None
            }
        })
        .unwrap_or(source.len());
    let start_line = line;
    let start_col = col;
    for (i, ch) in source.char_indices().skip_while(|(i, _)| *i < byte) {
        if i >= span.end {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (start_line, start_col, line, col)
}

#[cfg(test)]
mod minimal_repro_tests {
    use super::*;
    use crate::ast::span::Span;

    fn diag_at(start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            severity: TypeckSeverity::Error,
            message: "test".into(),
            span: Span::new(start, end),
            expected_type: None,
            found_type: None,
            context: None,
            suggestions: vec![],
            category: DiagnosticCategory::default(),
            code: Some("E0001".into()),
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        }
    }

    #[test]
    fn minimal_repro_basic_window() {
        let source = "line1\nline2\nline3\nline4_BAD\nline5\nline6\nline7\nline8\n";
        // Span over "BAD" inside line4.
        let bad_start = source.find("BAD").unwrap();
        let bad_end = bad_start + 3;
        let payload =
            VoxCompilerDiagnosticPayload::from_diagnostic(&diag_at(bad_start, bad_end), "f.vox", source);
        let mr = payload.minimal_repro.expect("repro present");
        // 3 lines before line 4 + line 4 + 3 lines after = lines 1..=7.
        assert_eq!(mr.excerpt_first_line, 1);
        assert!(mr.excerpt.contains("line1"));
        assert!(mr.excerpt.contains("line4_BAD"));
        assert!(mr.excerpt.contains("line7"));
        assert!(!mr.excerpt.contains("line8"), "tail clipped at context window");
        assert_eq!(mr.local_span.start_line, 4);
        assert_eq!(mr.local_span.end_line, 4);
    }

    #[test]
    fn minimal_repro_near_start_no_underflow() {
        let source = "BAD_line1\nline2\nline3\nline4\n";
        let payload =
            VoxCompilerDiagnosticPayload::from_diagnostic(&diag_at(0, 3), "f.vox", source);
        let mr = payload.minimal_repro.expect("repro present");
        assert_eq!(mr.excerpt_first_line, 1, "clipped to start of file");
        assert_eq!(mr.local_span.start_line, 1);
    }

    #[test]
    fn minimal_repro_near_end_no_overflow() {
        let source = "line1\nline2\nline3\nline4\nBAD_line5\n";
        let bad_start = source.find("BAD").unwrap();
        let bad_end = bad_start + 3;
        let payload = VoxCompilerDiagnosticPayload::from_diagnostic(
            &diag_at(bad_start, bad_end),
            "f.vox",
            source,
        );
        let mr = payload.minimal_repro.expect("repro present");
        // Diagnostic on line 5 of a 5-line file; tail clipped at file end.
        assert_eq!(mr.local_span.start_line, mr.excerpt.lines().count());
    }

    #[test]
    fn minimal_repro_single_line_file() {
        let source = "BAD";
        let payload =
            VoxCompilerDiagnosticPayload::from_diagnostic(&diag_at(0, 3), "f.vox", source);
        let mr = payload.minimal_repro.expect("repro present");
        assert_eq!(mr.excerpt, "BAD");
        assert_eq!(mr.excerpt_first_line, 1);
        assert_eq!(mr.local_span.start_line, 1);
    }

    #[test]
    fn minimal_repro_empty_source_is_none() {
        let payload = VoxCompilerDiagnosticPayload::from_diagnostic(&diag_at(0, 0), "f.vox", "");
        assert!(payload.minimal_repro.is_none());
    }

    #[test]
    fn minimal_repro_multi_line_span() {
        let source = "line1\nline2\nfn foo() {\n    BAD\n    BAD2\n}\nline7\nline8\nline9\n";
        let bad_start = source.find("    BAD").unwrap();
        let bad_end = source.find("BAD2").unwrap() + 4;
        let payload = VoxCompilerDiagnosticPayload::from_diagnostic(
            &diag_at(bad_start, bad_end),
            "f.vox",
            source,
        );
        let mr = payload.minimal_repro.expect("repro present");
        // Span covers lines 4-5; window is lines 1..=8.
        assert_eq!(mr.excerpt_first_line, 1);
        assert!(mr.excerpt.contains("line8"));
        assert!(!mr.excerpt.contains("line9"));
        assert_eq!(mr.local_span.start_line, 4);
        assert_eq!(mr.local_span.end_line, 5);
    }
}

/// A structured diagnostic emitted by the type checker and related frontend passes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Diagnostic {
    pub severity: TypeckSeverity,
    pub message: String,
    pub span: Span,
    pub expected_type: Option<String>,
    pub found_type: Option<String>,
    /// Optional source snippet for autofix / IDE.
    pub context: Option<String>,
    pub suggestions: Vec<String>,
    /// Origin category for filtering, metrics, and LSP `code` mapping.
    #[serde(default)]
    pub category: DiagnosticCategory,
    /// Stable code for stall detection and speech-to-code traces (`typecheck.reactive.state`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Optional structured fixes (additive; consumers ignore if unsupported).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<DiagnosticFix>,
    /// Line/column info enriched from source (optional, computed on demand).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_col: Option<LineCol>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_cases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ast_node_kind: Option<String>,
}

impl Diagnostic {
    /// Enrich this diagnostic with line/column data computed from `source`.
    ///
    /// Call on the way out of the compiler pipeline when a machine consumer
    /// (healing loop, LSP, `vox check --json`) needs precise cursor locations.
    #[must_use]
    pub fn with_line_col(mut self, source: &str) -> Self {
        let compute = |byte_offset: usize| -> (usize, usize) {
            let mut line = 1usize;
            let mut col = 1usize;
            for (i, ch) in source.char_indices() {
                if i == byte_offset {
                    break;
                }
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            (line, col)
        };
        let (line_start, col_start) = compute(self.span.start);
        let (line_end, col_end) = compute(self.span.end.min(source.len().saturating_sub(1)));
        self.line_col = Some(LineCol {
            line_start,
            col_start,
            line_end,
            col_end,
        });
        self
    }

    /// Add a machine-applicable suggestion / correction hint.
    #[must_use]
    pub fn with_suggestion(mut self, hint: impl Into<String>) -> Self {
        self.suggestions.push(hint.into());
        self
    }
    /// Build a simple error diagnostic (no type diff).
    #[must_use]
    pub fn error(message: String, span: Span, source: &str) -> Self {
        Self {
            severity: TypeckSeverity::Error,
            message,
            span,
            expected_type: None,
            found_type: None,
            context: Some(Self::capture_context(source, span)),
            suggestions: vec![],
            category: DiagnosticCategory::Typecheck,
            code: None,
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        }
    }

    /// Build a simple warning diagnostic (no type diff).
    #[must_use]
    pub fn warning(message: String, span: Span, source: &str) -> Self {
        Self {
            severity: TypeckSeverity::Warning,
            message,
            span,
            expected_type: None,
            found_type: None,
            context: Some(Self::capture_context(source, span)),
            suggestions: vec![],
            category: DiagnosticCategory::Typecheck,
            code: None,
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        }
    }

    /// HIR structural invariant violation (after lowering).
    #[must_use]
    pub fn hir_invariant(
        message: String,
        span: Span,
        source: &str,
        correction_hint: Option<String>,
    ) -> Self {
        Self {
            severity: TypeckSeverity::Error,
            message,
            span,
            expected_type: None,
            found_type: None,
            context: Some(Self::capture_context(source, span)),
            suggestions: correction_hint.into_iter().collect(),
            category: DiagnosticCategory::HirInvariant,
            code: None,
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        }
    }

    /// AST -> HIR lowering diagnostic surfaced through structured diagnostics.
    #[must_use]
    pub fn lowering(message: String, span: Span, source: &str) -> Self {
        Self {
            severity: TypeckSeverity::Error,
            message,
            span,
            expected_type: None,
            found_type: None,
            context: Some(Self::capture_context(source, span)),
            suggestions: vec![],
            category: DiagnosticCategory::Lowering,
            code: None,
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        }
    }

    /// Runtime/embedding contract diagnostic surfaced through structured diagnostics.
    #[must_use]
    pub fn runtime_contract(message: String, span: Span, source: &str) -> Self {
        Self {
            severity: TypeckSeverity::Error,
            message,
            span,
            expected_type: None,
            found_type: None,
            context: Some(Self::capture_context(source, span)),
            suggestions: vec![],
            category: DiagnosticCategory::RuntimeContract,
            code: None,
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            ast_node_kind: None,
        }
    }

    /// Extract a few lines around `span` for display.
    #[must_use]
    pub fn capture_context(source: &str, span: Span) -> String {
        let lines: Vec<&str> = source.lines().collect();
        if lines.is_empty() {
            return String::new();
        }
        let mut offset = 0usize;
        let mut start_line = 0usize;
        for (i, line) in lines.iter().enumerate() {
            let next = offset + line.len() + 1;
            if span.start >= offset && span.start < next {
                start_line = i;
                break;
            }
            offset = next;
        }
        let from = start_line.saturating_sub(1);
        let to = (start_line + 2).min(lines.len());
        lines[from..to].join("\n")
    }
}

// ── Phase 1 stable diagnostic codes (mesh-phase1-language-spine-plan-2026) ─────

/// Public constants for every diagnostic code introduced in Phase 1.
/// Use these instead of raw string literals so that IDs are greppable,
/// typo-proof, and covered by the namespace guard test.
pub mod codes {
    pub const TYPES_DURABLE_PROMISE_ARITY: &str = "vox/types/durable-promise-arity";
    pub const TYPES_FUTURE_DEPRECATED: &str = "vox/types/future-deprecated";
    pub const TYPES_PROMISE_DEPRECATED: &str = "vox/types/promise-deprecated";

    pub const API_MESH_PREFIX_DEPRECATED: &str = "vox/api/mesh-prefix-deprecated";

    pub const REMOTE_NON_SERIALIZABLE_PARAM: &str = "vox/remote/non-serializable-param";
    pub const REMOTE_NON_SERIALIZABLE_RETURN: &str = "vox/remote/non-serializable-return";

    pub const WORKFLOW_WITH_ID_NON_DETERMINISTIC: &str = "vox/workflow/with-id-non-deterministic";
    pub const WORKFLOW_NON_DETERMINISTIC_CALL: &str = "vox/workflow/non-deterministic-call";
    pub const WORKFLOW_SIDE_EFFECT_OUTSIDE_WORKFLOW: &str =
        "vox/workflow/side-effect-outside-workflow";

    pub const EFFECT_MISSING_DECLARATION: &str = "vox/effect/missing-declaration";

    /// All Phase-1 codes registered for stability, used by the namespace guard test.
    pub const ALL_PHASE_1: &[&str] = &[
        TYPES_DURABLE_PROMISE_ARITY,
        TYPES_FUTURE_DEPRECATED,
        TYPES_PROMISE_DEPRECATED,
        API_MESH_PREFIX_DEPRECATED,
        REMOTE_NON_SERIALIZABLE_PARAM,
        REMOTE_NON_SERIALIZABLE_RETURN,
        WORKFLOW_WITH_ID_NON_DETERMINISTIC,
        WORKFLOW_NON_DETERMINISTIC_CALL,
        WORKFLOW_SIDE_EFFECT_OUTSIDE_WORKFLOW,
        EFFECT_MISSING_DECLARATION,
    ];

    /// Every `Diagnostic.code` string used by the compiler frontend that must stay disjoint
    /// from `vox-code-audit` rule ids (`contracts/code-audit/rules.v1.yaml`).
    ///
    /// When adding a new stable diagnostic code in `vox-compiler`, append it here and run
    /// `cargo test -p vox-code-audit no_audit_rule_collides_with_compiler_diagnostic_code`.
    pub const ALL_COMPILER_DIAGNOSTIC_CODES: &[&str] = &[
        TYPES_DURABLE_PROMISE_ARITY,
        TYPES_FUTURE_DEPRECATED,
        TYPES_PROMISE_DEPRECATED,
        API_MESH_PREFIX_DEPRECATED,
        REMOTE_NON_SERIALIZABLE_PARAM,
        REMOTE_NON_SERIALIZABLE_RETURN,
        WORKFLOW_WITH_ID_NON_DETERMINISTIC,
        WORKFLOW_NON_DETERMINISTIC_CALL,
        WORKFLOW_SIDE_EFFECT_OUTSIDE_WORKFLOW,
        EFFECT_MISSING_DECLARATION,
        // Pipeline / parse / hygiene (E028 retired by ADR-041 — durability grammar is stable)
        "E0001",
        "E091",
        "W092",
        "W093",
        // Effects
        "E_EFFECT_PURE_CONFLICT",
        "E_EFFECT_DUPLICATE",
        // Match / state machine / URL
        "E0301",
        "E_SM_UNKNOWN_STATE",
        "E200",
        "E201",
        "E202",
        // Lint namespace
        "lint.theme_contrast",
        "lint.search_index_unknown_table",
        "lint.search_index_not_table",
        "lint.search_index_unknown_field",
        "lint.search_index_field_type",
        "lint.table_id_column",
        "lint.index_unknown_table",
        "lint.pure_shallow_violation",
        "lint.handler.uncancellable_async",
        "lint.legacy_component_fn",
        "lint.effect.unresolvable_deps",
        "lint.query_not_readonly",
        "lint.closure.stale_capture",
        "lint.form.unknown_endpoint",
        "lint.form.field_unmatched",
        "lint.form.field_type_mismatch",
        // Typecheck namespace
        "typecheck.deprecated_ident",
        "typecheck.arg_mismatch",
        // vox/* semantic checks
        "vox/auth/capability-leak",
        "vox/effect/pure-violation",
        "vox/taint/pii-leak",
        "vox/vector/dimension-mismatch",
        "vox/embed/zero-dimensions",
        "vox/ai/return-shape-not-codec'd",
        "vox/upload/empty-mime",
        "vox/upload/zero-max-bytes",
        "vox/webhook/replay-window-out-of-range",
        "vox/webhook/missing-secret-var",
        "vox/cors/credentials-with-wildcard",
        "vox/pii/unannotated-net-effect",
        "vox/tokens/contrast-violation",
        "vox/tokens/invalid-hex",
        "vox/async/missing-arm",
        "vox/train/cuda-required",
        "vox/layer/tier-inversion",
        "vox/layer/duplicate-mark",
        "vox/layer/reserved-tier",
        "vox/layer/dangling-mark",
        "vox/form/missing-label",
        // Missing `@uses(...)` effect declarations (HIR graft)
        "vox/effect/missing-net-decl",
        "vox/effect/missing-fs-decl",
        "vox/effect/missing-time-decl",
        "vox/effect/missing-random-decl",
        "vox/effect/missing-secret-decl",
        "vox/effect/missing-llm-decl",
        // Semantic UI labels (`semantic_ui.rs`)
        "vox/a11y/dialog-missing-label",
        "vox/a11y/menu-missing-label",
        "vox/a11y/listbox-missing-label",
        "vox/a11y/combobox-missing-label",
        "vox/a11y/tabs-missing-label",
    ];

    #[cfg(test)]
    mod guard_tests {
        use super::ALL_COMPILER_DIAGNOSTIC_CODES;
        use std::collections::HashSet;

        #[test]
        fn compiler_diagnostic_codes_are_unique() {
            let mut seen = HashSet::new();
            for code in ALL_COMPILER_DIAGNOSTIC_CODES {
                assert!(
                    seen.insert(*code),
                    "duplicate compiler diagnostic code in ALL_COMPILER_DIAGNOSTIC_CODES: {code}"
                );
            }
        }
    }
}
