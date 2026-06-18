//! Invoke federated workspace @tool / @resource functions via the Vox interpreter.

use std::path::Path;

use vox_compiler::eval::{Interpreter, value::VoxValue};
use vox_compiler::hir::HirFn;
use vox_compiler::hir::HirMcpTool;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

use super::WorkspaceMcpSurface;
use crate::params::ToolResult;

/// Dispatch a federated workspace tool by name.
pub fn dispatch_workspace_tool(
    surface: &WorkspaceMcpSurface,
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<String, String> {
    let entry = surface
        .tool_by_name(tool_name)
        .ok_or_else(|| format!("Unknown workspace tool: {tool_name}"))?;
    let source = std::fs::read_to_string(&entry.source_path)
        .map_err(|e| format!("read {}: {e}", entry.source_path.display()))?;
    let result = invoke_mcp_tool_fn(&entry.source_path, &source, tool_name, args)?;
    Ok(ToolResult::ok(result).to_json())
}

/// Read a federated workspace resource URI by invoking its nullary @mcp.resource fn.
pub fn dispatch_workspace_resource(
    surface: &WorkspaceMcpSurface,
    uri: &str,
) -> Result<String, String> {
    let entry = surface
        .resource_by_uri(uri)
        .ok_or_else(|| format!("Unknown workspace resource: {uri}"))?;
    let source = std::fs::read_to_string(&entry.source_path)
        .map_err(|e| format!("read {}: {e}", entry.source_path.display()))?;
    invoke_mcp_resource_fn(&entry.source_path, &source, &entry.func_name)
}

fn invoke_mcp_resource_fn(
    source_path: &Path,
    source: &str,
    func_name: &str,
) -> Result<String, String> {
    let tokens = lex(source);
    let module = parse(tokens).map_err(|errs| {
        errs.iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let hir = lower_module(&module);
    let resource = hir
        .mcp_resources
        .iter()
        .find(|r| r.func.name == func_name)
        .ok_or_else(|| {
            format!(
                "resource fn {func_name} not found in {}",
                source_path.display()
            )
        })?;

    let mut interp = Interpreter::new(100_000);
    interp.set_source_path(source_path);
    interp
        .run_module(&hir)
        .map_err(|e| format!("run_module: {e:?}"))?;
    register_mcp_fn_from_hir_fn(&mut interp, &resource.func);

    let out = interp
        .call(func_name, vec![])
        .map_err(|e| format!("call {func_name}: {e:?}"))?;
    voxvalue_to_string(out)
}

fn invoke_mcp_tool_fn(
    source_path: &Path,
    source: &str,
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let tokens = lex(source);
    let module = parse(tokens).map_err(|errs| {
        errs.iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let hir = lower_module(&module);
    let tool = hir
        .mcp_tools
        .iter()
        .find(|t| t.func.name == tool_name)
        .ok_or_else(|| format!("tool {tool_name} not found in {}", source_path.display()))?;

    let mut interp = Interpreter::new(100_000);
    interp.set_source_path(source_path);
    interp
        .run_module(&hir)
        .map_err(|e| format!("run_module: {e:?}"))?;
    register_mcp_fn(&mut interp, tool);

    let vox_args = json_args_to_vox(tool, args)?;
    let out = interp
        .call(tool_name, vox_args)
        .map_err(|e| format!("call {tool_name}: {e:?}"))?;
    voxvalue_to_json(out)
}

fn register_mcp_fn(interp: &mut Interpreter, tool: &HirMcpTool) {
    register_mcp_fn_from_hir_fn(interp, &tool.func);
}

fn register_mcp_fn_from_hir_fn(interp: &mut Interpreter, f: &HirFn) {
    let val = VoxValue::Fn {
        params: f.params.iter().map(|p| p.name.clone()).collect(),
        body: std::rc::Rc::new(f.body.clone()),
        env: interp.scope.clone(),
        name: f.name.clone(),
        is_versioned: f.is_versioned,
        is_traced: f.is_traced,
    };
    interp.scope.set(f.name.clone(), val.clone());
    interp.module_scope.set(f.name.clone(), val);
}

fn json_args_to_vox(tool: &HirMcpTool, args: &serde_json::Value) -> Result<Vec<VoxValue>, String> {
    let obj = args
        .as_object()
        .ok_or_else(|| "args must be a JSON object".to_string())?;
    tool.func
        .params
        .iter()
        .map(|p| {
            let v = obj
                .get(&p.name)
                .ok_or_else(|| format!("missing arg: {}", p.name))?;
            json_to_vox_value(v)
        })
        .collect()
}

fn json_to_vox_value(v: &serde_json::Value) -> Result<VoxValue, String> {
    match v {
        serde_json::Value::Null => Ok(VoxValue::Null),
        serde_json::Value::Bool(b) => Ok(VoxValue::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(VoxValue::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(VoxValue::Float(f))
            } else {
                Err("unsupported number".to_string())
            }
        }
        serde_json::Value::String(s) => Ok(VoxValue::Str(s.clone())),
        serde_json::Value::Array(items) => {
            let vals: Result<Vec<_>, _> = items.iter().map(json_to_vox_value).collect();
            Ok(VoxValue::list(vals?))
        }
        serde_json::Value::Object(_) => {
            Err("object args not supported for workspace MCP".to_string())
        }
    }
}

fn voxvalue_to_json(v: VoxValue) -> Result<serde_json::Value, String> {
    match v {
        VoxValue::Int(i) => Ok(serde_json::json!(i)),
        VoxValue::Float(f) => Ok(serde_json::json!(f)),
        VoxValue::Str(s) => Ok(serde_json::json!(s)),
        VoxValue::Bool(b) => Ok(serde_json::json!(b)),
        VoxValue::Null => Ok(serde_json::Value::Null),
        VoxValue::List(items) => {
            let vals: Result<Vec<_>, _> = items.iter().cloned().map(voxvalue_to_json).collect();
            Ok(serde_json::Value::Array(vals?))
        }
        VoxValue::Result(Ok(inner)) => voxvalue_to_json(*inner),
        VoxValue::Result(Err(inner)) => Err(format!("tool returned error: {inner:?}")),
        other => Err(format!("unsupported return type: {other:?}")),
    }
}

fn voxvalue_to_string(v: VoxValue) -> Result<String, String> {
    match v {
        VoxValue::Str(s) => Ok(s),
        VoxValue::Result(Ok(inner)) => voxvalue_to_string(*inner),
        VoxValue::Result(Err(inner)) => Err(format!("resource returned error: {inner:?}")),
        other => Err(format!("resource must return str, got: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_mcp::{WorkspaceMcpLoader, load_scan_config};
    use std::path::Path;

    #[test]
    fn dispatch_read_file_fixture() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let result = WorkspaceMcpLoader::load_repo(repo, &load_scan_config(repo)).unwrap();
        let json = dispatch_workspace_tool(
            &result.surface,
            "read_file",
            &serde_json::json!({ "path": "README.md" }),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["success"], true);
    }

    #[test]
    fn dispatch_golden_mcp_status_resource() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let result = WorkspaceMcpLoader::load_repo(repo, &load_scan_config(repo)).unwrap();
        result
            .surface
            .resource_by_uri("vox://golden/mcp-status")
            .expect("golden resource federated");
        let text =
            dispatch_workspace_resource(&result.surface, "vox://golden/mcp-status").expect("read");
        assert_eq!(text, "ok");
    }
}
