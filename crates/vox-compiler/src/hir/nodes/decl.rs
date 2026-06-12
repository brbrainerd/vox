//! HIR declarations: module shape, items, routes, tables (OP-0208).

use crate::ast::decl::fundecl::StyleBlock;
use crate::ast::span::Span;
use crate::hir::nodes::form::HirForm;
use crate::hir::nodes::tokens::HirTokensDecl;

use super::expr::HirExpr;
use super::stmt::HirStmt;
use super::stmt_expr::{DefId, HirParam, HirType};

/// Ownership buckets used to classify [`HirModule`] fields during the WebIR/AppContract migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirFieldOwnership {
    /// Stable semantic program representation that should remain in post-migration HIR.
    SemanticCore,
    /// Transitional data retained only to bridge legacy emit paths.
    MigrationOnly,
    /// Data that can be projected into an externalized app surface contract.
    AppContract,
    /// Native shell / mobile primitives (`@back_button`, `@deep_link`, `@push`) projected to `ShellProjectionModule`.
    Shell,
}

/// A fully lowered Vox module: every declaration category is collected into its own vector.
///
/// Empty vectors mean the construct was absent in source; there is no implicit ordering across
/// categories—only within each `Vec` matches source order for that kind.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HirModule {
    /// Resolved `import` entries.
    pub imports: Vec<HirImport>,
    /// Rust crate imports declared in source.
    pub rust_imports: Vec<HirRustImport>,
    /// Functions and `@component` bodies.
    pub functions: Vec<HirFn>,
    /// Algebraic and struct types.
    pub types: Vec<HirTypeDef>,

    /// `@test` functions.
    pub tests: Vec<HirFn>,
    /// `@example` functions — authored reference demonstrations for corpus / docs.
    #[serde(default)]
    pub examples: Vec<HirFn>,
    /// `@forall` properties.
    pub foralls: Vec<HirForall>,
    /// Unified endpoint functions (`@endpoint`, `@server`, `@query`, `@mutation`).
    pub endpoint_fns: Vec<HirEndpointFn>,
    /// Codex table schemas.
    pub tables: Vec<HirTable>,
    /// Table indexes.
    pub indexes: Vec<HirIndex>,
    /// Document collections (schemaless / doc store).
    pub collections: Vec<HirCollection>,
    /// Vector / embedding indexes.
    pub vector_indexes: Vec<HirVectorIndex>,
    /// Full-text search indexes.
    pub search_indexes: Vec<HirSearchIndex>,
    /// MCP tool handlers.
    pub mcp_tools: Vec<HirMcpTool>,
    /// MCP read-only resource handlers (`@mcp.resource`).
    pub mcp_resources: Vec<HirMcpResource>,
    /// Native agent definitions.
    pub agents: Vec<HirAgent>,
    /// Container environment specifications.
    pub environments: Vec<HirEnvironment>,

    // UI surfaces (AST-retained where needed for emit + WebIR projection)
    /// Reactive components (Path C).
    pub components: Vec<HirReactiveComponent>,
    /// Client routing blocks (`routes { … }`).
    pub client_routes: Vec<crate::ast::decl::RoutesDecl>,

    /// Typed URL path declarations (`url Name { … }`).
    pub url_decls: Vec<HirUrlDecl>,

    /// State machine declarations (`state_machine Name { … }`).
    pub state_machines: Vec<HirStateMachineDecl>,

    /// Typed parametric fragments (`fragment Name(args) { … }`) per ADR-033.
    /// Each fragment lowers to a typed React function component in the codegen
    /// layer; consumer components reference fragments by name in `<RenderFragment>`.
    #[serde(default)]
    pub fragments: Vec<HirFragmentDecl>,

    /// `.vox.ui` reactive modules (ADR-032). Each lowers to a generated React
    /// context + provider + `use<Name>()` hook in TSX emit.
    #[serde(default)]
    pub reactive_modules: Vec<HirReactiveModule>,

    /// Form declarations lowered from `@form`.
    #[serde(default)]
    pub forms: Vec<HirForm>,

    /// Back-button handler lowered from `@back_button` (at most one per module).
    #[serde(default)]
    pub back_button: Option<HirBackButton>,

    /// Deep-link / universal-link handler lowered from `@deep_link` (at most one per module).
    #[serde(default)]
    pub deep_link: Option<HirDeepLink>,

    /// Push-notification wiring lowered from `@push` (at most one per module).
    #[serde(default)]
    pub push: Option<HirPush>,

    /// Project-level design-token declarations (`tokens { … }`), CC-23.
    #[serde(default)]
    pub token_decls: Vec<HirTokensDecl>,

    /// Typed route ids derived from `routes { … }` blocks (GA-09a).
    /// Each entry drives `emit_route_id_module()` in codegen-ts.
    #[serde(default)]
    pub route_ids: Vec<crate::hir::nodes::boilerplate_grafts::HirRouteId>,

    /// Map of source spans to their inferred types, populated by the type checker.
    /// This supports performance-critical codegen optimizations (e.g. zero-copy emission).
    ///
    /// Serialized through string-keyed adapters because `serde_json` requires map keys to
    /// be strings; the `"start-end"` key format round-trips losslessly.
    #[serde(
        default,
        serialize_with = "serialize_inferred_types",
        deserialize_with = "deserialize_inferred_types"
    )]
    pub inferred_types: std::collections::HashMap<Span, HirType>,

    /// Declarations not yet represented as typed HIR vectors (unknown / future decl kinds).
    pub legacy_ast_nodes: Vec<crate::ast::decl::Decl>,
}

