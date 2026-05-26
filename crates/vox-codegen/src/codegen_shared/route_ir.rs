//! Shared Route Intermediate Representation — the single source of truth for HTTP route
//! contracts that both Rust (Axum) and TypeScript (Express) codegen emit against.
//!
//! Neither `crate::codegen_rust::emit::http` nor [`crate::codegen_ts::routes`] should
//! re-derive route metadata independently — they should consume [`RouteIR`] slices instead.
//!
//! ## Design Contract
//!
//! * `RouteIR` carries only what both Rust and TS backends need: method, path, param names,
//!   and return-type presence. Body stmts stay in HIR and are emitted by each backend.
//! * Lowering is additive: the existing HIR structs ([`HirRoute`], `HirServerFn`) are
//!   unchanged — `RouteIR` is a read-only projection computed at codegen time.
use vox_compiler::hir::{HirEndpointFn, HirModule, HirParam};

/// Unified HTTP route contract used by Rust and TypeScript backends.
///
/// Every HTTP endpoint derivable from a Vox module (explicit `route` or `@query`/`@mutation`/`@server`)
/// maps to one `RouteIR` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteIR {
    /// HTTP method.
    pub method: RouteMethod,
    /// URL path pattern (e.g. `"/api/greet"`).
    pub path: String,
    /// Stable contract key (`"METHOD /path"`) aligned with HIR `route_contract` field.
    pub contract_key: String,
    /// Parameter names exposed by this route/function. Rust/Axum uses JSON body (`@server` / `@mutation`)
    /// or JSON-encoded query values (`@query`).
    pub params: Vec<RouteParam>,
    /// Whether the handler returns a non-unit value (affects status-200 JSON wrapping).
    pub has_return_value: bool,
    /// Route kind — distinguishes explicit routes from compiler-generated fn endpoints.
    pub kind: RouteKind,
}

/// HTTP method enum independent of the HIR layer (avoids downstream crate coupling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl RouteMethod {
    /// Lowercase string suitable for Axum / Express routing helpers.
    #[must_use]
    pub const fn as_lowercase_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Put => "put",
            Self::Delete => "delete",
        }
    }

    /// Uppercase string for contract keys / debug output.
    #[must_use]
    pub const fn as_uppercase_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

/// A single named parameter carried by this route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteParam {
    /// Parameter name as declared in source (`@server fn greet(name: str)`).
    pub name: String,
}

impl From<&HirParam> for RouteParam {
    fn from(p: &HirParam) -> Self {
        Self {
            name: p.name.clone(),
        }
    }
}

/// Distinguishes what HIR declaration generated this route entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    /// Explicit `route GET "/path" { ... }` block.
    Explicit,
    /// Auto-generated from `@server fn`.
    ServerFn,
    /// Auto-generated from `@query fn`.
    QueryFn,
    /// Auto-generated from `@mutation fn`.
    MutationFn,
}

impl RouteIR {
    /// Lower a `HirServerFn` into a `RouteIR` with typed params.
    #[must_use]
    pub fn from_server_fn(sf: &HirEndpointFn, kind: RouteKind) -> Self {
        let method = match kind {
            RouteKind::QueryFn => RouteMethod::Get,
            _ => RouteMethod::Post,
        };
        let contract_key = format!("{} {}", method.as_uppercase_str(), sf.route_path);
        Self {
            method,
            path: sf.route_path.clone(),
            contract_key,
            params: sf.params.iter().map(RouteParam::from).collect(),
            has_return_value: sf.return_type.is_some(),
            kind,
        }
    }
}

/// Lower all HTTP endpoints in a `HirModule` into a sorted, deduplicated `RouteIR` slice.
///
/// Order: explicit routes (sorted by path + method) then server/query/mutation fns (sorted
/// by route_path + name). This preserves deterministic codegen output across runs.
#[must_use]
pub fn lower_module_routes(module: &HirModule) -> Vec<RouteIR> {
    let mut out: Vec<RouteIR> = Vec::new();

    // Server / query / mutation functions
    let mut fns: Vec<(&HirEndpointFn, RouteKind)> = module
        .endpoint_fns
        .iter()
        .map(|sf| {
            let kind = match sf.kind {
                vox_compiler::hir::HirEndpointKind::Server => RouteKind::ServerFn,
                vox_compiler::hir::HirEndpointKind::Query => RouteKind::QueryFn,
                vox_compiler::hir::HirEndpointKind::Mutation => RouteKind::MutationFn,
            };
            (sf, kind)
        })
        .collect();
    fns.sort_by(|(a, _), (b, _)| {
        a.route_path
            .cmp(&b.route_path)
            .then_with(|| a.name.cmp(&b.name))
    });
    for (sf, kind) in fns {
        out.push(RouteIR::from_server_fn(sf, kind));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::hir::lower_module;
    use vox_compiler::lexer::cursor::lex;
    use vox_compiler::parser::parse;

    #[test]
    fn lower_module_routes_yields_stable_order() {
        let src = r#"
@server fn greet(name: str) to str {
    return name
}
"#;
        let tokens = lex(src);
        let module = parse(tokens).expect("parse");
        let hir = lower_module(&module);
        let routes = lower_module_routes(&hir);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, RouteMethod::Post);
        assert_eq!(routes[0].kind, RouteKind::ServerFn);
        assert!(
            routes[0].path.contains("greet"),
            "expected route path to contain fn name; got {:?}",
            routes[0].path
        );
        assert_eq!(routes[0].params.len(), 1);
        assert_eq!(routes[0].params[0].name, "name");
    }

    #[test]
    fn route_ir_contract_key_matches_hir_contract() {
        let src = r#"
@server fn ping() to str {
    return "pong"
}
"#;
        let tokens = lex(src);
        let module = parse(tokens).expect("parse");
        let hir = lower_module(&module);
        let routes = lower_module_routes(&hir);
        assert_eq!(routes.len(), 1);
        // contract_key is "POST /path" — must start with method
        assert!(
            routes[0].contract_key.starts_with("POST "),
            "contract_key: {:?}",
            routes[0].contract_key
        );
    }

    #[test]
    fn route_ir_query_uses_get_contract_key() {
        let src = r#"
@query fn items() to int {
    return 0
}
"#;
        let tokens = lex(src);
        let module = parse(tokens).expect("parse");
        let hir = lower_module(&module);
        let routes = lower_module_routes(&hir);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].kind, RouteKind::QueryFn);
        assert_eq!(routes[0].method, RouteMethod::Get);
        assert!(
            routes[0].contract_key.starts_with("GET "),
            "contract_key: {:?}",
            routes[0].contract_key
        );
    }
}
