//! App-surface contract IR derived from semantic HIR.
//!
//! This module defines a serde-stable contract consumed by codegen and tooling so route/RPC
//! ownership does not remain split across ad-hoc emitter logic.

use serde::{Deserialize, Serialize};

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
    /// Streaming endpoints (`@endpoint(kind: stream)`). Routed as GET
    /// under `/api/stream/<name>` and emitted as SSE handlers
    /// (`Sse<Stream<...>>`) by the Rust codegen. The `interval_ms`
    /// field carries the parsed `every: "<duration>"` value, defaulting
    /// to 1000ms when absent.
    #[serde(default)]
    pub stream_fns: Vec<AppStreamFnContract>,
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

/// Streaming endpoint contract — surfaced to codegen as a distinct slice
/// so the SSE route emission stays separate from server-fn POST routes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStreamFnContract {
    pub name: String,
    pub route_path: String,
    pub signature: String,
    /// Tick interval in milliseconds, derived from `every: "<duration>"`.
    /// `None` indicates the codegen default (1000ms).
    #[serde(default)]
    pub interval_ms: Option<u64>,
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

    let server_fns = module
        .endpoint_fns
        .iter()
        .filter(|sf| sf.kind == crate::hir::HirEndpointKind::Server)
        .map(|sf| AppServerFnContract {
            name: sf.name.clone(),
            route_path: sf.route_path.clone(),
            signature: fn_signature(&sf.params, sf.return_type.as_ref()),
        })
        .collect();

    let query_fns = module
        .endpoint_fns
        .iter()
        .filter(|sf| sf.kind == crate::hir::HirEndpointKind::Query)
        .map(|qf| AppServerFnContract {
            name: qf.name.clone(),
            route_path: qf.route_path.clone(),
            signature: fn_signature(&qf.params, qf.return_type.as_ref()),
        })
        .collect();

    let wraps_db_transaction = !module.tables.is_empty();
    let mutation_fns = module
        .endpoint_fns
        .iter()
        .filter(|sf| sf.kind == crate::hir::HirEndpointKind::Mutation)
        .map(|mf| AppMutationContract {
            name: mf.name.clone(),
            route_path: mf.route_path.clone(),
            signature: fn_signature(&mf.params, mf.return_type.as_ref()),
            wraps_db_transaction,
        })
        .collect();

    let stream_fns = module
        .endpoint_fns
        .iter()
        .filter(|sf| sf.kind == crate::hir::HirEndpointKind::Stream)
        .map(|sf| AppStreamFnContract {
            name: sf.name.clone(),
            route_path: sf.route_path.clone(),
            signature: fn_signature(&sf.params, sf.return_type.as_ref()),
            interval_ms: sf
                .stream_interval
                .as_deref()
                .and_then(parse_duration_to_ms),
        })
        .collect();

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
        stream_fns,
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

/// Parse `every: "<duration>"` strings to milliseconds. Public on the
/// contract projection because both the contract and the codegen need
/// the same parser. Accepts `ms`, `s`, `m`, `h` suffixes.
fn parse_duration_to_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, unit) = if let Some(rest) = s.strip_suffix("ms") {
        (rest, 1u64)
    } else if let Some(rest) = s.strip_suffix('s') {
        (rest, 1000)
    } else if let Some(rest) = s.strip_suffix('m') {
        (rest, 60_000)
    } else if let Some(rest) = s.strip_suffix('h') {
        (rest, 3_600_000)
    } else {
        return None;
    };
    num.trim().parse::<u64>().ok().map(|n| n.saturating_mul(unit))
}

/// Canonical JSON bytes for stable app-contract hashing (sorted object keys at every depth).
pub fn canonical_app_contract_bytes(
    module: &AppContractModule,
) -> Result<Vec<u8>, serde_json::Error> {
    let mut v = serde_json::to_value(module)?;
    crate::canonical_json::sort_json_value_keys(&mut v);
    serde_json::to_vec(&v)
}
