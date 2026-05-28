//! Type-check passes for Tier-1 / Tier-2 boilerplate-reduction grafts.
//!
//! Each subsection enforces one structural rule named in the gap analysis.
//! Diagnostics use stable ids (`vox/<area>/<rule>`) per Phase 1 SSOT
//! Collapse's append-only-with-deprecation-aliases policy.

use std::collections::HashSet;

use crate::ast::span::Span;
use crate::hir::nodes::boilerplate_grafts::{
    HirAiStructuredOutput, HirCapabilityRequirement, HirEffectClass, HirEmbedDecl, HirPiiMarker,
    HirUploadType, HirUsesDecl, HirVectorType, HirWebhookDecl, PiiClass, WebhookProvider,
};
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

// ── GA-04 — Capability typecheck ──────────────────────────────────────────

/// Refuse compile when an endpoint response carries fields whose capability
/// the principal lacks.
///
/// `principal_caps` is the canonical-string capability set for the calling
/// principal (collected by the parser from `@auth(...)` and route-scoping).
/// `field_caps` lists the (field-name, required-capability) pairs from the
/// response shape. A leak is any field whose required capability is not in
/// `principal_caps`.
pub fn check_capability_leak(
    field_caps: &[(String, HirCapabilityRequirement)],
    principal_caps: &HashSet<String>,
    endpoint_span: Span,
) -> Vec<Diagnostic> {
    field_caps
        .iter()
        .filter(|(_, req)| !principal_caps.contains(&req.expr_canonical))
        .map(|(field, req)| Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "Capability leak: response field `{}` requires `{}` but the principal lacks it.",
                field, req.expr_canonical
            ),
            span: req.span,
            code: Some("vox/auth/capability-leak".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec![
                format!("Add `@require(can: {})` to the endpoint, or remove the field from the response.", req.expr_canonical),
                format!("Wrap the field in `Capability[{}, T]` to make the gating explicit.", req.expr_canonical),
            ],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some(format!("principal has `{}`", req.expr_canonical)),
            found_type: Some("missing capability".into()),
            context: Some(format!("endpoint at {endpoint_span:?}")),
            ast_node_kind: None,
        })
        .collect()
}

// ── GA-05 — Effect annotations ────────────────────────────────────────────

/// Refuse `@pure` callers of any function with non-empty `@uses(...)`.
pub fn check_pure_violation(
    caller_is_pure: bool,
    callee_uses: &HirUsesDecl,
    call_site: Span,
) -> Option<Diagnostic> {
    if !caller_is_pure || callee_uses.effects.is_empty() {
        return None;
    }
    let effect_names = callee_uses
        .effects
        .iter()
        .map(effect_class_name)
        .collect::<Vec<_>>()
        .join(", ");
    Some(Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "@pure violation: this callee declares effects ({effect_names}), but the calling function is `@pure`."
        ),
        span: call_site,
        code: Some("vox/effect/pure-violation".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec![
            "Remove `@pure` from the caller, or call this through an effect-tracked indirection."
                .into(),
        ],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some("no effects in @pure scope".into()),
        found_type: Some(format!("declares: {effect_names}")),
        context: None,
        ast_node_kind: None,
    })
}

/// Refuse `net.fetch(...)` (or any I/O builtin) without a matching `@uses`
/// declaration on the enclosing function.
pub fn check_missing_effect_decl(
    declared_effects: &HirUsesDecl,
    used_effect: HirEffectClass,
    call_site: Span,
) -> Option<Diagnostic> {
    let needed = effect_class_name(&used_effect);
    let already = declared_effects
        .effects
        .iter()
        .any(|e| effect_class_name(e) == needed);
    if already {
        return None;
    }
    let code = match used_effect {
        HirEffectClass::Net { .. } => "vox/effect/missing-net-decl",
        HirEffectClass::Fs => "vox/effect/missing-fs-decl",
        HirEffectClass::Time => "vox/effect/missing-time-decl",
        HirEffectClass::Random => "vox/effect/missing-random-decl",
        HirEffectClass::Secret => "vox/effect/missing-secret-decl",
        HirEffectClass::Llm => "vox/effect/missing-llm-decl",
    };
    Some(Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "Function uses effect `{needed}` but does not declare it. Add `@uses({needed})` to the function declaration."
        ),
        span: call_site,
        code: Some(code.into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec![format!("Add `@uses({needed})` to the enclosing function.")],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some(format!("@uses({needed}) declared")),
        found_type: Some("undeclared".into()),
        context: None,
        ast_node_kind: None,
    })
}

