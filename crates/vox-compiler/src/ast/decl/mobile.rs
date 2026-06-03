//! Mobile primitive declarations: `@back_button`, `@deep_link`, `@push`. These lower
//! through the `@vox/runtime` adapter (Tauri 2 on desktop, React Native + Expo on mobile).

use crate::ast::span::Span;

/// `@back_button { on_press: handler [fallback: handler] }` —
/// wires the `@vox/runtime` `onBackButton` handler (Tauri 2 event API on desktop, React Native `BackHandler` on mobile).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BackButtonDecl {
    /// Endpoint function called on back-press; returns bool (handled?).
    pub on_press: String,
    /// Optional fallback function or action when `on_press` returns false.
    pub fallback: Option<String>,
    /// Source span.
    pub span: Span,
}

/// `@deep_link { scheme: "…" on_link: handler [universal_link: "…"] }` —
/// wires the `@vox/runtime` `onDeepLink` handler (Tauri 2 event API on desktop, `expo-linking` on mobile).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeepLinkDecl {
    /// URL scheme (e.g. `"voxmental"`).
    pub scheme: String,
    /// Optional Apple universal link domain.
    pub universal_link: Option<String>,
    /// Endpoint function called with the opened URL; returns the target route path.
    pub on_link: String,
    /// Source span.
    pub span: Span,
}

/// `@push { [on_register: handler] [on_notification: handler] [on_action: handler] }` —
/// wires the `@vox/runtime` `installPushNotifications` registration + listeners (Tauri 2 on desktop, `expo-notifications` on mobile).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PushDecl {
    /// Endpoint called after push registration to store the token.
    pub on_register: Option<String>,
    /// Endpoint called when a notification is received in the foreground.
    pub on_notification: Option<String>,
    /// Endpoint called when the user taps a notification action.
    pub on_action: Option<String>,
    /// Source span.
    pub span: Span,
}