/// Snapshot of a post-migration semantic-only HIR shape.
///
/// This is intentionally a strict subset of [`HirModule`]: migration-only fields are dropped and
/// app-surface fields can be projected into `app_contract` while preserving semantic-core data.
#[derive(Debug, Clone, Default)]
pub struct SemanticHirModule {
    pub imports: Vec<HirImport>,
    pub rust_imports: Vec<HirRustImport>,
    pub functions: Vec<HirFn>,
    pub types: Vec<HirTypeDef>,

    pub tests: Vec<HirFn>,
    pub endpoint_fns: Vec<HirEndpointFn>,
    pub tables: Vec<HirTable>,
    pub indexes: Vec<HirIndex>,
    pub collections: Vec<HirCollection>,
    pub vector_indexes: Vec<HirVectorIndex>,
    pub search_indexes: Vec<HirSearchIndex>,
    pub mcp_tools: Vec<HirMcpTool>,
    pub mcp_resources: Vec<HirMcpResource>,
    pub agents: Vec<HirAgent>,
    pub environments: Vec<HirEnvironment>,
    pub components: Vec<HirReactiveComponent>,
    pub url_decls: Vec<HirUrlDecl>,
    pub fragments: Vec<HirFragmentDecl>,
    pub reactive_modules: Vec<HirReactiveModule>,
}

impl HirModule {
    /// Return a fixed ownership map for all top-level [`HirModule`] vectors/fields.
    #[must_use]
    pub fn field_ownership_map() -> Vec<(&'static str, HirFieldOwnership)> {
        vec![
            ("imports", HirFieldOwnership::SemanticCore),
            ("rust_imports", HirFieldOwnership::SemanticCore),
            ("functions", HirFieldOwnership::SemanticCore),
            ("types", HirFieldOwnership::SemanticCore),
            ("tests", HirFieldOwnership::SemanticCore),
            ("foralls", HirFieldOwnership::SemanticCore),
            ("endpoint_fns", HirFieldOwnership::AppContract),
            ("tables", HirFieldOwnership::SemanticCore),
            ("indexes", HirFieldOwnership::SemanticCore),
            ("collections", HirFieldOwnership::SemanticCore),
            ("vector_indexes", HirFieldOwnership::SemanticCore),
            ("search_indexes", HirFieldOwnership::SemanticCore),
            ("mcp_tools", HirFieldOwnership::AppContract),
            ("mcp_resources", HirFieldOwnership::AppContract),
            ("agents", HirFieldOwnership::SemanticCore),
            ("environments", HirFieldOwnership::SemanticCore),
            ("components", HirFieldOwnership::SemanticCore),
            ("client_routes", HirFieldOwnership::SemanticCore),
            ("url_decls", HirFieldOwnership::SemanticCore),
            ("state_machines", HirFieldOwnership::SemanticCore),
            ("fragments", HirFieldOwnership::SemanticCore),
            ("reactive_modules", HirFieldOwnership::SemanticCore),
            ("forms", HirFieldOwnership::AppContract),
            ("back_button", HirFieldOwnership::Shell),
            ("deep_link", HirFieldOwnership::Shell),
            ("push", HirFieldOwnership::Shell),
            ("token_decls", HirFieldOwnership::SemanticCore),
            ("route_ids", HirFieldOwnership::SemanticCore),
            ("legacy_ast_nodes", HirFieldOwnership::MigrationOnly),
        ]
    }

