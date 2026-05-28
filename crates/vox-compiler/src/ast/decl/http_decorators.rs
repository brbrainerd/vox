//! AST types for HTTP ergonomics decorators (GA-06) and PII taint (GA-23).
//!
//! `@cors`, `@rate_limit`, and `@pii` ride on `FnDecl` as optional sidecars
//! and lower to HIR `http_ergonomics` / `boilerplate_grafts` types in the
//! HIR phase.

use crate::ast::span::Span;

// ── GA-06 — @cors ──────────────────────────────────────────────────────────

/// Parsed `@cors(origins: [...], allow_credentials: bool)` decorator.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstCorsSpec {
    /// Allowed-origin patterns. `["*"]` = permissive.
    pub origins: Vec<String>,
    /// Whether the `Authorization` / `Cookie` headers are forwarded cross-origin.
    pub allow_credentials: bool,
    pub span: Span,
}

// ── GA-06 — @rate_limit ────────────────────────────────────────────────────

/// Parsed `@rate_limit(by: ip|user_id|api_key, window_secs: N, max: N)` decorator.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstRateLimitSpec {
    pub by: AstRateLimitBy,
    /// Sliding-window duration in seconds (default 60).
    pub window_secs: u64,
    /// Maximum requests per window (default 100).
    pub max_requests: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AstRateLimitBy {
    Ip,
    UserId,
    ApiKey,
}

// ── GA-23 — @pii ───────────────────────────────────────────────────────────

/// Parsed `@pii(class: email|name|phone|ip|financial|biometric)` decorator.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstPiiSpec {
    pub class: AstPiiClass,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AstPiiClass {
    Name,
    Email,
    Phone,
    Ip,
    FinancialData,
    BiometricData,
    Other(String),
}

impl AstPiiClass {
    /// Loose parser used by decorator lowering — never fails because unknown
    /// labels are absorbed via the `Other(String)` variant. We keep the
    /// `from_str` name for symmetry with the rest of the AST surface; the
    /// `should_implement_trait` lint is suppressed because implementing
    /// `FromStr` would require choosing an `Err` type that nothing here ever
    /// produces.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "name" => Self::Name,
            "email" => Self::Email,
            "phone" => Self::Phone,
            "ip" => Self::Ip,
            "financial" | "financial_data" => Self::FinancialData,
            "biometric" | "biometric_data" => Self::BiometricData,
            other => Self::Other(other.to_string()),
        }
    }
}
