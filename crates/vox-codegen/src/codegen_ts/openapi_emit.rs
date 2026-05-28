//! OpenAPI 3.1 emit driven by Contract IR.
//!
//! Reflects the [Wire Format v1 SSOT](../../../../../docs/src/architecture/wire-format-v1-ssot.md):
//! - Paths are absolute from the API host root (`/api/query/…`, `/api/mutation/…`, `/api/<server>`).
//! - `servers[0].url` is empty so OpenAPI tooling composes URLs without duplicating `/api`.
//! - Query endpoints → `GET`, mutations / server-fns → `POST`
//! - `Decimal` / `BigInt` → `type: string`
//! - `DateTime` → `type: string, format: date-time`
//! - Sum types → `oneOf` with shared `_tag` discriminant
//! - `Option<T>` → property absent from `required` list
//!
//! Output is canonical: identical Contract IR produces byte-identical bytes
//! (sorted keys via `serde_json::to_string_pretty` on `IndexMap`-like
//! ordering preserved by `BTreeMap`).
//!
//! See [`docs/src/architecture/external-frontend-interop-plan-2026.md`](../../../../../docs/src/architecture/external-frontend-interop-plan-2026.md)
//! Phase 2 — this is the artifact that unlocks `openapi-typescript`,
//! Orval, RTK Query, Postman, and similar TS consumers.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use vox_compiler::contract_ir::{
    ContractEndpoint, ContractIr, ContractType, ContractTypeKind, ContractVariant, HttpMethod,
    WireType,
};
use vox_compiler::hir::HirModule;

/// Emit an OpenAPI 3.1 specification for a Vox HIR module.
///
/// Returns canonical pretty-printed JSON.
pub fn generate_openapi(hir: &HirModule, package_name: &str, package_version: &str) -> String {
    let ir = vox_compiler::contract_ir::project(hir);
    emit_from_contract(&ir, package_name, package_version)
}

fn emit_from_contract(ir: &ContractIr, package_name: &str, version: &str) -> String {
    let mut spec = Map::new();
    spec.insert("openapi".into(), json!("3.1.0"));
    spec.insert(
        "info".into(),
        json!({
            "title": package_name,
            "version": version,
            "description": "Generated from Vox source. Conforms to wire-format-v1.\n\nQuery (`GET`) parameters: each query value is JSON text after URI decoding — see wire-format-v1 SSOT §2.1. The `schema` on each parameter describes the logical type after `JSON.parse`; it is not the raw query-string token shape.",
        }),
    );
    // Empty server URL: path keys are absolute (`/api/query/foo`). A non-empty base such as
    // `/api/v1` would make standard clients request `/api/v1/api/query/foo` (broken).
    spec.insert("servers".into(), json!([{ "url": "" }]));
    spec.insert("paths".into(), Value::Object(emit_paths(&ir.endpoints)));
    let mut schemas = emit_schemas(&ir.types);
    schemas.insert("ErrorEnvelope".into(), error_envelope_component_schema());
    spec.insert(
        "components".into(),
        json!({
            "schemas": Value::Object(schemas),
        }),
    );
    serde_json::to_string_pretty(&Value::Object(spec)).expect("OpenAPI emit must serialize")
}

fn error_envelope_component_schema() -> Value {
    json!({
        "type": "object",
        "description": "Wire-format v1 error body (SSOT §6). Returned for 4xx/5xx endpoint failures.",
        "required": ["ok", "code", "message"],
        "properties": {
            "ok": { "type": "boolean", "const": false },
            "code": { "type": "string", "description": "Stable SCREAMING_SNAKE_CASE machine code" },
            "message": { "type": "string" },
            "request_id": { "type": "string", "description": "Optional trace / request correlation id" },
            "details": { "description": "Optional structured payload; shape is code-specific" }
        }
    })
}

fn json_response_ref(description: &str, schema_ref: &str) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": { "$ref": format!("#/components/schemas/{schema_ref}") }
            }
        }
    })
}

fn emit_paths(endpoints: &[ContractEndpoint]) -> Map<String, Value> {
    let mut by_path: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for e in endpoints {
        let path_item = by_path.entry(e.path.clone()).or_default();
        path_item.insert(method_key(e.method), emit_operation(e));
    }
    by_path
        .into_iter()
        .map(|(k, v)| (k, Value::Object(v)))
        .collect()
}

fn method_key(m: HttpMethod) -> String {
    m.as_str().to_lowercase()
}

