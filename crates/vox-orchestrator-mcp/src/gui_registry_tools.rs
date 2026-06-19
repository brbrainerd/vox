use crate::params::ToolResult;
use crate::server_state::ServerState;
use serde_json::Value;

/// Retrieve the components registered in the primitive component registry.
pub async fn vox_gui_components(state: &ServerState, _args: serde_json::Value) -> String {
    let repo_root = &state.repository.root;
    let registry_path = repo_root.join("contracts/gui/component-registry.v1.json");

    match std::fs::read_to_string(&registry_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(val) => ToolResult::ok(val).to_json(),
            Err(e) => {
                ToolResult::<Value>::err(format!("Failed to parse component registry JSON: {e}"))
                    .to_json()
            }
        },
        Err(e) => {
            ToolResult::<Value>::err(format!("Failed to read component registry: {e}")).to_json()
        }
    }
}

/// Validate a Vox/VUV source string against the compile-time GUI guarantees
/// (contrast, layer-occlusion, a11y, structural web-IR checks) WITHOUT writing
/// any files — the external-validation API an AI UI generator calls to check its
/// output before a human sees it. Pipeline: lex → parse → lower_module (HIR) →
/// lower_hir_to_web_ir → validate_web_ir_with_registry. Returns `{ ok,
/// error_count, diagnostic_count, diagnostics[] }`. See design §3b / Track C.
pub async fn vox_validate_vuv(_state: &ServerState, args: serde_json::Value) -> String {
    let Some(source) = args.get("source").and_then(Value::as_str) else {
        return ToolResult::<Value>::err("missing required string field 'source'".to_string())
            .to_json();
    };

    let tokens = vox_compiler::lexer::cursor::lex(source);
    let module = match vox_compiler::parser::parse(tokens) {
        Ok(m) => m,
        Err(e) => {
            return ToolResult::<Value>::err(format!("parse error: {e:?}")).to_json();
        }
    };
    let hir = vox_compiler::hir::lower_module(&module);
    let (web_ir, _summary) =
        vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary(&hir);
    let diags = vox_codegen::web_ir::validate::validate_web_ir_with_registry(&web_ir, None);

    use vox_codegen::web_ir::WebIrDiagnosticSeverity;
    let error_count = diags
        .iter()
        .filter(|d| matches!(d.severity(), WebIrDiagnosticSeverity::Error))
        .count();
    let diagnostics: Vec<Value> = diags
        .iter()
        .map(|d| {
            serde_json::json!({
                "code": d.code,
                "message": d.message,
                "severity": format!("{:?}", d.severity()),
                "category": d.category,
            })
        })
        .collect();

    ToolResult::ok(serde_json::json!({
        "ok": error_count == 0,
        "error_count": error_count,
        "diagnostic_count": diags.len(),
        "diagnostics": diagnostics,
    }))
    .to_json()
}

/// Retrieve design tokens in W3C DTCG format.
pub async fn vox_gui_tokens(state: &ServerState, _args: serde_json::Value) -> String {
    let repo_root = &state.repository.root;
    let tokens_path = repo_root.join("vox.tokens.json");

    match std::fs::read_to_string(&tokens_path) {
        Ok(content) => match vox_compiler::tokens::TokenRegistry::load_from_str(&content) {
            Ok(reg) => {
                let dtcg_val = vox_codegen::codegen_ts::token_export::export_to_dtcg(&reg);
                ToolResult::ok(dtcg_val).to_json()
            }
            Err(e) => {
                ToolResult::<Value>::err(format!("Failed to parse vox.tokens.json: {e}")).to_json()
            }
        },
        Err(e) => {
            ToolResult::<Value>::err(format!("Failed to read vox.tokens.json: {e}")).to_json()
        }
    }
}
