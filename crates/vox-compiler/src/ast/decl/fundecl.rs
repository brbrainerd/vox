use crate::ast::expr::{Expr, Param};
use crate::ast::span::Span;
use crate::ast::stmt::Stmt;
use crate::ast::types::TypeExpr;

/// Verification mode for contracts and assertions.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VerifyMode {
    /// No runtime checks; contracts are stripped.
    Off,
    /// Preconditions are checked; postconditions are ignored (development safe).
    RequireOnly,
    /// Both preconditions and postconditions are checked.
    Full,
}

/// A postcondition requirement with an optional fallback for repair.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PostCondition {
    /// The condition expression to check.
    pub condition: Expr,
    /// Optional fallback function name to call if the condition fails.
    pub fallback: Option<String>,
}

/// Function declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FnDecl {
    /// The name of the function.
    pub name: String,
    /// Generic parameter names.
    pub generics: Vec<String>,
    /// List of function parameters.
    pub params: Vec<Param>,
    /// Explicit return type annotation.
    pub return_type: Option<TypeExpr>,
    /// The function body (sequence of statements).
    pub body: Vec<Stmt>,
    /// Whether the function is asynchronous.
    pub is_async: bool,
    /// Whether the function is marked as deprecated.
    pub is_deprecated: bool,
    /// Whether the function is pure (no side effects).
    pub is_pure: bool,
    /// Whether the function carries an `@reactive` decorator. When set, the auto-dep
    /// inference pass at `vox_codegen::codegen_ts::hir_emit::state_deps` is allowed to
    /// recurse into the body when this function is called from a reactive `derived` /
    /// `effect`, so that reads of reactive bindings through the call participate in
    /// dependency tracking. Default: `false` (the dep walker stops at the call site).
    #[serde(default)]
    pub is_reactive: bool,
    /// Explicit effect annotations from the `uses` clause.
    /// Empty means unannotated (unconstrained); `[Nothing]` means `uses nothing` (pure).
    #[serde(default)]
    pub effects: Vec<super::effect::EffectAnnotation>,
    /// Whether the function is subject to observability tracing.
    pub is_traced: bool,
    /// Whether the function body is implemented via an LLM.
    pub is_llm: bool,
    /// Optional specific LLM model to use for implementation.
    pub llm_model: Option<String>,
    /// `structured_output: TypeName` arg from `@ai(structured_output = TypeName)` (GA-21).
    #[serde(default)]
    pub ai_structured_output_type: Option<String>,
    /// `max_iterations: N` arg from `@ai(max_iterations = N)` (GA-21). Default 3.
    #[serde(default)]
    pub ai_max_iterations: u32,
    /// `task_category: Name` arg from extended `@ai(...)` fixture routing.
    #[serde(default)]
    pub ai_task_category: Option<String>,
    /// `strengths: [name, ...]` arg from extended `@ai(...)` fixture routing.
    #[serde(default)]
    pub ai_strengths: Vec<String>,
    /// `tier_max: Local|Light|Pro|Elite` arg from extended `@ai(...)` fixture routing.
    #[serde(default)]
    pub ai_tier_max: Option<String>,
    /// `cost_ceiling_usd_per_call: N` arg from extended `@ai(...)` fixture routing.
    #[serde(default)]
    pub ai_cost_ceiling_usd_per_call: Option<f64>,
    /// `@prompt(stage = ..., schema = ...)` proposed fixture stage.
    #[serde(default)]
    pub prompt_stage: Option<String>,
    /// `@prompt(stage = ..., schema = ...)` proposed fixture schema type.
    #[serde(default)]
    pub prompt_schema: Option<String>,
    /// Optional `redact` list from `@prompt(...)`.
    #[serde(default)]
    pub prompt_redact: Vec<String>,
    /// `@subagent(policy = ..., max_depth = ...)` proposed fixture policy.
    #[serde(default)]
    pub subagent_policy: Option<String>,
    /// Optional max dispatch chain depth from `@subagent`.
    #[serde(default)]
    pub subagent_max_depth: Option<u32>,
    /// Optional per-call budget ceiling from `@subagent`.
    #[serde(default)]
    pub subagent_budget_usd: Option<f64>,
    /// Optional dispatch description from `@subagent`.
    #[serde(default)]
    pub subagent_description: Option<String>,
    /// Optional explicit parallel fan-out toggle from `@subagent`.
    #[serde(default)]
    pub subagent_parallel: bool,
    /// Optional task complexity 0–10 for `@subagent(complexity = N)` dispatch routing.
    #[serde(default)]
    pub subagent_complexity: Option<u8>,
    /// `@search(corpus = ...)` corpus selector.
    #[serde(default)]
    pub search_corpus: Option<String>,
    /// `@search(query = ...)` query template.
    #[serde(default)]
    pub search_query: Option<String>,
    /// `@search(into = ...)` projection target type.
    #[serde(default)]
    pub search_into: Option<String>,
    /// Optional top-k count for search corpus calls.
    #[serde(default)]
    pub search_top_k: Option<u32>,
    /// Optional policy string for search routing guardrails.
    #[serde(default)]
    pub search_policy: Option<String>,
    /// `@hole(spec = ...)` deferred-fill spec text.
    #[serde(default)]
    pub hole_spec: Option<String>,
    /// `@hole(reviewer = ...)` reviewer mode.
    #[serde(default)]
    pub hole_reviewer: Option<String>,
    /// `@hole(cache_key = ...)` deterministic key.
    #[serde(default)]
    pub hole_cache_key: Option<String>,
    /// Optional constraints list from `@hole(...)`.
    #[serde(default)]
    pub hole_constraints: Vec<String>,
    /// `@embed(model: "...", dimensions: N, source_field: "...")` decorator (GA-24).
    #[serde(default)]
    pub embed: Option<super::embed_decorator::AstEmbedSpec>,
    /// Whether the function serves as a page layout.
    /// Whether the function is public.
    pub is_pub: bool,
    /// Whether the function records custom metrics.
    /// The name of the recorded metric.
    /// Whether the function is a health check endpoint.
    /// Optional authentication provider name.
    pub auth_provider: Option<String>,
    /// List of roles required to access the function.
    pub roles: Vec<String>,
    /// Optional CORS policy configuration.
    pub cors: Option<String>,
    /// `@webhook(provider:, secret:, replay_window_secs:)` decorator (GA-16).
    #[serde(default)]
    pub webhook: Option<super::webhook::AstWebhookSpec>,
    /// `@cors(origins:, allow_credentials:)` decorator (GA-06).
    #[serde(default)]
    pub cors_spec: Option<super::http_decorators::AstCorsSpec>,
    /// `@rate_limit(by:, window_secs:, max:)` decorator (GA-06).
    #[serde(default)]
    pub rate_limit: Option<super::http_decorators::AstRateLimitSpec>,
    /// `@pii(class:)` decorator — marks this fn as handling PII data (GA-23).
    #[serde(default)]
    pub pii: Option<super::http_decorators::AstPiiSpec>,
    /// `@layer(tier:)` decorator — declares the Z-tier for a component (GA-26).
    #[serde(default)]
    pub layer: Option<super::layer_decorator::AstLayerSpec>,
    /// Precondition expressions from `@require(expr)` decorators.
    pub preconditions: Vec<Expr>,
    /// Postcondition expressions from `@ensure(expr)` decorators.
    pub postconditions: Vec<PostCondition>,
    /// Class invariants (if this is a method).
    pub invariants: Vec<Expr>,
    /// How strictly to enforce contracts at runtime.
    pub verify_mode: VerifyMode,
    /// Optional strategy override for property testing.
    pub test_strategy: Option<String>,
    /// Whether this function is a mobile native implementation bridge.
    pub is_mobile_native: bool,
    /// When `Some`, this function is a TS-source FFI extern: the body is empty
    /// and codegen-TS emits an `import { name } from "<module>"` instead of
    /// generating a body. Codegen-Rust treats calls to such functions as an
    /// error (see plan 6).
    #[serde(default)]
    pub ts_extern_module: Option<String>,
    /// Whether the function is marked `@remote` — eligible for cross-node dispatch
    /// via the mesh (P1-T3). All parameters must be serializable.
    #[serde(default)]
    pub is_remote: bool,
    /// `@inference(model = "...")` — MENS inference routing (Mn-T4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference_model: Option<String>,
    /// `@training_step` — CUDA-gated training step surface (Mn-T5).
    #[serde(default)]
    pub training_step: bool,
    /// Source location.
    pub span: Span,
}

