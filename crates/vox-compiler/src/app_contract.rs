//! App-surface contract IR derived from semantic HIR.
//!
//! This module defines a serde-stable contract consumed by codegen and tooling so route/RPC
//! ownership does not remain split across ad-hoc emitter logic.

use serde::{Deserialize, Serialize};

use crate::contract_ir::ContractEndpointKind;
use crate::hir::HirModule;
use crate::typeck::env::TypeEnv;
use crate::typeck::registration::type_signature_from_hir;

/// Versioned schema for [`AppContractModule`].
pub const APP_CONTRACT_SCHEMA_VERSION: u32 = 2;
/// Default app HTTP port for generated server configuration.
pub const APP_DEFAULT_HTTP_PORT: u16 = 3000;
/// Default mobile-safe tap target baseline used by generated web templates.
pub const APP_MOBILE_MIN_TAP_TARGET_PX: u16 = 44;
/// Default viewport contract emitted by generated web app shells.
pub const APP_VIEWPORT_META_CONTENT: &str =
    "width=device-width, initial-scale=1.0, viewport-fit=cover";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContractModule {
    pub schema_version: u32,
    pub http_routes: Vec<AppHttpRouteContract>,
    pub server_fns: Vec<AppServerFnContract>,
    pub query_fns: Vec<AppServerFnContract>,
    pub mutation_fns: Vec<AppMutationContract>,
    /// MCP tools from `@mcp.tool` (names, descriptions, signatures) — machine-readable SSOT for tooling.
    #[serde(default)]
    pub mcp_tools: Vec<AppMcpToolContract>,
    /// MCP resources from `@mcp.resource` (URIs, descriptions, signatures).
    #[serde(default)]
    pub mcp_resources: Vec<AppMcpResourceContract>,
    pub server_config: AppServerConfigContract,
}

/// MCP tool surface derived from HIR (`@mcp.tool`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMcpToolContract {
    pub name: String,
    pub description: String,
    pub signature: String,
}

