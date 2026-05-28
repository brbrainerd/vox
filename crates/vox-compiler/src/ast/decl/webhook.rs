//! AST type for the `@webhook(provider:, secret:, replay_window_secs:)` decorator (GA-16).
//!
//! Lowered to [`crate::hir::nodes::boilerplate_grafts::HirWebhookDecl`] in the
//! HIR phase; validated by `typeck::boilerplate_grafts::check_webhook_decl`.

use crate::ast::span::Span;

/// Parsed `@webhook(...)` decorator on an endpoint or function.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AstWebhookSpec {
    /// One of `stripe`, `github`, `slack`, `custom`.
    pub provider: AstWebhookProvider,
    /// Replay-window tolerance in seconds (default 300).
    pub replay_window_secs: u64,
    /// Whether to enforce idempotency-key replay protection (default true).
    pub idempotent: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AstWebhookProvider {
    Stripe,
    Github,
    Slack,
    /// Custom provider — `secret_var` carries the env-var name. Empty string means missing.
    Custom {
        secret_var: String,
    },
}
