//! .vox Source code validation and machine-parsable diagnostics (repair loop support).
//!
//! Provides CLI parity with `vox check --output-format json` and LSP-style
//! diagnostics for tool-driven self-repair.

use crate::params::{
    DiagnosticInfo, FixInfo, ToolResult, ValidateFileParams, ValidateResponse,
    ValidateSourceParams, VoxCheckParams, VoxCheckResponse,
};
use crate::server_state::ServerState;

/// Convert `vox-lsp` LSP diagnostics into the MCP-facing [`DiagnosticInfo`] shape, preserving
/// the stable diagnostic `code` and any structured autofix suggestions carried in the
/// `Diagnostic.data` payload (`{ "suggestions": [...], "fixes": [...] }`, populated by
/// [`vox_lsp::typeck_diagnostic_to_lsp`]).
fn lsp_diagnostics_to_info(
    diagnostics: &[tower_lsp_server::ls_types::Diagnostic],
) -> Vec<DiagnosticInfo> {
    diagnostics
        .iter()
        .map(|d| {
            let code = match &d.code {
                Some(tower_lsp_server::ls_types::NumberOrString::String(s)) => Some(s.clone()),
                Some(tower_lsp_server::ls_types::NumberOrString::Number(n)) => Some(n.to_string()),
                None => None,
            };
            let fixes = d
                .data
                .as_ref()
                .and_then(|v| v.get("fixes"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(parse_fix).collect())
                .unwrap_or_default();
            DiagnosticInfo {
                severity: match d.severity {
                    Some(s) if s == tower_lsp_server::ls_types::DiagnosticSeverity::ERROR => {
                        "error".to_string()
                    }
                    _ => "warning".to_string(),
                },
                message: d.message.clone(),
                source: d.source.clone().unwrap_or_default(),
                start_line: d.range.start.line,
                start_col: d.range.start.character,
                end_line: d.range.end.line,
                end_col: d.range.end.character,
                code,
                fixes,
            }
        })
        .collect()
}

fn parse_fix(value: &serde_json::Value) -> Option<FixInfo> {
    let label = value.get("label")?.as_str()?.to_string();
    let replacement = value.get("replacement")?.as_str()?.to_string();
    let range = value.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    // Defensive conversion: out-of-range u64 → drop the fix instead of silently
    // truncating with `as u32`. LSP line numbers > u32::MAX are physically
    // impossible, but corrupt fix ranges should fail safe.
    Some(FixInfo {
        label,
        replacement,
        start_line: u32::try_from(start.get("line")?.as_u64()?).ok()?,
        start_col: u32::try_from(start.get("character")?.as_u64()?).ok()?,
        end_line: u32::try_from(end.get("line")?.as_u64()?).ok()?,
        end_col: u32::try_from(end.get("character")?.as_u64()?).ok()?,
    })
}

/// Pre-validation heuristics that short-circuit before HIR validation.
/// Returns `Some(error_json)` if a hard fail-fast pattern is found, else `None`.
///
/// Patterns are checked carefully to avoid false positives in comments / string
/// literals: `todo!()` / `unimplemented!()` are specific enough as substrings; the
/// macro/operator keyword guards anchor on the start of a trimmed line so a comment
/// like `// the macro expansion approach…` or `// addition operator returns u32`
/// does not trigger. The `// TODO` substring check (formerly part of the TOESTUB
/// pattern) was dropped — `// TODO` markers in human-maintained code are routine
/// and are not skeleton-code indicators.
fn pre_validation_guard<R: serde::Serialize>(text: &str) -> Option<String> {
    if text.contains("todo!()") || text.contains("unimplemented!()") {
        return Some(
            ToolResult::<R>::err_with_remediation(
                "LAZY_GENERATION_DETECTED: Found a TOESTUB pattern (e.g. todo!() or unimplemented!()) in your code output. You must emit the complete, fully-implemented code. Re-run your action with the actual logic.".to_string(),
                "Complete the skeleton code before validating or submitting.".to_string(),
            )
            .to_json(),
        );
    }
    // Line-anchored keyword checks: only flag when these tokens appear at the
    // start of a (trimmed) line, so substrings inside comments / string literals
    // don't false-positive. `macro_rules!` is a Rust-specific token that has no
    // legitimate use in Vox source even inside comments, so it stays as a global
    // substring.
    if text.contains("macro_rules!")
        || text.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("macro ") || t.starts_with("operator ")
        })
    {
        return Some(
            ToolResult::<R>::err_with_remediation(
                "UNSUPPORTED_SYNTAX: Vox is strictly constrained. Do not use macros or custom syntactic configurability. Use vox-skills for extended actions.".to_string(),
                "Remove custom macros and syntactic configurations. Rewrite using standard syntax and route out-of-band logic through MCP skills.".to_string(),
            )
            .to_json(),
        );
    }
    None
}

