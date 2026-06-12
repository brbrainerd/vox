//! Type-only helpers for `decl/head.rs` (parser god-object burndown).

use crate::ast::decl::effect::EffectAnnotation;
use crate::ast::decl::http_decorators::{AstCorsSpec, AstPiiSpec, AstRateLimitSpec};
use crate::ast::decl::layer_decorator::AstLayerSpec;
use crate::ast::decl::webhook::AstWebhookSpec;
use crate::ast::decl::{PostCondition, ReactBinding};
use crate::ast::span::Span;

/// AST decl types used by declaration-head parsing (keeps `head.rs` import block small).
pub(crate) use crate::ast::decl::{
    AstColorToken, AstFontToken, AstScalarToken, BackButtonDecl, Decl, DeepLinkDecl, EndpointDecl,
    EndpointKind, ExampleDecl, FieldConstraint, FnDecl, ForallDecl, FormDecl, FormField,
    ImportDecl, ImportPath, ImportPathKind, LoadingDecl, McpResourceDecl, McpToolDecl, PushDecl,
    ReactNamedImport, RustCrateImport, ScheduledDecl, TestDecl, TokensDecl,
};

/// Outcome of attempting a `import react …` component import (committed vs bail).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // staged for `head.rs` burndown — not wired yet
pub(crate) enum ReactImportAttempt {
    /// Not a react component import — caller should try another import form.
    NotThisForm,
    /// Parsed binding is committed; malformed input is a hard error (`Err`).
    Committed,
}

/// Binding shape selected while parsing `import react … from "…"`.
#[derive(Debug, Clone)]
#[allow(dead_code)] // staged for `head.rs` burndown — not wired yet
pub(crate) enum ReactImportBindingPhase {
    Named(Vec<ReactNamedImport>),
    Namespace { local_name: String },
    Default { local_name: String },
}

impl ReactImportBindingPhase {
    #[must_use]
    #[allow(dead_code)] // staged for `head.rs` burndown — not wired yet
    pub(crate) fn into_binding(self) -> ReactBinding {
        match self {
            Self::Named(names) => ReactBinding::Named(names),
            Self::Namespace { local_name } => ReactBinding::Namespace { local_name },
            Self::Default { local_name } => ReactBinding::Default { local_name },
        }
    }
}

/// Scratch space for `parse_fn_decl` decorator accumulation (reduces locals in `head.rs`).
#[derive(Debug, Default)]
#[allow(dead_code)] // staged for `head.rs` burndown — not wired yet
pub(crate) struct FnDeclDecoratorScratch {
    pub preconditions: Vec<crate::ast::expr::Expr>,
    pub postconditions: Vec<PostCondition>,
    pub invariants: Vec<crate::ast::expr::Expr>,
    pub is_mobile_native: bool,
    pub is_pure: bool,
    pub is_reactive: bool,
    pub is_versioned: bool,
    pub is_remote: bool,
    pub is_deprecated: bool,
    pub is_llm: bool,
    pub llm_model: Option<String>,
    pub ai_structured_output_type: Option<String>,
    pub ai_max_iterations: u32,
    pub ai_task_category: Option<String>,
    pub ai_strengths: Vec<String>,
    pub ai_tier_max: Option<String>,
    pub ai_cost_ceiling_usd_per_call: Option<f64>,
    pub prompt_stage: Option<String>,
    pub prompt_schema: Option<String>,
    pub prompt_redact: Vec<String>,
    pub subagent_policy: Option<String>,
    pub subagent_max_depth: Option<u32>,
    pub subagent_budget_usd: Option<f64>,
    pub subagent_description: Option<String>,
    pub subagent_parallel: bool,
    pub subagent_complexity: Option<u8>,
    pub search_corpus: Option<String>,
    pub search_query: Option<String>,
    pub search_into: Option<String>,
    pub search_top_k: Option<u32>,
    pub search_policy: Option<String>,
    pub hole_spec: Option<String>,
    pub hole_reviewer: Option<String>,
    pub hole_cache_key: Option<String>,
    pub hole_constraints: Vec<String>,
    pub embed_model: Option<String>,
    pub embed_dimensions: usize,
    pub embed_source_field: Option<String>,
    pub embed_span: Option<Span>,
    pub inference_model: Option<String>,
    pub training_step: bool,
    pub decorator_effects: Vec<EffectAnnotation>,
    pub webhook: Option<AstWebhookSpec>,
    pub cors_spec: Option<AstCorsSpec>,
    pub rate_limit: Option<AstRateLimitSpec>,
    pub pii: Option<AstPiiSpec>,
    pub layer: Option<AstLayerSpec>,
}

impl FnDeclDecoratorScratch {
    #[allow(dead_code)] // staged for `head.rs` burndown — not wired yet
    pub(crate) fn new() -> Self {
        Self {
            ai_max_iterations: 3,
            ..Self::default()
        }
    }
}