fn effect_class_name(e: &HirEffectClass) -> String {
    match e {
        HirEffectClass::Net { .. } => "net".into(),
        HirEffectClass::Fs => "fs".into(),
        HirEffectClass::Time => "time".into(),
        HirEffectClass::Random => "random".into(),
        HirEffectClass::Secret => "secret".into(),
        HirEffectClass::Llm => "llm".into(),
    }
}

// ── GA-23 — @pii taint propagation ────────────────────────────────────────

/// Refuse a PII-tainted value reaching a `@uses(net)` call site without
/// `redact()` / `consent_recorded()` clearing.
pub fn check_pii_leak(
    value_pii: &HirPiiMarker,
    redacted: bool,
    consent_recorded: bool,
    call_site: Span,
) -> Option<Diagnostic> {
    if redacted || consent_recorded {
        return None;
    }
    let class_name = pii_class_name(&value_pii.class);
    Some(Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "PII leak: a `{class_name}`-tainted value flows to a `@uses(net)` call without `redact()` or `consent_recorded()`."
        ),
        span: call_site,
        code: Some("vox/taint/pii-leak".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec![format!(
            "Wrap the value in `redact(value)` or record consent: `consent_recorded(user, \"{class_name}\")`."
        )],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some("redacted or consented".into()),
        found_type: Some(format!("tainted ({class_name})")),
        context: None,
        ast_node_kind: None,
    })
}

fn pii_class_name(c: &PiiClass) -> String {
    match c {
        PiiClass::Email => "email".into(),
        PiiClass::Phone => "phone".into(),
        PiiClass::Name => "name".into(),
        PiiClass::Address => "address".into(),
        PiiClass::GovernmentId => "government_id".into(),
        PiiClass::Ip => "ip".into(),
        PiiClass::FinancialData => "financial_data".into(),
        PiiClass::BiometricData => "biometric_data".into(),
        PiiClass::Other(s) => s.clone(),
    }
}

// ── GA-24 — Vector dimension mismatch ─────────────────────────────────────

/// Refuse passing a `Vector[N]` to a function expecting `Vector[M]` where N != M.
pub fn check_vector_dimension(
    expected: &HirVectorType,
    found: &HirVectorType,
    call_site: Span,
) -> Option<Diagnostic> {
    if expected.dimension == found.dimension {
        return None;
    }
    Some(Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "Vector dimension mismatch: expected `Vector[{}]`, found `Vector[{}]`.",
            expected.dimension, found.dimension
        ),
        span: call_site,
        code: Some("vox/vector/dimension-mismatch".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec![format!(
            "Re-embed with a model that produces a {}-dim vector, or change the call site's expected dimension to {}.",
            expected.dimension, found.dimension
        )],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some(format!("Vector[{}]", expected.dimension)),
        found_type: Some(format!("Vector[{}]", found.dimension)),
        context: None,
        ast_node_kind: None,
    })
}

// ── GA-24 — @embed dimension validation ──────────────────────────────────