/// Validate a .vox file using the full compiler pipeline (lexer → parser → typeck → HIR).
pub async fn validate_file(state: &ServerState, params: ValidateFileParams) -> String {
    let path = match super::workspace_path::resolve_existing_path_in_repository(state, &params.path)
    {
        Ok(p) => p,
        Err(e) => {
            return ToolResult::<ValidateResponse>::err_with_remediation(
                e.message(),
                e.remediation(),
            )
            .to_json();
        }
    };

    let text = match tokio::fs::read_to_string(&path).await {
        Ok(t) => t,
        Err(e) => {
            return ToolResult::<ValidateResponse>::err_with_remediation(
                format!("failed to read file: {e}"),
                super::workspace_path::REM_VALIDATE_IO,
            )
            .to_json();
        }
    };

    if let Some(early) = pre_validation_guard::<ValidateResponse>(&text) {
        return early;
    }

    #[cfg(feature = "oratio-rerank")]
    let correlation_id = vox_oratio::trace::new_correlation_id();
    #[cfg(not(feature = "oratio-rerank"))]
    let correlation_id = uuid::Uuid::new_v4().to_string();
    tracing::debug!(
        target: "vox_mcp_speech",
        correlation_id = %correlation_id,
        path = %params.path,
        bytes = text.len(),
        "validate_file: running HIR validation"
    );

    let diagnostics = vox_lsp::validate_document_with_hir(&text);
    let infos = lsp_diagnostics_to_info(&diagnostics);

    ToolResult::ok(ValidateResponse {
        count: infos.len(),
        diagnostics: infos,
        hir_validation_included: true,
        correlation_id: Some(correlation_id),
    })
    .to_json()
}

/// Validate Vox source passed as a string — no filesystem read. Returns the same shape as
/// [`validate_file`], populating `code` and `fixes` from the underlying `vox-lsp` diagnostics.
/// Hard cap on accepted source bytes. Mirrors the schemars `length(max = 131_072)`
/// constraint on [`ValidateSourceParams::source`], but enforced at the byte level
/// (schemars / JSON Schema `maxLength` counts Unicode code points, not bytes — a
/// 131_072-character string of multi-byte UTF-8 can exceed 128 KiB of payload).
const MAX_VALIDATE_SOURCE_BYTES: usize = 131_072;

pub async fn validate_source(_state: &ServerState, params: ValidateSourceParams) -> String {
    if params.source.len() > MAX_VALIDATE_SOURCE_BYTES {
        return ToolResult::<ValidateResponse>::err_with_remediation(
            format!(
                "SOURCE_TOO_LARGE: vox_validate_source rejects payloads larger than {} bytes (got {} bytes). Split the input or call vox_validate_file with the file on disk.",
                MAX_VALIDATE_SOURCE_BYTES,
                params.source.len()
            ),
            "Trim the source to under 128 KiB or write it to a file and call vox_validate_file."
                .to_string(),
        )
        .to_json();
    }
    if let Some(early) = pre_validation_guard::<ValidateResponse>(&params.source) {
        return early;
    }

    #[cfg(feature = "oratio-rerank")]
    let correlation_id = vox_oratio::trace::new_correlation_id();
    #[cfg(not(feature = "oratio-rerank"))]
    let correlation_id = uuid::Uuid::new_v4().to_string();
    tracing::debug!(
        target: "vox_mcp_speech",
        correlation_id = %correlation_id,
        virtual_path = ?params.virtual_path,
        bytes = params.source.len(),
        "validate_source: running HIR validation"
    );

    let diagnostics = vox_lsp::validate_document_with_hir(&params.source);
    let infos = lsp_diagnostics_to_info(&diagnostics);

    ToolResult::ok(ValidateResponse {
        count: infos.len(),
        diagnostics: infos,
        hir_validation_included: true,
        correlation_id: Some(correlation_id),
    })
    .to_json()
}

