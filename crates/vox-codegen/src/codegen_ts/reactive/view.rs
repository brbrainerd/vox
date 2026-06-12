use crate::codegen_ts::hir_emit::{EmitCtx, emit_hir_expr};
use crate::web_ir::{WebIrDiagnostic, WebIrModule};
use vox_compiler::hir::*;
fn web_ir_reactive_trace_enabled() -> bool {
    std::env::var("VOX_WEBIR_REACTIVE_TRACE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Which codegen path classified the reactive `view:` body after Web IR preview emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactiveViewEmitPathway {
    /// Clean Web IR TSX emitted; whitespace-normalized string ≠ legacy `emit_hir_expr` (stats-only).
    WebIrViewEmittedParityMismatch,
    /// Web IR preview TSX used for the view body (parity match).
    WebIrViewEmitted,
}

#[derive(Debug, Clone, Default)]
pub struct ReactiveViewBridgeStats {
    pub web_ir_view_emitted_parity_mismatch: u64,
    pub web_ir_view_emitted: u64,
    /// Blocking Web IR validate failures and missing view roots for this codegen run.
    pub reactive_view_emit_failures: Vec<WebIrDiagnostic>,
}

impl ReactiveViewBridgeStats {
    pub fn record_pathway(&mut self, p: ReactiveViewEmitPathway) {
        match p {
            ReactiveViewEmitPathway::WebIrViewEmittedParityMismatch => {
                self.web_ir_view_emitted_parity_mismatch += 1
            }
            ReactiveViewEmitPathway::WebIrViewEmitted => self.web_ir_view_emitted += 1,
        }
    }
}

/// Whitespace normalization for the reactive view parity guard (OP-0261 / OP-0179).
#[doc(hidden)]
pub fn normalize_reactive_view_jsx_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join("")
}

fn indent_view_for_return(view: &str) -> String {
    let pad = "    ";
    view.trim_end()
        .lines()
        .map(|line| {
            let t = line.trim_end();
            if t.is_empty() {
                String::new()
            } else {
                format!("{pad}{t}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn emit_reactive_view_body(
    component_name: &str,
    rc: &HirReactiveComponent,
    ctx: &EmitCtx<'_>,
    web: &WebIrModule,
    stats: &mut ReactiveViewBridgeStats,
) -> String {
    let Some(view) = &rc.view else {
        return String::new();
    };
    const FAIL_PLACEHOLDER: &str = "<>{/* webir reactive view emit failed */}</>";
    let diags = crate::web_ir::validate::validate_web_ir(web);
    let error_diags: Vec<WebIrDiagnostic> = diags
        .into_iter()
        .filter(|d| !crate::web_ir::validate::is_advisory_diagnostic(d))
        .collect();
    if !error_diags.is_empty() {
        stats.reactive_view_emit_failures.extend(error_diags);
        if web_ir_reactive_trace_enabled() {
            eprintln!(
                "[vox-webir-reactive] component={component_name} pathway=WebIrValidateFailedFailFast"
            );
        }
        return indent_view_for_return(FAIL_PLACEHOLDER);
    }
    let Some(tsx) = crate::web_ir::emit_tsx::emit_component_view_tsx(web, &rc.name) else {
        stats.reactive_view_emit_failures.push(WebIrDiagnostic {
            code: "codegen.reactive.no_web_ir_view_root".to_string(),
            message: format!("no Web IR view root for reactive component `{component_name}`"),
            span: None,
            category: Some("reactive".to_string()),
        });
        if web_ir_reactive_trace_enabled() {
            eprintln!(
                "[vox-webir-reactive] component={component_name} pathway=NoWebIrViewRootFailFast"
            );
        }
        return indent_view_for_return(FAIL_PLACEHOLDER);
    };
    let legacy = emit_hir_expr(view, ctx);
    let n_legacy = normalize_reactive_view_jsx_ws(&legacy);
    let n_tsx = normalize_reactive_view_jsx_ws(&tsx);
    let pathway = if n_legacy == n_tsx {
        ReactiveViewEmitPathway::WebIrViewEmitted
    } else {
        ReactiveViewEmitPathway::WebIrViewEmittedParityMismatch
    };
    stats.record_pathway(pathway);
    if web_ir_reactive_trace_enabled() {
        let label = if n_legacy == n_tsx {
            "WebIrViewEmitted"
        } else {
            "WebIrViewEmittedParityMismatch"
        };
        eprintln!("[vox-webir-reactive] component={component_name} pathway={label}");
    }
    // Convergence policy: Web IR output is canonical; parity is tracked for CI / migration only.
    indent_view_for_return(&tsx)
}