/// A scoped style block within a component.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StyleBlock {
    /// The CSS selector (e.g. ".btn", "h1").
    pub selector: String,
    /// List of (property, value) pairs.
    pub properties: Vec<(String, String)>,
    /// `true` when this block came from a `raw_css { }` escape hatch.
    /// Raw CSS values are allowed (as a warning) inside these blocks.
    pub is_raw_css: bool,
    /// Source location.
    pub span: Span,
}

/// Test declaration (wraps a function with @test semantics).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TestDecl {
    /// Human-readable label/description for the test.
    pub label: String,
    /// The underlying function implementing the test.
    pub func: FnDecl,
}

/// Property-based test declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ForallDecl {
    /// Human-readable description.
    pub label: String,
    /// How many generated iterations to run.
    pub iterations: u32,
    /// The underlying function implementing the property to check.
    pub func: FnDecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EndpointKind {
    Query,
    Mutation,
    Server,
}

/// Unified endpoint declaration (wraps a function with @endpoint semantics).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EndpointDecl {
    pub kind: EndpointKind,
    pub func: FnDecl,
}

/// Skill declaration: a modular AI capability.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillDecl {
    /// The underlying function implementing the skill.
    pub func: FnDecl,
}

/// Agent definition declaration: defines the core logic and interface for an AI agent.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentDefDecl {
    /// The underlying function implementing the agent's logic.
    pub func: FnDecl,
}

/// Scheduled function declaration — runs at a fixed interval or cron schedule.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScheduledDecl {
    /// The interval or cron schedule (e.g. "1h", "0 0 * * *").
    pub interval: String,
    /// The function to execute on schedule.
    pub func: FnDecl,
}

/// MCP tool declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpToolDecl {
    /// Human-readable description of the tool's purpose.
    pub description: String,
    /// The function implementing the tool's logic.
    pub func: FnDecl,
}

/// MCP resource declaration.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct McpResourceDecl {
    /// The URI identifying the resource.
    pub uri: String,
    /// Human-readable description of the resource.
    pub description: String,
    /// The function that serves the resource content.
    pub func: FnDecl,
}

/// Task declaration — a trust-gated, capability-checked execution unit (Wave 4).
///
/// Syntax:
/// ```vox
/// @task(trust = "user", caps = ["network.read", "db.write"])
/// fn send_notification(user_id: int, msg: str) to Result[Unit, str]:
///     ...
/// ```
///
/// Generated code gates execution behind `TrustPolicy::check()` and
/// `CapabilityPolicy::require_all()` before the function body runs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskDecl {
    /// The function implementing the task body.
    pub func: FnDecl,
    /// Minimum trust class required to invoke this task.
    /// Defaults to `"user"` if not specified.
    pub trust_class: String,
    /// List of required capabilities (e.g. `["network.read", "db.write"]`).
    pub capabilities: Vec<String>,
    /// Source span.
    pub span: Span,
}