    /// Project this module into a migration-free semantic snapshot.
    #[must_use]
    pub fn to_semantic_hir(&self) -> SemanticHirModule {
        SemanticHirModule {
            imports: self.imports.clone(),
            rust_imports: self.rust_imports.clone(),
            functions: self.functions.clone(),
            types: self.types.clone(),

            tests: self.tests.clone(),
            endpoint_fns: self.endpoint_fns.clone(),
            tables: self.tables.clone(),
            indexes: self.indexes.clone(),
            collections: self.collections.clone(),
            vector_indexes: self.vector_indexes.clone(),
            search_indexes: self.search_indexes.clone(),
            mcp_tools: self.mcp_tools.clone(),
            mcp_resources: self.mcp_resources.clone(),
            agents: self.agents.clone(),
            environments: self.environments.clone(),
            components: self.components.clone(),
            url_decls: self.url_decls.clone(),
            fragments: self.fragments.clone(),
            reactive_modules: self.reactive_modules.clone(),
        }
    }
}

/// A resolved import.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirImport {
    /// Module path segments (`["react"]`, `["std", "http"]`).
    pub module_path: Vec<String>,
    /// Imported symbol name.
    pub item: String,
    /// When `Some`, this is an external React/TS import (Phase 5); the emitted
    /// statement targets `"<spec>"`. The shape (default/named/namespace) is in
    /// [`Self::es_import_kind`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub es_module_specifier: Option<String>,
    /// Import shape for an external React/TS import. `None` for non-ES imports
    /// (symbol paths, local files). `Some` whenever `es_module_specifier` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub es_import_kind: Option<EsImportKind>,
    /// When `Some(path)`, this is an intra-project Vox-file import
    /// (`import "./helpers/foo.vox"`). The path is resolved relative to the
    /// importing file at run-module time; `pub fn`s from the target are
    /// merged into the importer's scope (with optional `as alias` namespace).
    /// See `docs/src/architecture/intra-project-imports-rfc-2026-05-23.md`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_file_path: Option<String>,
    /// When `Some(name)`, exported pubs from a local-file import are namespaced
    /// under this name (e.g. `import "./util.vox" as u` → `u.fn_name(...)`).
    /// When `None`, exported pubs merge directly into the importer's scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_file_alias: Option<String>,
    /// Span in source.
    pub span: Span,
}

/// Shape of an external React/TS import (Phase 5). Drives TS emission:
/// the local binding name is [`HirImport::item`] and the module is
/// [`HirImport::es_module_specifier`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EsImportKind {
    /// `import <item> from "<spec>"`
    Default,
    /// `import { <imported> as <item> } from "<spec>"` (no `as` when `imported == item`).
    Named {
        /// Name exported by the module.
        imported: String,
    },
    /// `import * as <item> from "<spec>"`
    Namespace,
}

/// A Rust crate import declaration lowered from source.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirRustImport {
    /// Dependency key / crate name.
    pub crate_name: String,
    /// Binding name in source scope.
    pub alias: String,
    /// Optional semantic version requirement.
    pub version: Option<String>,
    /// Optional local path source.
    pub path: Option<String>,
    /// Optional git source URL.
    pub git: Option<String>,
    /// Optional git revision / branch hint.
    pub rev: Option<String>,
    /// Span in source.
    pub span: Span,
}