/// Check a .vox file and return machine-readable diagnostics (parity with CLI `check --output-format json`).
pub async fn vox_check(state: &ServerState, params: VoxCheckParams) -> String {
    let path = match super::workspace_path::resolve_existing_path_in_repository(state, &params.path)
    {
        Ok(p) => p,
        Err(e) => {
            return ToolResult::<VoxCheckResponse>::err_with_remediation(
                e.message(),
                e.remediation(),
            )
            .to_json();
        }
    };

    let source = match tokio::fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<VoxCheckResponse>::err_with_remediation(
                format!("failed to read file: {e}"),
                super::workspace_path::REM_VALIDATE_IO,
            )
            .to_json();
        }
    };

    let diagnostics = vox_compiler::pipeline::check_file(&source, &params.path);
    let has_errors = diagnostics
        .iter()
        .any(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error);

    ToolResult::ok(VoxCheckResponse {
        count: diagnostics.len(),
        has_errors,
        diagnostics,
    })
    .to_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn parse_json(s: &str) -> Value {
        serde_json::from_str(s).expect("response is valid JSON")
    }

    #[test]
    fn parse_fix_extracts_label_replacement_and_range() {
        let v = serde_json::json!({
            "label": "Add alt attribute",
            "replacement": "alt=\"\"",
            "range": {
                "start": { "line": 3, "character": 5 },
                "end":   { "line": 3, "character": 9 }
            }
        });
        let fix = parse_fix(&v).expect("well-formed fix is parsed");
        assert_eq!(fix.label, "Add alt attribute");
        assert_eq!(fix.replacement, "alt=\"\"");
        assert_eq!((fix.start_line, fix.start_col), (3, 5));
        assert_eq!((fix.end_line, fix.end_col), (3, 9));
    }

    #[test]
    fn parse_fix_returns_none_for_missing_fields() {
        assert!(parse_fix(&serde_json::json!({ "label": "x" })).is_none());
    }

    #[tokio::test]
    async fn validate_source_returns_diagnostics_for_invalid_source() {
        // Construct a minimal ServerState — only `_state` is unused in `validate_source`,
        // so we can pass any valid instance. Use the public test helper if present; otherwise
        // construct via Default.
        let state = ServerState::new_test().await;
        let params = ValidateSourceParams {
            source: "fn ( {".to_string(), // deliberately malformed
            virtual_path: Some("test.vox".to_string()),
        };
        let json = validate_source(&state, params).await;
        let v = parse_json(&json);
        assert_eq!(v["success"], serde_json::Value::Bool(true));
        let count = v["data"]["count"].as_u64().unwrap_or(0);
        assert!(count > 0, "expected at least one diagnostic, got: {json}");
    }

    #[tokio::test]
    async fn validate_source_short_circuits_on_lazy_generation_pattern() {
        let state = ServerState::new_test().await;
        let params = ValidateSourceParams {
            source: "fn x() { todo!() }".to_string(),
            virtual_path: None,
        };
        let json = validate_source(&state, params).await;
        let v = parse_json(&json);
        assert_eq!(v["success"], serde_json::Value::Bool(false));
        assert!(
            v["error"]
                .as_str()
                .unwrap_or("")
                .contains("LAZY_GENERATION_DETECTED"),
            "expected LAZY_GENERATION_DETECTED, got: {json}"
        );
    }

    #[test]
    fn lsp_diagnostics_to_info_extracts_code_and_fixes() {
        use tower_lsp_server::ls_types::{
            Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
        };
        let d = Diagnostic {
            range: Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 4,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("web_ir.test.code".to_string())),
            code_description: None,
            source: Some("vox-lsp".to_string()),
            message: "test".to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({
                "suggestions": [],
                "fixes": [{
                    "label": "Replace foo with bar",
                    "replacement": "bar",
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end":   { "line": 1, "character": 3 }
                    }
                }]
            })),
        };
        let infos = lsp_diagnostics_to_info(&[d]);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].code.as_deref(), Some("web_ir.test.code"));
        assert_eq!(infos[0].fixes.len(), 1);
        assert_eq!(infos[0].fixes[0].label, "Replace foo with bar");
        assert_eq!(infos[0].fixes[0].replacement, "bar");
    }
}