/// Refuse compile when an `@embed` decorator declares `dimensions: 0` —
/// an embedding with zero dimensions cannot represent anything.
pub fn check_embed_dimensions(embed: &HirEmbedDecl) -> Option<Diagnostic> {
    if embed.dimension == 0 {
        return Some(Diagnostic {
            severity: TypeckSeverity::Error,
            message: format!(
                "`@embed` on `{}` declares `dimensions: 0`; an embedding must have at least one dimension.",
                if embed.source_field.is_empty() { "(unspecified field)" } else { &embed.source_field }
            ),
            span: embed.span,
            code: Some("vox/embed/zero-dimensions".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec![
                "Specify the output dimension of your model, e.g. `dimensions: 1536` for `text-embedding-3-small`.".into(),
            ],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("dimensions > 0".into()),
            found_type: Some("0".into()),
            context: None,
            ast_node_kind: None,
        });
    }
    None
}

// ── GA-21 — @ai return-shape codec check ──────────────────────────────────

/// Refuse compile when an `@ai`-annotated function returns a type that has
/// no wire codec — the structured-output validator can't operate on
/// uncodec'd shapes.
pub fn check_ai_return_shape(
    output: &HirAiStructuredOutput,
    has_wire_codec: bool,
) -> Option<Diagnostic> {
    if has_wire_codec {
        return None;
    }
    Some(Diagnostic {
        severity: TypeckSeverity::Error,
        message: format!(
            "`@ai` function returns `{}`, which has no wire codec. The structured-output validator cannot enforce schema conformance without one.",
            output.return_type
        ),
        span: output.span,
        code: Some("vox/ai/return-shape-not-codec'd".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec![format!(
            "Define `{}` as a struct or sum type so Contract IR derives a wire codec.",
            output.return_type
        )],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some("type with wire codec".into()),
        found_type: Some(output.return_type.clone()),
        context: None,
        ast_node_kind: None,
    })
}

/// Warn about unresolved `@hole(...)` fixtures.
///
/// `@hole` marks a function as intentionally incomplete ("work-in-progress");
/// it is semantically similar to a `// TODO` comment.  Emitting only a
/// warning (not an error) lets development branches land while clearly
/// signalling that the function body needs to be filled in later.  CI gates
/// that require zero warnings (e.g. `vox check --strict`) will catch this.
pub fn check_unfilled_fixture_hole(
    hole: &crate::hir::nodes::boilerplate_grafts::HirHoleFixture,
) -> Diagnostic {
    Diagnostic {
        severity: TypeckSeverity::Warning,
        message: format!(
            "`@hole` fixture (`cache_key = {}`) is unfilled; resolve it or suppress intentionally.",
            hole.cache_key
        ),
        span: hole.span,
        code: Some("vox/fixture/unfilled-hole".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec![
            "Replace `@hole(...)` with a concrete implementation.".into(),
            "For temporary suppression only, add `// toestub-ignore(vox/fixture/unfilled-hole)` with rationale.".into(),
        ],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some("filled fixture".into()),
        found_type: Some("unfilled hole".into()),
        context: None,
        ast_node_kind: Some("HirAiFixture::Hole".into()),
    }
}

/// Optional stale-hole guard backed by `contracts/reports/ai-fixture-holes/ledger.v1.json`.
pub fn check_fixture_hole_staleness(
    hole: &crate::hir::nodes::boilerplate_grafts::HirHoleFixture,
) -> Option<Diagnostic> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let ledger_path = repo_root.join("contracts/reports/ai-fixture-holes/ledger.v1.json");
    let raw = std::fs::read_to_string(&ledger_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let entries = json.get("entries")?.as_array()?;
    let stale = entries.iter().any(|entry| {
        let cache_key = entry.get("cache_key").and_then(|v| v.as_str());
        let is_stale = entry
            .get("stale")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        cache_key == Some(hole.cache_key.as_str()) && is_stale
    });
    if !stale {
        return None;
    }
    Some(Diagnostic {
        severity: TypeckSeverity::Warning,
        message: format!(
            "`@hole` fixture cache key `{}` is marked stale in the hole ledger.",
            hole.cache_key
        ),
        span: hole.span,
        code: Some("vox/fixture/stale-hole".into()),
        category: DiagnosticCategory::Typecheck,
        suggestions: vec![
            "Refresh the hole fixture output and update the stale-hole ledger entry.".into(),
        ],
        fixes: vec![],
        line_col: None,
        missing_cases: vec![],
        expected_type: Some("fresh hole fixture".into()),
        found_type: Some("stale hole fixture".into()),
        context: None,
        ast_node_kind: Some("HirAiFixture::Hole".into()),
    })
}

// ── AI-first fixtures — structural typecheck (catalog-backed IDs) ─────────

/// Task categories accepted for `@ai(task_category = …)`.
///
/// Keep in sync with `contracts/orchestration/model-routing.v1.yaml` (`task_categories`).
const AI_ROUTING_TASK_CATEGORIES: &[&str] = &[
    "CodeGen",
    "Testing",
    "Debugging",
    "TypeChecking",
    "Research",
    "Parsing",
    "Review",
    "General",
    "Ars",
    "Planning",
    "InterAgent",
    "ToolOrchestration",
    "Visus",
];

/// Must match `DispatchConfig::default().max_chain_depth` in `vox-orchestrator`.
const AI_SUBAGENT_DEFAULT_MAX_CHAIN_DEPTH: u32 = 5;

const VALID_PROMPT_STAGES: &[&str] = &[
    "Planner",
    "ClaimExtraction",
    "Verification",
    "Synthesis",
    "Judge",
    "SelfVerification",
];

#[inline]
fn redact_token_requires_env_capability(token: &str) -> bool {
    let t = token.trim();
    t.starts_with("OPENROUTER_") || t.starts_with("ANTHROPIC_") || t.starts_with("OPENAI_")
}

#[inline]
fn memory_lookup_key_well_formed(query: &str) -> bool {
    let q = query.trim();
    let parts: Vec<&str> = q.split(':').collect();
    parts.len() >= 3 && parts.iter().all(|p| !p.is_empty())
}

fn fn_declares_env_capability(f: &crate::hir::HirFn) -> bool {
    f.capabilities
        .iter()
        .any(|c| matches!(c, crate::hir::HirCapability::Env))
}

fn fn_declares_net_capability(f: &crate::hir::HirFn) -> bool {
    f.capabilities
        .iter()
        .any(|c| matches!(c, crate::hir::HirCapability::Net))
}

/// Emit diagnostics for `@ai` / `@prompt` / `@subagent` / `@search` fixtures attached to `f`.
#[must_use]
pub fn collect_ai_fixture_diagnostics(f: &crate::hir::HirFn) -> Vec<Diagnostic> {
    use crate::hir::nodes::boilerplate_grafts::HirAiFixture;

    let Some(fixture) = &f.ai_fixture else {
        return Vec::new();
    };

    let mut out = Vec::new();
    match fixture {
        HirAiFixture::IntentRouted(intent) => {
            if let Some(cat) = intent.task_category.as_deref() {
                if !AI_ROUTING_TASK_CATEGORIES.contains(&cat) {
                    out.push(Diagnostic {
                        severity: TypeckSeverity::Error,
                        message: format!(
                            "Unknown `@ai` task_category `{cat}`; expected one of the `task_categories` entries in `contracts/orchestration/model-routing.v1.yaml`."
                        ),
                        span: intent.span,
                        code: Some("vox/ai/unknown-task-category".into()),
                        category: DiagnosticCategory::Typecheck,
                        suggestions: vec![
                            "Pick a supported category such as `CodeGen`, `Research`, or `Review`.".into(),
                            "If you need a new category, extend `contracts/orchestration/model-routing.v1.yaml` first.".into(),
                        ],
                        fixes: vec![],
                        line_col: None,
                        missing_cases: vec![],
                        expected_type: Some("known task category".into()),
                        found_type: Some(cat.to_string()),
                        context: None,
                        ast_node_kind: Some("HirAiFixture::IntentRouted".into()),
                    });
                }
            }
        }
        HirAiFixture::Prompt(prompt) => {
            if !VALID_PROMPT_STAGES.contains(&prompt.stage.as_str()) {
                out.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "Invalid `@prompt` stage `{}`; expected one of: {}.",
                        prompt.stage,
                        VALID_PROMPT_STAGES.join(", ")
                    ),
                    span: prompt.span,
                    code: Some("vox/prompt/invalid-stage".into()),
                    category: DiagnosticCategory::Typecheck,
                    suggestions: vec![
                        "Use a `ResearchStage` variant name such as `Planner` or `Verification`."
                            .into(),
                    ],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: Some("ResearchStage".into()),
                    found_type: Some(prompt.stage.clone()),
                    context: None,
                    ast_node_kind: Some("HirAiFixture::Prompt".into()),
                });
            }
            for token in &prompt.redact {
                if redact_token_requires_env_capability(token) && !fn_declares_env_capability(f) {
                    out.push(Diagnostic {
                        severity: TypeckSeverity::Warning,
                        message: format!(
                            "`@prompt` redact token `{token}` looks like a secret/env name; declare `uses env` (and resolve via Clavis) so this stays explicit."
                        ),
                        span: prompt.span,
                        code: Some("vox/prompt/secret-leakage".into()),
                        category: DiagnosticCategory::Typecheck,
                        suggestions: vec!["Add `env` to the function `uses (...)` clause.".into()],
                        fixes: vec![],
                        line_col: None,
                        missing_cases: vec![],
                        expected_type: Some("uses env".into()),
                        found_type: Some("missing env capability".into()),
                        context: None,
                        ast_node_kind: Some("HirAiFixture::Prompt".into()),
                    });
                }
            }
        }
        HirAiFixture::Subagent(sub) => {
            if sub.max_depth >= AI_SUBAGENT_DEFAULT_MAX_CHAIN_DEPTH {
                out.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "`@subagent(max_depth = {})` exceeds the default dispatch chain bound (< {}).",
                        sub.max_depth, AI_SUBAGENT_DEFAULT_MAX_CHAIN_DEPTH
                    ),
                    span: sub.span,
                    code: Some("vox/subagent/chain-depth-exceeded".into()),
                    category: DiagnosticCategory::Typecheck,
                    suggestions: vec![format!(
                        "Lower `max_depth` to strictly less than {AI_SUBAGENT_DEFAULT_MAX_CHAIN_DEPTH}."
                    )],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: Some(format!("max_depth < {AI_SUBAGENT_DEFAULT_MAX_CHAIN_DEPTH}")),
                    found_type: Some(format!("{}", sub.max_depth)),
                    context: None,
                    ast_node_kind: Some("HirAiFixture::Subagent".into()),
                });
            }
            if sub.policy.eq_ignore_ascii_case("distributed") {
                out.push(Diagnostic {
                    severity: TypeckSeverity::Warning,
                    message: "`@subagent(policy = distributed)` requires mesh wiring (`populi-transport` on `vox-orchestrator` and bundle deps). Ensure your workspace enables it before relying on remote relay.".into(),
                    span: sub.span,
                    code: Some("vox/subagent/distributed-not-wired".into()),
                    category: DiagnosticCategory::Typecheck,
                    suggestions: vec![
                        "Enable `populi-transport` on `vox-orchestrator` for mesh-capable bundles.".into(),
                    ],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: Some("mesh-ready orchestrator build".into()),
                    found_type: Some("distributed policy".into()),
                    context: None,
                    ast_node_kind: Some("HirAiFixture::Subagent".into()),
                });
            }
        }
        HirAiFixture::Search(search) => {
            let corpus = search.corpus.trim();
            let corpus_lower = corpus.to_ascii_lowercase();
            if !matches!(corpus_lower.as_str(), "memory" | "docs" | "web") {
                out.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "Unsupported `@search` corpus `{corpus}`; allowed: `memory`, `docs`, `web`."
                    ),
                    span: search.span,
                    code: Some("vox/search/corpus-denied".into()),
                    category: DiagnosticCategory::Typecheck,
                    suggestions: vec![
                        "Use `corpus = memory` for RAG keys, `docs` for repo markdown search, or `web` for cascade-backed retrieval.".into(),
                    ],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: Some("memory|docs|web".into()),
                    found_type: Some(corpus.to_string()),
                    context: None,
                    ast_node_kind: Some("HirAiFixture::Search".into()),
                });
            } else if corpus_lower == "memory" && !memory_lookup_key_well_formed(&search.query) {
                out.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "`@search(corpus = memory)` query `{}` must look like `scope:account:key` (three non-empty segments).",
                        search.query.trim()
                    ),
                    span: search.span,
                    code: Some("vox/search/memory-key-invalid".into()),
                    category: DiagnosticCategory::Typecheck,
                    suggestions: vec![
                        "Example: `project:default:onboarding`.".into(),
                    ],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: Some("scope:account:key".into()),
                    found_type: Some(search.query.clone()),
                    context: None,
                    ast_node_kind: Some("HirAiFixture::Search".into()),
                });
            } else if corpus_lower == "web" && !fn_declares_net_capability(f) {
                out.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: "`@search(corpus = web)` performs networked retrieval; declare `@uses(net)`."
                        .into(),
                    span: search.span,
                    code: Some("vox/search/web-policy-denied".into()),
                    category: DiagnosticCategory::Typecheck,
                    suggestions: vec!["Add `net` to the function `uses (...)` clause.".into()],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: Some("uses net".into()),
                    found_type: Some("missing net capability".into()),
                    context: None,
                    ast_node_kind: Some("HirAiFixture::Search".into()),
                });
            }
        }
        HirAiFixture::ModelPin(_) | HirAiFixture::Hole(_) => {}
    }

    out
}