/// MCP resource surface derived from HIR (`@mcp.resource`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMcpResourceContract {
    pub uri: String,
    pub description: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppHttpRouteContract {
    pub method: String,
    pub path: String,
    pub route_contract: String,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppServerFnContract {
    pub name: String,
    pub route_path: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMutationContract {
    pub name: String,
    pub route_path: String,
    pub signature: String,
    pub wraps_db_transaction: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppServerConfigContract {
    pub bind_host: String,
    pub default_port: u16,
    pub port_env_var: String,
    pub dev_proxy_env_var: String,
    pub static_assets_embed_dir: String,
}

fn fn_signature(params: &[crate::hir::HirParam], ret: Option<&crate::hir::HirType>) -> String {
    let env = TypeEnv::new();
    type_signature_from_hir(params, ret, &env)
}

#[must_use]
pub fn project_app_contract(module: &HirModule) -> AppContractModule {
    let http_routes = Vec::new();

    // Derive the HTTP-endpoint lists from Contract IR — the single endpoint
    // lens (Phase 5.2). `contract_ir::project` projects `module.endpoint_fns`
    // 1:1 in source order, so we zip each `ContractEndpoint` back with its
    // originating `HirEndpointFn` to recover the HIR-level `signature` string
    // (the wire-level `WireType` carried by `ContractEndpoint` is lossy — e.g.
    // `int`/`i64`/`f64` all collapse to `Number` — so it cannot reconstruct the
    // signature). `name`, `route_path`, and the kind grouping come from
    // Contract IR; `wraps_db_transaction` stays module-level (it is not a
    // Contract IR property).
    let ir = crate::contract_ir::project(module);
    debug_assert_eq!(
        ir.endpoints.len(),
        module.endpoint_fns.len(),
        "contract_ir::project must project endpoint_fns 1:1 in source order"
    );

    let wraps_db_transaction = !module.tables.is_empty();

    let mut server_fns = Vec::new();
    let mut query_fns = Vec::new();
    let mut mutation_fns = Vec::new();

    for (ep, hir_fn) in ir.endpoints.iter().zip(module.endpoint_fns.iter()) {
        assert_eq!(
            ep.name, hir_fn.name,
            "contract_ir::project must preserve endpoint identity/order: IR gave `{}`, HIR has `{}`",
            ep.name, hir_fn.name
        );
        // ep.params is lossy WireType — derive the human-readable signature from HIR types.
        let signature = fn_signature(&hir_fn.params, hir_fn.return_type.as_ref());
        match ep.kind {
            ContractEndpointKind::Server => {
                server_fns.push(AppServerFnContract {
                    name: ep.name.clone(),
                    route_path: ep.path.clone(),
                    signature,
                });
            }
            ContractEndpointKind::Query => {
                query_fns.push(AppServerFnContract {
                    name: ep.name.clone(),
                    route_path: ep.path.clone(),
                    signature,
                });
            }
            ContractEndpointKind::Mutation => {
                mutation_fns.push(AppMutationContract {
                    name: ep.name.clone(),
                    route_path: ep.path.clone(),
                    signature,
                    wraps_db_transaction,
                });
            }
        }
    }

    let mcp_tools = module
        .mcp_tools
        .iter()
        .map(|t| AppMcpToolContract {
            name: t.func.name.clone(),
            description: t.description.clone(),
            signature: fn_signature(&t.func.params, t.func.return_type.as_ref()),
        })
        .collect();

    let mcp_resources = module
        .mcp_resources
        .iter()
        .map(|r| AppMcpResourceContract {
            uri: r.uri.clone(),
            description: r.description.clone(),
            signature: fn_signature(&r.func.params, r.func.return_type.as_ref()),
        })
        .collect();

    AppContractModule {
        schema_version: APP_CONTRACT_SCHEMA_VERSION,
        http_routes,
        server_fns,
        query_fns,
        mutation_fns,
        mcp_tools,
        mcp_resources,
        server_config: AppServerConfigContract {
            bind_host: std::net::Ipv4Addr::LOCALHOST.to_string(),
            default_port: APP_DEFAULT_HTTP_PORT,
            port_env_var: "VOX_PORT".to_string(),
            dev_proxy_env_var: "VOX_SSR_DEV_URL".to_string(),
            static_assets_embed_dir: "public/".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::lower_module;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn lower(src: &str) -> HirModule {
        let tokens = lex(src);
        let module = parse(tokens).expect("parse");
        lower_module(&module)
    }

    /// Characterization test (Phase 5.2): pins the current behavior of
    /// `project_app_contract` for the endpoint lists so the ContractIr-derived
    /// refactor stays byte-identical.
    ///
    /// Asserts: kind grouping (Server/Query/Mutation), source order, name,
    /// route_path, the full HIR-level `signature` string, and
    /// `wraps_db_transaction = true` for mutations when `@table` exists.
    #[test]
    fn project_app_contract_endpoint_lists_characterization() {
        let src = r#"
@table type Task { title: str done: bool }

@server fn sf_one(a: int) to str { return "x" }
@server fn sf_two() to int { return 0 }
@query fn q_one(n: int) to int { return n }
@mutation fn m_one(title: str) to int {
    db.Task.insert({ title: title, done: false })
    return 1
}
@mutation fn m_two(x: bool) to bool { return x }
"#;
        let hir = lower(src);
        let app = project_app_contract(&hir);

        // server_fns: source order, name, route_path, signature.
        assert_eq!(app.server_fns.len(), 2, "two @server endpoints");
        assert_eq!(app.server_fns[0].name, "sf_one");
        assert_eq!(app.server_fns[1].name, "sf_two");
        assert_eq!(app.server_fns[0].route_path, "/api/sf_one");
        assert_eq!(app.server_fns[1].route_path, "/api/sf_two");
        assert_eq!(
            app.server_fns[0].signature,
            "fn(a: int) -> str",
            "server_fns[0] signature is the HIR-level type signature"
        );
        assert_eq!(app.server_fns[1].signature, "fn() -> int");

        // query_fns.
        assert_eq!(app.query_fns.len(), 1, "one @query endpoint");
        assert_eq!(app.query_fns[0].name, "q_one");
        assert_eq!(app.query_fns[0].route_path, "/api/query/q_one");
        assert_eq!(app.query_fns[0].signature, "fn(n: int) -> int");

        // mutation_fns: source order + wraps_db_transaction true (tables exist).
        assert_eq!(app.mutation_fns.len(), 2, "two @mutation endpoints");
        assert_eq!(app.mutation_fns[0].name, "m_one");
        assert_eq!(app.mutation_fns[1].name, "m_two");
        assert_eq!(app.mutation_fns[0].route_path, "/api/mutation/m_one");
        assert_eq!(app.mutation_fns[1].route_path, "/api/mutation/m_two");
        assert_eq!(app.mutation_fns[0].signature, "fn(title: str) -> int");
        assert_eq!(app.mutation_fns[1].signature, "fn(x: bool) -> bool");
        assert!(
            app.mutation_fns[0].wraps_db_transaction,
            "wraps_db_transaction true when @table present"
        );
        assert!(app.mutation_fns[1].wraps_db_transaction);
    }

    /// Without any `@table`, mutations carry `wraps_db_transaction = false`.
    #[test]
    fn project_app_contract_mutation_no_tables_no_tx() {
        let src = r#"
@mutation fn m_only(x: int) to int { return x }
"#;
        let hir = lower(src);
        let app = project_app_contract(&hir);
        assert_eq!(app.mutation_fns.len(), 1);
        assert!(
            !app.mutation_fns[0].wraps_db_transaction,
            "no tables -> no transaction wrap"
        );
    }

    /// Characterization test: endpoints declared in interleaved kind order
    /// (`@server a`, `@mutation b`, `@query c`, `@server d`) must end up in
    /// the correct per-kind list with the correct name and route_path,
    /// proving that the ContractIr/HIR zip preserves identity across kind
    /// interleaving rather than only within homogeneous per-kind blocks.
    #[test]
    fn project_app_contract_interleaved_kinds_preserve_identity() {
        let src = r#"
@server fn a(x: int) to str { return "a" }
@mutation fn b(y: bool) to bool { return y }
@query fn c(n: int) to int { return n }
@server fn d() to int { return 0 }
"#;
        let hir = lower(src);
        let app = project_app_contract(&hir);

        // server_fns: a then d (source order within kind).
        assert_eq!(app.server_fns.len(), 2, "two @server endpoints");
        assert_eq!(app.server_fns[0].name, "a");
        assert_eq!(app.server_fns[0].route_path, "/api/a");
        assert_eq!(app.server_fns[1].name, "d");
        assert_eq!(app.server_fns[1].route_path, "/api/d");

        // mutation_fns: b only.
        assert_eq!(app.mutation_fns.len(), 1, "one @mutation endpoint");
        assert_eq!(app.mutation_fns[0].name, "b");
        assert_eq!(app.mutation_fns[0].route_path, "/api/mutation/b");

        // query_fns: c only.
        assert_eq!(app.query_fns.len(), 1, "one @query endpoint");
        assert_eq!(app.query_fns[0].name, "c");
        assert_eq!(app.query_fns[0].route_path, "/api/query/c");
    }
}

/// Canonical JSON bytes for stable app-contract hashing (sorted object keys at every depth).
pub fn canonical_app_contract_bytes(
    module: &AppContractModule,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut v = serde_json::to_value(module)?;
    crate::canonical_json::sort_json_value_keys(&mut v);
    serde_json::to_vec(&v)
}