/// A function or component in HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirFn {
    /// Stable definition id.
    pub id: DefId,
    /// Function name.
    pub name: String,
    /// Generic parameter names.
    pub generics: Vec<String>,
    /// Parameters with optional types and defaults.
    pub params: Vec<HirParam>,
    /// Return type, if annotated.
    pub return_type: Option<HirType>,
    /// Body statements.
    pub body: Vec<HirStmt>,
    /// Async function.
    pub is_async: bool,
    /// Exported from module.
    pub is_pub: bool,
    /// Whether this is a `@mobile.native` bridge.
    pub is_mobile_native: bool,
    /// `@pure` — metadata for pipeline tooling (effect guarantees are not proven in HIR).
    #[serde(default)]
    pub is_pure: bool,
    /// `@reactive` — opt-in marker that authorizes `vox_codegen::codegen_ts::hir_emit::state_deps`
    /// to recurse into this function's body when computing reactive dependencies for a
    /// caller's `derived` / `effect`. Without it, the dep walker stops at the call site.
    #[serde(default)]
    pub is_reactive: bool,
    /// `@versioned` / `@tracked` — see `FnDecl::is_versioned`. The `--mode interp`
    /// interpreter auto-records one `repo.snapshot()` checkpoint on this function's
    /// successful return (design §4.3). Inert (not read) in the compiled arms
    /// (`--mode script` / TS emit), which have no interpreter `RepoStore` — P5 is
    /// an interpreter-only feature, consistent with `db.*`'s in-memory `DbStore`.
    #[serde(default)]
    pub is_versioned: bool,
    /// Capabilities declared via `uses` clause. Empty = unannotated; `[Nothing]` = pure.
    #[serde(default)]
    pub capabilities: Vec<HirCapability>,
    /// `@remote` — eligible for cross-node dispatch via the mesh (P1-T3).
    #[serde(default)]
    pub is_remote: bool,
    /// Whether the function body is implemented via an LLM.
    #[serde(default)]
    pub is_llm: bool,
    /// Optional specific LLM model to use for implementation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_model: Option<String>,
    /// Structured-output contract for `@ai` functions (GA-21).
    /// When `Some`, `check_ai_return_shape()` verifies the return type has a wire codec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_structured_output: Option<crate::hir::nodes::boilerplate_grafts::HirAiStructuredOutput>,
    /// Unified AI fixture surface attached to this function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_fixture: Option<crate::hir::nodes::boilerplate_grafts::HirAiFixture>,
    /// Embedding spec from `@embed(model:, dimensions:, source_field:)` (GA-24).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed: Option<crate::hir::nodes::boilerplate_grafts::HirEmbedDecl>,
    /// `@deprecated` on the source `fn`.
    #[serde(default)]
    pub is_deprecated: bool,
    /// `@scheduled("…")` interval/cron string when this item was lowered from [`crate::ast::decl::Decl::Scheduled`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_interval: Option<String>,
    /// Durable execution classification (set when lowered from `workflow`/`activity`/`actor` source).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durability: Option<super::durability::DurabilityKind>,
    /// State fields declared in an `actor` body (`state name: Type`).
    /// Empty for non-actor functions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actor_state_fields: Vec<HirTableField>,
    /// Postconditions to check at runtime (used for @ai repair/fallback).
    #[serde(default)]
    pub postconditions: Vec<HirPostCondition>,
    /// `extern fn name(...) to T = "./module"` — TS-source FFI module path.
    /// When `Some`, body is empty and codegen-TS emits an import for the function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts_extern_module: Option<String>,
    /// P2-T1: Stable SHA3-512 content-hash (hex) of this function's compile inputs.
    /// Populated for `DurabilityKind::Workflow` and `DurabilityKind::Activity` by the
    /// HIR lowering pass. `None` for plain `fn` and for `actor` (actors live in mailboxes,
    /// not the bundle CAS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_hash: Option<String>,
    /// Span covering the declaration.
    pub span: Span,
    /// `@inference(model = "...")` — Mn-T4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_model: Option<String>,
    /// `@training_step` — Mn-T5.
    #[serde(default)]
    pub training_step: bool,
    /// `@distributed_train(...)` metadata when lowered from a workflow — Mn-T5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_train: Option<(String, u32)>,
}

/// A lowered postcondition with optional fallback.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirPostCondition {
    /// Condition expression.
    pub condition: HirExpr,
    /// Optional fallback function name.
    pub fallback: Option<String>,
}