// ── GA-12 — Upload size / MIME structural check ──────────────────────────

/// Refuse compile when a static literal `Upload[T]` configuration declares a
/// `max_bytes` of zero (clearly a typo) or a MIME pattern that cannot match
/// any real value (e.g., empty string).
pub fn check_upload_type(t: &HirUploadType) -> Vec<Diagnostic> {
    let mut diags = vec![];
    if t.mime_pattern.is_empty() {
        diags.push(Diagnostic {
            severity: TypeckSeverity::Error,
            message: "Upload[T] has an empty MIME pattern; nothing will validate.".into(),
            span: t.span,
            code: Some("vox/upload/empty-mime".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec![
                "Use `image/*`, `application/pdf`, or another concrete pattern.".into(),
            ],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("non-empty MIME pattern".into()),
            found_type: Some("\"\"".into()),
            context: None,
            ast_node_kind: None,
        });
    }
    if matches!(t.max_bytes, Some(0)) {
        diags.push(Diagnostic {
            severity: TypeckSeverity::Error,
            message: "Upload[T] has max_bytes = 0; no upload can ever pass.".into(),
            span: t.span,
            code: Some("vox/upload/zero-max-bytes".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec!["Remove `max_bytes` or set it to a non-zero value.".into()],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("max_bytes > 0".into()),
            found_type: Some("0".into()),
            context: None,
            ast_node_kind: None,
        });
    }
    diags
}

// ── GA-16 — Webhook decorator validation ─────────────────────────────────

/// Refuse compile when a `@webhook` decorator declares a replay window outside
/// the safe range (5s..1h) or pairs `Custom { secret_var }` with an empty
/// secret-var name.
pub fn check_webhook_decl(d: &HirWebhookDecl) -> Vec<Diagnostic> {
    let mut diags = vec![];
    if d.replay_window_secs < 5 || d.replay_window_secs > 3600 {
        diags.push(Diagnostic {
            severity: TypeckSeverity::Warning,
            message: format!(
                "Webhook replay window is {}s; outside the recommended 5..3600 range.",
                d.replay_window_secs
            ),
            span: d.span,
            code: Some("vox/webhook/replay-window-out-of-range".into()),
            category: DiagnosticCategory::Lint,
            suggestions: vec![
                "Use a 5–300s window for typical providers; 3600s only for very-high-skew sources."
                    .into(),
            ],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("5..=3600".into()),
            found_type: Some(d.replay_window_secs.to_string()),
            context: None,
            ast_node_kind: None,
        });
    }
    if let WebhookProvider::Custom { secret_var } = &d.provider
        && secret_var.is_empty()
    {
        diags.push(Diagnostic {
            severity: TypeckSeverity::Error,
            message: "Custom webhook provider declared without a secret-var name.".into(),
            span: d.span,
            code: Some("vox/webhook/missing-secret-var".into()),
            category: DiagnosticCategory::Typecheck,
            suggestions: vec![
                "Provide the env-var name carrying the HMAC secret, e.g., `secret: \"WEBHOOK_SECRET\"`.".into()
            ],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("non-empty secret-var name".into()),
            found_type: Some("\"\"".into()),
            context: None,
            ast_node_kind: None,
        });
    }
    diags
}

// ── GA-06 — CORS policy validation ───────────────────────────────────────

use crate::hir::nodes::http_ergonomics::HirCorsPolicy;

/// Warn when `allow_credentials: true` is combined with a wildcard `*` origin.
/// Browsers block credentialed requests to wildcard origins (CORS spec §4.9.2).
pub fn check_cors_policy(policy: &HirCorsPolicy) -> Vec<Diagnostic> {
    let mut diags = vec![];
    if policy.allow_credentials && policy.origins.iter().any(|o| o == "*") {
        diags.push(Diagnostic {
            severity: TypeckSeverity::Warning,
            message: "`allow_credentials: true` with origins `[\"*\"]` is rejected by browsers (CORS spec §4.9.2). List explicit origins instead.".into(),
            span: policy.span,
            code: Some("vox/cors/credentials-with-wildcard".into()),
            category: DiagnosticCategory::Lint,
            suggestions: vec![
                "Replace `[\"*\"]` with explicit origins, e.g. `[\"https://app.example.com\"]`.".into(),
            ],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("explicit origin list".into()),
            found_type: Some("[\"*\"]".into()),
            context: None,
            ast_node_kind: None,
        });
    }
    diags
}

// ── GA-23 — PII taint check ───────────────────────────────────────────────

use crate::hir::nodes::effect::{HirEffectKind, HirEffectSet};

/// Warn when a PII-tagged endpoint emits over the network without an explicit
/// `uses net` declaration — which would hide a potential exfiltration path.
pub fn check_pii_with_net_effect(
    pii: &HirPiiMarker,
    effects: &HirEffectSet,
    fn_name: &str,
) -> Option<Diagnostic> {
    let has_net = effects.iter().any(|e| matches!(e, HirEffectKind::Net));
    if has_net {
        // PII + net is fine as long as it's declared — declaration proves awareness.
        return None;
    }
    // If the fn body calls anything that could send data but lacks `uses net`,
    // the effects set is empty (unannotated). Warn to prompt explicit annotation.
    if effects.is_empty() {
        return Some(Diagnostic {
            severity: TypeckSeverity::Warning,
            message: format!(
                "`{}` is tagged `@pii` but has no `@uses(net)` declaration. If it transmits data, add `@uses(net)` to document the exfiltration surface.",
                fn_name
            ),
            span: pii.span,
            code: Some("vox/pii/unannotated-net-effect".into()),
            category: DiagnosticCategory::Lint,
            suggestions: vec![
                "Add `@uses(net)` if this endpoint sends PII over the network.".into(),
            ],
            fixes: vec![],
            line_col: None,
            missing_cases: vec![],
            expected_type: Some("@uses(net) annotation".into()),
            found_type: Some("no effect annotation".into()),
            context: None,
            ast_node_kind: None,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::span::Span;
    use crate::hir::nodes::boilerplate_grafts::*;

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn capability_leak_detected() {
        let req = HirCapabilityRequirement {
            expr_canonical: "Read.Email".into(),
            span: span(),
        };
        let principal: HashSet<String> = HashSet::new();
        let diags = check_capability_leak(&[("email".into(), req)], &principal, span());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_deref(), Some("vox/auth/capability-leak"));
    }

    #[test]
    fn capability_check_passes_when_held() {
        let req = HirCapabilityRequirement {
            expr_canonical: "Read.Email".into(),
            span: span(),
        };
        let mut principal = HashSet::new();
        principal.insert("Read.Email".into());
        let diags = check_capability_leak(&[("email".into(), req)], &principal, span());
        assert!(diags.is_empty());
    }

    #[test]
    fn pure_violation_caught() {
        let uses = HirUsesDecl {
            effects: vec![HirEffectClass::Net {
                retry: None,
                timeout_secs: None,
                idempotent: false,
            }],
            span: span(),
        };
        let diag = check_pure_violation(true, &uses, span()).unwrap();
        assert_eq!(diag.code.as_deref(), Some("vox/effect/pure-violation"));
    }

    #[test]
    fn pure_caller_of_pure_callee_passes() {
        let uses = HirUsesDecl {
            effects: vec![],
            span: span(),
        };
        assert!(check_pure_violation(true, &uses, span()).is_none());
    }

    #[test]
    fn missing_net_decl_detected() {
        let declared = HirUsesDecl {
            effects: vec![],
            span: span(),
        };
        let used = HirEffectClass::Net {
            retry: None,
            timeout_secs: None,
            idempotent: false,
        };
        let diag = check_missing_effect_decl(&declared, used, span()).unwrap();
        assert_eq!(diag.code.as_deref(), Some("vox/effect/missing-net-decl"));
    }

    #[test]
    fn declared_effect_passes() {
        let declared = HirUsesDecl {
            effects: vec![HirEffectClass::Fs],
            span: span(),
        };
        assert!(check_missing_effect_decl(&declared, HirEffectClass::Fs, span()).is_none());
    }

    #[test]
    fn pii_leak_detected() {
        let m = HirPiiMarker {
            class: PiiClass::Email,
            span: span(),
        };
        let diag = check_pii_leak(&m, false, false, span()).unwrap();
        assert_eq!(diag.code.as_deref(), Some("vox/taint/pii-leak"));
    }

    #[test]
    fn pii_redacted_passes() {
        let m = HirPiiMarker {
            class: PiiClass::Email,
            span: span(),
        };
        assert!(check_pii_leak(&m, true, false, span()).is_none());
        assert!(check_pii_leak(&m, false, true, span()).is_none());
    }

    #[test]
    fn vector_dim_mismatch_detected() {
        let v768 = HirVectorType {
            dimension: 768,
            span: span(),
        };
        let v1536 = HirVectorType {
            dimension: 1536,
            span: span(),
        };
        let diag = check_vector_dimension(&v768, &v1536, span()).unwrap();
        assert_eq!(diag.code.as_deref(), Some("vox/vector/dimension-mismatch"));
    }

    #[test]
    fn vector_same_dim_passes() {
        let v = HirVectorType {
            dimension: 768,
            span: span(),
        };
        assert!(check_vector_dimension(&v, &v, span()).is_none());
    }

    #[test]
    fn ai_return_without_codec_rejected() {
        let o = HirAiStructuredOutput {
            return_type: "MyOpaqueType".into(),
            max_iterations: 3,
            span: span(),
        };
        let diag = check_ai_return_shape(&o, false).unwrap();
        assert_eq!(
            diag.code.as_deref(),
            Some("vox/ai/return-shape-not-codec'd")
        );
    }

    #[test]
    fn ai_return_with_codec_passes() {
        let o = HirAiStructuredOutput {
            return_type: "Plan".into(),
            max_iterations: 3,
            span: span(),
        };
        assert!(check_ai_return_shape(&o, true).is_none());
    }

    #[test]
    fn upload_zero_bytes_rejected() {
        let t = HirUploadType {
            mime_pattern: "image/*".into(),
            max_bytes: Some(0),
            span: span(),
        };
        let diags = check_upload_type(&t);
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some("vox/upload/zero-max-bytes"))
        );
    }

    #[test]
    fn upload_empty_mime_rejected() {
        let t = HirUploadType {
            mime_pattern: "".into(),
            max_bytes: Some(1024),
            span: span(),
        };
        let diags = check_upload_type(&t);
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some("vox/upload/empty-mime"))
        );
    }

    #[test]
    fn webhook_custom_without_secret_rejected() {
        let d = HirWebhookDecl {
            provider: WebhookProvider::Custom {
                secret_var: "".into(),
            },
            idempotent: true,
            replay_window_secs: 60,
            span: span(),
        };
        let diags = check_webhook_decl(&d);
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some("vox/webhook/missing-secret-var"))
        );
    }

    #[test]
    fn webhook_replay_window_out_of_range_warns() {
        let d = HirWebhookDecl {
            provider: WebhookProvider::Stripe,
            idempotent: true,
            replay_window_secs: 1,
            span: span(),
        };
        let diags = check_webhook_decl(&d);
        assert!(
            diags
                .iter()
                .any(|d| d.code.as_deref() == Some("vox/webhook/replay-window-out-of-range"))
        );
    }
}