fn emit_operation(e: &ContractEndpoint) -> Value {
    let mut op = Map::new();
    op.insert("operationId".into(), json!(e.name));
    op.insert(
        "summary".into(),
        json!(format!("{} {}", e.method.as_str(), e.path)),
    );

    match e.method {
        HttpMethod::Get => {
            // Query endpoint params project to query-string parameters per
            // wire-format-v1 §2.1.
            let parameters: Vec<Value> = e
                .params
                .iter()
                .map(|f| {
                    json!({
                        "name": f.name,
                        "in": "query",
                        "required": !f.optional,
                        "description": "JSON-encoded value after URI decoding (`JSON.parse`), per wire-format-v1 §2.1.",
                        "schema": wire_schema(&f.ty),
                    })
                })
                .collect();
            op.insert("parameters".into(), Value::Array(parameters));
        }
        _ => {
            // Mutation / server endpoints take a JSON body.
            let mut props: Map<String, Value> = Map::new();
            let mut required: Vec<Value> = Vec::new();
            for f in &e.params {
                props.insert(f.name.clone(), wire_schema(&f.ty));
                if !f.optional {
                    required.push(Value::String(f.name.clone()));
                }
            }
            let mut body_schema = Map::new();
            body_schema.insert("type".into(), json!("object"));
            body_schema.insert("properties".into(), Value::Object(props));
            if !required.is_empty() {
                body_schema.insert("required".into(), Value::Array(required));
            }
            op.insert(
                "requestBody".into(),
                json!({
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": Value::Object(body_schema),
                        }
                    }
                }),
            );
        }
    }

    let mut responses = Map::new();
    responses.insert(
        "200".into(),
        json!({
            "description": "Success",
            "content": {
                "application/json": {
                    "schema": wire_schema(&e.response),
                }
            }
        }),
    );
    responses.insert(
        "400".into(),
        json_response_ref("Bad request or malformed parameters", "ErrorEnvelope"),
    );
    responses.insert(
        "429".into(),
        json_response_ref("Rate limited", "ErrorEnvelope"),
    );
    responses.insert(
        "500".into(),
        json_response_ref("Internal server error", "ErrorEnvelope"),
    );
    responses.insert(
        "default".into(),
        json_response_ref("Error — see wire-format-v1 §6", "ErrorEnvelope"),
    );
    op.insert("responses".into(), Value::Object(responses));
    Value::Object(op)
}

fn emit_schemas(types: &[ContractType]) -> Map<String, Value> {
    let mut out: BTreeMap<String, Value> = BTreeMap::new();
    for t in types {
        out.insert(t.name.clone(), emit_type_schema(t));
    }
    out.into_iter().collect()
}

fn emit_type_schema(t: &ContractType) -> Value {
    match &t.kind {
        ContractTypeKind::Struct { fields } => struct_schema(fields),
        ContractTypeKind::Sum { variants } => sum_schema(variants),
    }
}

fn struct_schema(fields: &[vox_compiler::contract_ir::ContractField]) -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), json!("object"));
    let mut properties: Map<String, Value> = Map::new();
    let mut required: Vec<Value> = Vec::new();
    for f in fields {
        properties.insert(f.name.clone(), wire_schema(&f.ty));
        if !f.optional {
            required.push(Value::String(f.name.clone()));
        }
    }
    schema.insert("properties".into(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".into(), Value::Array(required));
    }
    Value::Object(schema)
}

fn sum_schema(variants: &[ContractVariant]) -> Value {
    let one_of: Vec<Value> = variants
        .iter()
        .map(|v| {
            let mut props: Map<String, Value> = Map::new();
            let mut req: Vec<Value> = Vec::new();
            props.insert("_tag".into(), json!({ "type": "string", "const": v.tag }));
            req.push(Value::String("_tag".into()));
            for f in &v.fields {
                props.insert(f.name.clone(), wire_schema(&f.ty));
                if !f.optional {
                    req.push(Value::String(f.name.clone()));
                }
            }
            json!({
                "type": "object",
                "properties": Value::Object(props),
                "required": Value::Array(req),
            })
        })
        .collect();
    json!({
        "oneOf": Value::Array(one_of),
        "discriminator": { "propertyName": "_tag" },
    })
}