/// ADT / type definition in HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirTypeDef {
    /// Type definition id.
    pub id: DefId,
    /// Type name.
    pub name: String,
    /// Variants for sum types; empty for struct types and aliases.
    pub variants: Vec<HirVariant>,
    /// Struct fields for product types; empty for sum types and aliases.
    #[serde(default)]
    pub fields: Vec<(String, HirType)>,
    /// Exported type.
    pub is_pub: bool,
    /// Span covering the definition.
    pub span: Span,
}

/// One variant of an ADT.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirVariant {
    /// Constructor name.
    pub name: String,
    /// Named fields with types.
    pub fields: Vec<(String, HirType)>,
    /// Span covering the variant.
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HirEndpointKind {
    Query,
    Mutation,
    Server,
}

/// A server endpoint function — callable from the frontend, auto-generates API route + fetch wrapper.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirEndpointFn {
    /// Endpoint kind (query, mutation, server).
    pub kind: HirEndpointKind,
    /// Function id.
    pub id: DefId,
    /// Exposed name.
    pub name: String,
    /// Parameters.
    pub params: Vec<HirParam>,
    /// Return type.
    pub return_type: Option<HirType>,
    /// Function body.
    pub body: Vec<HirStmt>,
    /// HTTP path bound to this endpoint fn.
    pub route_path: String,
    /// `@pure` — endpoint is side-effect-free (informational; not enforced at call-graph level yet).
    #[serde(default)]
    pub is_pure: bool,
    /// Declared capability effects from `uses net, db, mcp(...)` (TASK-4.2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: super::effect::HirEffectSet,
    /// `@webhook(provider:, secret:, replay_window_secs:)` decorator (GA-16).
    #[serde(default)]
    pub webhook: Option<super::boilerplate_grafts::HirWebhookDecl>,
    /// `@cors(origins:, allow_credentials:)` sidecar (GA-06).
    #[serde(default)]
    pub cors: Option<super::http_ergonomics::HirCorsPolicy>,
    /// `@rate_limit(by:, window_secs:, max:)` sidecar (GA-06).
    #[serde(default)]
    pub rate_limit: Option<super::http_ergonomics::HirRateLimitPolicy>,
    /// `@pii(class:)` marker on this endpoint (GA-23).
    #[serde(default)]
    pub pii: Option<super::boilerplate_grafts::HirPiiMarker>,
    /// `@layer(tier:)` Z-tier annotation (GA-26).
    #[serde(default)]
    pub layer: Option<super::layer::HirLayerDecl>,
    /// Span covering the declaration.
    pub span: Span,
}

/// Table definition — a persistent record type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirTable {
    /// Table id.
    pub id: DefId,
    /// Table name.
    pub name: String,
    /// Columns.
    pub fields: Vec<HirTableField>,
    /// Public API surface.
    pub is_pub: bool,
    /// `@deprecated` table.
    pub is_deprecated: bool,
    /// Span covering the table.
    pub span: Span,
}

/// A field within a table definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirTableField {
    /// Column name.
    pub name: String,
    /// Column type.
    pub type_ann: HirType,
    /// Span covering the field.
    pub span: Span,
}

/// Index definition for a table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirIndex {
    /// Table being indexed.
    pub table_name: String,
    /// Index name.
    pub index_name: String,
    /// Indexed columns.
    pub columns: Vec<String>,
    /// Span covering the index decl.
    pub span: Span,
}

/// Document collection — fields mirror [`crate::ast::decl::CollectionDecl`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirCollection {
    pub id: DefId,
    pub name: String,
    pub fields: Vec<HirTableField>,
    pub is_pub: bool,
    pub has_spread: bool,
    pub span: Span,
}

/// Vector index metadata lowered from AST.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirVectorIndex {
    pub table_name: String,
    pub index_name: String,
    pub column: String,
    pub dimensions: u32,
    pub filter_fields: Vec<String>,
    pub span: Span,
}

/// Full-text search index lowered from AST.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirSearchIndex {
    pub table_name: String,
    pub index_name: String,
    pub search_field: String,
    pub filter_fields: Vec<String>,
    pub span: Span,
}

