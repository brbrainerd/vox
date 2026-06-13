//! Lower AST [`crate::Module`] to [`HirModule`] / [`crate::hir::TypedCoreIR_v2`].
//!
//! This module is the **HIR boundary** before `vox_codegen::web_ir::lower::project_web_from_core`.
//! Declaration arms here define what structured data reaches WebIR (`HirRoutes`,
//! reactive components, server/query/mutation `route_path` contracts). See internal Web IR
//! implementation blueprint (lane P → S).
//!
//! **Spans (OP-0038 / OP-S007):** AST spans are copied onto HIR nodes where the AST carries them; reactive
//! member lowering preserves per-member spans for state, derived values, and effects. WebIR lowering
//! may elide spans on some synthetic nodes until the span-table workstream lands—consumers should not
//! treat missing WebIR spans as a lowering failure.
//!
//! **Lowering buckets (OP-S005):** each `Decl` arm in `LowerCtx::lower` maps into a named field on
//! [`HirModule`] — for example `Decl::Import`→`imports`, `Decl::Routes`→`client_routes`,
//! `Decl::ReactiveComponent`→`components`, `Decl::HttpRoute` /
//! server/query/mutation→`routes` / `server_fns` / `query_fns` / `mutation_fns`, and tables/indices into
//! their respective vectors. Search `Decl::` in this file for the authoritative match.

use crate::ast::decl::*;
use crate::hir::def_map::DefMap;
use crate::hir::*;
use crate::web_prefixes::{MUTATION_FN_API_PREFIX, QUERY_FN_API_PREFIX, SERVER_FN_API_PREFIX};
use std::collections::HashMap;

mod async_flags;
pub use async_flags::has_async_stmts;
mod contracts;
mod db_select_normalize;
mod decl;
mod expr_db;
mod json_as;
#[path = "expr.rs"]
mod lowering_expr;
#[path = "stmt.rs"]
mod lowering_stmt;

/// Convert `HirCapability` (used on `HirFn`) to `HirEffectKind` (used on `HirEndpointFn`).
/// The two enums are parallel; `HirCapability::Nothing` has no `HirEffectKind` counterpart
/// and is filtered out.
fn cap_to_effect_kind(cap: &HirCapability) -> Option<crate::hir::nodes::effect::HirEffectKind> {
    use crate::hir::nodes::effect::HirEffectKind;
    match cap {
        HirCapability::Net => Some(HirEffectKind::Net),
        HirCapability::Db => Some(HirEffectKind::Db),
        HirCapability::Fs => Some(HirEffectKind::Fs),
        HirCapability::Env => Some(HirEffectKind::Env),
        HirCapability::Clock => Some(HirEffectKind::Clock),
        HirCapability::Random => Some(HirEffectKind::Random),
        HirCapability::Spawn => Some(HirEffectKind::Spawn),
        HirCapability::GpuCompute => Some(HirEffectKind::GpuCompute),
        HirCapability::Mutate => Some(HirEffectKind::Mutate),
        HirCapability::Vcs => Some(HirEffectKind::Vcs),
        HirCapability::Mcp(t) => Some(HirEffectKind::Mcp(t.clone())),
        HirCapability::Nothing => None,
    }
}

/// Configuration for HIR lowering.
#[derive(Debug, Clone, Default)]
pub struct LowerConfig {
    /// If true, `@test` declarations will be omitted from the output.
    pub strip_tests: bool,
}

/// Lower an AST Module to a HirModule.
pub fn lower_module(module: &Module) -> HirModule {
    lower_module_with_config(module, &LowerConfig::default())
}

/// Lower an AST Module to a HirModule with explicit configuration.
pub fn lower_module_with_config(module: &Module, config: &LowerConfig) -> HirModule {
    let mut ctx = LowerCtx::new(config.clone());
    ctx.lower(module)
}

struct LowerCtx {
    def_map: DefMap,
    config: LowerConfig,
    table_primary_keys: HashMap<String, Option<String>>,
    /// Auto-incremented counter for synthesised `__side_effect_<n>` activity names (P1-T7).
    synthesis_counter: usize,
    /// Inline activities synthesised from `side_effect { … }` blocks during lowering.
    /// Drained into `HirModule::functions` after the main declaration pass completes.
    synthesised_fns: Vec<crate::hir::nodes::HirFn>,
}

impl LowerCtx {
    fn new(config: LowerConfig) -> Self {
        Self {
            def_map: DefMap::new(),
            config,
            table_primary_keys: HashMap::new(),
            synthesis_counter: 0,
            synthesised_fns: Vec::new(),
        }
    }

    pub(crate) fn table_primary_key(&self, table_name: &str) -> Option<String> {
        self.table_primary_keys.get(table_name).cloned().flatten()
    }