fn wire_schema(ty: &WireType) -> Value {
    match ty {
        WireType::Number => json!({ "type": "number" }),
        WireType::String => json!({ "type": "string" }),
        WireType::Bool => json!({ "type": "boolean" }),
        // Wire-format-v1: encoded as JSON strings.
        WireType::DecimalString => json!({
            "type": "string",
            "x-vox-encoding": "decimal",
            "description": "Fixed-point decimal as string (wire-format-v1)",
        }),
        WireType::BigIntString => json!({
            "type": "string",
            "x-vox-encoding": "bigint",
            "description": "Big integer as string (wire-format-v1)",
        }),
        WireType::DateTimeString => json!({ "type": "string", "format": "date-time" }),
        WireType::Array(inner) => json!({ "type": "array", "items": wire_schema(inner) }),
        WireType::Tuple(elems) => {
            let items: Vec<Value> = elems.iter().map(wire_schema).collect();
            // OpenAPI 3.1 / JSON Schema 2020-12 tuple form (prefixItems).
            json!({
                "type": "array",
                "prefixItems": Value::Array(items.clone()),
                "minItems": items.len(),
                "maxItems": items.len(),
                "items": false,
            })
        }
        WireType::Ref(name) => json!({ "$ref": format!("#/components/schemas/{}", name) }),
        WireType::Unit => json!({ "type": "null" }),
        WireType::Unknown => json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::contract_ir::{ContractField, ContractType, ContractTypeKind, WireType};

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).expect("emitted spec must be valid JSON")
    }

    #[test]
    fn empty_module_emits_minimal_valid_spec() {
        let ir = ContractIr::default();
        let s = emit_from_contract(&ir, "demo", "0.1.0");
        let v = parse(&s);
        assert_eq!(v["openapi"], json!("3.1.0"));
        assert_eq!(v["info"]["title"], json!("demo"));
        assert_eq!(v["servers"][0]["url"], json!(""));
        assert!(v["paths"].as_object().unwrap().is_empty());
    }

    #[test]
    fn struct_with_decimal_field_widens_to_string_in_schema() {
        let ir = ContractIr {
            types: vec![ContractType {
                name: "Order".into(),
                kind: ContractTypeKind::Struct {
                    fields: vec![ContractField {
                        name: "total".into(),
                        ty: WireType::DecimalString,
                        optional: false,
                    }],
                },
            }],
            endpoints: vec![],
        };
        let v = parse(&emit_from_contract(&ir, "demo", "0.1.0"));
        let order = &v["components"]["schemas"]["Order"];
        assert_eq!(order["type"], json!("object"));
        assert_eq!(order["properties"]["total"]["type"], json!("string"));
        assert_eq!(
            order["properties"]["total"]["x-vox-encoding"],
            json!("decimal")
        );
        assert_eq!(order["required"], json!(["total"]));
    }

    #[test]
    fn optional_field_is_not_required() {
        let ir = ContractIr {
            types: vec![ContractType {
                name: "Profile".into(),
                kind: ContractTypeKind::Struct {
                    fields: vec![ContractField {
                        name: "nickname".into(),
                        ty: WireType::String,
                        optional: true,
                    }],
                },
            }],
            endpoints: vec![],
        };
        let v = parse(&emit_from_contract(&ir, "demo", "0.1.0"));
        let profile = &v["components"]["schemas"]["Profile"];
        assert!(
            profile.get("required").is_none() || profile["required"].as_array().unwrap().is_empty()
        );
    }

    #[test]
    fn sum_type_emits_oneof_with_tag_discriminator() {
        let ir = ContractIr {
            types: vec![ContractType {
                name: "Status".into(),
                kind: ContractTypeKind::Sum {
                    variants: vec![
                        ContractVariant {
                            tag: "Active".into(),
                            fields: vec![],
                        },
                        ContractVariant {
                            tag: "Banned".into(),
                            fields: vec![ContractField {
                                name: "reason".into(),
                                ty: WireType::String,
                                optional: false,
                            }],
                        },
                    ],
                },
            }],
            endpoints: vec![],
        };
        let v = parse(&emit_from_contract(&ir, "demo", "0.1.0"));
        let status = &v["components"]["schemas"]["Status"];
        assert!(status["oneOf"].is_array());
        assert_eq!(status["discriminator"]["propertyName"], json!("_tag"));
        let active = &status["oneOf"][0];
        assert_eq!(active["properties"]["_tag"]["const"], json!("Active"));
    }

    #[test]
    fn query_endpoint_uses_get_with_query_params() {
        let ir = ContractIr {
            types: vec![],
            endpoints: vec![ContractEndpoint {
                kind: vox_compiler::contract_ir::ContractEndpointKind::Query,
                name: "list_users".into(),
                method: HttpMethod::Get,
                path: "/list_users".into(),
                params: vec![ContractField {
                    name: "limit".into(),
                    ty: WireType::Number,
                    optional: true,
                }],
                response: WireType::Array(Box::new(WireType::Ref("User".into()))),
                is_pure: true,
            }],
        };
        let v = parse(&emit_from_contract(&ir, "demo", "0.1.0"));
        let op = &v["paths"]["/list_users"]["get"];
        assert_eq!(op["operationId"], json!("list_users"));
        assert_eq!(op["parameters"][0]["in"], json!("query"));
        assert_eq!(op["parameters"][0]["required"], json!(false));
        assert!(
            op["parameters"][0]["description"]
                .as_str()
                .unwrap_or("")
                .contains("JSON"),
            "expected JSON encoding hint on query params"
        );
        let resp = &op["responses"]["200"]["content"]["application/json"]["schema"];
        assert_eq!(resp["type"], json!("array"));
        assert_eq!(resp["items"]["$ref"], json!("#/components/schemas/User"));
    }

    #[test]
    fn components_include_error_envelope_and_errors_reference_it() {
        let ir = ContractIr {
            types: vec![],
            endpoints: vec![ContractEndpoint {
                kind: vox_compiler::contract_ir::ContractEndpointKind::Query,
                name: "ping".into(),
                method: HttpMethod::Get,
                path: "/api/query/ping".into(),
                params: vec![],
                response: WireType::String,
                is_pure: true,
            }],
        };
        let v = parse(&emit_from_contract(&ir, "demo", "0.1.0"));
        let env = &v["components"]["schemas"]["ErrorEnvelope"];
        assert_eq!(env["properties"]["ok"]["const"], json!(false));
        let op = &v["paths"]["/api/query/ping"]["get"];
        assert_eq!(
            op["responses"]["400"]["content"]["application/json"]["schema"]["$ref"],
            json!("#/components/schemas/ErrorEnvelope")
        );
        assert_eq!(
            op["responses"]["default"]["content"]["application/json"]["schema"]["$ref"],
            json!("#/components/schemas/ErrorEnvelope")
        );
    }

    #[test]
    fn openapi_paths_do_not_double_api_segment_when_server_url_empty() {
        let ir = ContractIr {
            types: vec![],
            endpoints: vec![ContractEndpoint {
                kind: vox_compiler::contract_ir::ContractEndpointKind::Query,
                name: "q".into(),
                method: HttpMethod::Get,
                path: "/api/query/q".into(),
                params: vec![],
                response: WireType::String,
                is_pure: true,
            }],
        };
        let v = parse(&emit_from_contract(&ir, "demo", "0.1.0"));
        let server = v["servers"][0]["url"].as_str().unwrap_or("");
        assert!(
            server.is_empty(),
            "expected empty server URL, got {server:?}"
        );
        for path_key in v["paths"].as_object().unwrap().keys() {
            let composed = format!("{server}{path_key}");
            assert!(
                !composed.contains("/api/v1/api/"),
                "unexpected doubled api segment in {composed}"
            );
            assert!(
                composed.starts_with("/api/"),
                "expected absolute /api path, got {composed}"
            );
        }
    }

    #[test]
    fn mutation_endpoint_uses_post_with_request_body() {
        let ir = ContractIr {
            types: vec![],
            endpoints: vec![ContractEndpoint {
                kind: vox_compiler::contract_ir::ContractEndpointKind::Mutation,
                name: "create_user".into(),
                method: HttpMethod::Post,
                path: "/create_user".into(),
                params: vec![ContractField {
                    name: "name".into(),
                    ty: WireType::String,
                    optional: false,
                }],
                response: WireType::Ref("User".into()),
                is_pure: false,
            }],
        };
        let v = parse(&emit_from_contract(&ir, "demo", "0.1.0"));
        let op = &v["paths"]["/create_user"]["post"];
        let body = &op["requestBody"]["content"]["application/json"]["schema"];
        assert_eq!(body["type"], json!("object"));
        assert_eq!(body["required"], json!(["name"]));
        assert_eq!(body["properties"]["name"]["type"], json!("string"));
    }

    #[test]
    fn output_is_byte_identical_for_identical_input() {
        let ir = ContractIr::default();
        let a = emit_from_contract(&ir, "demo", "0.1.0");
        let b = emit_from_contract(&ir, "demo", "0.1.0");
        assert_eq!(a, b);
    }
}