/// MCP tool declaration — a function exposed via the Model Context Protocol.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirMcpTool {
    /// Tool description for MCP clients.
    pub description: String,
    /// Underlying implementation.
    pub func: HirFn,
}

/// MCP resource — read-only URI-backed content for MCP `resources/read`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirMcpResource {
    /// Stable resource URI (e.g. `notes://recent`).
    pub uri: String,
    pub description: String,
    pub func: HirFn,
}

/// A property-based `@forall` test.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirForall {
    pub label: String,
    pub iterations: u32,
    pub func: HirFn,
}

/// Reactive component lowered to HIR (Path C).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirReactiveComponent {
    pub id: DefId,
    pub name: String,
    pub params: Vec<HirParam>,
    pub members: Vec<HirReactiveMember>,
    pub view: Option<HirExpr>,
    /// Scoped CSS blocks attached after the reactive body (Path C).
    pub styles: Vec<StyleBlock>,
    /// Optional `@layer(tier:)` Z-tier declaration (GA-26).
    #[serde(default)]
    pub layer: Option<crate::hir::nodes::layer::HirLayerDecl>,
    pub span: Span,
}

/// Native agent definition in HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirAgent {
    /// Agent id.
    pub id: DefId,
    /// Agent name.
    pub name: String,
    /// Optional version tag.
    pub version: Option<String>,
    /// Internal state fields.
    pub state_fields: Vec<HirTableField>,
    /// Event handlers.
    pub handlers: Vec<HirAgentHandler>,
    /// Version migration rules.
    pub migrations: Vec<HirMigrationRule>,
    /// `@deprecated` agent.
    pub is_deprecated: bool,
    /// Span covering the agent.
    pub span: Span,
}

/// Single agent handler.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirAgentHandler {
    /// Event name.
    pub event_name: String,
    /// Parameters.
    pub params: Vec<HirParam>,
    /// Return type.
    pub return_type: Option<HirType>,
    /// Handler body.
    pub body: Vec<HirStmt>,
    /// Tracing enabled.
    pub is_traced: bool,
    /// Span covering the handler.
    pub span: Span,
}

/// Agent migration rule.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirMigrationRule {
    pub from_version: String,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

/// Container environment specification in HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirEnvironment {
    pub name: String,
    pub base_image: Option<String>,
    pub packages: Vec<String>,
    pub env_vars: Vec<(String, String)>,
    pub exposed_ports: Vec<u16>,
    pub volumes: Vec<String>,
    pub workdir: Option<String>,
    pub cmd: Vec<String>,
    pub copy_instructions: Vec<(String, String)>,
    pub run_commands: Vec<String>,
    pub is_deprecated: bool,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HirReactiveMember {
    State(HirState),
    Derived(HirDerived),
    Effect(HirEffect),
    OnMount(HirOnMount),
    OnCleanup(HirOnCleanup),
    /// `let` / hook calls / assignments before `view:` (Path C prelude).
    Stmt(HirStmt),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirState {
    pub id: DefId,
    pub name: String,
    pub ty: Option<HirType>,
    pub init: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirDerived {
    pub id: DefId,
    pub name: String,
    pub ty: Option<HirType>,
    pub expr: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirEffect {
    pub body: HirExpr,
    /// Explicit dependency list from `effect depends_on (a, b):` clause.
    /// When `Some`, the lint `lint.effect.unresolvable_deps` is suppressed.
    pub explicit_deps: Option<Vec<String>>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirOnMount {
    pub body: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirOnCleanup {
    pub body: HirExpr,
    pub span: Span,
}

/// A capability declared via the `uses` clause: `fn f() uses net, db { … }`.
///
/// Distinct from [`HirEffect`] which is a reactive lifecycle effect (`effect { … }` block).
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum HirCapability {
    Net,
    Db,
    Fs,
    Env,
    Clock,
    Random,
    Spawn,
    GpuCompute,
    Mutate,
    /// Version-control / repository operations (`repo.*` / `vcs.*` builtins).
    Vcs,
    /// `mcp(tool_name)` — parameterized MCP tool call.
    Mcp(String),
    /// `uses nothing` — explicitly pure.
    Nothing,
}

impl HirCapability {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Net => "net",
            Self::Db => "db",
            Self::Fs => "fs",
            Self::Env => "env",
            Self::Clock => "clock",
            Self::Random => "random",
            Self::Spawn => "spawn",
            Self::GpuCompute => "gpu_compute",
            Self::Mutate => "mutate",
            Self::Vcs => "vcs",
            Self::Mcp(_) => "mcp",
            Self::Nothing => "nothing",
        }
    }
}

impl std::fmt::Display for HirCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mcp(tool) => write!(f, "mcp({tool})"),
            other => write!(f, "{}", other.as_str()),
        }
    }
}

