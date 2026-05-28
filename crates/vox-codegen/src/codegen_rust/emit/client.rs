use vox_compiler::ast::scalar_mapping::VoxScalar;
use vox_compiler::hir::{HirModule, HirType};

pub fn emit_api_client(module: &HirModule) -> String {
    if module.endpoint_fns.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("// Auto-generated API client for Vox server functions\n");
    out.push_str("// @query → GET + JSON query values; @server / @mutation → POST + JSON body.\n");
    out.push_str("// Do not edit manually — regenerated on each build.\n\n");

    out.push_str("const API_BASE = '';\n\n");

    for sf in &module.endpoint_fns {
        let is_query = sf.kind == vox_compiler::hir::HirEndpointKind::Query;
        out.push_str(&emit_api_ts_fn(sf, is_query));
    }

    out
}

fn emit_api_ts_fn(sf: &vox_compiler::hir::HirEndpointFn, is_query: bool) -> String {
    let params: Vec<String> = sf
        .params
        .iter()
        .map(|p| {
            let ts_type = p
                .type_ann
                .as_ref()
                .map_or("any".to_string(), hir_type_to_ts);
            format!("{}: {}", p.name, ts_type)
        })
        .collect();

    let return_ts = sf
        .return_type
        .as_ref()
        .map_or("any".to_string(), hir_type_to_ts);

    let mut out = String::new();
    out.push_str(&format!(
        "export async function {}({}): Promise<{}> {{\n",
        sf.name,
        params.join(", "),
        return_ts,
    ));
    let path = &sf.route_path;
    if is_query {
        if sf.params.is_empty() {
            out.push_str(&format!(
                "  const response = await fetch(`${{API_BASE}}{path}`, {{ method: 'GET' }});\n"
            ));
        } else {
            let obj_fields: Vec<String> = sf
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.name))
                .collect();
            out.push_str(&format!(
                "  const q: Record<string, unknown> = {{ {} }};\n",
                obj_fields.join(", ")
            ));
            out.push_str("  const sorted = Object.keys(q).sort();\n");
            out.push_str("  const qs = sorted.length ? ('?' + sorted.map(k => `${encodeURIComponent(k)}=${encodeURIComponent(JSON.stringify((q as Record<string, unknown>)[k]))}`).join('&')) : '';\n");
            out.push_str(&format!(
                "  const response = await fetch(`${{API_BASE}}{path}${{qs}}`, {{ method: 'GET' }});\n"
            ));
        }
    } else {
        out.push_str(&format!(
            "  const response = await fetch(`${{API_BASE}}{path}`, {{\n",
        ));
        out.push_str("    method: 'POST',\n");
        out.push_str("    headers: { 'Content-Type': 'application/json' },\n");
        let body_fields: Vec<String> = sf.params.iter().map(|p| p.name.clone()).collect();
        out.push_str(&format!(
            "    body: JSON.stringify({{ {} }}),\n",
            body_fields.join(", ")
        ));
        out.push_str("  });\n");
    }
    out.push_str("  if (!response.ok) throw new Error(`Server error: ${response.status}`);\n");
    out.push_str("  return response.json();\n");
    out.push_str("}\n\n");
    out
}

fn hir_type_to_ts(ty: &HirType) -> String {
    match ty {
        HirType::Named(name) => {
            if let Some(s) = VoxScalar::parse(name) {
                s.as_ts_primitive().to_string()
            } else {
                match name.as_str() {
                    "Unit" => "void".to_string(),
                    other => other.to_string(),
                }
            }
        }
        HirType::Generic(name, args) => {
            let args_str: Vec<String> = args.iter().map(hir_type_to_ts).collect();
            match name.as_str() {
                "list" | "List" => format!("{}[]", args_str.first().unwrap_or(&"any".to_string())),
                _ => format!("{}<{}>", name, args_str.join(", ")),
            }
        }
        HirType::Function(params, ret) => {
            let params_str: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| format!("arg{}: {}", i, hir_type_to_ts(p)))
                .collect();
            format!("({}) => {}", params_str.join(", "), hir_type_to_ts(ret))
        }
        HirType::Tuple(elems) => {
            let elems_str: Vec<String> = elems.iter().map(hir_type_to_ts).collect();
            format!("[{}]", elems_str.join(", "))
        }
        HirType::Unit => "void".to_string(),
        HirType::Decimal => "string".to_string(),
    }
}

