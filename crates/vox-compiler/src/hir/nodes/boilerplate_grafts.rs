//! HIR nodes for Tier-1 / Tier-2 boilerplate-reduction grafts.
//!
//! Each subsection is the HIR-side foundation for one graft from the
//! [boilerplate-reduction gap analysis](../../../../../docs/src/architecture/boilerplate-reduction-gap-analysis-2026.md).
//! The parser, typecheck, and codegen layers consume these types; the
//! enforcement rules live in `crates/vox-compiler/src/typeck/<graft>.rs` and
//! `crates/vox-codegen/src/...`.

use crate::ast::span::Span;

// ── GA-04 — Capability typecheck + @auth(provider:) ──────────────────────

/// A capability requirement attached to an `@endpoint` via `@require(can: …)`.
///
/// `expr_canonical` is the canonical-string form of the predicate (e.g.,
/// `"Read.Email"`, `"Workspace(ws_id).Write"`); typecheck consults this when
/// deciding whether the principal's capability set covers the response shape.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirCapabilityRequirement {
    pub expr_canonical: String,
    pub span: Span,
}

/// An `@auth(provider: …)` decorator on a route or endpoint group.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirAuthDecl {
    pub provider: AuthProvider,
    /// Redirect URL after successful auth.
    pub redirect: Option<String>,
    /// Scopes / claims requested.
    pub scopes: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AuthProvider {
    /// OAuth 2.0 Authorization-Code Flow + PKCE.
    OAuth { issuer: String, client_id: String },
    /// Bearer-token in `Authorization: Bearer …`.
    Bearer,
    /// API key in a named header.
    ApiKey { header_name: String },
}

// ── GA-05 — Effect annotations @uses(net|fs|...) ──────────────────────────

/// An `@uses(...)` effect annotation on a function declaration.
///
/// Effects are propagated transitively: a `@pure` caller of a function with
/// any non-empty `effects` set is rejected by `vox/effect/pure-violation`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirUsesDecl {
    pub effects: Vec<HirEffectClass>,
    pub span: Span,
}

/// One declared effect class; per-class parameters (retry, idempotency,
/// timeout for `Net`) carry alongside.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HirEffectClass {
    /// Network I/O. Optional retry / timeout / idempotency policy parameters.
    Net {
        retry: Option<RetryPolicy>,
        timeout_secs: Option<u64>,
        idempotent: bool,
    },
    /// Filesystem I/O.
    Fs,
    /// Wall-clock or monotonic time read.
    Time,
    /// Random-number generation.
    Random,
    /// Reads a value tagged `@secret` (Phase 4 redactor).
    Secret,
    /// LLM completion (composes with GA-21 + Net).
    Llm,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RetryPolicy {
    /// No retry.
    None,
    /// Exponential backoff up to `max_attempts`.
    ExpBackoff { max_attempts: u32 },
    /// Fixed retry interval.
    FixedInterval { max_attempts: u32, interval_ms: u64 },
}

// ── GA-23 — @pii taint companion to @secret ───────────────────────────────

/// A `@pii` taint marker on a type field or local binding.
///
/// PII propagation: any value derived from a `@pii`-marked source carries
/// the taint until cleared by `redact()` or `consent_recorded(...)`. A
/// tainted value reaching a `@uses(net)` call site is `vox/taint/pii-leak`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirPiiMarker {
    /// Class of PII for finer-grained policy: email, phone, name, address, etc.
    pub class: PiiClass,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PiiClass {
    Email,
    Phone,
    Name,
    Address,
    GovernmentId,
    Ip,
    FinancialData,
    BiometricData,
    /// Free-form for project-specific PII categories.
    Other(String),
}

// ── GA-09a — Routes-as-types (typed href) ─────────────────────────────────

/// A typed-route declaration produced from the existing `routes { … }` block.
///
/// Each route is materialized as a `RouteId` value at runtime; `<a href={…}>`
/// only accepts a `RouteId`, never a raw string (warns `vox/route/stringly-typed`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirRouteId {
    /// PascalCase route identifier, e.g., `UserProfile`.
    pub name: String,
    /// URL pattern with `:param` placeholders, e.g., `/users/:id`.
    pub url_pattern: String,
    /// Typed parameters required to construct this route.
    pub params: Vec<(String, String)>,
    /// Slug used by the analytics-event emitter.
    pub analytics_slug: String,
    pub span: Span,
}

// ── GA-12 — Upload[T] typed primitive ─────────────────────────────────────

/// An `Upload[T]` typed primitive, where `T` carries content-type / size
/// constraints at the type level.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirUploadType {
    /// MIME-type pattern (e.g., `"image/*"`, `"application/pdf"`).
    pub mime_pattern: String,
    /// Maximum allowed size in bytes (inclusive). `None` ⇒ no compile-time cap.
    pub max_bytes: Option<u64>,
    pub span: Span,
}

// ── GA-13 — Channel[Send, Recv] ──────────────────────────────────────────

/// A typed `Channel[Send, Recv]` declaration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirChannelType {
    /// Type-name of the server-to-client envelope.
    pub send_type: String,
    /// Type-name of the client-to-server envelope.
    pub recv_type: String,
    /// Whether the channel uses sequence numbers for replay-on-reconnect.
    pub replayable: bool,
    pub span: Span,
}

// ── GA-16 — @webhook decorator ────────────────────────────────────────────

/// A `@webhook(provider: …)` decorator on an endpoint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirWebhookDecl {
    pub provider: WebhookProvider,
    /// Whether to enforce idempotency-key replay protection.
    pub idempotent: bool,
    /// Replay window in seconds (timestamp-skew tolerance).
    pub replay_window_secs: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WebhookProvider {
    Stripe,
    Github,
    Slack,
    /// Custom HMAC-SHA256 with a named secret variable.
    Custom {
        secret_var: String,
    },
}