/// A typed URL path declaration lowered to HIR: `url Path { Home; Task(id: Id[Task]) }`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirUrlDecl {
    pub id: DefId,
    pub name: String,
    pub variants: Vec<HirUrlVariant>,
    pub is_pub: bool,
    pub span: Span,
}

/// A single variant in a [`HirUrlDecl`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirUrlVariant {
    pub name: String,
    pub args: Vec<HirUrlArg>,
    pub span: Span,
}

/// A parameter in a [`HirUrlVariant`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirUrlArg {
    pub name: String,
    pub optional: bool,
    pub ty: HirType,
    pub span: Span,
}

// ── State machine HIR types (TASK-4.1) ────────────────────────────────────

/// A `.vox.ui` reactive module lowered to HIR (ADR-032).
///
/// Members re-use the existing [`HirReactiveMember`] enum from reactive
/// component lowering, so the inner shape (state / derived / effect / on-mount /
/// on-cleanup) is identical to what a `component { }` body produces.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirReactiveModule {
    /// Module name. Filled in by codegen from the source file basename
    /// (the parser leaves it empty).
    pub name: String,
    /// Reactive members declared at module scope.
    pub members: Vec<HirReactiveMember>,
    /// Source span.
    pub span: Span,
}

/// A `fragment Name(args) { … }` declaration lowered to HIR (ADR-033).
///
/// Fragments emit as typed React function components with a fixed `Args` prop
/// shape derived from the parameter list, so consumers reference them via
/// `<RenderFragment of={Name} args={(...)} />` and the compiler validates
/// arity / types at the call site.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirFragmentDecl {
    /// Fragment name (PascalCase by convention).
    pub name: String,
    /// Typed parameters; same shape as a function parameter list.
    pub params: Vec<HirParam>,
    /// Single markup body. Today a generic [`HirExpr`]; the codegen layer asserts
    /// it's a markup-shaped expression (Phase 6 primitive call) at emit time.
    pub body: HirExpr,
    /// Source span.
    pub span: Span,
}

/// A `state_machine Name { … }` lowered to HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirStateMachineDecl {
    pub id: DefId,
    pub name: String,
    pub states: Vec<HirSmState>,
    pub transitions: Vec<HirSmTransition>,
    /// `partial state_machine` — exhaustiveness check is skipped.
    pub is_partial: bool,
    pub is_pub: bool,
    pub span: Span,
}

/// A state variant inside a state machine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirSmState {
    pub name: String,
    pub fields: Vec<HirSmField>,
    pub is_terminal: bool,
    pub span: Span,
}

/// A payload field on a state variant or event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirSmField {
    pub name: String,
    pub ty: Option<HirType>,
    pub span: Span,
}

/// A transition rule: `on Event from State -> Target`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirSmTransition {
    pub event_name: String,
    pub event_params: Vec<String>,
    pub from: HirSmFrom,
    pub to_state: String,
    pub span: Span,
}

/// The `from` clause of a transition.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HirSmFrom {
    Named(String),
    Any,
}

/// `@back_button` lowered to HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirBackButton {
    /// Endpoint function called on back-press; should return bool (handled?).
    pub on_press: String,
    /// Optional fallback identifier.
    pub fallback: Option<String>,
    /// Source span.
    pub span: Span,
}

/// `@deep_link` lowered to HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirDeepLink {
    /// URL scheme string.
    pub scheme: String,
    /// Optional Apple universal link domain.
    pub universal_link: Option<String>,
    /// Endpoint function called with the opened URL.
    pub on_link: String,
    /// Source span.
    pub span: Span,
}