/// Escape content for a Rust string literal (`"…"`).
fn rust_escape_double_quoted(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// JSON Schema object for one `inputSchema.properties` entry (emitted as JSON text inside generated `json!`).
fn hir_type_json_schema_property(type_ann: Option<&HirType>) -> String {
    match type_ann {
        Some(HirType::Named(t)) if t == "String" || t == "str" => {
            r#"{ "type": "string" }"#.to_string()
        }
        Some(HirType::Named(t)) if t == "i64" || t == "int" => {
            r#"{ "type": "integer" }"#.to_string()
        }
        Some(HirType::Named(t)) if t == "f64" || t == "float" => {
            r#"{ "type": "number" }"#.to_string()
        }
        Some(HirType::Named(t)) if t == "bool" => r#"{ "type": "boolean" }"#.to_string(),
        Some(HirType::Generic(name, args)) if name == "list" || name == "List" => {
            let item = args
                .first()
                .map(|a| hir_type_json_schema_property(Some(a)))
                .unwrap_or_else(|| r#"{ "type": "string" }"#.to_string());
            format!(r#"{{ "type": "array", "items": {item} }}"#)
        }
        Some(HirType::Tuple(elems)) => {
            if elems.is_empty() {
                return r#"{ "type": "array", "maxItems": 0 }"#.to_string();
            }
            let items: Vec<_> = elems
                .iter()
                .map(|e| hir_type_json_schema_property(Some(e)))
                .collect();
            let joined = items.join(", ");
            let n = elems.len();
            format!(
                r#"{{ "type": "array", "prefixItems": [{joined}], "minItems": {n}, "maxItems": {n} }}"#
            )
        }
        Some(HirType::Unit) => r#"{ "type": "null" }"#.to_string(),
        Some(HirType::Decimal) => r#"{ "type": "string" }"#.to_string(),
        Some(HirType::Function(_, _)) => r#"{ "type": "string" }"#.to_string(),
        Some(HirType::Named(_)) => r#"{ "type": "string" }"#.to_string(),
        Some(HirType::Generic(_, _)) => r#"{ "type": "string" }"#.to_string(),
        None => r#"{ "type": "string" }"#.to_string(),
    }
}

/// Emit the Rust expression that coerces a `serde_json::Value` MCP argument
/// to the concrete type implied by `type_ann`. Extracted from the inline
/// match closure in `emit_mcp_server` per CR-A1 (5 if-guarded arms → helper).
fn mcp_coerce_param_expr(name: &str, type_ann: Option<&HirType>) -> String {
    match type_ann {
        Some(HirType::Named(t)) if t == "String" || t == "str" => {
            format!("{name}.as_str().unwrap_or_default().to_string()")
        }
        Some(HirType::Named(t)) if t == "i64" || t == "int" => {
            format!("{name}.as_i64().unwrap_or(0)")
        }
        Some(HirType::Named(t)) if t == "f64" || t == "float" => {
            format!("{name}.as_f64().unwrap_or(0.0)")
        }
        Some(HirType::Named(t)) if t == "bool" => {
            format!("{name}.as_bool().unwrap_or(false)")
        }
        Some(HirType::Named(t)) if t == "dec" => {
            format!("rust_decimal::Decimal::from_str_exact({name}.as_str().unwrap_or(\"0\")).unwrap_or_default()")
        }
        Some(HirType::Decimal) => {
            format!("rust_decimal::Decimal::from_str_exact({name}.as_str().unwrap_or(\"0\")).unwrap_or_default()")
        }
        _ => name.to_string(),
    }
}

/// Generate the MCP (Model Context Protocol) JSON-RPC server binary.
/// Communicates over stdio: reads JSON-RPC requests from stdin, writes responses to stdout.
///
/// **Scope:** per-generated-crate tools from `@mcp.tool` and resources from `@mcp.resource` only.
/// The shipped `vox-mcp` binary uses [`contracts/mcp/tool-registry.canonical.yaml`](../../../../contracts/mcp/tool-registry.canonical.yaml).
/// Architecture SSOT: `docs/src/architecture/mcp-vox-language-exposure.md`.
pub fn emit_mcp_server(module: &HirModule, package_name: &str) -> String {
    let crate_name = package_name.replace('-', "_");
    let mut out = String::new();

    out.push_str("// MCP Server — Generated by Vox Compiler\n");
    out.push_str("// Implements the Model Context Protocol (JSON-RPC 2.0 over stdio)\n\n");
    out.push_str("use serde_json::{json, Value};\n");
    out.push_str("use std::io::{self, BufRead, Write};\n");
    out.push_str(&format!("use {}::*;\n\n", crate_name));

    // Tool dispatch function
    out.push_str("fn dispatch_tool(name: &str, args: &Value) -> Result<Value, String> {\n");
    out.push_str("    match name {\n");
    for tool in &module.mcp_tools {
        let fn_name = &tool.func.name;
        out.push_str(&format!("        \"{}\" => {{\n", fn_name));
        // Extract args
        for param in &tool.func.params {
            let p = &param.name;
            out.push_str(&format!(
                "            let {p} = args.get(\"{p}\").cloned().unwrap_or(Value::Null);\n"
            ));
        }
        // Call function — for now, wrap result in Ok(json)
        let arg_list: Vec<String> = tool
            .func
            .params
            .iter()
            .map(|p| mcp_coerce_param_expr(&p.name, p.type_ann.as_ref()))
            .collect();
        out.push_str(&format!(
            "            let result = {}({});\n",
            fn_name,
            arg_list.join(", ")
        ));
        out.push_str("            Ok(serde_json::to_value(result).unwrap_or(Value::Null))\n");
        out.push_str("        }\n");
    }
    out.push_str("        _ => Err(format!(\"Unknown tool: {}\", name)),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // Build tool list JSON
    out.push_str("fn tool_list() -> Value {\n");
    out.push_str("    json!({\n");
    out.push_str("        \"tools\": [\n");
    for (i, tool) in module.mcp_tools.iter().enumerate() {
        let trailing = if i + 1 < module.mcp_tools.len() {
            ","
        } else {
            ""
        };
        out.push_str("            {\n");
        out.push_str(&format!(
            "                \"name\": \"{}\",\n",
            tool.func.name
        ));
        out.push_str(&format!(
            "                \"description\": \"{}\",\n",
            tool.description.replace('"', "\\\"")
        ));
        // Build inputSchema from params
        out.push_str("                \"inputSchema\": {\n");
        out.push_str("                    \"type\": \"object\",\n");
        out.push_str("                    \"properties\": {\n");
        for (j, param) in tool.func.params.iter().enumerate() {
            let schema = hir_type_json_schema_property(param.type_ann.as_ref());
            let param_trailing = if j + 1 < tool.func.params.len() {
                ","
            } else {
                ""
            };
            out.push_str(&format!(
                "                        \"{}\": {}{}\n",
                param.name, schema, param_trailing
            ));
        }
        out.push_str("                    },\n");
        // All params required
        let required: Vec<String> = tool
            .func
            .params
            .iter()
            .map(|p| format!("\"{}\"", p.name))
            .collect();
        out.push_str(&format!(
            "                    \"required\": [{}]\n",
            required.join(", ")
        ));
        out.push_str("                }\n");
        out.push_str(&format!("            }}{}\n", trailing));
    }
    out.push_str("        ]\n");
    out.push_str("    })\n");
    out.push_str("}\n\n");

    if !module.mcp_resources.is_empty() {
        out.push_str("// Resource dispatch (nullary fns only; URI match)\n");
        out.push_str("fn dispatch_resource(uri: &str) -> Result<Value, String> {\n");
        out.push_str("    match uri {\n");
        for res in &module.mcp_resources {
            let lit = rust_escape_double_quoted(&res.uri);
            let fn_name = &res.func.name;
            out.push_str(&format!(
                "        \"{lit}\" => {{\n            let result = {fn_name}();\n            Ok(serde_json::to_value(result).unwrap_or(Value::Null))\n        }}\n"
            ));
        }
        out.push_str("        _ => Err(format!(\"Unknown resource URI: {}\", uri)),\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");

        out.push_str("fn resource_list() -> Value {\n");
        out.push_str("    json!({\n");
        out.push_str("        \"resources\": [\n");
        for (i, res) in module.mcp_resources.iter().enumerate() {
            let trailing = if i + 1 < module.mcp_resources.len() {
                ","
            } else {
                ""
            };
            let ue = rust_escape_double_quoted(&res.uri);
            let ne = rust_escape_double_quoted(&res.func.name);
            let de = rust_escape_double_quoted(&res.description);
            out.push_str("            {\n");
            out.push_str(&format!("                \"uri\": \"{ue}\",\n"));
            out.push_str(&format!("                \"name\": \"{ne}\",\n"));
            out.push_str(&format!("                \"description\": \"{de}\",\n"));
            out.push_str("                \"mimeType\": \"text/plain\"\n");
            out.push_str(&format!("            }}{trailing}\n"));
        }
        out.push_str("        ]\n");
        out.push_str("    })\n");
        out.push_str("}\n\n");
    }

    // Main loop — JSON-RPC over stdio
    out.push_str("fn main() {\n");
    out.push_str("    let stdin = io::stdin();\n");
    out.push_str("    let mut stdout = io::stdout();\n");
    out.push_str("    for line in stdin.lock().lines() {\n");
    out.push_str("        let line = match line {\n");
    out.push_str("            Ok(l) => l,\n");
    out.push_str("            Err(_) => break,\n");
    out.push_str("        };\n");
    out.push_str("        if line.trim().is_empty() { continue; }\n");
    out.push_str("        let request: Value = match serde_json::from_str(&line) {\n");
    out.push_str("            Ok(v) => v,\n");
    out.push_str("            Err(e) => {\n");
    out.push_str("                let err = json!({\n");
    out.push_str("                    \"jsonrpc\": \"2.0\",\n");
    out.push_str("                    \"error\": { \"code\": -32700, \"message\": format!(\"Parse error: {e}\") },\n");
    out.push_str("                    \"id\": null\n");
    out.push_str("                });\n");
    out.push_str("                writeln!(stdout, \"{}\", err).ok();\n");
    out.push_str("                stdout.flush().ok();\n");
    out.push_str("                continue;\n");
    out.push_str("            }\n");
    out.push_str("        };\n");
    out.push_str("        let id = request.get(\"id\").cloned().unwrap_or(Value::Null);\n");
    out.push_str(
        "        let method = request.get(\"method\").and_then(|m| m.as_str()).unwrap_or(\"\");\n",
    );
    out.push_str("        let params = request.get(\"params\").cloned().unwrap_or(json!({}));\n\n");
    emit_mcp_method_dispatch(&mut out, module, package_name);
    out.push_str("        writeln!(stdout, \"{}\", response).ok();\n");
    out.push_str("        stdout.flush().ok();\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    out
}

/// Emit the `let response = match method { … };` block for the MCP stdio loop.
/// Extracted from `emit_mcp_server` per CR-A1: the inline block had ~14 decision
/// points (initialize if, tools/call inner match, resources conditional block).
fn emit_mcp_method_dispatch(out: &mut String, module: &HirModule, package_name: &str) {
    out.push_str("        let response = match method {\n");
    // initialize
    out.push_str("            \"initialize\" => json!({\n");
    out.push_str("                \"jsonrpc\": \"2.0\",\n");
    out.push_str("                \"id\": id,\n");
    out.push_str("                \"result\": {\n");
    out.push_str("                    \"protocolVersion\": \"2024-11-05\",\n");
    if module.mcp_resources.is_empty() {
        out.push_str("                    \"capabilities\": { \"tools\": {} },\n");
    } else {
        out.push_str("                    \"capabilities\": { \"tools\": {}, \"resources\": { \"subscribe\": false } },\n");
    }
    out.push_str(&format!(
        "                    \"serverInfo\": {{ \"name\": \"{}\", \"version\": \"0.1.0\" }}\n",
        package_name
    ));
    out.push_str("                }\n");
    out.push_str("            }),\n");
    // tools/list
    out.push_str("            \"tools/list\" => {\n");
    out.push_str("                let tools = tool_list();\n");
    out.push_str("                json!({\n");
    out.push_str("                    \"jsonrpc\": \"2.0\",\n");
    out.push_str("                    \"id\": id,\n");
    out.push_str("                    \"result\": tools\n");
    out.push_str("                })\n");
    out.push_str("            },\n");
    // tools/call
    emit_mcp_tools_call_arm(out);
    // resources/list + resources/read (only when module has resources)
    if !module.mcp_resources.is_empty() {
        emit_mcp_resource_arms(out);
    }
    // notifications (no-ops)
    out.push_str(
        "            \"notifications/initialized\" | \"notifications/cancelled\" => continue,\n",
    );
    // unknown method
    out.push_str("            _ => json!({\n");
    out.push_str("                \"jsonrpc\": \"2.0\",\n");
    out.push_str("                \"id\": id,\n");
    out.push_str("                \"error\": { \"code\": -32601, \"message\": format!(\"Method not found: {}\", method) }\n");
    out.push_str("            }),\n");
    out.push_str("        };\n\n");
}

fn emit_mcp_tools_call_arm(out: &mut String) {
    out.push_str("            \"tools/call\" => {\n");
    out.push_str("                let tool_name = params.get(\"name\").and_then(|n| n.as_str()).unwrap_or(\"\");\n");
    out.push_str("                let tool_args = params.get(\"arguments\").cloned().unwrap_or(json!({}));\n");
    out.push_str("                match dispatch_tool(tool_name, &tool_args) {\n");
    out.push_str("                    Ok(result) => json!({\n");
    out.push_str("                        \"jsonrpc\": \"2.0\",\n");
    out.push_str("                        \"id\": id,\n");
    out.push_str("                        \"result\": {\n");
    out.push_str("                            \"content\": [{ \"type\": \"text\", \"text\": serde_json::to_string(&result).unwrap_or_default() }]\n");
    out.push_str("                        }\n");
    out.push_str("                    }),\n");
    out.push_str("                    Err(e) => json!({\n");
    out.push_str("                        \"jsonrpc\": \"2.0\",\n");
    out.push_str("                        \"id\": id,\n");
    out.push_str("                        \"result\": {\n");
    out.push_str("                            \"isError\": true,\n");
    out.push_str(
        "                            \"content\": [{ \"type\": \"text\", \"text\": e }]\n",
    );
    out.push_str("                        }\n");
    out.push_str("                    }),\n");
    out.push_str("                }\n");
    out.push_str("            },\n");
}

fn emit_mcp_resource_arms(out: &mut String) {
    out.push_str("            \"resources/list\" => {\n");
    out.push_str("                let r = resource_list();\n");
    out.push_str("                json!({\n");
    out.push_str("                    \"jsonrpc\": \"2.0\",\n");
    out.push_str("                    \"id\": id,\n");
    out.push_str("                    \"result\": r\n");
    out.push_str("                })\n");
    out.push_str("            },\n");
    out.push_str("            \"resources/read\" => {\n");
    out.push_str(
        "                let uri = params.get(\"uri\").and_then(|u| u.as_str()).unwrap_or(\"\");\n",
    );
    out.push_str("                match dispatch_resource(uri) {\n");
    out.push_str("                    Ok(val) => {\n");
    out.push_str(
        "                        let text = serde_json::to_string(&val).unwrap_or_default();\n",
    );
    out.push_str("                        json!({\n");
    out.push_str("                            \"jsonrpc\": \"2.0\",\n");
    out.push_str("                            \"id\": id,\n");
    out.push_str("                            \"result\": {\n");
    out.push_str(
        "                                \"contents\": [{ \"uri\": uri, \"mimeType\": \"text/plain\", \"text\": text }]\n",
    );
    out.push_str("                            }\n");
    out.push_str("                        })\n");
    out.push_str("                    }\n");
    out.push_str("                    Err(e) => json!({\n");
    out.push_str("                        \"jsonrpc\": \"2.0\",\n");
    out.push_str("                        \"id\": id,\n");
    out.push_str("                        \"error\": { \"code\": -32602, \"message\": e }\n");
    out.push_str("                    }),\n");
    out.push_str("                }\n");
    out.push_str("            },\n");
}