// ── GA-17 — Paginated[T] cursor codec ─────────────────────────────────────

/// A `Paginated[T]` typed primitive with HMAC-signed cursor codec.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirPaginatedType {
    /// Type-name of the row being paginated.
    pub item_type: String,
    /// Default page size if the caller does not specify.
    pub default_page_size: u32,
    /// Whether to expose `prev_cursor` in addition to `next_cursor`.
    pub bidirectional: bool,
    pub span: Span,
}

// ── GA-21 — @ai structured output ─────────────────────────────────────────

/// Companion structured-output specification for an `@ai`-annotated function.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirAiStructuredOutput {
    /// Type-name the LLM's structured output must conform to.
    pub return_type: String,
    /// Maximum re-prompt iterations on schema-validation failure.
    pub max_iterations: u32,
    pub span: Span,
}

/// Unified AI fixture carrier attached to `HirFn`.
///
/// This enum centralizes AI-adjacent language surfaces so we do not continue
/// growing independent sibling fields on `HirFn`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum HirAiFixture {
    /// Current shipping `@ai(model=..., structured_output=..., max_iterations=...)` surface.
    ModelPin(HirAiModelPinFixture),
    /// Intent-routed model-selection extension for `@ai(...)`.
    IntentRouted(HirAiIntentFixture),
    /// Stage-aware prompt fixture (proposed).
    Prompt(HirPromptFixture),
    /// Subagent dispatch fixture (proposed).
    Subagent(HirSubagentFixture),
    /// Deferred fill / hole fixture (proposed).
    Hole(HirHoleFixture),
    /// Search substitution fixture (proposed).
    Search(HirSearchFixture),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirAiModelPinFixture {
    pub model: Option<String>,
    pub structured_output: Option<HirAiStructuredOutput>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirAiIntentFixture {
    pub task_category: Option<String>,
    pub strengths: Vec<String>,
    pub tier_max: Option<String>,
    pub cost_ceiling_usd_per_call: Option<f64>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirPromptFixture {
    pub stage: String,
    pub schema: String,
    pub redact: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirSubagentFixture {
    pub policy: String,
    pub max_depth: u32,
    pub budget_usd: Option<f64>,
    pub description: Option<String>,
    pub parallel: bool,
    /// Override dispatch complexity (0–10); when `None`, codegen derives a default.
    pub complexity: Option<u8>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirHoleFixture {
    pub spec: String,
    pub reviewer: String,
    pub cache_key: String,
    pub constraints: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirSearchFixture {
    pub corpus: String,
    pub query: String,
    pub into_type: String,
    pub top_k: Option<u32>,
    pub policy: Option<String>,
    pub span: Span,
}

// ── GA-24 — Vector[N] + @embed ────────────────────────────────────────────

/// A `Vector[N]` typed primitive with statically known dimension `N`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirVectorType {
    pub dimension: usize,
    pub span: Span,
}

/// An `@embed(model: …)` decorator on a `@table` field.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirEmbedDecl {
    /// Embedding model identifier, e.g., `"text-embedding-3-small"`.
    pub model: String,
    /// Field path to embed, relative to the enclosing `@table`.
    pub source_field: String,
    /// Output dimension for the chosen model. Used to type-check `Vector[N]`.
    pub dimension: usize,
    pub span: Span,
}

// ── GA-15 — Offline + CRDT ────────────────────────────────────────────────

/// An `@offline_capable(strategy: …)` decorator on a `routes` block.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirOfflineCapableDecl {
    pub strategy: OfflineStrategy,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OfflineStrategy {
    /// Cache first; serve from cache, revalidate in background.
    StaleWhileRevalidate,
    /// Cache only; never hit network.
    CacheFirst,
    /// Network first; fall back to cache on failure.
    NetworkFirst,
}

/// A `@collaborative` decorator on a `RichText` or `BlockTree` field.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HirCollaborativeDecl {
    /// CRDT backend (Yjs, Automerge, etc.).
    pub backend: CrdtBackend,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CrdtBackend {
    Yjs,
    Automerge,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::span::Span;

    fn span() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn pii_class_email_round_trips() {
        let m = HirPiiMarker {
            class: PiiClass::Email,
            span: span(),
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: HirPiiMarker = serde_json::from_str(&json).unwrap();
        assert_eq!(back.class, PiiClass::Email);
    }

    #[test]
    fn retry_policy_variants_serialize() {
        for p in [
            RetryPolicy::None,
            RetryPolicy::ExpBackoff { max_attempts: 3 },
            RetryPolicy::FixedInterval {
                max_attempts: 5,
                interval_ms: 1000,
            },
        ] {
            let s = serde_json::to_string(&p).unwrap();
            let back: RetryPolicy = serde_json::from_str(&s).unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn vector_type_carries_dimension() {
        let v768 = HirVectorType {
            dimension: 768,
            span: span(),
        };
        let v1536 = HirVectorType {
            dimension: 1536,
            span: span(),
        };
        assert_ne!(v768.dimension, v1536.dimension);
    }

    #[test]
    fn webhook_providers_distinct() {
        let s = WebhookProvider::Stripe;
        let g = WebhookProvider::Github;
        let c = WebhookProvider::Custom {
            secret_var: "SECRET".into(),
        };
        assert_ne!(s, g);
        assert_ne!(s, c);
        assert_ne!(g, c);
    }

    #[test]
    fn offline_strategy_distinct() {
        assert_ne!(
            OfflineStrategy::StaleWhileRevalidate,
            OfflineStrategy::CacheFirst
        );
        assert_ne!(OfflineStrategy::CacheFirst, OfflineStrategy::NetworkFirst);
    }
}