/// `@push` lowered to HIR.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirPush {
    /// Endpoint called after push registration to store the token.
    pub on_register: Option<String>,
    /// Endpoint called when a notification is received in the foreground.
    pub on_notification: Option<String>,
    /// Endpoint called when the user taps a notification action.
    pub on_action: Option<String>,
    /// Source span.
    pub span: Span,
}

/// Serialize `HashMap<Span, HirType>` with `"start-end"` string keys so the JSON output is
/// `serde_json`-compatible (which requires map keys to be strings).
///
/// Paired with [`deserialize_inferred_types`] for a lossless round-trip.
fn serialize_inferred_types<S>(
    map: &std::collections::HashMap<Span, HirType>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    // BTreeMap-ordered output for determinism — keeps emitted Rust source byte-stable.
    let mut ordered: std::collections::BTreeMap<(usize, usize), &HirType> =
        std::collections::BTreeMap::new();
    for (span, ty) in map {
        ordered.insert((span.start, span.end), ty);
    }
    let mut s = serializer.serialize_map(Some(ordered.len()))?;
    for ((start, end), ty) in ordered {
        s.serialize_entry(&format!("{start}-{end}"), ty)?;
    }
    s.end()
}

/// Deserialize the `"start-end"` string keys back into `Span`s. Errors on malformed keys.
fn deserialize_inferred_types<'de, D>(
    deserializer: D,
) -> Result<std::collections::HashMap<Span, HirType>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    let stringified: std::collections::HashMap<String, HirType> =
        std::collections::HashMap::<String, HirType>::deserialize(deserializer)?;
    let mut out = std::collections::HashMap::with_capacity(stringified.len());
    for (key, val) in stringified {
        let (start_s, end_s) = key.split_once('-').ok_or_else(|| {
            serde::de::Error::custom(format!(
                "invalid inferred_types span key `{key}`: expected `<start>-<end>`"
            ))
        })?;
        let start: usize = start_s.parse::<usize>().map_err(|e| {
            serde::de::Error::custom(format!("invalid start `{start_s}` in `{key}`: {e}"))
        })?;
        let end: usize = end_s.parse::<usize>().map_err(|e| {
            serde::de::Error::custom(format!("invalid end `{end_s}` in `{key}`: {e}"))
        })?;
        out.insert(Span { start, end }, val);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{HirFieldOwnership, HirModule};

    #[test]
    fn hir_field_ownership_map_contains_migration_markers() {
        let map = HirModule::field_ownership_map();
        assert!(
            map.iter().any(|(name, own)| *name == "legacy_ast_nodes"
                && *own == HirFieldOwnership::MigrationOnly)
        );
        assert!(
            map.iter().any(
                |(name, own)| *name == "endpoint_fns" && *own == HirFieldOwnership::AppContract
            )
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "functions" && *own == HirFieldOwnership::SemanticCore)
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "back_button" && *own == HirFieldOwnership::Shell)
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "deep_link" && *own == HirFieldOwnership::Shell)
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "push" && *own == HirFieldOwnership::Shell)
        );
        assert!(map.iter().any(
            |(name, own)| *name == "state_machines" && *own == HirFieldOwnership::SemanticCore
        ));
        assert!(
            map.iter()
                .any(|(name, own)| *name == "route_ids" && *own == HirFieldOwnership::SemanticCore)
        );
        assert!(
            map.iter().any(
                |(name, own)| *name == "token_decls" && *own == HirFieldOwnership::SemanticCore
            )
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "mcp_tools" && *own == HirFieldOwnership::AppContract)
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "mcp_resources"
                    && *own == HirFieldOwnership::AppContract)
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "foralls" && *own == HirFieldOwnership::SemanticCore)
        );
        assert!(
            map.iter()
                .any(|(name, own)| *name == "client_routes"
                    && *own == HirFieldOwnership::SemanticCore)
        );
    }

    #[test]
    fn semantic_hir_projection_drops_migration_vectors() {
        let hir = HirModule::default();
        let projected = hir.to_semantic_hir();
        // These vectors are migration-only and intentionally absent from `SemanticHirModule`.
        assert!(projected.functions.is_empty());
    }
}