    pub(crate) fn next_synthesis_counter(&mut self) -> usize {
        let n = self.synthesis_counter;
        self.synthesis_counter += 1;
        n
    }

    fn lower(&mut self, module: &Module) -> HirModule {
        let mut hir = HirModule::default();
        self.table_primary_keys = module
            .declarations
            .iter()
            .filter_map(|decl| {
                let Decl::Table(t) = decl else {
                    return None;
                };
                Some((t.name.clone(), t.primary_key.clone()))
            })
            .collect();

        for decl in &module.declarations {
            match decl {
                Decl::Import(imp) => {
                    for path in &imp.paths {
                        match &path.kind {
                            ImportPathKind::SymbolPath { segments } => {
                                let (mod_path, item) = if segments.len() > 1 {
                                    let item = segments.last().expect("segments non-empty").clone();
                                    let mod_path = segments[..segments.len() - 1].to_vec();
                                    (mod_path, item)
                                } else {
                                    (vec![], segments[0].clone())
                                };
                                hir.imports.push(HirImport {
                                    module_path: mod_path,
                                    item: path.alias.clone().unwrap_or(item),
                                    es_module_specifier: None,
                                    es_import_kind: None,
                                    local_file_path: None,
                                    local_file_alias: None,
                                    span: path.span,
                                });
                            }
                            ImportPathKind::RustCrate(spec) => {
                                let alias = path
                                    .alias
                                    .clone()
                                    .unwrap_or_else(|| spec.crate_name.clone());
                                hir.rust_imports.push(HirRustImport {
                                    crate_name: spec.crate_name.clone(),
                                    alias,
                                    version: spec.version.clone(),
                                    path: spec.path.clone(),
                                    git: spec.git.clone(),
                                    rev: spec.rev.clone(),
                                    span: path.span,
                                });
                            }
                            ImportPathKind::ReactComponent {
                                module_specifier,
                                binding,
                            } => {
                                let spec = module_specifier.clone();
                                let mut push_es = |item: String, kind: EsImportKind| {
                                    hir.imports.push(HirImport {
                                        module_path: Vec::new(),
                                        item,
                                        es_module_specifier: Some(spec.clone()),
                                        es_import_kind: Some(kind),
                                        local_file_path: None,
                                        local_file_alias: None,
                                        span: path.span,
                                    });
                                };
                                match binding {
                                    ReactBinding::Default { local_name } => {
                                        push_es(local_name.clone(), EsImportKind::Default);
                                    }
                                    ReactBinding::Namespace { local_name } => {
                                        push_es(local_name.clone(), EsImportKind::Namespace);
                                    }
                                    ReactBinding::Named(names) => {
                                        for n in names {
                                            push_es(
                                                n.local.clone(),
                                                EsImportKind::Named {
                                                    imported: n.imported.clone(),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            ImportPathKind::LocalFile { path: file_path } => {
                                // Intra-project Vox file import (RFC 2026-05-23).
                                // Resolution + cycle detection happens at
                                // `Interpreter::run_module` time via
                                // `local_file_path`.
                                hir.imports.push(HirImport {
                                    module_path: Vec::new(),
                                    item: String::new(),
                                    es_module_specifier: None,
                                    es_import_kind: None,
                                    local_file_path: Some(file_path.clone()),
                                    local_file_alias: path.alias.clone(),
                                    span: path.span,
                                });
                            }
                        }
                    }
                }
                Decl::Function(f) => {
                    hir.functions.push(self.lower_fn(f));
                }
                Decl::TypeDef(t) => {
                    hir.types.push(self.lower_typedef(t));
                    // RFC json-as-rfc-2026-05-24 §6: synthesise from_json / to_json for
                    // `@json_as`-annotated types. The synthesised HirFns are collected here
                    // and drained into hir.functions (below) so typeck / eval / codegen see
                    // them as ordinary top-level functions — no special-casing needed.
                    hir.functions
                        .extend(json_as::synthesise_json_as_fns(self, t));
                }

                Decl::McpTool(m) => {
                    let func = self.lower_fn(&m.func);
                    hir.mcp_tools.push(HirMcpTool {
                        description: m.description.clone(),
                        func,
                    });
                }
                Decl::McpResource(m) => {
                    let func = self.lower_fn(&m.func);
                    hir.mcp_resources.push(HirMcpResource {
                        uri: m.uri.clone(),
                        description: m.description.clone(),
                        func,
                    });
                }
                Decl::Test(t) => {
                    if !self.config.strip_tests {
                        hir.tests.push(self.lower_fn(&t.func));
                    }
                }
                Decl::Example(e) => {
                    // Examples are not stripped by `strip_tests` — they serve as
                    // canonical demonstrations for corpus mining and docs tooling.
                    hir.examples.push(self.lower_fn(&e.func));
                }
                Decl::Forall(f) => {
                    let func = self.lower_fn(&f.func);
                    hir.foralls.push(HirForall {
                        label: f.label.clone(),
                        iterations: f.iterations,
                        func,
                    });
                }
                // `route_path` is the stable HTTP contract surface for WebIR `RouteNode` / client stubs.
                Decl::Endpoint(e) => {
                    let lowered = self.lower_fn(&e.func);
                    let (kind, prefix) = match e.kind {
                        crate::ast::decl::EndpointKind::Query => {
                            (crate::hir::HirEndpointKind::Query, QUERY_FN_API_PREFIX)
                        }
                        crate::ast::decl::EndpointKind::Mutation => (
                            crate::hir::HirEndpointKind::Mutation,
                            MUTATION_FN_API_PREFIX,
                        ),
                        crate::ast::decl::EndpointKind::Server => {
                            (crate::hir::HirEndpointKind::Server, SERVER_FN_API_PREFIX)
                        }
                    };
                    let route_path = format!("{prefix}{}", lowered.name);
                    let webhook = e.func.webhook.as_ref().map(|w| {
                        use crate::ast::decl::webhook::AstWebhookProvider as A;
                        use crate::hir::nodes::boilerplate_grafts::{
                            HirWebhookDecl, WebhookProvider as H,
                        };
                        let provider = match &w.provider {
                            A::Stripe => H::Stripe,
                            A::Github => H::Github,
                            A::Slack => H::Slack,
                            A::Custom { secret_var } => H::Custom {
                                secret_var: secret_var.clone(),
                            },
                        };
                        HirWebhookDecl {
                            provider,
                            idempotent: w.idempotent,
                            replay_window_secs: w.replay_window_secs,
                            span: w.span,
                        }
                    });
                    let cors = e.func.cors_spec.as_ref().map(|c| {
                        crate::hir::nodes::http_ergonomics::HirCorsPolicy {
                            origins: c.origins.clone(),
                            allow_credentials: c.allow_credentials,
                            span: c.span,
                        }
                    });
                    let rate_limit = e.func.rate_limit.as_ref().map(|r| {
                        use crate::ast::decl::http_decorators::AstRateLimitBy as A;
                        use crate::hir::nodes::http_ergonomics::{
                            HirRateLimitPolicy, RateLimitBy as H,
                        };
                        HirRateLimitPolicy {
                            by: match r.by {
                                A::Ip => H::Ip,
                                A::UserId => H::UserId,
                                A::ApiKey => H::ApiKey,
                            },
                            window_secs: r.window_secs,
                            max_requests: r.max_requests,
                            span: r.span,
                        }
                    });
                    let pii = e.func.pii.as_ref().map(|p| {
                        use crate::ast::decl::http_decorators::AstPiiClass as A;
                        use crate::hir::nodes::boilerplate_grafts::{HirPiiMarker, PiiClass as H};
                        let class = match &p.class {
                            A::Name => H::Name,
                            A::Email => H::Email,
                            A::Phone => H::Phone,
                            A::Ip => H::Ip,
                            A::FinancialData => H::FinancialData,
                            A::BiometricData => H::BiometricData,
                            A::Other(_) => H::Email, // best-effort fallback
                        };
                        HirPiiMarker {
                            class,
                            span: p.span,
                        }
                    });
                    let layer = e.func.layer.as_ref().and_then(|l| {
                        crate::hir::nodes::layer::LayerTier::from_str(&l.tier).map(|tier| {
                            crate::hir::nodes::layer::HirLayerDecl { tier, span: l.span }
                        })
                    });
                    hir.endpoint_fns.push(crate::hir::HirEndpointFn {
                        kind,
                        id: lowered.id,
                        name: lowered.name.clone(),
                        params: lowered.params.clone(),
                        return_type: lowered.return_type.clone(),
                        body: lowered.body.clone(),
                        route_path,
                        is_pure: lowered.is_pure,
                        effects: lowered
                            .capabilities
                            .iter()
                            .filter_map(cap_to_effect_kind)
                            .collect(),
                        webhook,
                        cors,
                        rate_limit,
                        pii,
                        layer,
                        span: lowered.span,
                    });
                }
                Decl::Table(t) => {
                    hir.tables.push(self.lower_table(t));
                }
                Decl::Index(idx) => {
                    hir.indexes.push(HirIndex {
                        table_name: idx.table_name.clone(),
                        index_name: idx.index_name.clone(),
                        columns: idx.columns.clone(),
                        span: idx.span,
                    });
                }
                Decl::Collection(c) => {
                    hir.collections.push(self.lower_collection(c));
                }
                Decl::VectorIndex(v) => {
                    hir.vector_indexes.push(HirVectorIndex {
                        table_name: v.table_name.clone(),
                        index_name: v.index_name.clone(),
                        column: v.column.clone(),
                        dimensions: v.dimensions,
                        filter_fields: v.filter_fields.clone(),
                        span: v.span,
                    });
                }
                Decl::SearchIndex(s) => {
                    hir.search_indexes.push(HirSearchIndex {
                        table_name: s.table_name.clone(),
                        index_name: s.index_name.clone(),
                        search_field: s.search_field.clone(),
                        filter_fields: s.filter_fields.clone(),
                        span: s.span,
                    });
                }
                Decl::ReactiveComponent(c) => {
                    hir.components.push(self.lower_reactive_component(c));
                }
                Decl::Routes(r) => {
                    hir.client_routes.push(r.clone());
                    // GA-09a: derive typed RouteId from each route entry.
                    for entry in &r.entries {
                        hir.route_ids.push(route_entry_to_route_id(entry));
                    }
                }
                Decl::Form(f) => {
                    hir.forms.push(self.lower_form(f));
                }
                Decl::V0Component(_) | Decl::Page(_) | Decl::Loading(_) => {
                    // Retired/legacy UI declarations: silently dropped from HIR.
                }
                Decl::Url(u) => {
                    hir.url_decls.push(self.lower_url_decl(u));
                }
                Decl::StateMachine(s) => {
                    hir.state_machines.push(self.lower_state_machine(s));
                }
                Decl::Fragment(f) => {
                    let params = f
                        .params
                        .iter()
                        .map(|p| self.lower_param(p))
                        .collect::<Vec<_>>();
                    let body = self.lower_expr(&f.body);
                    hir.fragments.push(crate::hir::nodes::HirFragmentDecl {
                        name: f.name.clone(),
                        params,
                        body,
                        span: f.span,
                    });
                }
                Decl::ReactiveModule(rm) => {
                    // ADR-032: lower module-scope reactive members the same way
                    // `lower_reactive_component` lowers in-component members. The
                    // module name is filled in by codegen from the source basename
                    // (parser leaves it empty); preserve whatever the AST has.
                    use crate::ast::decl::ReactiveMemberDecl;
                    use crate::hir::nodes::{
                        HirDerived, HirEffect, HirOnCleanup, HirOnMount, HirReactiveMember,
                        HirReactiveModule, HirState,
                    };
                    self.def_map.push_scope();
                    let members: Vec<HirReactiveMember> = rm
                        .members
                        .iter()
                        .map(|m| match m {
                            ReactiveMemberDecl::State(s) => HirReactiveMember::State(HirState {
                                id: self.def_map.define(s.name.clone()),
                                name: s.name.clone(),
                                ty: s.ty.as_ref().map(|t| self.lower_type(t)),
                                init: self.lower_expr(&s.init),
                                span: s.span,
                            }),
                            ReactiveMemberDecl::Derived(d) => {
                                HirReactiveMember::Derived(HirDerived {
                                    id: self.def_map.define(d.name.clone()),
                                    name: d.name.clone(),
                                    ty: d.ty.as_ref().map(|t| self.lower_type(t)),
                                    expr: self.lower_expr(&d.expr),
                                    span: d.span,
                                })
                            }
                            ReactiveMemberDecl::Effect(e) => HirReactiveMember::Effect(HirEffect {
                                body: self.lower_expr(&e.body),
                                explicit_deps: e.explicit_deps.clone(),
                                span: e.span,
                            }),
                            ReactiveMemberDecl::OnMount(m) => {
                                HirReactiveMember::OnMount(HirOnMount {
                                    body: self.lower_expr(&m.body),
                                    span: m.span,
                                })
                            }
                            ReactiveMemberDecl::OnCleanup(c) => {
                                HirReactiveMember::OnCleanup(HirOnCleanup {
                                    body: self.lower_expr(&c.body),
                                    span: c.span,
                                })
                            }
                            ReactiveMemberDecl::Stmt(s) => {
                                HirReactiveMember::Stmt(self.lower_stmt(s))
                            }
                        })
                        .collect();
                    self.def_map.pop_scope();
                    hir.reactive_modules.push(HirReactiveModule {
                        name: rm.name.clone(),
                        members,
                        span: rm.span,
                    });
                }
                Decl::Workflow(w) => {
                    let mut f = self.lower_workflow(w);
                    f.durability = Some(crate::hir::nodes::DurabilityKind::Workflow);
                    f.generated_hash = Some(stamp_durable_hash(&f));
                    hir.functions.push(f);
                }
                Decl::Activity(a) => {
                    let mut f = self.lower_activity(a);
                    f.durability = Some(crate::hir::nodes::DurabilityKind::Activity);
                    f.generated_hash = Some(stamp_durable_hash(&f));
                    hir.functions.push(f);
                }
                Decl::Actor(a) => {
                    let mut f = self.lower_actor(a);
                    f.durability = Some(crate::hir::nodes::DurabilityKind::Actor);
                    hir.functions.push(f);
                    // Synthesize one HirFn per handler so typecheck/codegen/runtime
                    // planning can see the handler bodies.
                    for mut handler_fn in self.lower_actor_handlers(a) {
                        handler_fn.durability = Some(crate::hir::nodes::DurabilityKind::Actor);
                        hir.functions.push(handler_fn);
                    }
                }
                Decl::Agent(a) => {
                    hir.agents.push(self.lower_agent(a));
                }
                Decl::Environment(e) => {
                    hir.environments.push(self.lower_environment(e));
                }
                Decl::Scheduled(s) => {
                    let mut lowered = self.lower_fn(&s.func);
                    lowered.schedule_interval = Some(s.interval.clone());
                    hir.functions.push(lowered);
                }
                Decl::BackButton(b) => {
                    hir.back_button = Some(crate::hir::nodes::HirBackButton {
                        on_press: b.on_press.clone(),
                        fallback: b.fallback.clone(),
                        span: b.span,
                    });
                }
                Decl::DeepLink(d) => {
                    hir.deep_link = Some(crate::hir::nodes::HirDeepLink {
                        scheme: d.scheme.clone(),
                        universal_link: d.universal_link.clone(),
                        on_link: d.on_link.clone(),
                        span: d.span,
                    });
                }
                Decl::Push(p) => {
                    hir.push = Some(crate::hir::nodes::HirPush {
                        on_register: p.on_register.clone(),
                        on_notification: p.on_notification.clone(),
                        on_action: p.on_action.clone(),
                        span: p.span,
                    });
                }
                Decl::Tokens(t) => {
                    use crate::hir::nodes::tokens::{
                        HirColorToken, HirFontToken, HirScalarToken, HirShadowToken, HirTokensDecl,
                    };
                    hir.token_decls.push(HirTokensDecl {
                        span: t.span,
                        colors: t
                            .colors
                            .iter()
                            .map(|c| HirColorToken {
                                name: c.name.clone(),
                                light: c.light.clone(),
                                dark: c.dark.clone(),
                                pair_bg: c.pair_bg.clone(),
                                span: c.span,
                            })
                            .collect(),
                        spacing: t
                            .spacing
                            .iter()
                            .map(|s| HirScalarToken {
                                name: s.name.clone(),
                                value: s.value.clone(),
                                span: s.span,
                            })
                            .collect(),
                        radius: t
                            .radius
                            .iter()
                            .map(|r| HirScalarToken {
                                name: r.name.clone(),
                                value: r.value.clone(),
                                span: r.span,
                            })
                            .collect(),
                        shadows: t
                            .shadows
                            .iter()
                            .map(|s| HirShadowToken {
                                name: s.name.clone(),
                                value: s.value.clone(),
                                span: s.span,
                            })
                            .collect(),
                        fonts: t
                            .fonts
                            .iter()
                            .map(|f| HirFontToken {
                                name: f.name.clone(),
                                family: f.family.clone(),
                                span: f.span,
                            })
                            .collect(),
                    });
                }
                _ => {
                    hir.legacy_ast_nodes.push(decl.clone());
                }
            }
        }

        // Analyze async logic
        for f in &mut hir.functions {
            if async_flags::has_async_stmts(&f.body) {
                f.is_async = true;
            }
        }
        for t in &mut hir.tests {
            if async_flags::has_async_stmts(&t.body) {
                t.is_async = true;
            }
        }
        for e in &mut hir.examples {
            if async_flags::has_async_stmts(&e.body) {
                e.is_async = true;
            }
        }

        db_select_normalize::normalize_db_select_projections(&mut hir);

        // Drain synthesised side_effect activities (P1-T7) into the function list.
        hir.functions.append(&mut self.synthesised_fns);

        hir
    }
}

/// GA-09a: Derive a typed `HirRouteId` from a single `RouteEntry`.
///
/// Extracts `:param` segments from the path and derives the analytics slug
/// from the component name via snake_case conversion.
fn route_entry_to_route_id(
    entry: &crate::ast::decl::ui::RouteEntry,
) -> crate::hir::nodes::boilerplate_grafts::HirRouteId {
    let params: Vec<(String, String)> = entry
        .path
        .split('/')
        .filter(|seg| seg.starts_with(':'))
        .map(|seg| (seg[1..].to_string(), "string".to_string()))
        .collect();
    let analytics_slug = pascal_to_snake(&entry.component_name);
    crate::hir::nodes::boilerplate_grafts::HirRouteId {
        name: entry.component_name.clone(),
        url_pattern: entry.path.clone(),
        params,
        analytics_slug,
        span: entry.span,
    }
}

fn pascal_to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Compute a deterministic SHA3-512 hex hash over a durable function's compile inputs.
///
/// Inputs: durability label, name, param names+types (Debug repr), return type, body
/// (Debug repr), and vox compiler version. Whitespace/comment changes in the source do
/// not affect this hash — HIR Debug output is canonical.
fn strip_spans(v: &mut serde_json::Value) {
    // Span objects serialize as {"start": N, "end": N} — detect by shape.
    if let serde_json::Value::Object(map) = v {
        if map.len() == 2 && map.contains_key("start") && map.contains_key("end") {
            *v = serde_json::Value::Null;
            return;
        }
        // Named "span" field on a struct — remove it then recurse.
        map.remove("span");
        for val in map.values_mut() {
            strip_spans(val);
        }
        return;
    }
    if let serde_json::Value::Array(arr) = v {
        for val in arr.iter_mut() {
            strip_spans(val);
        }
    }
}

fn stamp_durable_hash(f: &crate::hir::nodes::HirFn) -> String {
    use sha3::{Digest, Sha3_512};
    // Serialize only semantic fields, then strip all span objects so that
    // whitespace-only edits (which change source positions) don't bust the hash.
    let semantic = serde_json::json!({
        "kind": f.durability.as_ref().map(|d| d.label()),
        "name": f.name,
        "params": serde_json::to_value(&f.params).unwrap_or(serde_json::Value::Null),
        "return_type": serde_json::to_value(&f.return_type).unwrap_or(serde_json::Value::Null),
        "body": serde_json::to_value(&f.body).unwrap_or(serde_json::Value::Null),
        "ver": env!("CARGO_PKG_VERSION"),
    });
    let mut semantic = semantic;
    strip_spans(&mut semantic);
    let canonical = serde_json::to_string(&semantic).unwrap_or_default();
    let digest = Sha3_512::digest(canonical.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::cursor::lex;
    use crate::parser::parse;
    use crate::web_prefixes::{MUTATION_FN_API_PREFIX, QUERY_FN_API_PREFIX};

    fn lower_str(source: &str) -> HirModule {
        let tokens = lex(source);
        let module = parse(tokens).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
        lower_module(&module)
    }

    // `hir_lowering_leaves_no_legacy_nodes_for_core_web_decls` was retired:
    // its fixture relied on the tombstoned `http` and `@component` constructs,
    // which the parser now rejects outright, so the lowering shape it pinned
    // is unreachable. `golden_crud_api_vox_lowers_without_legacy_nodes` below
    // covers the no-legacy-nodes contract on live syntax.

    #[test]
    fn golden_crud_api_vox_lowers_without_legacy_nodes() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/golden/crud_api.vox"
        ));
        let tokens = lex(src);
        let module = parse(tokens).expect("examples/golden/crud_api.vox must parse");
        let hir = lower_module(&module);
        assert!(
            hir.legacy_ast_nodes.is_empty(),
            "unexpected legacy AST: {:?}",
            hir.legacy_ast_nodes
        );
        assert_eq!(hir.tables.len(), 1);
        assert_eq!(hir.endpoint_fns.len(), 4);
    }

    #[test]
    fn hir_lowering_db_filter_becomes_filter_record_ir() {
        let src = r#"
@table type User { name: str active: bool }
fn f() to int {
    return len(db.User.filter({ active: true }))
}
"#;
        let hir = lower_str(src);
        assert!(hir.legacy_ast_nodes.is_empty());
        let body = &hir.functions[0].body;
        let mut found = false;
        for st in body {
            if let crate::hir::HirStmt::Return { value: Some(e), .. } = st
                && let crate::hir::HirExpr::Call(callee, cargs, _, _) = e
                && let crate::hir::HirExpr::Ident(name, _) = callee.as_ref()
                && name == "len"
                && cargs.len() == 1
                && let crate::hir::HirExpr::MethodCall(_, method, _, Some(plan), _) =
                    &cargs[0].value
                && method == "filter"
                && plan.op == crate::hir::HirDbTableOp::FilterRecord
            {
                found = true;
            }
        }
        assert!(found, "expected FilterRecord in len(db.User.filter(...))");
    }

    #[test]
    fn hir_lowering_db_filter_count_chain_becomes_count_with_filter_args() {
        let src = r#"
@table type User { name: str active: bool }
fn f() to int {
    return db.User.filter({ active: true }).count()
}
"#;
        let hir = lower_str(src);
        assert!(hir.legacy_ast_nodes.is_empty());
        let body = &hir.functions[0].body;
        let mut found = false;
        for st in body {
            if let crate::hir::HirStmt::Return { value: Some(e), .. } = st
                && let crate::hir::HirExpr::MethodCall(_, method, args, Some(plan), _) = e
                && method == "count"
                && plan.op == crate::hir::HirDbTableOp::Count
                && args.len() == 1
            {
                found = true;
            }
        }
        assert!(
            found,
            "expected filter(...).count() to lower to DbTableOp::Count with filter args"
        );
    }

    #[test]
    fn hir_lowering_db_filter_order_limit_chain_keeps_modifiers() {
        let src = r#"
@table type User { name: str active: bool }
fn f() to Unit {
    db.User.filter({ active: true }).order_by("name", "desc").limit(5)
}
"#;
        let hir = lower_str(src);
        assert!(hir.legacy_ast_nodes.is_empty());
        let body = &hir.functions[0].body;
        let mut found = false;
        for st in body {
            if let crate::hir::HirStmt::Expr { expr, .. } = st
                && let crate::hir::HirExpr::MethodCall(_, _, _, Some(plan), _) = expr
                && plan.op == crate::hir::HirDbTableOp::FilterRecord
                && matches!(plan.order_by, Some(ref ob) if ob.0 == "name" && !ob.1)
                && plan.has_limit
            {
                found = true;
            }
        }
        assert!(found, "expected DbTableOp with order_by+limit modifiers");
    }

    #[test]
    fn hir_lowering_preserves_table_primary_key_and_db_plan_pk() {
        let src = r#"
@table(pk: email) type User { email: str active: bool }
fn f() to Unit {
    db.User.get("a@example.com")
}
"#;
        let hir = lower_str(src);
        assert_eq!(hir.tables.len(), 1);
        assert_eq!(hir.tables[0].primary_key.as_deref(), Some("email"));
        let mut found = false;
        for st in &hir.functions[0].body {
            if let crate::hir::HirStmt::Expr { expr, .. } = st
                && let crate::hir::HirExpr::MethodCall(_, method, _, Some(plan), _) = expr
                && method == "get"
            {
                assert_eq!(plan.primary_key.as_deref(), Some("email"));
                found = true;
            }
        }
        assert!(found, "expected get() call with attached db plan");
    }

    #[test]
    fn hir_lowering_db_all_select_sets_projection() {
        let src = r#"
@table type User { name: str active: bool }
fn f() to int {
    return len(db.User.all().select("name", "active"))
}
"#;
        let hir = lower_str(src);
        assert!(hir.legacy_ast_nodes.is_empty());
        let body = &hir.functions[0].body;
        let mut found = false;
        for st in body {
            if let crate::hir::HirStmt::Return { value: Some(e), .. } = st
                && let crate::hir::HirExpr::Call(callee, cargs, ..) = e
                && let crate::hir::HirExpr::Ident(fn_name, _) = callee.as_ref()
                && fn_name == "len"
                && cargs.len() == 1
                && let crate::hir::HirExpr::MethodCall(_, method, _, Some(plan), _) =
                    &cargs[0].value
                && method == "select"
                && plan.op == crate::hir::HirDbTableOp::All
                && plan.projection.as_ref().is_some_and(|c: &Vec<String>| {
                    c.len() == 2 && c[0] == "name" && c[1] == "active"
                })
            {
                found = true;
            }
        }
        assert!(
            found,
            "expected All+projection on db.User.all().select(...) chain"
        );
    }

    #[test]
    fn hir_lowering_db_where_object_builds_predicate_plan() {
        let src = r#"
@table type User { name: str age: int active: bool }
fn f() to int {
    return len(db.User.where({ age: { gte: 18 }, active: { eq: true } }))
}
"#;
        let hir = lower_str(src);
        assert!(hir.legacy_ast_nodes.is_empty());
        let body = &hir.functions[0].body;
        let mut found = false;
        for st in body {
            if let crate::hir::HirStmt::Return { value: Some(e), .. } = st
                && let crate::hir::HirExpr::Call(_, cargs, _, _) = e
                && let crate::hir::HirExpr::MethodCall(_, method, _, Some(plan), _) =
                    &cargs[0].value
                && method == "where"
            {
                found = matches!(
                    plan.predicate,
                    Some(crate::hir::HirDbPredicate::And(ref parts)) if parts.len() == 2
                );
            }
        }
        assert!(found, "expected where(...) predicate in DbQueryPlan");
    }

    #[test]
    fn hir_lowering_db_plan_capabilities_parse_chain_modifiers() {
        let src = r#"
@table type User { name: str active: bool }
fn f() to Unit {
    db.User.filter({ active: true }).using("hybrid").live("users.active").scope("populi").sync().limit(5)
}
"#;
        let hir = lower_str(src);
        assert!(hir.legacy_ast_nodes.is_empty());
        let body = &hir.functions[0].body;
        let mut found = false;
        for st in body {
            if let crate::hir::HirStmt::Expr { expr, .. } = st
                && let crate::hir::HirExpr::MethodCall(_, _, _, Some(plan), _) = expr
            {
                found = plan.capabilities.requires_sync
                    && plan.capabilities.live_topic.as_deref() == Some("users.active")
                    && plan.capabilities.orchestration_scope.as_deref() == Some("populi")
                    && matches!(
                        plan.capabilities.retrieval_mode,
                        Some(crate::hir::HirDbRetrievalMode::Hybrid)
                    );
            }
        }
        assert!(
            found,
            "expected chain modifiers to populate plan capabilities"
        );
    }

    /// `@query` and `@mutation` are the canonical replacements for the
    /// v0.5-era `@endpoint(kind: query)` / `@endpoint(kind: mutation)`
    /// forms (retired in v0.6.0 per Phase H step 18).  Both produce
    /// `HirEndpointFn` entries with the same `route_path` prefix.
    #[test]
    fn hir_lowering_maps_endpoint_query_and_mutation_decls() {
        let src = r#"
@table type User { name: str active: bool }
@query fn q1() to int { return 0 }
@mutation fn m1() to Unit {
    db.User.insert({ name: "a", active: true })
}
"#;
        let hir = lower_str(src);
        assert!(hir.legacy_ast_nodes.is_empty());
        assert_eq!(hir.endpoint_fns.len(), 2);
        assert_eq!(
            hir.endpoint_fns[0].route_path,
            format!("{QUERY_FN_API_PREFIX}q1")
        );
        assert_eq!(
            hir.endpoint_fns[1].route_path,
            format!("{MUTATION_FN_API_PREFIX}m1")
        );
    }

    /// Collection / vector / search declarations must lower to HIR vectors (not `legacy_ast_nodes`).
    #[test]
    fn hir_lowering_maps_collection_vector_search_out_of_legacy() {
        use crate::ast::decl::{
            CollectionDecl, Decl, Module, SearchIndexDecl, TableDecl, TableField, VectorIndexDecl,
        };
        use crate::ast::span::Span;
        use crate::ast::types::TypeExpr;

        let sp = Span::new(0, 0);
        let table = TableDecl {
            name: "Doc".into(),
            fields: vec![TableField {
                name: "title".into(),
                type_ann: TypeExpr::Named {
                    name: "str".into(),
                    span: sp,
                },
                description: None,
                span: sp,
            }],
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            is_pub: true,
            is_deprecated: false,
            primary_key: None,
            is_extern: false,
            source: None,
            span: sp,
        };
        let col = CollectionDecl {
            name: "Notes".into(),
            fields: vec![],
            description: None,
            is_pub: false,
            has_spread: false,
            span: sp,
        };
        let vix = VectorIndexDecl {
            table_name: "Doc".into(),
            index_name: "emb".into(),
            column: "v".into(),
            dimensions: 384,
            filter_fields: vec![],
            span: sp,
        };
        let six = SearchIndexDecl {
            table_name: "Doc".into(),
            index_name: "titles".into(),
            search_field: "title".into(),
            filter_fields: vec![],
            span: sp,
        };
        let module = Module {
            declarations: vec![
                Decl::Table(table),
                Decl::Collection(col),
                Decl::VectorIndex(vix),
                Decl::SearchIndex(six),
            ],
            span: sp,
        };
        let hir = lower_module(&module);
        assert!(
            hir.legacy_ast_nodes.is_empty(),
            "legacy_ast_nodes should not contain db index decls, got {:?}",
            hir.legacy_ast_nodes
        );
        assert_eq!(hir.collections.len(), 1);
        assert_eq!(hir.vector_indexes.len(), 1);
        assert_eq!(hir.search_indexes.len(), 1);
    }

    // `test_hir_lowering_environment` was retired: the `environment` top-level
    // construct was removed from the parser ("Unexpected token at top level"),
    // so the environments lowering it pinned is unreachable from source.

    #[test]
    fn hir_lowering_url_decl_goes_to_url_decls_not_legacy() {
        let src = "url Path {\nHome\nTask(id: str)\n}";
        let hir = lower_str(src);
        assert!(
            hir.legacy_ast_nodes.is_empty(),
            "url decl must not fall into legacy_ast_nodes, got {:?}",
            hir.legacy_ast_nodes
        );
        assert_eq!(hir.url_decls.len(), 1);
        assert_eq!(hir.url_decls[0].name, "Path");
        assert_eq!(hir.url_decls[0].variants.len(), 2);
        assert_eq!(hir.url_decls[0].variants[0].name, "Home");
        assert_eq!(hir.url_decls[0].variants[1].name, "Task");
        assert_eq!(hir.url_decls[0].variants[1].args.len(), 1);
        assert_eq!(hir.url_decls[0].variants[1].args[0].name, "id");
    }
}
